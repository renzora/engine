//! World-space game-UI panels: render a game-UI `.html` template onto a quad
//! in the 3D scene (a monitor on a wall, a floating menu in VR) instead of —
//! or alongside — the normal fullscreen canvas.
//!
//! Serializable authoring component [`WorldUiPanel`] holds the template path,
//! physical size and texture resolution; this module resolves it at runtime
//! into three pieces, tracked on the non-serialized [`WorldUiPanelLive`]:
//!
//! 1. an offscreen `Image` (same descriptor recipe as the editor's
//!    `canvas_render` — bevy_ui's render graph needs `RENDER_ATTACHMENT`);
//! 2. a dedicated `Camera2d` targeting it, `IsolatedCamera` so scene-wide
//!    effects skip it, with a transparent clear so the quad composites;
//! 3. a fullscreen UI root routed to that camera via `UiTargetCamera` and
//!    carrying `HtmlTemplatePath` — `MarkupPlugin`'s observer chain builds
//!    the template onto it exactly like the fullscreen path.
//!
//! The panel entity itself gains an unlit, alpha-blended quad showing the
//! image. `UiTargetCamera` is runtime-derived (scene saves deny it), so this
//! survives save/load by reconstruction, like every other asset-path
//! component in the engine.

use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::ui::UiTargetCamera;
use serde::{Deserialize, Serialize};

use super::components::{HtmlTemplatePath, HuiBuildOnSelf};

/// A game-UI template rendered onto a quad in the 3D world.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct WorldUiPanel {
    /// Asset-relative path to the game-UI `.html` template.
    pub template: String,
    /// Quad size in world meters (width, height).
    #[serde(default = "default_panel_size")]
    #[reflect(default = "default_panel_size")]
    pub size: Vec2,
    /// Offscreen texture resolution in pixels.
    #[serde(default = "default_panel_resolution")]
    #[reflect(default = "default_panel_resolution")]
    pub resolution: UVec2,
}

fn default_panel_size() -> Vec2 {
    Vec2::new(1.6, 0.9)
}
fn default_panel_resolution() -> UVec2 {
    UVec2::new(1280, 720)
}

impl Default for WorldUiPanel {
    fn default() -> Self {
        Self {
            template: String::new(),
            size: default_panel_size(),
            resolution: default_panel_resolution(),
        }
    }
}

/// Runtime pieces backing a resolved panel. Not reflect-registered: rebuilt
/// from [`WorldUiPanel`] on load, never serialized.
#[derive(Component)]
pub struct WorldUiPanelLive {
    camera: Entity,
    ui_root: Entity,
    /// The offscreen target — also the picking pointer's render target.
    image: Handle<Image>,
}

pub(crate) fn register(app: &mut App) {
    app.register_type::<WorldUiPanel>();
    app.add_systems(
        Update,
        (
            publish_game_mouse_ray,
            sync_world_ui_panels,
            cleanup_world_ui_panels,
            drive_panel_pointers,
        )
            .chain(),
    );
}

/// Shipped-game mouse → world-UI ray (pointer id 2). Editor sessions publish
/// the same id from the viewport instead (renzora_viewport, where the
/// panel-relative cursor math lives), so this gates itself off there.
fn publish_game_mouse_ray(
    session: Option<Res<renzora::EditorSession>>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), (With<Camera3d>, Without<renzora::IsolatedCamera>)>,
    mouse: Res<ButtonInput<MouseButton>>,
    pointers: Option<ResMut<renzora::WorldUiPointers>>,
) {
    let Some(mut pointers) = pointers else { return };
    if session.is_some_and(|s| s.0) {
        return;
    }
    pointers.0.retain(|r| r.id != 2);
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some((camera, camera_tf)) = cameras.iter().find(|(c, _)| c.is_active) else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_tf, cursor) else {
        return;
    };
    pointers.0.push(renzora::core::WorldUiPointerRay {
        id: 2,
        ray,
        trigger: if mouse.pressed(MouseButton::Left) { 1.0 } else { 0.0 },
    });
}

/// One picking pointer per [`renzora::WorldUiPointerRay`] source, driven by
/// ray-vs-quad hits. bevy_ui's focus system consumes the pointer entities +
/// `PointerInput` messages exactly like the mouse, so `Interaction`, hover
/// states, `on_ui` markup callbacks — everything — works on world panels
/// unmodified.
#[derive(Default)]
struct PanelPointer {
    entity: Option<Entity>,
    last_position: Option<Vec2>,
    pressed: bool,
}

fn drive_panel_pointers(
    mut commands: Commands,
    rays: Option<Res<renzora::WorldUiPointers>>,
    panels: Query<(&WorldUiPanel, &WorldUiPanelLive, &GlobalTransform)>,
    mut pointer_inputs: MessageWriter<bevy::picking::pointer::PointerInput>,
    mut locations: Query<&mut bevy::picking::pointer::PointerLocation>,
    mut state: Local<std::collections::HashMap<u8, PanelPointer>>,
) {
    use bevy::picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
    };

    const TRIGGER_PRESS: f32 = 0.6;
    const TRIGGER_RELEASE: f32 = 0.4;

    let Some(rays) = rays else { return };

    // A producer that stopped publishing (cursor left the viewport, session
    // ended) must not leave its pointer stuck hovering/pressed: release and
    // park pointers whose ray is absent this frame.
    let live_ids: Vec<u8> = rays.0.iter().map(|r| r.id).collect();
    for (id, pointer) in state.iter_mut() {
        if live_ids.contains(id) {
            continue;
        }
        let Some(entity) = pointer.entity else { continue };
        if pointer.pressed {
            pointer.pressed = false;
            if let Ok(Some(location)) = locations.get(entity).map(|l| l.location.clone()) {
                pointer_inputs.write(PointerInput::new(
                    PointerId::Custom(uuid::Uuid::from_u128(0x7e5a_0000_u128 + *id as u128)),
                    location,
                    PointerAction::Release(PointerButton::Primary),
                ));
            }
        }
        pointer.last_position = None;
        if let Ok(mut pointer_location) = locations.get_mut(entity) {
            if pointer_location.location.is_some() {
                pointer_location.location = None;
            }
        }
    }

    for ray_source in rays.0.iter() {
        let pointer = state.entry(ray_source.id).or_default();
        let pointer_id =
            PointerId::Custom(uuid::Uuid::from_u128(0x7e5a_0000_u128 + ray_source.id as u128));
        let entity = *pointer.entity.get_or_insert_with(|| {
            commands
                .spawn((
                    pointer_id,
                    PointerLocation { location: None },
                    Name::new(format!("World UI Pointer {}", ray_source.id)),
                    renzora::HideInHierarchy,
                ))
                .id()
        });

        // Nearest panel hit along the ray.
        let mut best: Option<(f32, Location)> = None;
        for (panel, live, transform) in panels.iter() {
            let normal = transform.affine().matrix3 * Vec3::Z;
            let denom = ray_source.ray.direction.dot(normal);
            if denom.abs() < 1e-6 {
                continue;
            }
            let t = (transform.translation() - ray_source.ray.origin).dot(normal) / denom;
            if t <= 0.0 || best.as_ref().is_some_and(|(bt, _)| *bt < t) {
                continue;
            }
            let hit = ray_source.ray.origin + *ray_source.ray.direction * t;
            let local = transform.affine().inverse().transform_point3(hit);
            let half = panel.size * 0.5;
            if local.x.abs() > half.x || local.y.abs() > half.y {
                continue;
            }
            // Quad-local XY → UI pixels (UI y grows downward).
            let uv = Vec2::new(
                local.x / panel.size.x + 0.5,
                0.5 - local.y / panel.size.y,
            );
            let position = uv * panel.resolution.as_vec2();
            best = Some((
                t,
                Location {
                    target: bevy::camera::NormalizedRenderTarget::Image(
                        live.image.clone().into(),
                    ),
                    position,
                },
            ));
        }

        match best {
            Some((_, location)) => {
                let delta = pointer
                    .last_position
                    .map(|previous| location.position - previous)
                    .unwrap_or(Vec2::ZERO);
                pointer.last_position = Some(location.position);
                if let Ok(mut pointer_location) = locations.get_mut(entity) {
                    pointer_location.location = Some(location.clone());
                }
                pointer_inputs.write(PointerInput::new(
                    pointer_id,
                    location.clone(),
                    PointerAction::Move { delta },
                ));
                if !pointer.pressed && ray_source.trigger > TRIGGER_PRESS {
                    pointer.pressed = true;
                    pointer_inputs.write(PointerInput::new(
                        pointer_id,
                        location,
                        PointerAction::Press(PointerButton::Primary),
                    ));
                } else if pointer.pressed && ray_source.trigger < TRIGGER_RELEASE {
                    pointer.pressed = false;
                    pointer_inputs.write(PointerInput::new(
                        pointer_id,
                        location,
                        PointerAction::Release(PointerButton::Primary),
                    ));
                }
            }
            None => {
                // Ray left every panel: release a held press so buttons don't
                // stick, then park the pointer off-target.
                if pointer.pressed {
                    pointer.pressed = false;
                    if let Ok(Some(location)) =
                        locations.get(entity).map(|l| l.location.clone())
                    {
                        pointer_inputs.write(PointerInput::new(
                            pointer_id,
                            location,
                            PointerAction::Release(PointerButton::Primary),
                        ));
                    }
                }
                pointer.last_position = None;
                if let Ok(mut pointer_location) = locations.get_mut(entity) {
                    if pointer_location.location.is_some() {
                        pointer_location.location = None;
                    }
                }
            }
        }
    }
}

/// Resolve added/changed panels. A change tears the old camera/root down and
/// rebuilds — template edits, resolution changes and size changes all take
/// the same (cheap, rare) path.
fn sync_world_ui_panels(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    panels: Query<(Entity, &WorldUiPanel, Option<&WorldUiPanelLive>), Changed<WorldUiPanel>>,
) {
    for (entity, panel, live) in panels.iter() {
        if let Some(live) = live {
            commands.entity(live.camera).try_despawn();
            commands.entity(live.ui_root).try_despawn();
        }
        if panel.template.is_empty() {
            commands.entity(entity).remove::<WorldUiPanelLive>();
            continue;
        }

        // Offscreen target — canvas_render's recipe.
        let size = Extent3d {
            width: panel.resolution.x.clamp(16, 7680),
            height: panel.resolution.y.clamp(16, 4320),
            depth_or_array_layers: 1,
        };
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: Some("world_ui_panel"),
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
            ..default()
        };
        image.resize(size);
        let image_handle = images.add(image);

        let camera = commands
            .spawn((
                Camera2d,
                Camera {
                    // Transparent clear: the quad composites over the scene,
                    // so empty template regions must stay see-through.
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    order: -30,
                    ..default()
                },
                RenderTarget::Image(image_handle.clone().into()),
                renzora::IsolatedCamera,
                renzora::HideInHierarchy,
                Name::new("World UI Panel Camera"),
            ))
            .id();

        let ui_root = commands
            .spawn((
                Name::new("World UI Panel Root"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                UiTargetCamera(camera),
                HtmlTemplatePath(panel.template.clone()),
                HuiBuildOnSelf,
                renzora::HideInHierarchy,
            ))
            .id();

        commands.entity(entity).insert((
            Mesh3d(meshes.add(Rectangle::new(panel.size.x, panel.size.y))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(image_handle.clone()),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            })),
            WorldUiPanelLive {
                camera,
                ui_root,
                image: image_handle,
            },
        ));
    }
}

/// Tear down the camera + UI root when the panel component is removed.
fn cleanup_world_ui_panels(
    mut commands: Commands,
    orphaned: Query<(Entity, &WorldUiPanelLive), Without<WorldUiPanel>>,
) {
    for (entity, live) in orphaned.iter() {
        commands.entity(live.camera).try_despawn();
        commands.entity(live.ui_root).try_despawn();
        commands
            .entity(entity)
            .remove::<(WorldUiPanelLive, Mesh3d, MeshMaterial3d<StandardMaterial>)>();
    }
}
