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
use renzora_plugin::ecs::{self, Component as _};

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
    for row in 0..call.entity_count {
        let t = &mut *(*call.cells.add(row * call.cell_count) as *mut sys::Transform);
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
    (i.add_system)(
        host,
        sys::Schedule::Update,
        stamp,
        &sys::QueryDesc { terms: terms.as_ptr(), term_count: terms.len() },
        std::ptr::null_mut(),
    );
    sys::InitResult::Ok
}

#[test]
fn with_and_without_actually_filter() {
    let mut app = test_app();
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
