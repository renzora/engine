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
use spinner::Spinner;

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

    let result = abi_host::init_plugin(app.world_mut(), spinner::renzora_plugin_init);
    assert_eq!(result, sys::InitResult::Ok, "plugin init failed");

    // The host learned about a component it has no Rust type for.
    let spinner_id = app
        .world()
        .resource::<abi_host::PluginComponents>()
        .0
        .get("spinner::Spinner")
        .copied()
        .expect("Spinner was not registered");

    // Spawn an entity carrying both — the plugin component goes in by raw bytes,
    // exactly as a scene loader or the inspector would have to do it.
    let spinner = Spinner { speed: 2.0 };
    let entity = app.world_mut().spawn(Transform::IDENTITY).id();
    // SAFETY: `spinner_id` was registered with this exact layout.
    unsafe {
        let bytes = std::slice::from_raw_parts(
            (&spinner as *const Spinner).cast::<u8>(),
            size_of::<Spinner>(),
        )
        .to_vec();
        bevy::ptr::OwningPtr::make(bytes.into_boxed_slice(), |ptr| {
            app.world_mut()
                .entity_mut(entity)
                .insert_by_id(spinner_id, ptr);
        });
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

    let result = abi_host::init_plugin(app.world_mut(), spinner::renzora_plugin_init);
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
        bevy::ptr::OwningPtr::make(Vec::<u8>::new().into_boxed_slice(), |ptr| {
            app.world_mut().entity_mut(e).insert_by_id(id, ptr);
        });
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
        abi_host::init_plugin(app.world_mut(), spinner::renzora_plugin_init),
        sys::InitResult::Ok
    );

    let schemas = app.world().resource::<abi_host::PluginComponentSchemas>();
    let info = schemas
        .0
        .iter()
        .find(|s| s.type_path == "spinner::Spinner")
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
