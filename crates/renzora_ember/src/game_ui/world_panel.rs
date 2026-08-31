//! World-space UI: render a [`UiCanvas`] whose `render_space == "world"` onto a
//! plane in the 3D scene (a monitor on a wall, a floating menu in VR) instead of
//! the normal fullscreen canvas.
//!
//! The canvas entity is *everything at once* — the 3D object (Transform-placed
//! quad/mesh), the template holder, and the script host — so `{{ }}` bindings
//! resolve against it natively, exactly like a screen canvas. That's the whole
//! reason this replaced the old separate `WorldUiPanel`: a script attached to the
//! canvas is on the same entity the bindings read.
//!
//! A world canvas resolves at runtime into three pieces tracked on the
//! non-serialized [`WorldUiPanelLive`]:
//!
//! 1. an offscreen `Image` (same descriptor recipe as `canvas_render`);
//! 2. a dedicated `Camera2d` targeting it (`IsolatedCamera`, transparent clear);
//! 3. a UI root routed to that camera via `UiTargetCamera`, carrying a copy of
//!    the template path plus [`MarkupBindingHost`]`(canvas)` so the markup builds
//!    there but its bindings resolve against the **canvas** (where the script is).
//!
//! The canvas itself gains an unlit quad showing the image (texture mode) or has
//! its laid-out UI emitted as 3D geometry (mesh mode, see `world_ui_mesh`). The
//! runtime pieces are rebuilt from the serialized [`UiCanvas`] on load, so they
//! survive save/load without being serialized themselves.

use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::ui::{ComputedNode, ComputedUiTargetCamera, ContentSize, UiTargetCamera};

use crate::markup::binding::MarkupBindingHost;

use super::components::{HtmlTemplatePath, UiCanvas};

/// World meters per reference pixel for a world canvas at unit `Transform` scale.
/// Chosen so a 1280×720 reference reads ~1.6×0.9 m by default; the entity's
/// `Transform` then scales it to taste (there is deliberately no size field).
const WORLD_CANVAS_UNITS_PER_PX: f32 = 0.00125;

/// The offscreen layout/RTT resolution of a world canvas — its reference dims.
pub(crate) fn canvas_resolution(canvas: &UiCanvas) -> UVec2 {
    UVec2::new(
        canvas.reference_width.max(16.0) as u32,
        canvas.reference_height.max(16.0) as u32,
    )
}

/// The world-space quad size at unit scale (before the entity `Transform`).
pub(crate) fn canvas_size(canvas: &UiCanvas) -> Vec2 {
    canvas_resolution(canvas).as_vec2() * WORLD_CANVAS_UNITS_PER_PX
}

/// The generated, camera-routed UI root a world canvas's markup builds under.
///
/// Exists so the canvas invariants in `GameUiPlugin` can tell this root apart
/// from a genuinely orphaned widget (a widget with no `UiCanvas` ancestor renders
/// into the editor's own UI). A world root is parentless *by design* (routed by
/// `UiTargetCamera`, not ancestry), so without this marker every one would get a
/// stray `UiCanvas` healed around it moments after spawning.
#[derive(Component)]
pub struct WorldUiRoot;

/// Back-link from a world canvas's generated entities (its offscreen camera, its
/// UI root, its emitted mesh children) to the [`UiCanvas`] that owns them.
///
/// Those entities are NOT children of the canvas (the camera/root are routed by
/// `UiTargetCamera`, not ancestry), so deleting the canvas — or switching it back
/// to screen space — leaves them orphaned. [`cleanup_orphaned_world_ui`] uses this
/// link to tear them down in either case.
#[derive(Component)]
pub(crate) struct WorldUiPanelOwner(pub Entity);

/// Runtime pieces backing a resolved world canvas. Not reflect-registered: rebuilt
/// from [`UiCanvas`] on load, never serialized.
#[derive(Component)]
pub struct WorldUiPanelLive {
    pub(crate) camera: Entity,
    pub(crate) ui_root: Entity,
    /// The offscreen target — also the picking pointer's render target.
    pub(crate) image: Handle<Image>,
}

pub(crate) fn register(app: &mut App) {
    // The consumer owns the resource and the ordering: producers live in crates
    // that never link this one (renzora_xr, renzora_viewport), so they can only
    // declare set membership. `init_resource` here means world canvases work even
    // in a build with no XR and no editor viewport.
    app.init_resource::<renzora::WorldUiPointers>();
    app.configure_sets(
        Update,
        renzora::WorldUiPointerSet::Publish.before(renzora::WorldUiPointerSet::Consume),
    );
    app.add_systems(
        Update,
        publish_game_mouse_ray.in_set(renzora::WorldUiPointerSet::Publish),
    );
    app.add_systems(
        Update,
        // Canvases resolve before the pointers that hit-test them, so a canvas
        // added this frame is clickable this frame rather than next.
        (
            sync_world_ui_canvases,
            cleanup_orphaned_world_ui,
            drive_world_ui_pointers,
        )
            .chain()
            .in_set(renzora::WorldUiPointerSet::Consume),
    );
    // Scene load inserts components by reflection, which does NOT bump change
    // ticks, so `Changed<UiCanvas>`/`Changed<HtmlTemplatePath>` never fires for a
    // loaded canvas and it would stay unbuilt. These observers force the change on
    // the reflection insert so the sync rebuilds it next frame. Both are needed
    // because the two components load in an unspecified order.
    app.add_observer(mark_canvas_changed_on_canvas_insert);
    app.add_observer(mark_canvas_changed_on_path_insert);
}

/// Bump a [`UiCanvas`]'s change tick so `sync_world_ui_canvases` reprocesses it.
fn touch_canvas(world: &mut World, entity: Entity) {
    if let Some(mut canvas) = world.get_mut::<UiCanvas>(entity) {
        canvas.set_changed();
    }
}

fn mark_canvas_changed_on_canvas_insert(trigger: On<Insert, UiCanvas>, mut commands: Commands) {
    let entity = trigger.entity;
    commands.queue(move |world: &mut World| touch_canvas(world, entity));
}

fn mark_canvas_changed_on_path_insert(
    trigger: On<Insert, HtmlTemplatePath>,
    canvases: Query<(), With<UiCanvas>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    // Only canvases — a plain markup holder is handled by the generic pipeline.
    if canvases.get(entity).is_err() {
        return;
    }
    commands.queue(move |world: &mut World| touch_canvas(world, entity));
}

/// Shipped-game mouse → world-UI ray (pointer id 2). Editor sessions publish the
/// same id from the viewport instead (renzora_viewport), so this gates off there.
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
/// states, `on_ui` markup callbacks — everything — works on world canvases
/// unmodified.
#[derive(Default)]
struct PanelPointer {
    entity: Option<Entity>,
    last_position: Option<Vec2>,
    pressed: bool,
}

fn drive_world_ui_pointers(
    mut commands: Commands,
    rays: Option<Res<renzora::WorldUiPointers>>,
    canvases: Query<(&UiCanvas, &WorldUiPanelLive, &GlobalTransform)>,
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
    // ended) must not leave its pointer stuck hovering/pressed: release and park
    // pointers whose ray is absent this frame.
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

        // Nearest canvas hit along the ray.
        let mut best: Option<(f32, Location)> = None;
        for (canvas, live, transform) in canvases.iter() {
            let size = canvas_size(canvas);
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
            let half = size * 0.5;
            if local.x.abs() > half.x || local.y.abs() > half.y {
                continue;
            }
            // Quad-local XY → UI pixels (UI y grows downward).
            let uv = Vec2::new(local.x / size.x + 0.5, 0.5 - local.y / size.y);
            let position = uv * canvas_resolution(canvas).as_vec2();
            best = Some((
                t,
                Location {
                    target: bevy::camera::NormalizedRenderTarget::Image(live.image.clone().into()),
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
                // Ray left every canvas: release a held press so buttons don't
                // stick, then park the pointer off-target.
                if pointer.pressed {
                    pointer.pressed = false;
                    if let Ok(Some(location)) = locations.get(entity).map(|l| l.location.clone()) {
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

/// Resolve added/changed world canvases. A change tears the old camera/root down
/// and rebuilds — template edits, reference-size and mode changes all take the
/// same (cheap, rare) path. Triggered by a change to *either* the [`UiCanvas`] or
/// the [`HtmlTemplatePath`].
#[allow(clippy::too_many_arguments)]
fn sync_world_ui_canvases(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    canvases: Query<
        (
            Entity,
            &UiCanvas,
            &HtmlTemplatePath,
            Option<&WorldUiPanelLive>,
            Option<&Node>,
        ),
        Or<(Changed<UiCanvas>, Changed<HtmlTemplatePath>)>,
    >,
    children: Query<&Children>,
    text_geom: Query<(), With<super::world_ui_mesh::WorldUiTextGeom>>,
    has_node: Query<(), With<Node>>,
) {
    for (entity, canvas, template, live, had_node) in canvases.iter() {
        // A screen-space canvas is handled by the normal UI pipeline. If it *was*
        // a world canvas (has live pieces), tear them down and hand it back: drop
        // the quad + generated entities and restore a screen root `Node`.
        if !canvas.is_world() {
            if let Some(live) = live {
                commands.entity(live.camera).try_despawn();
                commands.entity(live.ui_root).try_despawn();
                commands.entity(entity).remove::<(
                    WorldUiPanelLive,
                    Mesh3d,
                    MeshMaterial3d<StandardMaterial>,
                    super::world_ui_mesh::WorldUiMeshBuilt,
                )>();
                commands
                    .entity(entity)
                    .insert(super::components::canvas_root_node(canvas));
            }
            continue;
        }

        // (Re)build from scratch. Drop the previous camera/root + any emitted mesh
        // state so a live mode switch matches a fresh reload.
        if let Some(live) = live {
            commands.entity(live.camera).try_despawn();
            commands.entity(live.ui_root).try_despawn();
        }
        commands
            .entity(entity)
            .remove::<super::world_ui_mesh::WorldUiMeshBuilt>();
        // Despawn anything this entity built as a screen canvas or emitted as mesh
        // text: the screen template's UI-node children (they'd be a broken taffy
        // subtree once the canvas loses its own `Node`) and any prior text meshes.
        // The world template rebuilds fresh on the generated `ui_root` below.
        if let Ok(ch) = children.get(entity) {
            for c in ch.iter() {
                if has_node.get(c).is_ok() || text_geom.get(c).is_ok() {
                    commands.entity(c).try_despawn();
                }
            }
        }
        // A world canvas is a 3D object, not a screen node — fully un-UI it so
        // bevy_ui stops laying it out (into the screen corner) and lets the mesh
        // render as a plain 3D object. `ComputedNode`/`ContentSize` linger after a
        // bare `Node` removal, so strip them explicitly.
        commands.entity(entity).remove::<(
            Node,
            ComputedNode,
            ComputedUiTargetCamera,
            ContentSize,
            UiTargetCamera,
            GlobalZIndex,
        )>();
        // On the screen→world flip the canvas still carries the `Transform` that UI
        // layout left on it (≈ screen-centre pixels) — that would fling the quad
        // far off. Reset to the origin so it lands in front of the user, ready to
        // be grabbed. Only on the transition (had a `Node`), so re-syncs from a
        // template edit don't clobber where the user placed it.
        if had_node.is_some() {
            commands.entity(entity).insert(Transform::default());
        }

        let size = canvas_size(canvas);

        if template.0.is_empty() {
            // No template → a plain dark surface (sized by the Transform). The
            // inspector's "+" authors a template when the user wants one.
            commands.entity(entity).insert((
                Mesh3d(meshes.add(Rectangle::new(size.x, size.y))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.07, 0.08, 0.10),
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })),
            ));
            commands.entity(entity).remove::<WorldUiPanelLive>();
            continue;
        }

        // Offscreen target — canvas_render's recipe.
        let res = canvas_resolution(canvas);
        let extent = Extent3d {
            width: res.x.clamp(16, 7680),
            height: res.y.clamp(16, 4320),
            depth_or_array_layers: 1,
        };
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: Some("world_ui_canvas"),
                size: extent,
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
        image.resize(extent);
        let image_handle = images.add(image);

        let camera = commands
            .spawn((
                Camera2d,
                Camera {
                    // Transparent clear: the quad composites over the scene.
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    order: -30,
                    ..default()
                },
                RenderTarget::Image(image_handle.clone().into()),
                renzora::IsolatedCamera,
                renzora::HideInHierarchy,
                WorldUiPanelOwner(entity),
                Name::new("World UI Canvas Camera"),
            ))
            .id();

        let ui_root = commands
            .spawn((
                Name::new("World UI Canvas Root"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                UiTargetCamera(camera),
                // The template builds here (routed to the offscreen camera), but
                // its `{{ }}` bindings must resolve against the CANVAS — that's
                // where the script and any game components live. `MarkupBindingHost`
                // redirects the build host so `{{ health }}` / `fill="health"`
                // read the canvas, exactly like a screen canvas reads itself.
                HtmlTemplatePath(template.0.clone()),
                MarkupBindingHost(entity),
                WorldUiRoot,
                WorldUiPanelOwner(entity),
                renzora::HideInHierarchy,
            ))
            .id();

        commands.entity(entity).insert((
            Mesh3d(meshes.add(Rectangle::new(size.x, size.y))),
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

/// Tear down any entity generated *for* a world canvas — its offscreen camera, its
/// UI root, or a mesh child emitted by mesh mode — once its owner is no longer a
/// live world canvas (deleted, or switched back to screen space).
///
/// This is the catch-all: it keys off [`WorldUiPanelOwner`] on the *generated*
/// entities, which survive even when the canvas entity is fully deleted (the
/// common case: select it, press Delete). `try_despawn` is recursive, so
/// despawning the UI root drops its whole built subtree too.
fn cleanup_orphaned_world_ui(
    mut commands: Commands,
    owned: Query<(Entity, &WorldUiPanelOwner)>,
    canvases: Query<&UiCanvas>,
) {
    for (entity, owner) in owned.iter() {
        let owner_is_world = canvases.get(owner.0).map(|c| c.is_world()).unwrap_or(false);
        if !owner_is_world {
            commands.entity(entity).try_despawn();
        }
    }
}
