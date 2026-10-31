//! A UI node that draws many rectangles and labels cheaply.
//!
//! The profiler's timeline and graphs are thousands of bars that move every
//! time the view does. As UI nodes each would be laid out by taffy and
//! respawned or restyled per change, which is the pattern that has dragged
//! the editor down before. Here the bars are ONE mesh: a camera of the
//! canvas's own renders the mesh, plus a small pool of text labels, into an
//! image, and the node shows that image. Ember lays out a box; the box's
//! contents never touch the UI tree.
//!
//! A view fills [`CanvasContent`] (rectangles and labels in logical pixels from
//! the node's top-left) and sets `dirty`; the mesh is rebuilt only then.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite::Anchor;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

/// Render layers canvases use start here, clear of the default layer the
/// window's camera draws.
const FIRST_LAYER: usize = 20;

/// Labels are drawn up to this many per canvas; beyond it the smallest rects
/// simply go unlabelled.
const MAX_LABELS: usize = 400;

/// Rects a canvas draws, and the fixed size of its mesh (see `draw_canvases`
/// for why it is fixed). 32,768 quads is 4.5 MB of vertices and indices; the
/// busiest timeline view draws a few thousand, since spans under a third of a
/// pixel are skipped.
const MAX_RECTS: usize = 32_768;

/// One filled rectangle, in logical pixels from the canvas's top-left.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

/// One line of text, top-left anchored, in logical pixels.
#[derive(Clone, Debug)]
pub(crate) struct CLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size: f32,
    pub color: Color,
}

/// What a canvas shows. Fill it and set `dirty`.
#[derive(Component, Default)]
pub(crate) struct CanvasContent {
    pub background: Option<Color>,
    pub rects: Vec<CRect>,
    pub labels: Vec<CLabel>,
    pub dirty: bool,
}

/// The canvas's machinery: its camera, image, mesh and label pool.
#[derive(Component)]
pub(crate) struct Canvas {
    layer: usize,
    camera: Entity,
    mesh: Handle<Mesh>,
    image: Option<Handle<Image>>,
    /// The image's size in physical pixels.
    size: UVec2,
    scale: f32,
    labels: Vec<Entity>,
}

impl Canvas {
    /// The canvas's size in logical pixels, for views laying out content.
    pub fn logical_size(&self) -> Vec2 {
        self.size.as_vec2() / self.scale.max(f32::EPSILON)
    }
}

/// Counts canvases, so each gets a render layer of its own. Pass `&mut .0` to
/// [`spawn_canvas`].
#[derive(Resource, Default)]
pub(crate) struct NextLayer(pub usize);

pub(crate) fn build(app: &mut App) {
    app.init_resource::<NextLayer>().add_systems(
        PostUpdate,
        (fit_canvases, draw_canvases)
            .chain()
            .after(bevy::ui::UiSystems::Layout),
    );
}

/// A canvas node, filling its parent. Add children to it for anything that
/// should sit over the drawing (it is an ordinary UI node).
pub(crate) fn spawn_canvas(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    next: &mut usize,
) -> Entity {
    let layer = FIRST_LAYER + *next;
    *next += 1;
    let mesh = meshes.add(empty_mesh());
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -10 - layer as isize,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                is_active: false,
                ..default()
            },
            RenderLayers::layer(layer),
            Name::new("companion-canvas-camera"),
        ))
        .id();
    commands.spawn((
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::WHITE))),
        Transform::IDENTITY,
        RenderLayers::layer(layer),
        Name::new("companion-canvas-mesh"),
    ));
    commands
        .spawn((
            // Absolute, filling its box. In the flow, an `ImageNode` asks for
            // its image's size, and the image is made at the node's size, so
            // the canvas could only ever grow: the timeline pushed the strip
            // under it out of the window. Out of the flow, the box is sized by
            // the layout and the canvas follows it.
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            RelativeCursorPosition::default(),
            Canvas { layer, camera, mesh, image: None, size: UVec2::ZERO, scale: 1.0, labels: Vec::new() },
            CanvasContent::default(),
            Name::new("companion-canvas"),
        ))
        .id()
}

fn empty_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new())
        .with_inserted_indices(Indices::U32(Vec::new()))
}

/// Keep each canvas's image the size of its node, in physical pixels.
fn fit_canvases(
    mut canvases: Query<(&mut Canvas, &ComputedNode, &mut ImageNode, &mut CanvasContent)>,
    mut cameras: Query<(&mut Camera, Option<&mut RenderTarget>)>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    for (mut canvas, node, mut image_node, mut content) in &mut canvases {
        let size = node.size().ceil().as_uvec2().max(UVec2::ONE);
        let scale = 1.0 / node.inverse_scale_factor().max(f32::EPSILON);
        if size == canvas.size && (scale - canvas.scale).abs() < f32::EPSILON && canvas.image.is_some() {
            continue;
        }
        let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Bgra8UnormSrgb, None);
        // No CPU copy: only the GPU draws into or samples a render target, and
        // wgpu zero-fills one created without data.
        image.data = None;
        let handle = images.add(image);
        if let Some(old) = canvas.image.replace(handle.clone()) {
            images.remove(&old);
        }
        canvas.size = size;
        canvas.scale = scale;
        image_node.image = handle.clone();
        if let Ok((mut camera, target)) = cameras.get_mut(canvas.camera) {
            camera.is_active = true;
            match target {
                Some(mut target) => *target = RenderTarget::from(handle),
                None => {
                    commands.entity(canvas.camera).insert(RenderTarget::from(handle));
                }
            }
        }
        // Everything was laid out for the old size.
        content.dirty = true;
    }
}

/// Rebuild the mesh and labels of every canvas whose content changed.
fn draw_canvases(
    mut canvases: Query<(&mut Canvas, &mut CanvasContent)>,
    mut cameras: Query<&mut Camera>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut labels: Query<(&mut Text2d, &mut TextFont, &mut TextColor, &mut Transform, &mut Visibility)>,
    mut commands: Commands,
) {
    for (mut canvas, mut content) in &mut canvases {
        if !content.dirty || canvas.size == UVec2::ZERO {
            continue;
        }
        content.dirty = false;
        let sf = canvas.scale;
        let half = canvas.size.as_vec2() / 2.0;
        // Logical px from the top-left, to the camera's centred, y-up space.
        let to_cam = |x: f32, y: f32| [x * sf - half.x, half.y - y * sf, 0.0];

        if let Ok(mut camera) = cameras.get_mut(canvas.camera) {
            camera.clear_color = ClearColorConfig::Custom(content.background.unwrap_or(Color::NONE));
        }

        // Always exactly MAX_RECTS quads: the unused ones zero-sized, which
        // draw nothing. A mesh whose size changed every redraw was placed
        // again by Bevy's mesh allocator each time, and the allocator's
        // buffers only grow: the companion's GPU memory climbed 256 MB at a
        // time while the profiler drew live, past 3.5 GB in a long session.
        // One size means one allocation, kept for the canvas's life.
        let mut positions = Vec::with_capacity(MAX_RECTS * 4);
        let mut colors = Vec::with_capacity(MAX_RECTS * 4);
        let mut indices = Vec::with_capacity(MAX_RECTS * 6);
        for r in content.rects.iter().take(MAX_RECTS) {
            if r.w <= 0.0 || r.h <= 0.0 {
                continue;
            }
            let base = positions.len() as u32;
            positions.push(to_cam(r.x, r.y));
            positions.push(to_cam(r.x + r.w, r.y));
            positions.push(to_cam(r.x + r.w, r.y + r.h));
            positions.push(to_cam(r.x, r.y + r.h));
            let c = r.color.to_linear().to_f32_array();
            colors.extend([c, c, c, c]);
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
        positions.resize(MAX_RECTS * 4, [0.0; 3]);
        colors.resize(MAX_RECTS * 4, [0.0; 4]);
        indices.resize(MAX_RECTS * 6, 0);
        if let Some(mut mesh) = meshes.get_mut(&canvas.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.insert_indices(Indices::U32(indices));
        }

        let wanted = content.labels.len().min(MAX_LABELS);
        while canvas.labels.len() < wanted {
            let label = commands
                .spawn((
                    Text2d::new(""),
                    TextFont::default(),
                    TextColor(Color::WHITE),
                    Anchor::TOP_LEFT,
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    RenderLayers::layer(canvas.layer),
                ))
                .id();
            canvas.labels.push(label);
        }
        for (i, entity) in canvas.labels.iter().enumerate() {
            let Ok((mut text, mut font, mut color, mut transform, mut visibility)) = labels.get_mut(*entity) else {
                continue;
            };
            match content.labels.get(i).filter(|_| i < wanted) {
                Some(label) => {
                    if text.0 != label.text {
                        text.0.clone_from(&label.text);
                    }
                    font.font_size = bevy::text::FontSize::Px(label.size * sf);
                    color.0 = label.color;
                    let [x, y, _] = to_cam(label.x, label.y);
                    transform.translation = Vec3::new(x.round(), y.round(), 1.0);
                    *visibility = Visibility::Inherited;
                }
                None => *visibility = Visibility::Hidden,
            }
        }
    }
}
