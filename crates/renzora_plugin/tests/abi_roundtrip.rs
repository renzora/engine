//! End-to-end test of the `renzora_plugin` C ABI.
//!
//! Links the reference plugin as an rlib and calls its `renzora_plugin_init`
//! directly, so this covers the ABI mechanism *without* `dlopen` in the picture —
//! if it fails, the fault is in registration / query building / dispatch /
//! marshalling, not in library loading.
//!
//! Run with `renzora test`; `cargo test` cannot link natively on Windows
//! (CLAUDE.md §2).

use bevy::prelude::*;
use renzora_plugin::host as abi_host;
use renzora_plugin::sys;
use renzora_plugin::ecs;

/// A plugin-owned component, defined here rather than borrowed from
/// `plugins/spinner`: linking that would pull it into the engine's workspace and
/// undo its isolation. Defining it here also means these tests exercise the API
/// itself rather than one example's use of it.
#[derive(renzora_plugin::Component)]
#[repr(C)]
struct Spinner {
    speed: f32,
}

impl Default for Spinner {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

fn spin(mut q: ecs::Query<(&mut ecs::Transform, &Spinner)>, time: ecs::Res<ecs::Time>) {
    for (t, s) in &mut q {
        t.rotate_y(s.speed * time.delta_secs());
    }
}

/// Stands in for a real plugin's `renzora_plugin_init`.
unsafe extern "C" fn spinner_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Spinner>()
        .add_systems(ecs::Schedule::Update, spin);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

/// Tag the streaming plugin below uses. Arbitrary — the point is only that the
/// host routes by it.
#[cfg(feature = "http")]
const STREAM_TAG: u64 = 0xBEEF;

/// What the streaming plugin saw, in order: `(body, kind)`.
///
/// A `static` because the "plugin" here is compiled into the test binary, so it
/// really can share one — a `dlopen`'d plugin would use its own. That is the
/// standard shape for plugin state anyway, since a system must be zero-sized
/// and cannot capture.
#[cfg(feature = "http")]
static STREAM_LOG: std::sync::Mutex<Vec<(String, u32)>> = std::sync::Mutex::new(Vec::new());

/// Drain every chunk available this frame, stopping at the terminal one.
#[cfg(feature = "http")]
fn read_stream(http: renzora_plugin::http::Http) {
    while let Some(chunk) = http.poll_stream(STREAM_TAG) {
        if let Ok(mut log) = STREAM_LOG.lock() {
            log.push((chunk.data.clone(), chunk.kind.0));
        }
        if chunk.is_last() {
            break;
        }
    }
}

#[cfg(feature = "http")]
unsafe extern "C" fn stream_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.add_systems(ecs::Schedule::Update, read_stream);
    sys::InitResult::Ok
}

/// A streaming response reaches the plugin in order, exactly once each, and the
/// end marker is consumed rather than re-delivered forever.
///
/// That last clause is the one worth a test. A terminal chunk has an empty body,
/// so the guest's "am I the consuming pass" test — a non-null buffer with
/// capacity — would be false for it if the guest sized its buffer from
/// `body_len`. It allocates a one-byte scratch buffer instead; without that the
/// host never removes the marker and every subsequent frame re-reads it, so the
/// plugin's `while let` never terminates.
#[cfg(feature = "http")]
#[test]
fn a_streamed_response_arrives_in_order_and_ends_once() {
    use renzora_plugin::host::{PluginHttpInbox, PluginHttpResponse};

    let mut app = test_app();
    let _guard = plugin_lock();
    STREAM_LOG.lock().unwrap().clear();
    // The real engine gets this from `plugin_bridge::install`, which lives in
    // renzora_scripting — the crate that owns the HTTP client. This test app has
    // no bridge, so the landing zone has to be added by hand.
    app.init_resource::<PluginHttpInbox>();

    assert_eq!(
        unsafe { abi_host::init_plugin(app.world_mut(), stream_init) },
        sys::InitResult::Ok,
    );

    let queue = |world: &mut World, body: &str, kind: sys::HttpChunkKind| {
        world
            .resource_mut::<PluginHttpInbox>()
            .0
            .push(PluginHttpResponse {
                tag: STREAM_TAG,
                status: 200,
                body: body.to_string(),
                chunk: Some(kind),
            });
    };

    queue(app.world_mut(), "one", sys::HttpChunkKind::Data);
    queue(app.world_mut(), "two", sys::HttpChunkKind::Data);
    queue(app.world_mut(), "", sys::HttpChunkKind::End);

    app.update();

    let seen = STREAM_LOG.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![
            ("one".to_string(), sys::HttpChunkKind::Data.0),
            ("two".to_string(), sys::HttpChunkKind::Data.0),
            (String::new(), sys::HttpChunkKind::End.0),
        ],
        "chunks arrived out of order, were dropped, or the end marker was missed"
    );

    // Nothing is left behind: an unconsumed end marker would sit here forever
    // and be re-delivered on every future frame.
    assert!(
        app.world().resource::<PluginHttpInbox>().0.is_empty(),
        "the inbox still holds chunks after the plugin drained the stream"
    );

    // And a second frame adds nothing, which is the re-delivery bug stated as an
    // assertion rather than as a comment.
    app.update();
    assert_eq!(
        STREAM_LOG.lock().unwrap().len(),
        3,
        "a chunk was delivered more than once"
    );
}

/// A whole-body response and a stream chunk share one queue, and `poll` must not
/// hand over a piece of a stream as if it were a complete body.
#[cfg(feature = "http")]
#[test]
fn poll_does_not_steal_stream_chunks() {
    use renzora_plugin::host::{PluginHttpInbox, PluginHttpResponse};

    let mut app = test_app();
    let _guard = plugin_lock();
    STREAM_LOG.lock().unwrap().clear();
    // The real engine gets this from `plugin_bridge::install`, which lives in
    // renzora_scripting — the crate that owns the HTTP client. This test app has
    // no bridge, so the landing zone has to be added by hand.
    app.init_resource::<PluginHttpInbox>();

    assert_eq!(
        unsafe { abi_host::init_plugin(app.world_mut(), stream_init) },
        sys::InitResult::Ok,
    );

    // A chunk queued FIRST, so a `poll` that matched on tag alone would take it.
    app.world_mut()
        .resource_mut::<PluginHttpInbox>()
        .0
        .push(PluginHttpResponse {
            tag: STREAM_TAG,
            status: 200,
            body: "chunk".into(),
            chunk: Some(sys::HttpChunkKind::End),
        });

    app.update();

    assert_eq!(
        STREAM_LOG.lock().unwrap().len(),
        1,
        "the stream poller did not receive its chunk"
    );
}

/// Serialises every test that loads a plugin.
///
/// A component's id is cached on a per-type `static`, and a system reads that
/// cache at run time to find its resource slot. That is correct for the real
/// engine, where there is exactly one host world per process — but this binary
/// runs many worlds concurrently, and two tests sharing a type also share its
/// cell while registering into different worlds. They usually agree, because a
/// fresh `MinimalPlugins` world assigns ids in the same order, which is what
/// makes the resulting failure an occasional flake rather than an honest error.
///
/// The lock is poison-tolerant: a test that panics while holding it has already
/// reported its own failure, and turning that into a cascade of unrelated
/// failures would bury the real one.
fn plugin_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Minimal app: no rendering, no windowing — just the ECS, the clock, and a
/// schedule for the plugin to insert into.
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // The plugin resolves `Transform` by type path through the reflection
    // registry, so the host must have registered it. This mirrors the real
    // contract: a host component is only reachable from a plugin if it's
    // `register_type`'d.
    app.register_type::<Transform>();
    // Bevy registers components lazily, so `Transform` has no `ComponentId` until
    // something uses it — and a plugin loading at startup would resolve nothing.
    // The real engine gets this for free via `TransformPlugin`; `MinimalPlugins`
    // does not include it.
    app.world_mut().register_component::<Transform>();
    app
}

#[test]
fn plugin_registers_and_mutates_host_components() {
    let mut app = test_app();
    let _guard = plugin_lock();

    let result = abi_host::init_plugin(app.world_mut(), spinner_init);
    assert_eq!(result, sys::InitResult::Ok, "plugin init failed");

    // The host learned about a component it has no Rust type for.
    let spinner_id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get(<Spinner as renzora_plugin::ecs::Component>::TYPE_PATH)
        .copied()
        .expect("Spinner was not registered");

    // Spawn an entity carrying both — the plugin component goes in by raw bytes,
    // exactly as a scene loader or the inspector would have to do it.
    let spinner = Spinner { speed: 2.0 };
    let entity = app.world_mut().spawn(Transform::IDENTITY).id();
    // SAFETY: `spinner_id` was registered with this exact layout.
    unsafe {
        let mut bytes = std::slice::from_raw_parts(
            (&spinner as *const Spinner).cast::<u8>(),
            size_of::<Spinner>(),
        )
        .to_vec();
        let ptr =
            bevy::ptr::OwningPtr::new(std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()));
        app.world_mut()
            .entity_mut(entity)
            .insert_by_id(spinner_id, ptr);
        std::mem::forget(bytes);
    }

    let before = app.world().entity(entity).get::<Transform>().unwrap().rotation;

    // Two updates so the clock has a non-zero delta on the second.
    app.update();
    app.update();

    let after = app.world().entity(entity).get::<Transform>().unwrap().rotation;

    assert_ne!(
        before, after,
        "plugin system did not mutate the host's Transform — the dispatcher, \
         the marshalling, or the write-back pass is broken"
    );
}

#[test]
fn unknown_host_component_fails_loudly() {
    // Same plugin, but `Transform` is never registered — `component_id_by_name`
    // must return INVALID and the plugin must refuse to load rather than
    // silently install a system whose query matches nothing.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);

    let result = abi_host::init_plugin(app.world_mut(), spinner_init);
    assert_eq!(result, sys::InitResult::Failed);
}

// ── Filter terms ─────────────────────────────────────────────────────────────
//
// A test-local plugin rather than the spinner, because filters need components
// the test controls. Without this, `With`/`Without` could be silently ignored
// and every assertion above would still pass.

const MARKER: &str = "test::Marker";
const EXCLUDED: &str = "test::Excluded";

/// Stamps a sentinel into `translation.x` so the test can see exactly which
/// entities the query matched.
unsafe extern "C" fn stamp(call: *const sys::SystemCall) -> sys::SystemStatus {
    let call = &*call;
    let view = &*call.views;
    for row in 0..view.entity_count {
        let t = &mut *(*view.cells.add(row * view.cell_count) as *mut sys::Transform);
        t.translation.x = 1.0;
    }
    // Raw `sys` systems own their own panic discipline; this one cannot panic.
    sys::SystemStatus::Ok
}

unsafe extern "C" fn filter_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let i = &*iface;
    let zst = |name| {
        (i.register_component)(
            host,
            &sys::ComponentDesc {
                // Zero-sized marker: size 0 / align 1 is a legal layout and the
                // cheapest possible filter component. No fields and no default —
                // a marker has nothing to edit and nothing to initialise.
                name: sys::StrRef::new(name),
                size: 0,
                align: 1,
                drop: None,
                display_name: sys::StrRef::new(""),
                fields: std::ptr::null(),
                field_count: 0,
                default_init: None,
            },
        )
    };
    let marker = zst(MARKER);
    let excluded = zst(EXCLUDED);
    let transform = (i.component_id_by_name)(
        host,
        sys::StrRef::new("bevy_transform::components::transform::Transform"),
    );

    let terms = [
        sys::Term { component: transform, access: sys::Access::Write },
        sys::Term { component: marker, access: sys::Access::With },
        sys::Term { component: excluded, access: sys::Access::Without },
    ];
    let query = sys::QueryDesc {
        terms: terms.as_ptr(),
        term_count: terms.len(),
    };
    (i.add_system)(
        host,
        &sys::SystemDesc {
            entry: stamp,
            schedule: sys::Schedule::Update,
            queries: &query,
            query_count: 1,
            resources: std::ptr::null(),
            resource_count: 0,
            user: std::ptr::null_mut(),
            flags: 0,
        },
    );
    sys::InitResult::Ok
}

#[test]
fn with_and_without_actually_filter() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(abi_host::init_plugin(app.world_mut(), filter_init), sys::InitResult::Ok);

    let ids = app.world().resource::<abi_host::PluginComponents>().0.clone();
    let (marker, excluded) = (ids[MARKER], ids[EXCLUDED]);

    let add_zst = |app: &mut App, e: bevy::prelude::Entity, id| unsafe {
        // Zero-sized: any aligned non-null pointer is valid, and nothing is read.
        let ptr = bevy::ptr::OwningPtr::new(std::ptr::NonNull::<u8>::dangling());
        app.world_mut().entity_mut(e).insert_by_id(id, ptr);
    };

    let matched = app.world_mut().spawn(Transform::IDENTITY).id();
    let has_excluded = app.world_mut().spawn(Transform::IDENTITY).id();
    let no_marker = app.world_mut().spawn(Transform::IDENTITY).id();
    add_zst(&mut app, matched, marker);
    add_zst(&mut app, has_excluded, marker);
    add_zst(&mut app, has_excluded, excluded);

    app.update();

    let x = |app: &App, e: bevy::prelude::Entity| {
        app.world().entity(e).get::<Transform>().unwrap().translation.x
    };
    assert_eq!(x(&app, matched), 1.0, "With<Marker> should have matched");
    assert_eq!(x(&app, has_excluded), 0.0, "Without<Excluded> should have rejected this");
    assert_eq!(x(&app, no_marker), 0.0, "entity lacking Marker should not match");
}

#[test]
fn schema_reaches_the_host() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), spinner_init),
        sys::InitResult::Ok
    );

    let schemas = app.world().resource::<abi_host::PluginComponentSchemas>();
    let info = schemas
        .0
        .iter()
        .find(|s| s.type_path == <Spinner as renzora_plugin::ecs::Component>::TYPE_PATH)
        .expect("no schema recorded for Spinner");

    assert_eq!(info.display_name, "Spinner");
    assert_eq!(info.size, size_of::<Spinner>());
    assert_eq!(info.fields.len(), 1);
    assert_eq!(info.fields[0].name, "speed");
    assert_eq!(info.fields[0].kind, sys::FieldKind::F32);
    assert_eq!(info.fields[0].offset, 0);

    // The default must be a *useful* value, not zeroed — a Spinner with speed 0
    // is indistinguishable from a broken plugin.
    assert_eq!(info.default_value.len(), size_of::<Spinner>());
    let speed = f32::from_ne_bytes(info.default_value[0..4].try_into().unwrap());
    assert_eq!(speed, 1.0, "default must be useful, not zeroed");
}

// ── Panic containment ────────────────────────────────────────────────────────

/// Increments, then panics. Written against the ergonomic layer because that is
/// where the `catch_unwind` guard lives — testing `sys` directly would prove
/// nothing about the thing under test.
fn panicking_system(mut q: renzora_plugin::ecs::Query<&mut renzora_plugin::ecs::Transform>) {
    for t in &mut q {
        t.translation.x += 1.0;
        panic!("deliberate test panic");
    }
}

unsafe extern "C" fn panic_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = renzora_plugin::ecs::App::new(iface, host);
    app.add_systems(renzora_plugin::ecs::Schedule::Update, panicking_system);
    sys::InitResult::Ok
}

#[test]
fn a_panicking_system_does_not_kill_the_host() {
    // NOTE: this test prints a panic + backtrace on purpose. That output is the
    // guard working, not a failure.
    let mut app = test_app();
    assert_eq!(abi_host::init_plugin(app.world_mut(), panic_init), sys::InitResult::Ok);

    let e = app.world_mut().spawn(Transform::IDENTITY).id();

    // Reaching the second update at all is most of the point — an unguarded
    // panic unwinding out of an `extern "C"` fn aborts the process, and an
    // aborted test binary reports as a failure, not an assertion.
    app.update();
    app.update();
    app.update();

    let x = app.world().entity(e).get::<Transform>().unwrap().translation.x;
    assert_eq!(
        x, 0.0,
        "a panicking system's partial writes must not reach the world"
    );
}

/// Four fields, non-zero defaults — the shape that misbehaved in the editor
/// while a single-field component was fine.
#[derive(renzora_plugin::Component)]
#[repr(C)]
struct Multi {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
}

impl Default for Multi {
    fn default() -> Self {
        Self { a: 3.0, b: 1.0, c: 2.0, d: 4.0 }
    }
}

unsafe extern "C" fn multi_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Multi>();
    sys::InitResult::Ok
}

#[test]
fn multi_field_schema_and_default() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(abi_host::init_plugin(app.world_mut(), multi_init), sys::InitResult::Ok);

    let schemas = app.world().resource::<abi_host::PluginComponentSchemas>();
    let info = schemas
        .0
        .iter()
        .find(|s| s.type_path.ends_with("::Multi"))
        .expect("no schema for Multi");

    // Every field present, at its real offset.
    let names: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c", "d"], "schema lost fields");
    let offsets: Vec<usize> = info.fields.iter().map(|f| f.offset).collect();
    assert_eq!(offsets, [0, 4, 8, 12], "field offsets wrong");

    // And the default must be the real one, not zeroes.
    assert_eq!(info.size, 16);
    assert_eq!(info.default_value.len(), 16, "default not captured");
    let vals: Vec<f32> = info
        .default_value
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(vals, [3.0, 1.0, 2.0, 4.0], "default_init produced wrong bytes");
}

/// Insert a plugin component from raw bytes and read the values back.
///
/// The existing round-trip test inserts a `Spinner` but only asserts that the
/// Transform moved — any non-zero garbage speed still rotates, so it passed
/// while the inserted bytes were wrong. This asserts the values themselves.
#[test]
fn insert_by_id_writes_the_actual_bytes() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(abi_host::init_plugin(app.world_mut(), multi_init), sys::InitResult::Ok);

    let (cid, default_value) = {
        let s = app.world().resource::<abi_host::PluginComponentSchemas>();
        let i = s.0.iter().find(|s| s.type_path.ends_with("::Multi")).unwrap();
        (i.id, i.default_value.clone())
    };

    let e = app.world_mut().spawn_empty().id();
    unsafe {
        let mut bytes = default_value.clone();
        let ptr = bevy::ptr::OwningPtr::new(
            std::ptr::NonNull::new(bytes.as_mut_ptr()).unwrap().cast(),
        );
        app.world_mut().entity_mut(e).insert_by_id(cid, ptr);
        std::mem::forget(bytes);
    }

    let read: Vec<f32> = unsafe {
        let ptr = app.world().entity(e).get_by_id(cid).unwrap();
        std::slice::from_raw_parts(ptr.as_ptr().cast::<f32>(), 4).to_vec()
    };
    assert_eq!(
        read,
        [3.0, 1.0, 2.0, 4.0],
        "inserted component does not hold the default values"
    );
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Spawns one entity carrying a `Multi`, then removes itself so it runs once.
fn spawner(mut q: ecs::Query<&mut ecs::Transform>, mut cmds: ecs::Commands) {
    for t in &mut q {
        // Marks the trigger entity as done by moving it, so the assertion can
        // tell the system actually ran.
        t.translation.x = 42.0;
    }
    // Bevy's idiom: a bundle tuple, not spawn_empty-then-insert.
    cmds.spawn((Multi::default(), ecs::Transform::IDENTITY));
}

unsafe extern "C" fn spawner_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Multi>()
        .add_systems(ecs::Schedule::Update, spawner);
    sys::InitResult::Ok
}

#[test]
fn a_plugin_can_spawn_and_insert() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(abi_host::init_plugin(app.world_mut(), spawner_init), sys::InitResult::Ok);

    let cid = {
        let s = app.world().resource::<abi_host::PluginComponentSchemas>();
        s.0.iter().find(|s| s.type_path.ends_with("::Multi")).unwrap().id
    };

    let trigger = app.world_mut().spawn(Transform::IDENTITY).id();
    app.update();

    assert_eq!(
        app.world().entity(trigger).get::<Transform>().unwrap().translation.x,
        42.0,
        "the system did not run"
    );

    // One entity spawned BY THE PLUGIN, carrying a component the engine has no
    // Rust type for, with the values the plugin chose.
    let spawned: Vec<bevy::prelude::Entity> = app
        .world_mut()
        .iter_entities()
        .filter(|e| e.contains_id(cid))
        .map(|e| e.id())
        .collect();
    assert_eq!(spawned.len(), 1, "expected exactly one spawned entity");

    let vals: Vec<f32> = unsafe {
        let p = app.world().entity(spawned[0]).get_by_id(cid).unwrap();
        std::slice::from_raw_parts(p.as_ptr().cast::<f32>(), 4).to_vec()
    };
    assert_eq!(vals, [3.0, 1.0, 2.0, 4.0], "inserted component holds wrong data");
}

#[test]
fn reserved_ids_are_not_reused_across_frames() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(abi_host::init_plugin(app.world_mut(), spawner_init), sys::InitResult::Ok);
    let cid = {
        let s = app.world().resource::<abi_host::PluginComponentSchemas>();
        s.0.iter().find(|s| s.type_path.ends_with("::Multi")).unwrap().id
    };

    app.world_mut().spawn(Transform::IDENTITY);
    app.update();
    let after_one = app.world_mut().iter_entities().filter(|e| e.contains_id(cid)).count();
    app.update();
    let after_two = app.world_mut().iter_entities().filter(|e| e.contains_id(cid)).count();

    // The spawner runs every frame, so two frames must produce two distinct
    // entities. If reserved ids were being recycled the second spawn would land
    // on the first entity and the count would stay at 1.
    assert_eq!(after_one, 1);
    assert_eq!(after_two, 2, "second spawn did not produce a new entity");
}

// ── Resources and query filters ──────────────────────────────────────────────

/// A plugin-owned resource — global state the host has no Rust type for.
#[derive(renzora_plugin::Resource)]
#[repr(C)]
struct Score {
    total: i32,
}

impl Clone for Score {
    fn clone(&self) -> Self {
        Self { total: self.total }
    }
}
impl Copy for Score {}

impl Default for Score {
    fn default() -> Self {
        Self { total: 7 }
    }
}

#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Tag {
    _v: f32,
}

#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Boost {
    amount: i32,
}

/// Counts matched entities into a resource, so the assertion reads the plugin's
/// own state rather than a side effect on host components.
/// Its own types rather than `Tag`/`Score`: the id a plugin resolves is cached on
/// a per-type static, so two tests sharing a type also share that cell while
/// running in different worlds — the second to register wins and the first reads
/// an id that means nothing in its own world. One host world per process is the
/// real contract; the test binary is the exception.
#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Counted {
    _v: f32,
}

#[derive(renzora_plugin::Resource, Clone, Copy)]
#[repr(C)]
struct Tally {
    total: i32,
}

impl Default for Tally {
    fn default() -> Self {
        Self { total: 7 }
    }
}

fn count_all(q: ecs::Query<ecs::Entity, ecs::With<Counted>>, mut tally: ecs::ResMut<Tally>) {
    tally.total = q.len() as i32;
}

unsafe extern "C" fn resource_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Counted>()
        .init_resource::<Tally>()
        .add_systems(ecs::Schedule::Update, count_all);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

/// Read a plugin resource back out of the world by id.
fn read_resource<T: Copy>(world: &World, id: bevy::ecs::component::ComponentId) -> T {
    let ptr = world
        .get_resource_by_id(id)
        .expect("resource was never inserted");
    // SAFETY: registered with `T`'s layout by the plugin under test.
    unsafe { ptr.deref::<T>().to_owned() }
}

/// Components and resources share one registry here because they share one in
/// Bevy — a resource is a component on a hidden entity.
fn plugin_id(world: &World, type_path: &str) -> bevy::ecs::component::ComponentId {
    *world
        .resource::<abi_host::PluginComponents>()
        .0
        .get(type_path)
        .expect("resource was not registered")
}

#[test]
fn a_plugin_owns_a_resource_and_it_survives_registration() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), resource_init),
        sys::InitResult::Ok
    );

    let id = plugin_id(
        app.world(),
        <Tally as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );

    // `init_resource` inserted the plugin's `Default`, not zeroes — a resource
    // that silently starts at 0 looks identical to one a system already reset.
    assert_eq!(read_resource::<Tally>(app.world(), id).total, 7);

    // Registration goes through the component path, so without an explicit flag
    // a resource shows up in Add Component and lands on whatever entity happened
    // to be selected.
    let schemas = app.world().resource::<abi_host::PluginComponentSchemas>();
    let info = schemas
        .0
        .iter()
        .find(|i| i.id == id)
        .expect("resource has no schema");
    assert!(info.is_resource, "a resource was recorded as an addable component");
    assert!(
        schemas.0.iter().any(|i| !i.is_resource),
        "components in the same plugin must stay addable"
    );

    app.update();
}

#[test]
fn a_system_writes_through_res_mut() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), resource_init),
        sys::InitResult::Ok
    );
    let id = plugin_id(
        app.world(),
        <Tally as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );

    let counted_id = plugin_id(
        app.world(),
        <Counted as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    for _ in 0..3 {
        let e = app.world_mut().spawn_empty().id();
        insert_raw(app.world_mut(), e, counted_id, &Counted::default());
    }
    // One the filter must exclude, so a query that matched everything would
    // still be caught.
    app.world_mut().spawn(Transform::IDENTITY);
    app.update();

    // The query has a filter but no data terms, so this also covers the
    // "filter-only query still yields rows" path.
    assert_eq!(
        read_resource::<Tally>(app.world(), id).total,
        3,
        "ResMut write-back did not reach the world"
    );
}

/// Sums `Boost` where present. The point is that entities *without* `Boost` still
/// match — an `Option` that filtered would make this identical to `&Boost`.
fn sum_optional(q: ecs::Query<(&Tag, Option<&Boost>)>, mut score: ecs::ResMut<Score>) {
    let mut seen = 0;
    let mut sum = 0;
    for (_, b) in &q {
        seen += 1;
        if let Some(b) = b {
            sum += b.amount;
        }
    }
    score.total = seen * 100 + sum;
}

unsafe extern "C" fn optional_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Tag>()
        .register_component::<Boost>()
        .init_resource::<Score>()
        .add_systems(ecs::Schedule::Update, sum_optional);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn optional_data_matches_entities_that_lack_it() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), optional_init),
        sys::InitResult::Ok
    );
    let tag_id = plugin_id(
        app.world(),
        <Tag as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let boost_id = plugin_id(
        app.world(),
        <Boost as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let score_id = plugin_id(
        app.world(),
        <Score as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );

    let with = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), with, tag_id, &Tag::default());
    insert_raw(app.world_mut(), with, boost_id, &Boost { amount: 5 });
    let without = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), without, tag_id, &Tag::default());

    app.update();

    // Two matched (so the `Option` did not filter) and only one contributed an
    // amount (so the absent cell really did arrive as `None`, not as zeroes that
    // happen to sum the same).
    assert_eq!(
        read_resource::<Score>(app.world(), score_id).total,
        2 * 100 + 5,
        "an `Option<&T>` term filtered the query instead of yielding None"
    );
}

/// Insert a plugin component onto an entity by raw bytes, the way the inspector
/// and the scene loader have to.
fn insert_raw<T>(world: &mut World, entity: Entity, id: bevy::ecs::component::ComponentId, value: &T) {
    // SAFETY: `id` was registered with `T`'s exact layout.
    unsafe {
        let mut bytes =
            std::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()).to_vec();
        let ptr =
            bevy::ptr::OwningPtr::new(std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()));
        world.entity_mut(entity).insert_by_id(id, ptr);
        std::mem::forget(bytes);
    }
}

/// Matches an entity with either marker. Without `Or` this needs two systems and
/// a way to merge their results.
fn count_either(
    q: ecs::Query<ecs::Entity, ecs::Or<(ecs::With<Tag>, ecs::With<Boost>)>>,
    mut score: ecs::ResMut<Score>,
) {
    score.total = q.len() as i32;
}

unsafe extern "C" fn or_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Tag>()
        .register_component::<Boost>()
        .init_resource::<Score>()
        .add_systems(ecs::Schedule::Update, count_either);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn or_matches_either_branch() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), or_init),
        sys::InitResult::Ok
    );
    let tag_id = plugin_id(
        app.world(),
        <Tag as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let boost_id = plugin_id(
        app.world(),
        <Boost as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let score_id = plugin_id(
        app.world(),
        <Score as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );

    let a = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), a, tag_id, &Tag::default());
    let b = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), b, boost_id, &Boost { amount: 1 });
    let c = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), c, tag_id, &Tag::default());
    insert_raw(app.world_mut(), c, boost_id, &Boost { amount: 2 });
    // Matches neither branch.
    app.world_mut().spawn(Transform::IDENTITY);

    app.update();

    assert_eq!(
        read_resource::<Score>(app.world(), score_id).total,
        3,
        "`Or` matched the wrong set — 3 entities carry at least one marker"
    );
}

/// Its own types, for the reason given on [`Counted`].
#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Scaled {
    factor: f32,
}

#[derive(renzora_plugin::Resource, Clone, Copy)]
#[repr(C)]
struct Gain {
    amount: f32,
}

impl Default for Gain {
    fn default() -> Self {
        Self { amount: 3.0 }
    }
}

/// Reads a resource without writing it. A `Res` and a `ResMut` reach the value by
/// different accessors host-side, and only one of them works for a system that
/// declared read access — so covering `ResMut` alone leaves half the path untested.
fn apply_gain(mut q: ecs::Query<&mut Scaled>, gain: ecs::Res<Gain>) {
    for s in &mut q {
        s.factor = gain.amount;
    }
}

unsafe extern "C" fn read_only_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Scaled>()
        .init_resource::<Gain>()
        .add_systems(ecs::Schedule::Update, apply_gain);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn a_system_reads_a_resource_it_does_not_write() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), read_only_init),
        sys::InitResult::Ok
    );
    let scaled_id = plugin_id(
        app.world(),
        <Scaled as renzora_plugin::ecs::Component>::TYPE_PATH,
    );

    let e = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), e, scaled_id, &Scaled { factor: 0.0 });

    app.update();

    let ptr = app
        .world()
        .entity(e)
        .get_by_id(scaled_id)
        .expect("component vanished");
    // SAFETY: registered with `Scaled`'s layout.
    let got = unsafe { ptr.deref::<Scaled>().factor };
    assert_eq!(
        got, 3.0,
        "`Res<T>` handed the system a null pointer — a read-only declaration \
         cannot be resolved with the mutable accessor"
    );
}

// ── Correctness regressions ──────────────────────────────────────────────────

/// Inserting a HOST component from a plugin sends the frozen mirror, which is a
/// different size and a different layout from the real thing —
/// `sys::Transform` is 40 bytes with rotation at offset 12, `bevy::Transform` is
/// 48 with rotation at 16. Passing the bytes through unmarshalled read 8 bytes
/// past the buffer and produced a scrambled transform.
#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Placer {
    _v: f32,
}

fn place(q: ecs::Query<ecs::Entity, ecs::With<Placer>>, mut cmds: ecs::Commands) {
    for e in &q {
        cmds.entity(e).insert(ecs::Transform {
            translation: ecs::Vec3 { x: 1.0, y: 2.0, z: 3.0 },
            rotation: ecs::Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
            scale: ecs::Vec3 { x: 4.0, y: 5.0, z: 6.0 },
        });
    }
}

unsafe extern "C" fn placer_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Placer>()
        // Required: `insert` resolves the id the host assigned at init, and a
        // type the plugin only ever inserts never gets one.
        .register_component::<ecs::Transform>()
        .add_systems(ecs::Schedule::Update, place);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn inserting_a_host_transform_marshals_rather_than_memcpy() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), placer_init),
        sys::InitResult::Ok
    );
    let placer_id = plugin_id(
        app.world(),
        <Placer as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let e = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), e, placer_id, &Placer::default());

    app.update();
    // The insert is a Command, so it lands during the next apply.
    app.update();

    let t = *app
        .world()
        .entity(e)
        .get::<Transform>()
        .expect("Transform was never inserted");
    assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(t.scale, Vec3::new(4.0, 5.0, 6.0));
    assert!(
        t.rotation.is_normalized(),
        "rotation came through as {:?} — the 40-byte mirror was memcpy'd into a \
         48-byte type instead of being marshalled",
        t.rotation
    );
}

/// `Or<T>` is itself a `QueryFilter`, so nesting one is ordinary code. A flat
/// branch walk drops the inner brackets while still emitting the inner terms,
/// which silently turns the inner `Or` into an `AND`.
#[derive(renzora_plugin::Resource, Clone, Copy)]
#[repr(C)]
struct NestedTally {
    total: i32,
}

impl Default for NestedTally {
    fn default() -> Self {
        Self { total: -1 }
    }
}

fn count_nested(
    q: ecs::Query<
        ecs::Entity,
        ecs::Or<(ecs::With<Tag>, ecs::Or<(ecs::With<Boost>, ecs::With<Scaled>)>)>,
    >,
    mut tally: ecs::ResMut<NestedTally>,
) {
    tally.total = q.len() as i32;
}

unsafe extern "C" fn nested_or_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Tag>()
        .register_component::<Boost>()
        .register_component::<Scaled>()
        .init_resource::<NestedTally>()
        .add_systems(ecs::Schedule::Update, count_nested);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn a_nested_or_still_matches_either_inner_branch() {
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), nested_or_init),
        sys::InitResult::Ok
    );
    let tag = plugin_id(app.world(), <Tag as renzora_plugin::ecs::Component>::TYPE_PATH);
    let boost = plugin_id(app.world(), <Boost as renzora_plugin::ecs::Component>::TYPE_PATH);
    let scaled = plugin_id(app.world(), <Scaled as renzora_plugin::ecs::Component>::TYPE_PATH);
    let tally = plugin_id(
        app.world(),
        <NestedTally as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );

    // One per branch. Collapsing the inner `Or` to an AND would need Boost AND
    // Scaled together and would match only the third.
    let a = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), a, tag, &Tag::default());
    let b = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), b, boost, &Boost { amount: 1 });
    let c = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), c, scaled, &Scaled { factor: 1.0 });
    app.world_mut().spawn(Transform::IDENTITY);

    app.update();

    assert_eq!(
        read_resource::<NestedTally>(app.world(), tally).total,
        3,
        "a nested `Or` collapsed into an `AND`"
    );
}

#[test]
fn a_resource_is_listed_once_however_many_systems_take_it() {
    let mut app = test_app();
    let _guard = plugin_lock();
    // `resource_init` registers Tally once; two systems both naming it would
    // each drive a registration, and an unguarded push listed it per system.
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), resource_init),
        sys::InitResult::Ok
    );
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), resource_init),
        sys::InitResult::Ok
    );
    let id = plugin_id(
        app.world(),
        <Tally as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );
    let listed = app.world().resource::<abi_host::PluginResources>();
    assert_eq!(
        listed.0.iter().filter(|i| **i == id).count(),
        1,
        "the same resource was listed once per referencing system"
    );
}

/// A destructor cannot be honoured across the boundary yet, so it must be
/// refused with a message that says so. This used to be
/// `desc.drop.map(|_| unimplemented!(..))`, which reads like a guard but is not
/// one — `Option::map` evaluates its body, so it panicked and `guard_host`
/// reported only "host call 'register_component' panicked".
/// Registers a component that declares a destructor, by hand — the derive never
/// emits one, so this is the shape a plugin author reaches for deliberately.
unsafe extern "C" fn droppy_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    unsafe extern "C" fn nop_drop(_: *mut u8) {}
    static FIELDS: &[sys::FieldDesc] = &[];
    let desc = sys::ComponentDesc {
        name: sys::StrRef::new("test::Droppy"),
        size: 4,
        align: 4,
        drop: Some(nop_drop),
        display_name: sys::StrRef::new("Droppy"),
        fields: FIELDS.as_ptr(),
        field_count: 0,
        default_init: None,
    };
    let id = ((*iface).register_component)(host, &desc);
    if id.is_valid() {
        // Reaching here means the host accepted a component whose destructor it
        // can never run.
        return sys::InitResult::Ok;
    }
    sys::InitResult::Failed
}

#[test]
fn a_component_declaring_a_destructor_is_refused_not_panicked() {
    let mut app = test_app();
    let _guard = plugin_lock();
    let result = abi_host::init_plugin(app.world_mut(), droppy_init);
    assert_eq!(
        result,
        sys::InitResult::Failed,
        "a component with a destructor was accepted; its drop would never run"
    );
    // The refusal must be a refusal, not a caught panic — `guard_host` turns a
    // panic into the same INVALID id, so the distinguishing evidence is that the
    // component really was not registered.
    // `get_resource`, not `resource`: a world where nothing registered
    // successfully has no `PluginComponents` at all, and that is the pass case.
    assert!(
        app.world()
            .get_resource::<abi_host::PluginComponents>()
            .is_none_or(|c| !c.0.contains_key("test::Droppy")),
        "the component was registered despite being refused"
    );
}

// ── ABI hygiene ──────────────────────────────────────────────────────────────

/// Registers a component whose field claims a kind this build has never heard
/// of — exactly what a plugin built against a newer ABI writes.
///
/// The seven plugin-written enums are newtypes over `u32` rather than
/// `#[repr(u32)]` enums for this case alone. Materialising an out-of-range
/// discriminant into a Rust enum is undefined behaviour, and `register_component`
/// reads this schema straight out of plugin memory with `from_raw_parts`, so the
/// enum would be constructed before anything had a chance to check it.
unsafe extern "C" fn future_kind_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    static FIELDS: &[sys::FieldDesc] = &[
        sys::FieldDesc {
            name: sys::StrRef::new("known"),
            kind: sys::FieldKind::F32,
            offset: 0,
        },
        sys::FieldDesc {
            name: sys::StrRef::new("from_the_future"),
            // Deliberately out of range for this build.
            kind: sys::FieldKind(9999),
            offset: 4,
        },
    ];
    let desc = sys::ComponentDesc {
        name: sys::StrRef::new("test::FutureKind"),
        size: 8,
        align: 4,
        drop: None,
        display_name: sys::StrRef::new("FutureKind"),
        fields: FIELDS.as_ptr(),
        field_count: FIELDS.len(),
        default_init: None,
    };
    let id = ((*iface).register_component)(host, &desc);
    if id.is_valid() {
        sys::InitResult::Ok
    } else {
        sys::InitResult::Failed
    }
}

#[test]
fn a_field_kind_from_a_newer_abi_does_not_poison_the_host() {
    let _guard = plugin_lock();
    let mut app = test_app();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), future_kind_init),
        sys::InitResult::Ok,
        "one unrecognised field kind rejected the whole component"
    );

    let schemas = app.world().resource::<abi_host::PluginComponentSchemas>();
    let info = schemas
        .0
        .iter()
        .find(|i| i.type_path == "test::FutureKind")
        .expect("component was not registered");

    // Both fields survive: the schema is data, and forgetting the one it cannot
    // draw would silently change the component's shape.
    assert_eq!(info.fields.len(), 2);
    assert!(info.fields[0].kind.is_known());
    assert!(
        !info.fields[1].kind.is_known(),
        "an out-of-range kind was silently normalised — the inspector would then \
         draw it as whatever it collapsed to"
    );
    assert_eq!(info.fields[1].kind.0, 9999);
}

#[test]
fn an_access_kind_from_a_newer_abi_refuses_the_system() {
    let _guard = plugin_lock();
    let mut app = test_app();

    unsafe extern "C" fn entry(_: *const sys::SystemCall) -> sys::SystemStatus {
        sys::SystemStatus::Ok
    }
    unsafe extern "C" fn init(
        iface: *const sys::Interface,
        host: *mut sys::Host,
    ) -> sys::InitResult {
        let transform = ((*iface).component_id_by_name)(
            host,
            sys::StrRef::new("bevy_transform::components::transform::Transform"),
        );
        // A term this build cannot interpret. Skipping it would shift every
        // later cell index, so the system must be refused outright rather than
        // registered with data the plugin will read at the wrong offsets.
        let terms = [
            sys::Term {
                component: transform,
                access: sys::Access(200),
            },
            sys::Term {
                component: transform,
                access: sys::Access::Read,
            },
        ];
        let query = sys::QueryDesc {
            terms: terms.as_ptr(),
            term_count: terms.len(),
        };
        let status = ((*iface).add_system)(
            host,
            &sys::SystemDesc {
                entry,
                schedule: sys::Schedule::Update,
                queries: &query,
                query_count: 1,
                resources: core::ptr::null(),
                resource_count: 0,
                user: core::ptr::null_mut(),
                flags: 0,
            },
        );
        // Refused, and it says so — the point of the return value.
        if status == sys::RegisterStatus::UnknownComponent {
            sys::InitResult::Ok
        } else {
            sys::InitResult::Failed
        }
    }

    assert_eq!(
        abi_host::init_plugin(app.world_mut(), init),
        sys::InitResult::Ok,
        "`add_system` did not report the unknown access kind — it used to return \
         nothing, so a refusal was indistinguishable from success"
    );
    // And nothing was left half-registered.
    app.update();
    app.update();
}

// ── Multiple queries per system ──────────────────────────────────────────────

#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Source {
    value: f32,
}

#[derive(renzora_plugin::Component, Default)]
#[repr(C)]
struct Sink {
    value: f32,
}

#[derive(renzora_plugin::Resource, Clone, Copy, Default)]
#[repr(C)]
struct Totals {
    sources: i32,
    sinks: i32,
}

/// Two queries in one system, over **disjoint** component sets.
///
/// A single flat term list could not express this: both queries' terms merged
/// into one builder and AND-ed, so this matched only entities carrying `Source`
/// AND `Sink` — and both parameters then read the same cells, so `sinks` saw
/// `Source` bytes.
fn pump(
    sources: ecs::Query<&Source>,
    mut sinks: ecs::Query<&mut Sink>,
    mut totals: ecs::ResMut<Totals>,
) {
    let mut sum = 0.0;
    for s in &sources {
        sum += s.value;
    }
    totals.sources = sources.len() as i32;
    totals.sinks = sinks.len() as i32;
    for s in &mut sinks {
        s.value = sum;
    }
}

unsafe extern "C" fn pump_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Source>()
        .register_component::<Sink>()
        .init_resource::<Totals>()
        .add_systems(ecs::Schedule::Update, pump);
    if app.unresolved_component().is_some() || app.rejected_system().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn two_queries_in_one_system_stay_separate() {
    let _guard = plugin_lock();
    let mut app = test_app();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), pump_init),
        sys::InitResult::Ok
    );
    let source_id = plugin_id(
        app.world(),
        <Source as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let sink_id = plugin_id(
        app.world(),
        <Sink as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let totals_id = plugin_id(
        app.world(),
        <Totals as renzora_plugin::ecs::Resource>::TYPE_PATH,
    );

    // Deliberately disjoint: no entity carries both.
    for v in [1.0f32, 2.0, 4.0] {
        let e = app.world_mut().spawn_empty().id();
        insert_raw(app.world_mut(), e, source_id, &Source { value: v });
    }
    let sink_a = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), sink_a, sink_id, &Sink { value: 0.0 });
    let sink_b = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), sink_b, sink_id, &Sink { value: 0.0 });

    app.update();

    let totals = read_resource::<Totals>(app.world(), totals_id);
    assert_eq!(
        (totals.sources, totals.sinks),
        (3, 2),
        "the two queries merged — an AND of disjoint sets matches nothing"
    );

    for e in [sink_a, sink_b] {
        let ptr = app.world().entity(e).get_by_id(sink_id).unwrap();
        let got = unsafe { ptr.deref::<Sink>().value };
        assert_eq!(got, 7.0, "the second query wrote the wrong cells");
    }
}

/// The write-back used to be unconditional, so a system that merely *read*
/// through `&mut` marked every matched component changed every frame. That is
/// not a plugin-local cost: `Changed<Transform>` anywhere in the engine becomes
/// true whenever any plugin looks at a transform.
fn looks_but_does_not_touch(mut q: ecs::Query<&mut Spinner>) {
    for s in &mut q {
        // Read it, write the same value back. Nothing has changed.
        let v = s.speed;
        s.speed = v;
    }
}

unsafe extern "C" fn passive_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Spinner>()
        .add_systems(ecs::Schedule::Update, looks_but_does_not_touch);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

#[test]
fn an_unchanged_component_is_not_marked_changed() {
    let _guard = plugin_lock();
    let mut app = test_app();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), passive_init),
        sys::InitResult::Ok
    );
    let spinner_id = plugin_id(
        app.world(),
        <Spinner as renzora_plugin::ecs::Component>::TYPE_PATH,
    );
    let e = app.world_mut().spawn_empty().id();
    insert_raw(app.world_mut(), e, spinner_id, &Spinner { speed: 1.0 });

    // Two updates: the first clears the insert's own change tick.
    app.update();
    app.update();
    let ticks_after_quiet_frame = app
        .world()
        .entity(e)
        .get_change_ticks_by_id(spinner_id)
        .expect("component present")
        .changed;

    app.update();
    let ticks_now = app
        .world()
        .entity(e)
        .get_change_ticks_by_id(spinner_id)
        .expect("component present")
        .changed;

    assert_eq!(
        ticks_after_quiet_frame, ticks_now,
        "a system that wrote back an identical value still bumped the change \
         tick — every `&mut` cell was written unconditionally"
    );
}

// ── The bsn! macro ───────────────────────────────────────────────────────────

/// The macro's whole job is turning tokens back into source text. Getting the
/// spacing wrong is invisible at the call site and fatal at parse time, so the
/// output is asserted directly rather than only through a round trip.
#[test]
fn bsn_renders_tokens_back_to_parseable_source() {
    let renzora_plugin::ecs::Scene(text) = renzora_plugin::bsn! {
        #Cube
        Transform { translation: Vec3(0.0, 0.5, 0.0) }
        PointLight { intensity: 4000.0, shadows_enabled: true }
    };
    assert!(text.contains("#Cube"), "{text}");
    assert!(
        text.replace(' ', "").contains("Transform{translation:Vec3(0.0,0.5,0.0)}"),
        "{text}"
    );
    // Two components in a row must stay two words.
    assert!(
        !text.contains("}PointLight") && text.contains("} PointLight"),
        "adjacent components ran together: {text}"
    );
}

#[test]
fn a_negative_literal_survives_rendering() {
    // `TokenStream::to_string` renders this as `- 2.5`, which RON then refuses
    // to read as a number. The renderer exists because of this case.
    let renzora_plugin::ecs::Scene(text) = renzora_plugin::bsn! {
        Transform { translation: Vec3(-2.5, 4.5, 9.0) }
    };
    assert!(text.contains("-2.5"), "negative literal was split: {text}");
    assert!(!text.contains("- 2.5"), "{text}");
}

#[test]
fn a_path_keeps_its_colons() {
    let renzora_plugin::ecs::Scene(text) = renzora_plugin::bsn! {
        myplugin::Spinner { speed: 1.0 }
    };
    assert!(text.contains("myplugin::Spinner"), "{text}");
}

#[test]
fn children_and_lists_render_their_brackets() {
    let renzora_plugin::ecs::Scene(text) = renzora_plugin::bsn_list! {
        ( Marker Children [ Marker, Marker ] ),
        ( Marker )
    };
    assert!(text.contains('['), "{text}");
    assert!(text.contains(']'), "{text}");
    // A comma between entities needs the space that keeps `),(` readable, and
    // more importantly must survive at all.
    assert_eq!(text.matches("Marker").count(), 4, "{text}");
}

#[test]
fn a_string_literal_survives_with_its_quotes() {
    let renzora_plugin::ecs::Scene(text) = renzora_plugin::bsn! {
        Name("Hello, world")
    };
    assert!(text.contains(r#""Hello, world""#), "{text}");
}

// ── Hot reload ───────────────────────────────────────────────────────────────
//
// Driven through `init_plugin_gen` directly rather than by loading two real
// `.dll`s. What matters is the generation logic — that old systems retire, that a
// failed reload leaves the previous build running, and that state survives — and
// none of that needs a second compiled artifact to exercise.

/// A second "build" of the spinner: same component layout, but the system spins
/// the other way. Which build is live is then observable from the sign of the
/// rotation rather than from a counter.
fn spin_backwards(mut q: ecs::Query<(&mut ecs::Transform, &Spinner)>, time: ecs::Res<ecs::Time>) {
    for (t, s) in &mut q {
        t.rotate_y(-s.speed * time.delta_secs());
    }
}

unsafe extern "C" fn spinner_init_v2(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Spinner>()
        .add_systems(ecs::Schedule::Update, spin_backwards);
    if app.unresolved_component().is_some() {
        return sys::InitResult::Failed;
    }
    sys::InitResult::Ok
}

/// A build whose component grew a field — the case that must be refused, because
/// entities already carrying `Spinner` were allocated for the old layout.
///
/// Built as a raw `ComponentDesc` rather than through `ecs::App`, because the
/// derive necessarily reports the layout of a real Rust type and the point here is
/// to present the host with the SAME NAME at a DIFFERENT size. That is what a
/// recompiled plugin looks like from the host's side, and the host is what is
/// under test.
unsafe extern "C" fn spinner_init_relayout(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    const NAME: &str = "abi_roundtrip::Spinner";
    let fields = [
        sys::FieldDesc {
            name: sys::StrRef::new("speed"),
            kind: sys::FieldKind::F32,
            offset: 0,
        },
        sys::FieldDesc {
            name: sys::StrRef::new("wobble"),
            kind: sys::FieldKind::F32,
            offset: 4,
        },
    ];
    let desc = sys::ComponentDesc {
        name: sys::StrRef::new(NAME),
        size: 8,
        align: 4,
        drop: None,
        display_name: sys::StrRef::new(""),
        fields: fields.as_ptr(),
        field_count: fields.len(),
        default_init: None,
    };
    ((*iface).register_component)(host, &desc);
    sys::InitResult::Ok
}

/// Spawn an entity with a `Transform` and a raw `Spinner`, the way a scene loader
/// or the inspector has to — a plugin component has no Rust-side Bevy identity.
fn spawn_spinner(app: &mut App, speed: f32) -> Entity {
    let id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get(<Spinner as renzora_plugin::ecs::Component>::TYPE_PATH)
        .copied()
        .expect("Spinner was not registered");
    let spinner = Spinner { speed };
    let entity = app.world_mut().spawn(Transform::IDENTITY).id();
    // SAFETY: `id` was registered with this exact layout.
    unsafe {
        let mut bytes = std::slice::from_raw_parts(
            (&spinner as *const Spinner).cast::<u8>(),
            size_of::<Spinner>(),
        )
        .to_vec();
        let ptr =
            bevy::ptr::OwningPtr::new(std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()));
        app.world_mut().entity_mut(entity).insert_by_id(id, ptr);
        std::mem::forget(bytes);
    }
    entity
}

/// Read a raw `Spinner`'s `speed` back out of host storage.
fn spinner_speed(app: &App, entity: Entity) -> f32 {
    let id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get(<Spinner as renzora_plugin::ecs::Component>::TYPE_PATH)
        .copied()
        .expect("Spinner was not registered");
    let ptr = app
        .world()
        .entity(entity)
        .get_by_id(id)
        .expect("entity is not carrying Spinner");
    // SAFETY: `speed` is at offset 0 of the layout this id was registered with.
    unsafe { ptr.as_ptr().cast::<f32>().read_unaligned() }
}

#[test]
fn a_reload_retires_the_previous_builds_systems() {
    let mut app = test_app();
    let _guard = plugin_lock();
    let counter = abi_host::PluginGeneration::default();

    assert_eq!(
        abi_host::init_plugin_gen(app.world_mut(), spinner_init, counter.clone(), 0, 0),
        sys::InitResult::Ok
    );
    let e = spawn_spinner(&mut app, 1.0);
    // Two updates so the clock has a non-zero delta on the second.
    app.update();
    app.update();
    assert_ne!(
        app.world().entity(e).get::<Transform>().unwrap().rotation,
        Quat::IDENTITY,
        "the first build never ran"
    );

    // Reload: register v2, then bump the counter exactly as the loader does.
    assert_eq!(
        abi_host::init_plugin_gen(app.world_mut(), spinner_init_v2, counter.clone(), 1, 0),
        sys::InitResult::Ok
    );
    counter.store(1, std::sync::atomic::Ordering::Relaxed);

    let before = app.world().entity(e).get::<Transform>().unwrap().rotation;
    app.update();
    let after = app.world().entity(e).get::<Transform>().unwrap().rotation;
    assert_ne!(before, after, "neither build ran after the reload");

    // v2 spins the opposite way at the same speed, so if BOTH builds were live the
    // two rotations would cancel and `after` would equal `before`. A net change in
    // the negative direction is only possible with v1 retired.
    let (_, angle, _) = after.to_euler(EulerRot::XYZ);
    let (_, before_angle, _) = before.to_euler(EulerRot::XYZ);
    assert!(
        angle < before_angle,
        "expected the reloaded build to spin backwards; the old system is still running \
         (before {before_angle}, after {angle})"
    );
}

#[test]
fn a_reload_keeps_the_component_data_entities_already_have() {
    let mut app = test_app();
    let _guard = plugin_lock();
    let counter = abi_host::PluginGeneration::default();

    abi_host::init_plugin_gen(app.world_mut(), spinner_init, counter.clone(), 0, 0);
    let e = spawn_spinner(&mut app, 7.5);

    abi_host::init_plugin_gen(app.world_mut(), spinner_init_v2, counter.clone(), 1, 0);
    counter.store(1, std::sync::atomic::Ordering::Relaxed);
    app.update();

    // The whole reason hot-reload is tractable: plugin component data lives in the
    // host's ECS, so a swap never touches it. Nothing serialises or restores.
    assert_eq!(
        spinner_speed(&app, e),
        7.5,
        "the reload lost the entity's component data"
    );
}

#[test]
fn a_reload_that_changes_a_layout_is_refused() {
    let mut app = test_app();
    let _guard = plugin_lock();
    let counter = abi_host::PluginGeneration::default();

    abi_host::init_plugin_gen(app.world_mut(), spinner_init, counter.clone(), 0, 0);

    // Adding a field moves nothing for the plugin but invalidates every live
    // instance, so the host must refuse rather than let it register.
    let result = abi_host::init_plugin_gen(app.world_mut(), spinner_init_relayout, counter.clone(), 1, 0);
    assert_eq!(
        result,
        sys::InitResult::Failed,
        "a component that grew a field was accepted — live entities are now misread"
    );

    // And the refusal must leave the counter alone, or the previous build's
    // systems would retire with nothing replacing them.
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "a refused reload still retired the running build"
    );
}

/// A resource-only system: no `Query`, so nothing to iterate.
fn bump(mut score: ecs::ResMut<Score>) {
    score.total += 1;
}

unsafe extern "C" fn resource_only_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.init_resource::<Score>()
        .add_systems(ecs::Schedule::Update, bump);
    sys::InitResult::Ok
}

#[test]
fn a_system_with_no_queries_still_runs() {
    let mut app = test_app();
    let _guard = plugin_lock();

    assert_eq!(
        abi_host::init_plugin(app.world_mut(), resource_only_init),
        sys::InitResult::Ok
    );

    let id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get(<Score as renzora_plugin::ecs::Resource>::TYPE_PATH)
        .copied()
        .expect("Score was not registered");

    app.update();
    app.update();

    // `add_system` used to require at least one query, so a system touching only
    // resources was refused — the plugin loaded and the system simply never ran.
    // Nothing reported it except one line at startup.
    let total = {
        let ptr = app
            .world()
            .get_resource_by_id(id)
            .expect("Score resource is missing");
        unsafe { ptr.as_ptr().cast::<i32>().read_unaligned() }
    };
    assert!(
        total >= 2,
        "a resource-only system did not run (total {total})"
    );
}

/// A `Score` that grew a field, presented under the same name — what a plugin
/// recompiled with an extra field on its resource looks like to the host.
unsafe extern "C" fn score_init_relayout(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let fields = [
        sys::FieldDesc {
            name: sys::StrRef::new("total"),
            kind: sys::FieldKind::I32,
            offset: 0,
        },
        sys::FieldDesc {
            name: sys::StrRef::new("streak"),
            kind: sys::FieldKind::I32,
            offset: 4,
        },
    ];
    let desc = sys::ComponentDesc {
        name: sys::StrRef::new(<Score as renzora_plugin::ecs::Resource>::TYPE_PATH),
        size: 8,
        align: 4,
        drop: None,
        display_name: sys::StrRef::new(""),
        fields: fields.as_ptr(),
        field_count: fields.len(),
        default_init: None,
    };
    ((*iface).register_resource)(host, &desc);
    sys::InitResult::Ok
}

#[test]
fn a_resource_that_grew_a_field_is_refused_too() {
    let mut app = test_app();
    let _guard = plugin_lock();
    let counter = abi_host::PluginGeneration::default();

    abi_host::init_plugin_gen(app.world_mut(), resource_only_init, counter.clone(), 0, 0);

    // `register_resource` short-circuits on a known name and never reaches
    // `register_component`, so the layout guard living only in the latter covered
    // components and missed every resource. That is the worse half: a resource is
    // one allocation, and a grown struct writes straight off the end of it.
    let result =
        abi_host::init_plugin_gen(app.world_mut(), score_init_relayout, counter.clone(), 1, 0);
    assert_eq!(
        result,
        sys::InitResult::Failed,
        "a resource that grew a field was accepted — writes now land outside its allocation"
    );
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "a refused reload still retired the running build"
    );

    // The part the return value does not cover, and the part that was actually
    // broken: a refused build's systems registered at generation 1 while the
    // counter stayed at 0, and the staleness test let them run anyway. Both builds
    // then ticked, the new one reading a layout the host had just rejected.
    //
    // `bump` increments by exactly 1 per update, so two live copies show up as a
    // count that outruns the number of frames.
    let id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get(<Score as renzora_plugin::ecs::Resource>::TYPE_PATH)
        .copied()
        .expect("Score was not registered");
    let read = |app: &App| {
        let ptr = app.world().get_resource_by_id(id).expect("Score is missing");
        unsafe { ptr.as_ptr().cast::<i32>().read_unaligned() }
    };

    let before = read(&app);
    app.update();
    let after = read(&app);
    assert_eq!(
        after - before,
        1,
        "expected exactly one live build after a refused reload, saw {} increments",
        after - before
    );
}

// ── Input ────────────────────────────────────────────────────────────────────

/// Writes `1.0` into `Score.total` when W is held, so the test can see whether the
/// snapshot reached the plugin.
fn read_input(input: ecs::Res<ecs::Input>, mut score: ecs::ResMut<Score>) {
    if input.pressed(sys::Key::W) {
        score.total = 1;
    }
    if input.just_pressed(sys::Key::Space) {
        score.total = 2;
    }
}

unsafe extern "C" fn input_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.init_resource::<Score>()
        .add_systems(ecs::Schedule::Update, read_input);
    sys::InitResult::Ok
}

#[test]
fn a_plugin_reads_the_keyboard() {
    let mut app = test_app();
    let _guard = plugin_lock();
    // The host normally installs this from `RenzoraPluginHostPlugin`; a
    // `MinimalPlugins` test app has no input plugins at all, which is also the
    // headless-server case.
    app.init_resource::<abi_host::input::PluginInput>();

    assert_eq!(
        abi_host::init_plugin(app.world_mut(), input_init),
        sys::InitResult::Ok
    );
    let id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get(<Score as renzora_plugin::ecs::Resource>::TYPE_PATH)
        .copied()
        .expect("Score was not registered");
    let read = |app: &App| {
        let ptr = app.world().get_resource_by_id(id).expect("Score is missing");
        unsafe { ptr.as_ptr().cast::<i32>().read_unaligned() }
    };

    // Nothing pressed: the plugin must see an absent key as up, not as garbage.
    // `Score::default()` is deliberately 7, so an untouched resource is
    // distinguishable from one a system wrote 0 into.
    app.update();
    assert_eq!(read(&app), 7, "input was reported as pressed with nothing set");

    // Press W by writing the snapshot directly. Driving `ButtonInput<KeyCode>`
    // would test Bevy's input plugin; what matters here is that the flattened
    // bitset crosses the boundary and is read at the right bit.
    {
        let mut state = app.world_mut().resource_mut::<abi_host::input::PluginInput>();
        sys::InputState::set_key(&mut state.0.keys_down, sys::Key::W);
    }
    app.update();
    assert_eq!(read(&app), 1, "a held key did not reach the plugin");

    // And `just_pressed` is a separate set, not inferred from `down`.
    {
        let mut state = app.world_mut().resource_mut::<abi_host::input::PluginInput>();
        state.0 = Default::default();
        sys::InputState::set_key(&mut state.0.keys_just_pressed, sys::Key::Space);
    }
    app.update();
    assert_eq!(read(&app), 2, "just_pressed did not reach the plugin");
}

#[test]
fn a_key_from_a_newer_abi_reads_as_up_rather_than_aliasing() {
    // The bitset is 256 bits and `Key::COUNT` is well under that, so an unknown
    // value has a bit position — testing it unchecked would return some other
    // key's state. A plugin built against a newer ABI must see `false`.
    let mut state = sys::InputState::default();
    sys::InputState::set_key(&mut state.keys_down, sys::Key::W);
    let future = sys::Key(sys::Key::COUNT + 40);
    assert!(!future.is_known());
    assert!(!state.pressed(future), "an unknown key aliased onto a known one");
    // And setting one is ignored rather than corrupting a neighbour.
    sys::InputState::set_key(&mut state.keys_down, future);
    assert!(state.pressed(sys::Key::W));
}


// ── Services ─────────────────────────────────────────────────────────────────

/// Plays a clip on every entity it can see, so the test can inspect what the
/// host parked. `renzora_animation` is not linked here, and that is the point:
/// the mechanism carries these bytes without knowing what they mean.
fn play_something(q: ecs::Query<ecs::Entity, ecs::With<Spinner>>, mut cmds: ecs::Commands) {
    use renzora_plugin::anim::AnimCommands;
    for e in &q {
        cmds.entity(e).play_animation_with("run", 2.0, false);
    }
}

unsafe extern "C" fn anim_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Spinner>()
        .add_systems(ecs::Schedule::Update, play_something);
    sys::InitResult::Ok
}

#[test]
fn a_service_call_reaches_the_host_queue_untouched() {
    use renzora_plugin::anim;
    let mut app = test_app();
    let _guard = plugin_lock();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), anim_init),
        sys::InitResult::Ok
    );
    let entity = spawn_spinner(&mut app, 1.0);
    app.update();

    let queue = app.world().resource::<abi_host::PluginServiceCalls>();
    assert_eq!(queue.0.len(), 1, "expected exactly one parked call");
    let call = &queue.0[0];
    assert_eq!(call.entity, entity);
    assert_eq!(call.service, anim::SERVICE);
    assert_eq!(call.op, anim::AnimOp::Play.0);

    // The host stored bytes it never interpreted; decoding is the consumer's job.
    assert_eq!(call.payload.len(), core::mem::size_of::<anim::AnimCommand>());
    let cmd = unsafe { call.payload.as_ptr().cast::<anim::AnimCommand>().read_unaligned() };
    // The name crossed inline, so it survived the sink's byte copy — a pointer
    // here would have dangled into a plugin stack frame that is gone.
    assert_eq!(cmd.name.as_str(), "run");
    assert_eq!(cmd.value, 2.0);
    assert_eq!(cmd.flag, 0, "looping = false did not cross");
}

/// A consumer must take only its own service, or a second bridge silently loses
/// its calls whenever an unrelated crate is linked.
#[test]
fn taking_one_service_leaves_the_others_alone() {
    use renzora_plugin::host::ServiceCall;
    let other = sys::service_id("renzora.audio");
    // The entity is irrelevant here — what is under test is that `take` splits
    // the queue by service and leaves the rest intact.
    let e = Entity::PLACEHOLDER;
    let mut queue = abi_host::PluginServiceCalls(vec![
        ServiceCall { entity: e, service: renzora_plugin::anim::SERVICE, op: 0, payload: vec![] },
        ServiceCall { entity: e, service: other, op: 7, payload: vec![9] },
        ServiceCall { entity: e, service: renzora_plugin::anim::SERVICE, op: 1, payload: vec![] },
    ]);

    let mine = queue.take(renzora_plugin::anim::SERVICE);
    assert_eq!(mine.len(), 2);
    assert_eq!(queue.0.len(), 1, "another service's call was eaten");
    assert_eq!(queue.0[0].service, other);
    assert_eq!(queue.0[0].op, 7);
}

/// Two services must not collide, and an id must be stable across builds — it is
/// baked into every plugin binary that ever shipped.
#[test]
fn service_ids_are_distinct_and_stable() {
    assert_ne!(sys::service_id("renzora.animation"), sys::service_id("renzora.audio"));
    assert_eq!(renzora_plugin::anim::SERVICE, sys::service_id("renzora.animation"));
    // FNV-1a offset basis, i.e. the hash of nothing. Pinned so a change to the
    // hash function shows up here rather than as every plugin silently missing.
    assert_eq!(sys::fnv1a(""), 0xcbf2_9ce4_8422_2325);
}

// ── Animation vocabulary ─────────────────────────────────────────────────────

/// A name over the cap must be dropped, not truncated: a shortened name resolves
/// to no clip, which presents as "animation is broken" rather than as a limit.
#[test]
fn an_over_long_animation_name_is_refused_rather_than_truncated() {
    use renzora_plugin::anim::{AnimName, NAME_CAP};
    assert!(AnimName::new(&"a".repeat(NAME_CAP + 1)).is_none());
    let exact = "b".repeat(NAME_CAP);
    assert_eq!(AnimName::new(&exact).expect("exactly the cap must fit").as_str(), exact);
}

/// A `len` past the buffer must clamp rather than read off the end — the engine
/// reads this out of plugin memory and cannot trust the length.
#[test]
fn an_animation_name_with_a_bogus_length_is_clamped() {
    use renzora_plugin::anim::{AnimName, NAME_CAP};
    let mut name = AnimName::new("run").unwrap();
    name.len = 255;
    assert_eq!(name.as_bytes().len(), NAME_CAP);
}

/// Non-UTF-8 bytes must read as empty rather than panicking the engine.
#[test]
fn an_animation_name_that_is_not_utf8_reads_as_empty() {
    use renzora_plugin::anim::AnimName;
    let mut name = AnimName::EMPTY;
    name.bytes[0] = 0xff;
    name.len = 1;
    assert_eq!(name.as_str(), "");
}

#[test]
fn an_animation_op_from_a_newer_build_is_recognisable_as_unknown() {
    use renzora_plugin::anim::AnimOp;
    let future = AnimOp(99);
    assert!(!future.is_known());
    assert_eq!(future.name(), "?");
}

/// `is_clip` compares hashes, so it must distinguish names and must not treat
/// "nothing playing" as a match for the empty string.
#[test]
fn anim_state_name_comparison_distinguishes_clips() {
    use renzora_plugin::anim::{name_hash, AnimState};
    let mut state = AnimState { clip: name_hash("run"), ..Default::default() };
    assert!(state.is_clip("run"));
    assert!(!state.is_clip("walk"));

    // The bridge writes 0 rather than `name_hash("")` when nothing is playing,
    // precisely so this stays false — the empty name has a real hash.
    state.clip = 0;
    assert!(!state.is_clip(""), "an idle animator matched the empty clip name");
    assert_ne!(name_hash(""), 0);
}

/// The mirror is read straight out of a query cell and wrapped
/// `#[repr(transparent)]` engine-side, so its size is part of the contract.
#[test]
fn the_anim_state_mirror_has_a_stable_layout() {
    use renzora_plugin::anim::AnimState;
    assert_eq!(core::mem::size_of::<AnimState>(), 32);
    assert_eq!(core::mem::align_of::<AnimState>(), 8);
}


// ── Generated meshes ─────────────────────────────────────────────────────────

/// Which `add_mesh_data` case the shared init fn should run.
///
/// A plugin's entry point is a plain `extern "C" fn` with no state, so the case
/// is selected through a static and read back the same way. `plugin_lock()`
/// serialises the tests, which is what makes that safe.
static MESH_CASE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static MESH_RESULT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

fn v3(x: f32, y: f32, z: f32) -> ecs::Vec3 {
    ecs::Vec3 { x, y, z }
}

unsafe extern "C" fn mesh_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    // One triangle, and every case below is a mutation of it.
    let tri = [v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0)];
    let handle = match MESH_CASE.load(std::sync::atomic::Ordering::Relaxed) {
        // Valid, indexed, with the host deriving normals and UVs.
        0 => app.add_mesh_data(&tri, None, None, Some(&[0, 1, 2])),
        // An index past the end of the vertex list.
        1 => app.add_mesh_data(&tri, None, None, Some(&[0, 1, 7])),
        // Fewer normals than vertices.
        2 => app.add_mesh_data(&tri, Some(&[v3(0.0, 1.0, 0.0)]), None, None),
        // Fewer UVs than vertices.
        3 => app.add_mesh_data(&tri, None, Some(&[[0.0, 0.0]]), None),
        // Indices that are not a whole number of triangles.
        4 => app.add_mesh_data(&tri, None, None, Some(&[0, 1])),
        // Unindexed positions that are not a whole number of triangles.
        5 => app.add_mesh_data(&tri[..2], None, None, None),
        // No geometry at all.
        _ => app.add_mesh_data(&[], None, None, None),
    };
    MESH_RESULT.store(handle.0, std::sync::atomic::Ordering::Relaxed);
    sys::InitResult::Ok
}

/// Run one case and report whether the host accepted it.
fn run_mesh_case(case: u32) -> (bool, usize) {
    let mut app = test_app();
    // `add_mesh_data` needs `Assets<Mesh>`, which `MinimalPlugins` has no reason
    // to provide. Adding it directly keeps these tests about the ABI's
    // validation rather than about bringing up a renderer.
    app.init_resource::<Assets<Mesh>>();
    MESH_CASE.store(case, std::sync::atomic::Ordering::Relaxed);
    MESH_RESULT.store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), mesh_init),
        sys::InitResult::Ok
    );
    let handle = sys::AssetHandle(MESH_RESULT.load(std::sync::atomic::Ordering::Relaxed));
    let stored = app.world().resource::<Assets<Mesh>>().len();
    (handle.is_valid(), stored)
}

#[test]
fn generated_geometry_becomes_a_mesh_asset() {
    let _guard = plugin_lock();
    let (ok, stored) = run_mesh_case(0);
    assert!(ok, "a valid triangle was refused");
    assert_eq!(stored, 1, "the mesh was accepted but not stored");
}

/// The important one. An out-of-range index is not a soft failure downstream:
/// wgpu reads past the vertex buffer and faults the process, taking the editor
/// with it. It has to be caught before the mesh reaches the GPU.
#[test]
fn an_out_of_range_index_is_refused_rather_than_uploaded() {
    let _guard = plugin_lock();
    let (ok, stored) = run_mesh_case(1);
    assert!(!ok, "an out-of-range index produced a mesh");
    assert_eq!(stored, 0);
}

/// A short attribute array is refused, not padded. Padding renders with subtly
/// wrong shading or UVs on the tail vertices, which is harder to notice — and
/// harder to trace back — than getting nothing at all.
#[test]
fn an_attribute_count_that_disagrees_with_the_vertices_is_refused() {
    let _guard = plugin_lock();
    assert!(!run_mesh_case(2).0, "short normals were accepted");
    assert!(!run_mesh_case(3).0, "short uvs were accepted");
}

/// Both the indexed and unindexed paths must be a whole number of triangles.
#[test]
fn a_partial_triangle_is_refused() {
    let _guard = plugin_lock();
    assert!(!run_mesh_case(4).0, "a partial indexed triangle was accepted");
    assert!(!run_mesh_case(5).0, "a partial unindexed triangle was accepted");
}

#[test]
fn geometry_with_no_positions_is_refused() {
    let _guard = plugin_lock();
    assert!(!run_mesh_case(6).0, "an empty mesh was accepted");
}

/// Omitting normals must *derive* them, not leave the attribute off — a mesh
/// with no `ATTRIBUTE_NORMAL` fails pipeline specialization at draw time, which
/// surfaces far from the plugin that caused it.
#[test]
fn omitted_normals_and_uvs_are_filled_in_by_the_host() {
    let _guard = plugin_lock();
    let mut app = test_app();
    app.init_resource::<Assets<Mesh>>();
    MESH_CASE.store(0, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), mesh_init),
        sys::InitResult::Ok
    );
    let meshes = app.world().resource::<Assets<Mesh>>();
    let (_, mesh) = meshes.iter().next().expect("mesh was not stored");
    assert!(
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some(),
        "normals were neither supplied nor derived"
    );
    assert!(
        mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some(),
        "UVs were neither supplied nor defaulted"
    );
}

// ── Reading meshes ───────────────────────────────────────────────────────────

static MESH_READ_TRIS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MESH_READ_RAN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Reads the mesh off every entity it can see and records the triangle count.
fn read_meshes(q: ecs::Query<ecs::Entity, ecs::With<Spinner>>, meshes: ecs::Meshes) {
    MESH_READ_RAN.store(true, std::sync::atomic::Ordering::Relaxed);
    for e in &q {
        if let Some(data) = meshes.read(e) {
            MESH_READ_TRIS.store(data.triangles().len(), std::sync::atomic::Ordering::Relaxed);
        }
    }
}

unsafe extern "C" fn mesh_read_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let mut app = ecs::App::new(iface, host);
    app.register_component::<Spinner>()
        .add_systems(ecs::Schedule::Update, read_meshes);
    sys::InitResult::Ok
}

/// The dispatcher gained `Res<Assets<Mesh>>` and `Query<&Mesh3d>` to serve
/// `Meshes`. Bevy validates system-param access when the system is built, so a
/// conflict with the dynamic `FilteredEntityMut` queries would panic here — the
/// point of this test is that building and running it does not.
#[test]
fn a_system_can_take_meshes_without_conflicting_with_its_queries() {
    let mut app = test_app();
    let _guard = plugin_lock();
    MESH_READ_RAN.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), mesh_read_init),
        sys::InitResult::Ok
    );
    spawn_spinner(&mut app, 1.0);
    app.update();
    assert!(
        MESH_READ_RAN.load(std::sync::atomic::Ordering::Relaxed),
        "the system never ran — its params were rejected"
    );
}

/// An entity with no mesh reads as `None`, not as an empty mesh. The difference
/// matters: a plugin polls until the asset loads, and "no mesh at all" and "a
/// mesh with zero triangles" would otherwise be indistinguishable.
#[test]
fn reading_an_entity_with_no_mesh_yields_nothing() {
    let mut app = test_app();
    let _guard = plugin_lock();
    MESH_READ_TRIS.store(usize::MAX, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), mesh_read_init),
        sys::InitResult::Ok
    );
    spawn_spinner(&mut app, 1.0);
    app.update();
    assert_eq!(
        MESH_READ_TRIS.load(std::sync::atomic::Ordering::Relaxed),
        usize::MAX,
        "a meshless entity produced mesh data"
    );
}

/// Unindexed geometry still yields triangles — `triangles()` exists so callers
/// do not each reimplement that branch.
#[test]
fn triangles_are_derived_for_unindexed_geometry() {
    let data = ecs::MeshData {
        positions: vec![
            ecs::Vec3 { x: 0.0, y: 0.0, z: 0.0 },
            ecs::Vec3 { x: 1.0, y: 0.0, z: 0.0 },
            ecs::Vec3 { x: 0.0, y: 0.0, z: 1.0 },
        ],
        ..Default::default()
    };
    assert_eq!(data.triangles(), vec![[0, 1, 2]]);

    let indexed = ecs::MeshData {
        indices: vec![2, 1, 0],
        ..data
    };
    assert_eq!(indexed.triangles(), vec![[2, 1, 0]]);
}

// ── Change-detection filters ────────────────────────────────────────────────
//
// The regression these exist for is not "does the filter work" — it is row
// compaction. `gather` and `scatter` are two independent walks of the same
// query, each indexing the staging buffers by enumeration ordinal. The moment
// `gather` can skip a row, `scatter` must skip exactly the same ones or every
// staged row after the first divergence is written to the WRONG ENTITY.

const TICKED: &str = "test::Ticked";

/// Registers one system whose query is `Query<&mut Transform, FILTER<Ticked>>`.
///
/// `FILTER` comes from a static so the two tests can share the plugin.
static TICK_ACCESS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(12);

unsafe extern "C" fn tick_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let i = &*iface;
    let ticked = (i.register_component)(
        host,
        &sys::ComponentDesc {
            name: sys::StrRef::new(TICKED),
            size: 4,
            align: 4,
            drop: None,
            display_name: sys::StrRef::new(""),
            fields: std::ptr::null(),
            field_count: 0,
            default_init: None,
        },
    );
    let transform = (i.component_id_by_name)(
        host,
        sys::StrRef::new("bevy_transform::components::transform::Transform"),
    );
    let terms = [
        sys::Term { component: transform, access: sys::Access::Write },
        sys::Term {
            component: ticked,
            access: sys::Access(TICK_ACCESS.load(std::sync::atomic::Ordering::Relaxed)),
        },
    ];
    let query = sys::QueryDesc { terms: terms.as_ptr(), term_count: terms.len() };
    let status = (i.add_system)(
        host,
        &sys::SystemDesc {
            entry: stamp,
            schedule: sys::Schedule::Update,
            queries: &query,
            query_count: 1,
            resources: std::ptr::null(),
            resource_count: 0,
            user: std::ptr::null_mut(),
            flags: 0,
        },
    );
    if status == sys::RegisterStatus::Ok {
        sys::InitResult::Ok
    } else {
        sys::InitResult::Failed
    }
}

/// The compaction regression test, and the most valuable one here.
///
/// Five entities, only the middle one changed. If `scatter` does not replay
/// `gather`'s skip decisions, the stamp lands on entity 0 instead of entity 2 —
/// a cross-entity write no other test in this suite would notice.
#[test]
fn write_back_lands_on_the_filtered_row_not_the_first_one() {
    let _guard = plugin_lock();
    let mut app = test_app();
    TICK_ACCESS.store(12, std::sync::atomic::Ordering::Relaxed); // Changed
    assert_eq!(abi_host::init_plugin(app.world_mut(), tick_init), sys::InitResult::Ok);

    let ticked = app.world().resource::<abi_host::PluginComponents>().0[TICKED];
    let ids: Vec<_> = (0..5)
        .map(|_| {
            let e = app.world_mut().spawn(Transform::IDENTITY).id();
            unsafe {
                let v: u32 = 0;
                bevy::ptr::OwningPtr::make(v, |ptr| {
                    app.world_mut().entity_mut(e).insert_by_id(ticked, ptr);
                });
            }
            e
        })
        .collect();

    // One frame so every insert stops counting as a change.
    app.update();
    for e in &ids {
        app.world_mut().entity_mut(*e).get_mut::<Transform>().unwrap().translation.x = 0.0;
    }
    app.update();

    // Now touch ONLY the middle one.
    {
        let mut e = app.world_mut().entity_mut(ids[2]);
        // `MutUntyped::as_mut` is what sets the changed tick — the same call
        // `write_cell` makes, so this is exactly what a real mutation looks like.
        let mut m = e.get_mut_by_id(ticked).unwrap();
        let _ = m.as_mut();
    }
    app.update();

    let x = |app: &App, e: bevy::prelude::Entity| {
        app.world().entity(e).get::<Transform>().unwrap().translation.x
    };
    assert_eq!(x(&app, ids[2]), 1.0, "the changed row should have been stamped");
    for (n, e) in ids.iter().enumerate() {
        if n != 2 {
            assert_eq!(
                x(&app, *e),
                0.0,
                "entity {n} was written to — gather/scatter desynced and the staged row \
                 landed on the wrong entity"
            );
        }
    }
}

/// A tick filter inside `Or` must refuse the system, not register an `Or` with
/// an empty branch — an empty branch matches every entity in the world.
#[test]
fn a_tick_filter_inside_an_or_refuses_the_system() {
    let _guard = plugin_lock();
    let mut app = test_app();
    assert_eq!(
        abi_host::init_plugin(app.world_mut(), or_tick_init),
        sys::InitResult::Failed,
        "a change-tick term inside `Or` must refuse the system"
    );
}

unsafe extern "C" fn or_tick_init(
    iface: *const sys::Interface,
    host: *mut sys::Host,
) -> sys::InitResult {
    let i = &*iface;
    let a = (i.register_component)(
        host,
        &sys::ComponentDesc {
            name: sys::StrRef::new("test::OrTickA"),
            size: 0,
            align: 1,
            drop: None,
            display_name: sys::StrRef::new(""),
            fields: std::ptr::null(),
            field_count: 0,
            default_init: None,
        },
    );
    let transform = (i.component_id_by_name)(
        host,
        sys::StrRef::new("bevy_transform::components::transform::Transform"),
    );
    let terms = [
        sys::Term { component: transform, access: sys::Access::Write },
        sys::Term { component: sys::ComponentId::INVALID, access: sys::Access::OrBegin },
        sys::Term { component: a, access: sys::Access::Changed },
        sys::Term { component: sys::ComponentId::INVALID, access: sys::Access::OrNext },
        sys::Term { component: a, access: sys::Access::With },
        sys::Term { component: sys::ComponentId::INVALID, access: sys::Access::OrEnd },
    ];
    let query = sys::QueryDesc { terms: terms.as_ptr(), term_count: terms.len() };
    let status = (i.add_system)(
        host,
        &sys::SystemDesc {
            entry: stamp,
            schedule: sys::Schedule::Update,
            queries: &query,
            query_count: 1,
            resources: std::ptr::null(),
            resource_count: 0,
            user: std::ptr::null_mut(),
            flags: 0,
        },
    );
    if status == sys::RegisterStatus::Ok {
        sys::InitResult::Ok
    } else {
        sys::InitResult::Failed
    }
}
