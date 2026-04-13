//! Default VR panel layout — arranges panels in an arc around the user.

use bevy::prelude::*;

use crate::panel_quad::spawn_vr_panel;

/// Panel definition for initial layout.
pub struct InitialPanel {
    pub panel_type: &'static str,
    pub width_m: f32,
    pub height_m: f32,
}

/// Default panels to spawn when entering VR editor mode.
pub const DEFAULT_PANELS: &[InitialPanel] = &[
    InitialPanel {
        panel_type: "toolbar",
        width_m: 0.25,
        height_m: 0.35,
    },
    InitialPanel {
        panel_type: "vr_session",
        width_m: 0.6,
        height_m: 0.8,
    },
    InitialPanel {
        panel_type: "hierarchy",
        width_m: 0.5,
        height_m: 0.8,
    },
    InitialPanel {
        panel_type: "inspector",
        width_m: 0.5,
        height_m: 0.9,
    },
    InitialPanel {
        panel_type: "console",
        width_m: 0.7,
        height_m: 0.5,
    },
];

/// Spawn default panels in an arc around the user at head height.
///
/// The arc is centered at `center_pos` (typically user's head position),
/// facing inward, with `radius` meters distance.
pub fn spawn_default_layout(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    center_pos: Vec3,
    radius: f32,
) {
    let panel_count = DEFAULT_PANELS.len();
    let arc_span = std::f32::consts::FRAC_PI_3; // 60° total arc
    let start_angle = -arc_span / 2.0;

    for (i, panel_def) in DEFAULT_PANELS.iter().enumerate() {
        let t = if panel_count > 1 {
            i as f32 / (panel_count - 1) as f32
        } else {
            0.5
        };
        let angle = start_angle + t * arc_span;

        // Position on the arc (Y = head height, XZ = arc around user)
        let x = center_pos.x + radius * angle.sin();
        let z = center_pos.z - radius * angle.cos();
        let y = center_pos.y;

        // Face toward user — looking_at points -Z at target, then rotate 180°
        // around Y so the +Z face (with correct UVs) points toward the user.
        let mut position = Transform::from_xyz(x, y, z)
            .looking_at(Vec3::new(center_pos.x, y, center_pos.z), Vec3::Y);
        position.rotate_y(std::f32::consts::PI);

        spawn_vr_panel(
            commands,
            meshes,
            materials,
            images,
            panel_def.panel_type,
            position,
            panel_def.width_m,
            panel_def.height_m,
            512.0,
        );
    }
}
