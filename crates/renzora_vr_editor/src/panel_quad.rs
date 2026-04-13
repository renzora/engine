//! VR panel quad spawning and despawning.
//!
//! Each VR panel consists of two entities:
//! - A **context entity**: Camera3d + RenderTarget::Image + EguiMultipassSchedule
//!   → bevy_egui renders an independent egui context to an Image texture.
//! - A **quad entity**: Mesh3d(Plane3d) + StandardMaterial textured with the image
//!   → visible in 3D VR space, with picking support for desktop mouse interaction.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::picking::Pickable;
use bevy::render::render_resource::{Extent3d, TextureUsages};
use bevy_egui::EguiMultipassSchedule;
use bevy_egui::BevyEguiEntityCommandsExt;

use crate::{VrPanel, VrPanelBacking, VrPanelPass, PANEL_DEPTH, next_vr_panel_id};

/// Spawn a VR panel as a floating 3D quad with its own egui render context.
///
/// Returns the quad entity (which has the `VrPanel` component).
///
/// # Parameters
/// - `panel_type`: Identifier for panel content routing (e.g. "vr_session", "hierarchy")
/// - `position`: World-space transform for the quad
/// - `width_m` / `height_m`: Physical size in meters
/// - `ppm`: Pixels per meter — controls render resolution (512 = good balance)
pub fn spawn_vr_panel(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    panel_type: &str,
    position: Transform,
    width_m: f32,
    height_m: f32,
    ppm: f32,
) -> Entity {
    let px_w = (width_m * ppm) as u32;
    let px_h = (height_m * ppm) as u32;

    // Render target image (transparent background, zero-initialized)
    let image = images.add({
        let size = Extent3d {
            width: px_w,
            height: px_h,
            depth_or_array_layers: 1,
        };
        let mut img = Image {
            data: Some(vec![0; (px_w * px_h * 4) as usize]),
            ..default()
        };
        img.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        img.texture_descriptor.size = size;
        img
    });

    // Each VR panel needs a unique schedule ID for bevy_egui's multi-pass system.
    let panel_id = next_vr_panel_id();

    // Camera that renders egui to the image texture.
    // RenderLayers::none() means no 3D geometry is rendered — only the egui pass.
    let context_entity = commands
        .spawn((
            Camera3d::default(),
            RenderLayers::none(),
            Camera {
                order: -10,
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            EguiMultipassSchedule::new(VrPanelPass(panel_id)),
            Name::new(format!("VR Panel Context: {}", panel_type)),
        ))
        .id();

    // Material: unlit, opaque, textured with egui render target
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(image.clone()),
        alpha_mode: AlphaMode::Opaque,
        unlit: true,
        ..default()
    });

    // Front face mesh — Plane3d normal faces +Z, offset slightly forward so it
    // sits in front of the backing cuboid (avoids z-fighting).
    let quad_mesh = meshes.add(
        Plane3d::new(Vec3::Z, Vec2::new(width_m / 2.0, height_m / 2.0)).mesh(),
    );

    // Backing cuboid — solid dark material, gives the panel physical depth
    let backing_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.14),
        emissive: LinearRgba::new(0.03, 0.03, 0.04, 1.0),
        unlit: true,
        ..default()
    });
    let backing_mesh = meshes.add(Cuboid::new(width_m, height_m, PANEL_DEPTH));

    // Spawn the visible quad entity with picking support
    let quad = commands
        .spawn((
            Mesh3d(quad_mesh),
            MeshMaterial3d(material),
            position,
            VrPanel {
                panel_type: panel_type.to_string(),
                context_entity,
                image_handle: image,
                width_meters: width_m,
                height_meters: height_m,
                schedule_id: panel_id,
            },
            Pickable {
                should_block_lower: false,
                is_hoverable: true,
            },
            Name::new(format!("VR Panel: {}", panel_type)),
        ))
        .add_picking_observers_for_context(context_entity)
        .with_child((
            VrPanelBacking,
            Mesh3d(backing_mesh),
            MeshMaterial3d(backing_mat),
            // Center the backing behind the front face
            Transform::from_xyz(0.0, 0.0, -(PANEL_DEPTH / 2.0 + 0.001)),
            Name::new(format!("VR Panel Backing: {}", panel_type)),
        ))
        .id();

    quad
}

/// Despawn a VR panel — removes both the quad entity and its context camera entity.
pub fn despawn_vr_panel(commands: &mut Commands, quad_entity: Entity, panel: &VrPanel) {
    commands.entity(panel.context_entity).despawn();
    commands.entity(quad_entity).despawn();
}
