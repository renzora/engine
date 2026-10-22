//! Editor-only half of `renzora_particles`: the `.vfx` effect editor, plus the
//! scene-side entry points.
//!
//! **The effect editor** opens when a `.vfx` becomes the active document
//! (double-click it in the Assets panel) and lives in the Particles workspace:
//!
//! - [`preview_panel`] / [`preview`]: the effect playing live on its own
//!   render layer, with backdrop, speed, emitter motion, loop and restart
//!   controls and per-emitter stats;
//! - [`graph`]: one emitter's node graph on ember's node graph view (the
//!   material editor's widget and conventions), a tab per emitter, inline
//!   value editors ([`pin_editors`], [`curve_editor`]) and compile errors on
//!   their nodes;
//! - [`emitter_settings`]: the effect and the active emitter's non-graph
//!   settings (capacity, space, texture, sprite sheet, blend, facing);
//! - [`templates`]: New Effect, a gallery of the built-in effects, also on the
//!   Assets panel's create menu;
//! - [`animate`] / [`timeline`]: right-click a value field to key it over the
//!   effect's time, and the route the Timeline panel edits keyed parameters
//!   through.
//!
//! All of them edit one working copy ([`state::VfxEditorState`]). An observer
//! turns every change into an undo step and a live update of the asset, so
//! every `ParticleSystem` using the file (the preview and the scene) follows
//! the edit before it is saved. [`model`] is the pure edit layer under it.
//!
//! **Scene side**: Add Entity > Effects > Particle System, the Particle System
//! inspector (path, Play / Pause / Stop / Restart, time scale), and the
//! viewport gizmo ([`gizmo`]).
//!
//! **Old effects** (the retired Hanabi system's `.particle` files and scene
//! components): the runtime already plays them; [`legacy`] makes them
//! editable, converting every `.particle` in a project to a `.vfx` beside it
//! on open, pointing the scene's effects at those and opening a `.particle` as
//! its `.vfx`. [`convert`] writes the files, and [`convert_actions`] offers the
//! same conversion by hand from the Assets panel and the Effect slot.
//! [`workspace`] brings the effect panels forward when a `.vfx` opens.

pub mod animate;
pub mod convert;
pub mod convert_actions;
mod curve_editor;
mod emitter_settings;
mod gizmo;
mod graph;
mod inspector_ui;
pub mod legacy;
pub mod model;
mod pin_editors;
mod preview;
mod preview_panel;
pub mod state;
pub mod templates;
pub mod timeline;
pub mod workspace;

use bevy::prelude::*;
use renzora::{AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry, RenzoraShellExt};
use renzora_editor_framework::SplashState;
use renzora_particles::{OneShot, ParticlePlayback, ParticleSystem, PlayState};

/// The Effect slot. It also takes a `.particle`, because that is what a
/// project made before this system is full of: the drop converts it beside
/// itself (or reuses the earlier conversion) and the slot gets the `.vfx`.
/// Converting in `set_fn` rather than in the drop handler keeps the inspector
/// generic, and redo (which calls `set_fn` with the same `.particle` again)
/// lands on the reused file rather than a second copy.
fn path_field() -> FieldDef {
    FieldDef {
        name: "Effect",
        field_type: FieldType::Asset {
            extensions: vec!["vfx".to_string(), "particle".to_string()],
        },
        get_fn: |world, entity| {
            world.get::<ParticleSystem>(entity).map(|s| {
                FieldValue::Asset((!s.path.is_empty()).then(|| s.path.clone()))
            })
        },
        set_fn: set_effect_path,
    }
}

/// The Effect slot's write: a dropped `.particle` is converted (or its earlier
/// conversion reused) and the `.vfx` assigned. Shared by the declarative field
/// and the native drawer so the two cannot drift.
pub(crate) fn set_effect_path(world: &mut World, entity: Entity, value: FieldValue) {
    if let FieldValue::Asset(path) = value {
        let path = match path {
            Some(p) => match convert_actions::effect_for_dropped_path(world, &p) {
                Some(p) => p,
                None => return,
            },
            None => String::new(),
        };
        if let Some(mut system) = world.get_mut::<ParticleSystem>(entity) {
            system.path = path;
        }
    }
}

/// Open the entity's effect in the Particles workspace. A built-in
/// (`embedded://`) effect is not a project file and has nothing to save into,
/// so it opens the template gallery instead, which copies one into the project.
pub(crate) fn edit_effect(w: &mut World, e: Entity) {
    let Some(mut path) = w.get::<ParticleSystem>(e).map(|s| s.path.clone()) else { return };
    // An old `.particle` that is still what the entity points at (its `.vfx`
    // was not written yet): convert it now, point the entity at the result,
    // and edit that, rather than sending the user to the gallery.
    if convert::is_particle_path(&path) && !path.contains("://") {
        let Some(vfx) = convert_actions::effect_for_dropped_path(w, &path) else { return };
        if let Some(mut system) = w.get_mut::<ParticleSystem>(e) {
            system.path = vfx.clone();
        }
        path = vfx;
    }
    if path.starts_with("embedded://") || !path.to_ascii_lowercase().ends_with(".vfx") {
        templates::request_gallery(w);
        return;
    }
    let Some(abs) = w.get_resource::<renzora::core::CurrentProject>().map(|p| p.resolve_path(&path)) else { return };
    renzora_editor_framework::open_asset_tab(w, &abs, renzora_editor_framework::DocTabKind::Particle);
}

/// An action button; the inspector calls `set_fn` when it is clicked.
fn button(name: &'static str, icon: &'static str, set_fn: fn(&mut World, Entity, FieldValue)) -> FieldDef {
    FieldDef {
        name,
        field_type: FieldType::Button { icon },
        get_fn: |_, _| None,
        set_fn,
    }
}

fn with_playback(world: &mut World, entity: Entity, apply: impl FnOnce(&mut ParticlePlayback)) {
    if let Some(mut playback) = world.get_mut::<ParticlePlayback>(entity) {
        apply(&mut playback);
    }
}

fn particle_system_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "particle_system",
        display_name: "Particle System",
        icon: "sparkle",
        category: "effects",
        has_fn: |world, entity| world.get::<ParticleSystem>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ParticleSystem::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(ParticleSystem, ParticlePlayback)>();
        }),
        // The header toggle is play/stop: the closest thing an effect has to
        // being switched off.
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<ParticlePlayback>(entity)
                .is_some_and(|p| p.state != PlayState::Stopped)
        }),
        set_enabled_fn: Some(|world, entity, on| {
            with_playback(world, entity, |p| if on { p.play() } else { p.stop() });
        }),
        fields: vec![
            path_field(),
            // Opens the effect editor on this entity's file. A built-in
            // (`embedded://`) effect is not a project file and has nothing to
            // save into; New Effect copies one into the project instead.
            button("Edit Effect", "pencil-simple", |w, e, _| edit_effect(w, e)),
            button("Play", "play", |w, e, _| with_playback(w, e, ParticlePlayback::play)),
            button("Pause", "pause", |w, e, _| with_playback(w, e, ParticlePlayback::pause)),
            button("Stop", "stop", |w, e, _| with_playback(w, e, ParticlePlayback::stop)),
            button("Restart", "arrow-counter-clockwise", |w, e, _| {
                with_playback(w, e, ParticlePlayback::restart)
            }),
            FieldDef {
                name: "Time Scale",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<ParticlePlayback>(entity)
                        .map(|p| FieldValue::Float(p.time_scale))
                },
                set_fn: |world, entity, value| {
                    if let FieldValue::Float(v) = value {
                        with_playback(world, entity, |p| p.time_scale = v.max(0.0));
                    }
                },
            },
            FieldDef {
                name: "When Finished",
                field_type: FieldType::Enum {
                    options: &["Keep", "Deactivate", "Despawn"],
                },
                get_fn: |world, entity| {
                    world.get::<ParticlePlayback>(entity).map(|p| {
                        FieldValue::Enum(
                            match p.one_shot {
                                OneShot::Keep => "Keep",
                                OneShot::Deactivate => "Deactivate",
                                OneShot::Despawn => "Despawn",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |world, entity, value| {
                    if let FieldValue::Enum(choice) = value {
                        let one_shot = match choice.as_str() {
                            "Deactivate" => OneShot::Deactivate,
                            "Despawn" => OneShot::Despawn,
                            _ => OneShot::Keep,
                        };
                        with_playback(world, entity, |p| p.one_shot = one_shot);
                    }
                },
            },
        ],
    }
}

fn particle_system_preset() -> EntityPreset {
    EntityPreset {
        id: "particle_system",
        display_name: "Particle System",
        icon: "sparkle",
        category: "effects",
        spawn_fn: |world| {
            world
                .spawn((Name::new("Particle System"), ParticleSystem::default()))
                .id()
        },
    }
}

/// Editor-scope companion to `renzora_particles::ParticlesPlugin`.
#[derive(Default)]
pub struct ParticlesEditorPlugin;

impl Plugin for ParticlesEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParticlesEditorPlugin");
        app.register_inspector(particle_system_entry());
        // The drawn layout; the declarative fields stay registered under it
        // for search, keyframing and anything else that reads them.
        app.register_native_inspector_ui("particle_system", inspector_ui::particle_system_ui);
        app.register_entity_preset(particle_system_preset());
        app.add_systems(Update, (gizmo::draw_particle_gizmos, inspector_ui::transport_click, inspector_ui::edit_effect_click));
        convert_actions::register(app);
        // Old effects: the project-open conversion, then scene effects moved
        // onto what it wrote. Not gated on the editor state: they key off
        // `CurrentProject`, which exists exactly when there is a project.
        app.init_resource::<legacy::LegacyScan>().add_systems(
            Update,
            (
                legacy::start_scan,
                legacy::finish_scan,
                legacy::write_inline_effects,
                legacy::repoint_old_effects,
            )
                .chain(),
        );

        // The effect editor.
        app.init_resource::<state::VfxEditorState>()
            .init_resource::<state::VfxShadow>()
            .init_resource::<timeline::TimelineLink>()
            .register_shell_panel(preview::PANEL, "Effect Preview", "magic-wand", "Particle")
            .register_shell_panel(graph::PANEL, "Effect Graph", "graph", "Particle")
            .register_shell_panel(emitter_settings::PANEL, "Emitter Settings", "sliders-horizontal", "Particle")
            .add_plugins((
                preview::VfxPreviewPlugin,
                preview_panel::VfxPreviewPanel,
                graph::VfxGraphPanel,
                emitter_settings::EmitterSettingsPanel,
                curve_editor::CurveEditorPlugin,
                templates::TemplateGalleryPlugin,
            ));
        // Document orchestration runs whatever panel is showing: the open
        // effect is pushed live to the scene even with only the viewport up.
        app.add_systems(
            Update,
            (
                // First, so a `.particle` that just became the active
                // document is already its `.vfx` when the document loads.
                legacy::open_particle_documents_as_vfx,
                state::load_active_document,
                state::adopt_external_changes,
                state::observe_edits.run_if(|s: Res<state::VfxEditorState>| s.is_open()),
                workspace::ensure_panels,
            )
                .chain()
                .run_if(in_state(SplashState::Editor)),
        );
        app.add_systems(Update, animate::pin_context_menu.run_if(in_state(SplashState::Editor)));
    }
}

renzora::add!(ParticlesEditorPlugin, Editor);
