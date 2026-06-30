//! Bevy-native (ember) inspector drawer for [`renzora::CameraPresets`].
//!
//! Lists each named camera angle with three actions:
//! - **Go to** drives the editor fly-camera to the preset (preview the angle),
//! - **Snap to Viewport** overwrites the preset with the *current* editor view,
//! - **Delete** removes it.
//!
//! A *Capture current view* button adds a new preset from the current editor
//! view. Presets store the editor camera's world-space pose, persist on the
//! camera entity (and into the scene RON), and a script on the same entity can
//! jump to one with `goto_camera_preset("name")`.
//!
//! Mirrors the dynamic-list pattern in [`crate::scripts`]: the drawer owns a
//! `PresetsRoot` container that a `rebuild_camera_presets` system re-fills
//! whenever the preset set changes, and click actions defer through
//! [`EditorCommands`].

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora::{CameraPreset, CameraPresets};
use renzora_camera::OrbitCameraState;
use renzora_editor_framework::camera::editor_camera_world_pose;
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::theme::{rgb, section_bg, text_muted};
use renzora_ember::widgets::{bind_text_input, icon_button, icon_label_button, text_input};

pub fn register(app: &mut App) {
    use renzora_editor_framework::{AppEditorExt, SplashState};
    app.register_native_inspector_ui("camera_presets", camera_presets_native);
    app.add_systems(
        Update,
        (
            rebuild_camera_presets,
            capture_preset_click,
            goto_preset_click,
            snap_preset_click,
            delete_preset_click,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
}

#[derive(Component)]
struct PresetsRoot {
    entity: Entity,
    sig: Option<u64>,
}

#[derive(Component)]
struct CapturePresetBtn {
    entity: Entity,
}

#[derive(Component)]
struct GotoPresetBtn {
    entity: Entity,
    index: usize,
}

#[derive(Component)]
struct SnapPresetBtn {
    entity: Entity,
    index: usize,
}

#[derive(Component)]
struct DeletePresetBtn {
    entity: Entity,
    index: usize,
}

fn camera_presets_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            PresetsRoot { entity, sig: None },
            Name::new("camera-presets-root"),
        ))
        .id()
}

// ── Rebuild ──────────────────────────────────────────────────────────────────

fn presets_sig(world: &World, entity: Entity, root: Entity) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.to_bits().hash(&mut h);
    if let Some(p) = world.get::<CameraPresets>(entity) {
        p.presets.len().hash(&mut h);
        for preset in &p.presets {
            preset.name.hash(&mut h);
        }
    } else {
        0u8.hash(&mut h);
    }
    h.finish()
}

fn rebuild_camera_presets(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut q = world.query::<(Entity, &PresetsRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> =
        q.iter(world).map(|(re, pr)| (re, pr.entity, pr.sig)).collect();

    for (root, entity, old_sig) in roots {
        let sig = presets_sig(world, entity, root);
        if old_sig == Some(sig) {
            continue;
        }
        // Snapshot preset names for this rebuild.
        let names: Vec<String> = world
            .get::<CameraPresets>(entity)
            .map(|p| p.presets.iter().map(|pr| pr.name.clone()).collect())
            .unwrap_or_default();
        let existing: Vec<Entity> = world
            .get::<Children>(root)
            .map(|c| c.iter().collect())
            .unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            if names.is_empty() {
                let l = muted_label(
                    &mut commands,
                    &fonts,
                    &renzora::lang::t("comp.camera_presets.empty"),
                );
                commands.entity(root).add_child(l);
            }
            for (i, name) in names.iter().enumerate() {
                let row = build_preset_row(&mut commands, &fonts, entity, i, name);
                commands
                    .entity(row)
                    .insert(BackgroundColor(inspector_stripe(i)));
                commands.entity(root).add_child(row);
            }
            let capture = build_capture_button(&mut commands, &fonts, entity);
            commands.entity(root).add_child(capture);
        }
        queue.apply(world);
        if let Some(mut pr) = world.get_mut::<PresetsRoot>(root) {
            pr.sig = Some(sig);
        }
    }
}

fn build_preset_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    index: usize,
    name: &str,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            Name::new("camera-preset-row"),
        ))
        .id();

    // Editable name (two-way bound to the preset at this index by position).
    let ti = text_input(commands, &fonts.ui, &renzora::lang::t("comp.camera_presets.name_placeholder"), name);
    commands.entity(ti).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        ..default()
    });
    bind_text_input(
        commands,
        ti,
        move |w| {
            w.get::<CameraPresets>(entity)
                .and_then(|p| p.presets.get(index))
                .map(|pr| pr.name.clone())
                .unwrap_or_default()
        },
        move |w, v: String| {
            if let Some(mut p) = w.get_mut::<CameraPresets>(entity) {
                if let Some(pr) = p.presets.get_mut(index) {
                    pr.name = v;
                }
            }
        },
    );

    // Go to: drive the editor camera to this preset's angle (preview it).
    let goto = icon_button(commands, fonts, "eye");
    commands
        .entity(goto)
        .insert(GotoPresetBtn { entity, index });

    // Snap to Viewport: overwrite this preset with the current editor view.
    let snap = icon_button(commands, fonts, "frame-corners");
    commands
        .entity(snap)
        .insert(SnapPresetBtn { entity, index });

    // Delete.
    let del = icon_button(commands, fonts, "trash");
    commands
        .entity(del)
        .insert(DeletePresetBtn { entity, index });

    commands.entity(row).add_children(&[ti, goto, snap, del]);
    row
}

fn build_capture_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let btn = icon_label_button(commands, fonts, "plus", &renzora::lang::t("comp.camera_presets.capture"));
    commands.entity(btn).insert((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(5.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(rgb(section_bg())),
        CapturePresetBtn { entity },
    ));
    btn
}

// ── Click handlers ────────────────────────────────────────────────────────────

/// World-space pose of the focused editor camera, or `None` (with a toast) if
/// there isn't one.
fn editor_pose_or_warn(w: &mut World) -> Option<Transform> {
    match editor_camera_world_pose(w) {
        Some(gt) => Some(gt.compute_transform()),
        None => {
            if let Some(mut t) = w.get_resource_mut::<renzora_ui::Toasts>() {
                t.warning(renzora::lang::t("comp.camera_presets.no_camera"));
            }
            None
        }
    }
}

fn capture_preset_click(
    q: Query<(&Interaction, &CapturePresetBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let entity = btn.entity;
        cmds.push(move |w: &mut World| {
            let Some(pose) = editor_pose_or_warn(w) else {
                return;
            };
            // Lazily insert the component on first capture.
            if w.get::<CameraPresets>(entity).is_none() {
                w.entity_mut(entity).insert(CameraPresets::default());
            }
            if let Some(mut p) = w.get_mut::<CameraPresets>(entity) {
                let name = format!("Preset {}", p.presets.len() + 1);
                p.presets.push(CameraPreset::from_transform(name, &pose));
            }
            if let Some(mut t) = w.get_resource_mut::<renzora_ui::Toasts>() {
                t.info(renzora::lang::t("comp.camera_presets.captured"));
            }
        });
    }
}

/// Drive the editor fly-camera to a preset's stored angle. Sets the
/// `OrbitCameraState` (the orbit controller overwrites the camera `Transform`
/// each frame, so writing the transform directly wouldn't stick).
fn goto_preset_click(
    q: Query<(&Interaction, &GotoPresetBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, index) = (btn.entity, btn.index);
        cmds.push(move |w: &mut World| {
            let pose = w
                .get::<CameraPresets>(entity)
                .and_then(|p| p.presets.get(index))
                .map(|pr| pr.to_transform());
            if let (Some(pose), Some(mut orbit)) =
                (pose, w.get_resource_mut::<OrbitCameraState>())
            {
                orbit.set_from_view(pose.translation, pose.rotation);
            }
        });
    }
}

/// Overwrite a preset with the current editor view ("Snap to Viewport").
fn snap_preset_click(
    q: Query<(&Interaction, &SnapPresetBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, index) = (btn.entity, btn.index);
        cmds.push(move |w: &mut World| {
            let Some(pose) = editor_pose_or_warn(w) else {
                return;
            };
            if let Some(mut p) = w.get_mut::<CameraPresets>(entity) {
                if let Some(pr) = p.presets.get_mut(index) {
                    pr.translation = pose.translation;
                    pr.rotation = pose.rotation;
                }
            }
            if let Some(mut t) = w.get_resource_mut::<renzora_ui::Toasts>() {
                t.info(renzora::lang::t("comp.camera_presets.snapped"));
            }
        });
    }
}

fn delete_preset_click(
    q: Query<(&Interaction, &DeletePresetBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, index) = (btn.entity, btn.index);
        cmds.push(move |w: &mut World| {
            if let Some(mut p) = w.get_mut::<CameraPresets>(entity) {
                if index < p.presets.len() {
                    p.presets.remove(index);
                }
            }
        });
    }
}

fn muted_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::all(Val::Px(6.0)),
                ..default()
            },
        ))
        .id()
}
