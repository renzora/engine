//! Per-frame groom update: advance the verlet sim (or hold the rest shape),
//! then rebuild the camera-facing ribbon mesh and sync the material/visibility.
//!
//! Strands are simulated in **world space** with the root pinned to its animated
//! surface position (`GlobalTransform × rest_local[0]`), so hair lags when the
//! head turns and follows the model when static. The same verlet formulation as
//! the bone-based approach, applied to free strand points instead of joints.

use crate::generate::HairGroomData;
use crate::mesh::build_ribbons;
use crate::Hair;
use bevy::prelude::*;
use renzora::PlayModeState;

const MAX_DT: f32 = 1.0 / 30.0;

pub fn simulate_grooms(
    time: Res<Time>,
    play_mode: Option<Res<PlayModeState>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut grooms: Query<(&Hair, &GlobalTransform, &mut HairGroomData)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visibilities: Query<&mut Visibility>,
) {
    let scripts_running = play_mode
        .as_ref()
        .map(|p| p.is_scripts_running())
        .unwrap_or(false);
    let dt = time.delta_secs().min(MAX_DT);

    // Billboard toward the active camera (fall back to any camera, then origin).
    let camera = cameras
        .iter()
        .find(|(c, _)| c.is_active)
        .or_else(|| cameras.iter().next())
        .map(|(_, g)| g.translation())
        .unwrap_or(Vec3::ZERO);

    for (hair, gtransform, mut data) in &mut grooms {
        // Keep the material colour and visibility live.
        let color = Color::srgb(hair.color.x, hair.color.y, hair.color.z);
        if let Some(mut mat) = materials.get_mut(&data.material) {
            if mat.base_color != color {
                mat.base_color = color;
            }
        }
        if let Ok(mut vis) = visibilities.get_mut(data.render_entity) {
            *vis = if hair.enabled {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if !hair.enabled {
            continue; // hidden — skip the sim and the rebuild entirely
        }

        let simulating = scripts_running && hair.simulate;
        step_strands(&mut data, gtransform, simulating, dt, hair);
        if simulating {
            data.sim_seeded = true;
        }

        if let Some(mut mesh) = meshes.get_mut(&data.mesh) {
            build_ribbons(&data.strands, camera, &mut mesh);
        }
    }
}

/// Advance (or reset) every strand of one groom for this frame.
fn step_strands(
    data: &mut HairGroomData,
    g: &GlobalTransform,
    simulating: bool,
    dt: f32,
    hair: &Hair,
) {
    let gravity = Vec3::NEG_Y * 9.81 * hair.gravity;
    let keep = (1.0 - hair.damping.clamp(0.0, 1.0)).powf(dt * 60.0);
    let stiff = 1.0 - (1.0 - hair.stiffness.clamp(0.0, 1.0)).powf(dt * 60.0);
    let seeded = data.sim_seeded;

    for strand in &mut data.strands {
        let m = strand.rest_local.len();
        if m == 0 {
            continue;
        }
        let root_world = g.transform_point(strand.rest_local[0]);

        // Static (editing / sim off) or first sim frame: snap to the grown rest
        // shape in world space and clear velocity, so there is no pop on Play.
        if !simulating || !seeded {
            for i in 0..m {
                let w = g.transform_point(strand.rest_local[i]);
                strand.world[i] = w;
                strand.prev[i] = w;
            }
            continue;
        }

        // Integrate free points (root stays pinned to the surface).
        strand.world[0] = root_world;
        strand.prev[0] = root_world;
        for i in 1..m {
            let target = g.transform_point(strand.rest_local[i]);
            let pos = strand.world[i];
            let vel = (pos - strand.prev[i]) * keep;
            let mut next = pos + vel + gravity * (dt * dt);
            next = next.lerp(target, stiff); // spring back toward the grown shape
            strand.prev[i] = pos;
            strand.world[i] = next;
        }

        // Hold each segment at its rest length (root → tip), and hard-clamp any
        // point that has strayed absurdly far (teleport/scene-load blow-up).
        strand.world[0] = root_world;
        for i in 1..m {
            let a = g.transform_point(strand.rest_local[i - 1]);
            let b = g.transform_point(strand.rest_local[i]);
            let len = (b - a).length();
            let parent = strand.world[i - 1];
            let dir = (strand.world[i] - parent)
                .try_normalize()
                .unwrap_or_else(|| (b - a).normalize_or_zero());
            let mut wp = parent + dir * len;
            if (wp - b).length() > len * 8.0 + 1.0 {
                wp = b;
            }
            strand.world[i] = wp;
        }
    }
}
