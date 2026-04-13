//! VR panel spawner — wrist/radial menu for creating and closing panels.
//!
//! Menu button on left controller opens a floating panel listing available panel
//! types. Selecting one spawns a new panel 1m in front of the user's head.
//! An X button on each panel's title bar closes it.

use bevy::prelude::*;

use renzora_xr::VrControllerState;
use renzora_xr::reexports::{OxrViews, XrTrackingRoot};

use crate::panel_quad::{despawn_vr_panel, spawn_vr_panel};
use crate::VrPanel;

/// Available panel types that can be spawned in VR.
pub const AVAILABLE_PANELS: &[(&str, &str)] = &[
    // ── Core editor ──
    ("toolbar", "Toolbar"),
    ("hierarchy", "Hierarchy"),
    ("inspector", "Inspector"),
    ("console", "Console"),
    ("assets", "Assets"),
    ("history", "History"),
    ("settings", "Settings"),
    // ── Material ──
    ("material_graph", "Material Graph"),
    ("node_library", "Node Library"),
    // ── Animation ──
    ("animation", "Animation"),
    ("timeline", "Timeline"),
    // ── Scripting ──
    ("code_editor", "Code Editor"),
    ("script_variables", "Script Variables"),
    // ── Audio ──
    ("mixer", "Audio Mixer"),
    // ── Level tools ──
    ("level_tools", "Level Tools"),
    ("shape_library", "Shape Library"),
    ("particle_editor", "Particle Editor"),
    // ── Physics ──
    ("physics_debug", "Physics Debug"),
    ("physics_properties", "Physics Properties"),
    ("physics_forces", "Physics Forces"),
    ("physics_metrics", "Physics Metrics"),
    ("physics_playground", "Physics Playground"),
    ("physics_scenarios", "Physics Scenarios"),
    ("collision_viz", "Collision Viz"),
    // ── Performance / debug ──
    ("performance", "Performance"),
    ("ecs_stats", "ECS Stats"),
    ("render_stats", "Render Stats"),
    ("render_pipeline", "Render Pipeline"),
    ("system_profiler", "System Profiler"),
    ("memory_profiler", "Memory Profiler"),
    ("camera_debug", "Camera Debug"),
    ("culling_debug", "Culling Debug"),
    ("stress_test", "Stress Test"),
    ("movement_trails", "Movement Trails"),
    ("state_recorder", "State Recorder"),
    ("arena_presets", "Arena Presets"),
    ("gamepad", "Gamepad"),
    // ── VR-specific ──
    ("vr_session", "VR Session"),
    ("vr_settings", "VR Settings"),
    ("vr_devices", "VR Devices"),
    ("vr_performance", "VR Performance"),
    ("vr_input_debug", "Input Debug"),
];

/// Tracks the wrist menu state and pending spawn/close requests.
#[derive(Resource, Default)]
pub struct VrPanelMenu {
    /// Whether the wrist menu is currently open.
    pub menu_open: bool,
    /// Panel type to spawn (set by menu selection, consumed by spawn system).
    pub pending_spawn: Option<String>,
    /// Panel entity to close (set by close button, consumed by close system).
    pub pending_close: Option<Entity>,
    /// Entity of the spawned wrist menu panel (if any).
    pub menu_panel_entity: Option<Entity>,
    /// Debounce: was menu button pressed last frame?
    was_menu_pressed: bool,
}

/// System: toggle wrist menu on menu button press, spawn panels from pending requests.
pub fn vr_panel_menu_system(
    controllers: Option<Res<VrControllerState>>,
    mut menu: ResMut<VrPanelMenu>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    existing_panels: Query<&VrPanel>,
    tracking_root: Query<(&GlobalTransform, &Transform), With<XrTrackingRoot>>,
    views: Option<Res<OxrViews>>,
) {
    // Toggle menu on left controller menu button press (rising edge)
    if let Some(ref ctrl) = controllers {
        if ctrl.left.menu && !menu.was_menu_pressed {
            menu.menu_open = !menu.menu_open;
        }
        menu.was_menu_pressed = ctrl.left.menu;

        // Spawn/despawn menu panel based on menu_open state
        if menu.menu_open && menu.menu_panel_entity.is_none() {
            // Position the menu near the left wrist
            let root_tf = tracking_root.single().map(|(g, _)| *g).unwrap_or(GlobalTransform::IDENTITY);
            let wrist_pos = root_tf.transform_point(ctrl.left.grip_position);
            let wrist_rot = root_tf.to_isometry().rotation * ctrl.left.grip_rotation;
            // Offset up and forward from left wrist
            let menu_pos = wrist_pos + wrist_rot * Vec3::new(0.0, 0.15, -0.15);
            let position = Transform::from_translation(menu_pos)
                .looking_at(wrist_pos + Vec3::new(0.0, 0.3, 0.0), Vec3::Y);

            let entity = spawn_vr_panel(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut images,
                "wrist_menu",
                position,
                0.35,
                0.5,
                512.0,
            );
            menu.menu_panel_entity = Some(entity);
        } else if !menu.menu_open {
            if let Some(entity) = menu.menu_panel_entity.take() {
                if let Ok(panel) = existing_panels.get(entity) {
                    despawn_vr_panel(&mut commands, entity, panel);
                }
            }
        }
    }

    // Process pending spawn request
    if let Some(panel_type) = menu.pending_spawn.take() {
        // Count all existing panels to spread new ones out
        let total_panels = existing_panels.iter().count();

        // Get head position and forward direction in world space
        let (head_pos, head_forward) = if let (Some(ref views), Ok((root_global, root_local))) =
            (&views, tracking_root.single())
        {
            if !views.is_empty() {
                let pose = &views[0].pose;
                let head_tracking_pos = Vec3::new(
                    pose.position.x,
                    pose.position.y,
                    pose.position.z,
                );
                let head_tracking_rot = Quat::from_xyzw(
                    pose.orientation.x,
                    pose.orientation.y,
                    pose.orientation.z,
                    pose.orientation.w,
                );
                let world_pos = root_global.transform_point(head_tracking_pos);
                let world_rot = root_local.rotation * head_tracking_rot;
                let fwd = (world_rot * Vec3::NEG_Z).normalize();
                (world_pos, fwd)
            } else {
                (Vec3::new(0.0, 1.5, 0.0), Vec3::NEG_Z)
            }
        } else {
            (Vec3::new(0.0, 1.5, 0.0), Vec3::NEG_Z)
        };

        // Flatten forward to horizontal plane
        let flat_forward = Vec3::new(head_forward.x, 0.0, head_forward.z)
            .normalize_or(Vec3::NEG_Z);
        let flat_right = Vec3::new(-flat_forward.z, 0.0, flat_forward.x);

        // Spawn 1.5m in front of head, at head height.
        // Offset sideways by 0.7m per existing panel for spacing.
        let offset_x = total_panels as f32 * 0.7;
        let spawn_pos = head_pos + flat_forward * 1.5 + flat_right * offset_x;
        // Face the panel toward the user's head
        let look_target = Vec3::new(head_pos.x, spawn_pos.y, head_pos.z);
        let position = Transform::from_translation(spawn_pos)
            .looking_at(look_target, Vec3::Y);

        let (width, height) = panel_size_for_type(&panel_type);

        spawn_vr_panel(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
            &panel_type,
            position,
            width,
            height,
            512.0,
        );

        // Close menu after spawn
        menu.menu_open = false;
        if let Some(entity) = menu.menu_panel_entity.take() {
            if let Ok(panel) = existing_panels.get(entity) {
                despawn_vr_panel(&mut commands, entity, panel);
            }
        }
    }
}

/// System: handle panel close requests.
pub fn handle_panel_close(
    mut menu: ResMut<VrPanelMenu>,
    mut commands: Commands,
    panels: Query<(Entity, &VrPanel)>,
) {
    if let Some(close_entity) = menu.pending_close.take() {
        if let Ok((entity, panel)) = panels.get(close_entity) {
            despawn_vr_panel(&mut commands, entity, panel);
        }
    }
}

/// Get default panel dimensions (width, height) in meters for a panel type.
fn panel_size_for_type(panel_type: &str) -> (f32, f32) {
    match panel_type {
        // Core editor
        "toolbar" => (0.25, 0.35),
        "wrist_menu" => (0.35, 0.5),
        "hierarchy" => (0.5, 0.8),
        "inspector" => (0.5, 0.9),
        "console" => (0.7, 0.5),
        "assets" => (0.7, 0.6),
        "history" => (0.5, 0.6),
        "settings" => (0.6, 0.8),
        // Material
        "material_graph" => (0.8, 0.6),
        "node_library" => (0.4, 0.7),
        // Animation
        "animation" => (0.7, 0.6),
        "timeline" => (0.8, 0.5),
        // Scripting
        "code_editor" => (0.8, 0.9),
        "script_variables" => (0.5, 0.6),
        // Audio
        "mixer" => (0.6, 0.5),
        // Level tools
        "level_tools" => (0.5, 0.7),
        "shape_library" => (0.5, 0.7),
        "particle_editor" => (0.6, 0.7),
        // Physics
        "physics_debug" => (0.6, 0.6),
        "physics_properties" => (0.5, 0.7),
        "physics_forces" => (0.5, 0.6),
        "physics_metrics" => (0.5, 0.5),
        "physics_playground" => (0.6, 0.7),
        "physics_scenarios" => (0.5, 0.6),
        "collision_viz" => (0.5, 0.5),
        // Performance / debug
        "vr_performance" | "performance" => (0.5, 0.6),
        "ecs_stats" | "render_stats" => (0.5, 0.5),
        "render_pipeline" => (0.6, 0.6),
        "system_profiler" => (0.6, 0.7),
        "memory_profiler" => (0.5, 0.6),
        "camera_debug" => (0.5, 0.5),
        "culling_debug" => (0.5, 0.5),
        "stress_test" => (0.5, 0.6),
        "movement_trails" => (0.5, 0.5),
        "state_recorder" => (0.5, 0.6),
        "arena_presets" => (0.5, 0.6),
        "gamepad" => (0.5, 0.5),
        // VR-specific
        "vr_session" | "vr_settings" | "vr_devices" | "vr_input_debug" => (0.5, 0.6),
        _ => (0.6, 0.8),
    }
}
