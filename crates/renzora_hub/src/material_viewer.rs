//! Native material / shader preview for the marketplace item overlay.
//!
//! When the overlay opens on a "Materials & Shaders" asset, we download the
//! shader source, compile it with [`renzora_shader`], render it on a **selectable
//! primitive** (sphere / cube / plane / torus) to an offscreen texture, and show
//! that texture as the overlay's main viewer — the same `Handle<Image>`-in-an-
//! `ImageNode` path the model turntable and image gallery use.
//!
//! It mirrors the website's shader preview: pick a shape, and the shader's
//! `@param` annotations become **live controls** (float sliders, colour sliders)
//! that recompile the material in place. That reuse is deliberate — the website's
//! WASM preview and this native viewer both drive `renzora_shader`, so a shader
//! previews identically in the browser and the editor.
//!
//! Compilation, param extraction and the `CodeShaderMaterial` pipeline all live
//! in `renzora_shader` (registered by `ShaderPlugin`, always in the editor); this
//! module is only the offscreen rig + the overlay's controls.

use std::collections::HashMap;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::Hdr;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use crossbeam_channel::{unbounded, Receiver, TryRecvError};

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora::SplashState;
use renzora_auth::marketplace::AssetSummary;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{bind_display, keyed_list, Bound, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::slider;
use renzora_grid::{InfiniteGrid, InfiniteGridSettings};
use renzora_shader::file::{self, ParamType, ParamValue, ShaderParam};
use renzora_shader::registry::{self, ShaderBackendRegistry};
use renzora_shader::runtime::{CodeShaderMaterial, ShaderCache, ShaderUniforms};

/// A dedicated render layer — distinct from the model viewer (13) and every
/// other preview so nothing cross-contaminates.
const MATERIAL_VIEWER_LAYER: usize = 14;

/// Offscreen render-target resolution (16:9, matches the overlay header viewer).
const RTT_W: u32 = 640;
const RTT_H: u32 = 360;

// ── Types ───────────────────────────────────────────────────────────────────

/// The preview primitive the shader is rendered on. Mirrors the website's mesh
/// selector.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MatShape {
    #[default]
    Sphere,
    Cube,
    Plane,
    Torus,
}

impl MatShape {
    fn all() -> [MatShape; 4] {
        [MatShape::Sphere, MatShape::Cube, MatShape::Plane, MatShape::Torus]
    }
    fn label(self) -> &'static str {
        match self {
            MatShape::Sphere => "Sphere",
            MatShape::Cube => "Cube",
            MatShape::Plane => "Plane",
            MatShape::Torus => "Torus",
        }
    }
    fn mesh(self) -> Mesh {
        match self {
            // ico(5) gives a smooth, seam-free sphere for shaders that key off UV.
            MatShape::Sphere => Sphere::new(0.9).mesh().ico(5).unwrap(),
            MatShape::Cube => Cuboid::new(1.4, 1.4, 1.4).into(),
            MatShape::Plane => Plane3d::new(Vec3::Y, Vec2::splat(1.1)).into(),
            MatShape::Torus => Torus::new(0.45, 0.9).into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum MatStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed,
}

/// RTT handle the overlay binds its `ImageNode` to.
#[derive(Resource)]
pub(crate) struct MaterialPreviewImage {
    pub handle: Handle<Image>,
    #[allow(dead_code)]
    pub size: (u32, u32),
}

/// The persistent rig, created once and reused for every asset. Only the subject
/// mesh is touched per-asset (shape swap); the camera/lights/grid are static.
#[derive(Resource)]
struct MatViewerRig {
    subject: Entity,
}

/// Per-asset preview session state. Reset on open/close.
#[derive(Resource, Default)]
struct MaterialPreview {
    /// Whether the open asset is a material/shader (drives the header + controls).
    is_material: bool,
    asset_id: Option<String>,
    /// In-flight source download (native only).
    rx: Option<Receiver<Result<Vec<u8>, String>>>,
    /// The shader source, once downloaded — kept so param edits can recompile.
    source: Option<String>,
    /// Extracted `@param`s (name → param), the model behind the live controls.
    params: HashMap<String, ShaderParam>,
    /// Bumped whenever the *set* of params changes (a new shader loads), so the
    /// controls' keyed list rebuilds its rows. A param *value* edit does NOT bump
    /// it (that must not rebuild the slider mid-drag).
    params_version: u64,
    shape: MatShape,
    status: MatStatus,
    active: bool,
    /// The live material; its shader handle is swapped in place on recompile.
    mat_handle: Option<Handle<CodeShaderMaterial>>,
    /// A param value or shape changed → recompile next frame.
    dirty: bool,
}

// ── Controls markers ──────────────────────────────────────────────────────────

#[derive(Component)]
struct ShapeBtn(MatShape);
/// A float/int param slider (`Bound<f32>` 0..1 mapped onto `[min, max]`).
#[derive(Component)]
struct MatFloatParam {
    name: String,
    min: f32,
    max: f32,
}
/// One RGB channel of a colour param (`Bound<f32>` = 0..1 channel value).
#[derive(Component)]
struct MatColorParam {
    name: String,
    channel: usize,
}

// ── Public API (called from item_overlay) ──────────────────────────────────────

/// True for the categories that get a live shader preview.
pub(crate) fn is_material_category(category: &str) -> bool {
    let c = category.to_lowercase();
    c.contains("material") || c.contains("shader")
}

/// The RTT handle for the overlay's `ImageNode` binding.
pub(crate) fn preview_image_handle(w: &World) -> Option<Handle<Image>> {
    w.get_resource::<MaterialPreviewImage>().map(|p| p.handle.clone())
}

/// True once the material has compiled and is rendering — show the RTT.
pub(crate) fn material_ready(w: &World) -> bool {
    w.get_resource::<MaterialPreview>()
        .map(|p| p.is_material && p.status == MatStatus::Ready)
        .unwrap_or(false)
}

/// True while the shader is downloading / compiling — show the placeholder.
pub(crate) fn material_loading(w: &World) -> bool {
    w.get_resource::<MaterialPreview>()
        .map(|p| p.is_material && p.status == MatStatus::Loading)
        .unwrap_or(false)
}

/// True when the overlay is showing a material preview (ready or loading) — used
/// to hide the static image gallery. A failed compile falls back to the gallery.
pub(crate) fn material_active(w: &World) -> bool {
    w.get_resource::<MaterialPreview>()
        .map(|p| p.is_material && p.status != MatStatus::Failed)
        .unwrap_or(false)
}

/// Begin a preview for `asset`. For a material/shader category this kicks the
/// source download and activates the offscreen camera; for anything else it
/// leaves the rig inert so the overlay shows its gallery / model turntable.
pub(crate) fn open_material_preview(world: &mut World, asset: &AssetSummary) {
    let is_material = is_material_category(&asset.category);
    let Some(mut preview) = world.get_resource_mut::<MaterialPreview>() else {
        return;
    };
    *preview = MaterialPreview {
        is_material,
        ..default()
    };
    if !is_material {
        return;
    }
    preview.asset_id = Some(asset.id.clone());

    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = unbounded();
        preview.rx = Some(rx);
        preview.status = MatStatus::Loading;
        preview.active = true;
        // The shader is (almost always) the asset's single file, so the public
        // `preview-file` proxy serves it directly for free assets; a paid asset
        // 401s → status Failed → the overlay falls back to the gallery.
        let url = renzora_auth::marketplace::preview_file_url(&asset.id);
        std::thread::spawn(move || {
            let _ = tx.send(renzora_auth::marketplace::download_file(&url));
        });
    }
    #[cfg(target_arch = "wasm32")]
    {
        preview.status = MatStatus::Failed;
    }
}

/// Tear down when the overlay closes: reset session state and idle the camera.
pub(crate) fn close_material_preview(world: &mut World) {
    if let Some(mut preview) = world.get_resource_mut::<MaterialPreview>() {
        *preview = MaterialPreview::default();
    }
}

// ── Setup ───────────────────────────────────────────────────────────────────

fn setup_material_viewer(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = Extent3d {
        width: RTT_W,
        height: RTT_H,
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        data: Some(vec![0u8; (size.width * size.height * 4) as usize]),
        ..default()
    };
    image.texture_descriptor.size = size;
    image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let handle = images.add(image);
    commands.insert_resource(MaterialPreviewImage {
        handle: handle.clone(),
        size: (RTT_W, RTT_H),
    });

    commands.spawn((
            Camera3d::default(),
            (Hdr, NormalPrepass, DepthPrepass, MotionVectorPrepass),
            Msaa::Off,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.13, 0.16)),
                // A distinct negative order from the model viewer (-8) so the two
                // offscreen cameras never contend.
                order: -9,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(handle.into()),
            Transform::from_xyz(0.0, 1.1, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
            AmbientLight {
                color: Color::srgb(0.85, 0.88, 1.0),
                brightness: 350.0,
                affects_lightmapped_meshes: false,
            },
            RenderLayers::layer(MATERIAL_VIEWER_LAYER),
            MaterialPreviewCamera,
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Marketplace Material Preview Camera"),
        ));

    // Three-point rig (key / fill / rim) — same studio setup as the model viewer.
    for (color, lux, rot, name) in [
        (Color::srgb(1.0, 0.97, 0.92), 5500.0, (-0.7, 0.5, 0.0), "Key"),
        (Color::srgb(0.62, 0.72, 0.92), 1600.0, (0.25, -0.9, 0.0), "Fill"),
        (Color::srgb(0.9, 0.94, 1.0), 4200.0, (-0.35, 2.5, 0.0), "Rim"),
    ] {
        commands.spawn((
            DirectionalLight {
                color,
                illuminance: lux,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, rot.0, rot.1, rot.2)),
            RenderLayers::layer(MATERIAL_VIEWER_LAYER),
            MaterialPreviewLight,
            HideInHierarchy,
            EditorLocked,
            Name::new(format!("Marketplace Material Preview {name} Light")),
        ));
    }

    // Studio ground grid at the subject's feet (the primitives sit around ±1).
    commands.spawn((
        InfiniteGrid,
        InfiniteGridSettings {
            x_axis_color: Color::srgb(0.75, 0.35, 0.38),
            z_axis_color: Color::srgb(0.35, 0.55, 0.85),
            minor_line_color: Color::srgba(0.55, 0.58, 0.64, 0.4),
            major_line_color: Color::srgba(0.72, 0.76, 0.82, 0.7),
            fadeout_distance: 22.0,
            dot_fadeout_strength: 0.25,
            scale: 2.0,
        },
        Transform::from_xyz(0.0, -1.0, 0.0),
        RenderLayers::layer(MATERIAL_VIEWER_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Marketplace Material Preview Grid"),
    ));

    // Subject — a spinning primitive. Starts on a neutral standard material until
    // a shader compiles; a compile failure leaves this in place (gallery fallback).
    let subject = commands
        .spawn((
            Mesh3d(meshes.add(MatShape::default().mesh())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.6, 0.6, 0.64),
                perceptual_roughness: 0.5,
                ..default()
            })),
            Transform::default(),
            RenderLayers::layer(MATERIAL_VIEWER_LAYER),
            MaterialPreviewSubject,
            HideInHierarchy,
            EditorLocked,
            Name::new("Marketplace Material Preview Subject"),
        ))
        .id();

    commands.insert_resource(MatViewerRig { subject });
    commands.init_resource::<MaterialPreview>();
}

#[derive(Component)]
struct MaterialPreviewCamera;
#[derive(Component)]
struct MaterialPreviewLight;
#[derive(Component)]
struct MaterialPreviewSubject;

// ── Lifecycle systems ───────────────────────────────────────────────────────

fn sync_material_camera_active(
    preview: Res<MaterialPreview>,
    mut cameras: Query<&mut Camera, With<MaterialPreviewCamera>>,
) {
    let want = preview.active && preview.status != MatStatus::Failed;
    for mut cam in cameras.iter_mut() {
        if cam.is_active != want {
            cam.is_active = want;
        }
    }
}

fn spin_subject(
    time: Res<Time>,
    preview: Res<MaterialPreview>,
    mut subjects: Query<&mut Transform, With<MaterialPreviewSubject>>,
) {
    if !preview.active || preview.status != MatStatus::Ready {
        return;
    }
    for mut t in subjects.iter_mut() {
        t.rotate_y(time.delta_secs() * 0.5);
    }
}

/// Compile `source` (with its params baked in as WGSL constants) to a shader
/// handle, or `None` on a transpile error. Shared by first-load and recompile.
fn compile(
    source: &str,
    params: &HashMap<String, ShaderParam>,
    registry: &ShaderBackendRegistry,
    cache: &mut ShaderCache,
    shaders: &mut Assets<Shader>,
) -> Option<Handle<Shader>> {
    let language = file::detect_language(source);
    let compiled = match registry.transpile(language, source) {
        Ok(c) => c,
        Err(e) => {
            warn!("[material_viewer] shader compile error: {e}");
            return None;
        }
    };
    let param_consts = file::params_to_wgsl(params);
    let wgsl = if param_consts.is_empty() {
        compiled
    } else {
        registry::inject_param_constants(compiled, &param_consts)
    };
    let label = format!("material-preview://{language}");
    Some(cache.get_or_insert(&wgsl, &label, shaders))
}

/// Drain the source download: compile the shader, extract params, and swap it
/// onto the subject. On any failure, fall back to the gallery.
#[allow(clippy::too_many_arguments)]
fn poll_material_download(
    mut preview: ResMut<MaterialPreview>,
    rig: Option<Res<MatViewerRig>>,
    registry: Option<Res<ShaderBackendRegistry>>,
    mut cache: Option<ResMut<ShaderCache>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut code_materials: ResMut<Assets<CodeShaderMaterial>>,
    mut commands: Commands,
) {
    let Some(rx) = preview.rx.take() else {
        return;
    };
    let (Some(rig), Some(registry), Some(cache)) = (rig, registry, cache.as_mut()) else {
        // Resources not ready yet — put the receiver back and retry next frame.
        preview.rx = Some(rx);
        return;
    };
    match rx.try_recv() {
        Ok(Ok(bytes)) => {
            let source = String::from_utf8_lossy(&bytes).to_string();
            let params = file::extract_params(&source);
            let Some(handle) = compile(&source, &params, &registry, cache, &mut shaders) else {
                preview.status = MatStatus::Failed;
                return;
            };
            let mat = code_materials.add(CodeShaderMaterial {
                uniforms: ShaderUniforms::default(),
                shader_handle: handle,
                alpha_mode: AlphaMode::Blend,
            });
            commands
                .entity(rig.subject)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(mat.clone()));
            preview.mat_handle = Some(mat);
            preview.source = Some(source);
            preview.params = params;
            preview.params_version = preview.params_version.wrapping_add(1);
            preview.status = MatStatus::Ready;
        }
        Ok(Err(e)) => {
            info!("[material_viewer] shader download failed: {e}");
            preview.status = MatStatus::Failed;
        }
        Err(TryRecvError::Empty) => preview.rx = Some(rx),
        Err(TryRecvError::Disconnected) => preview.status = MatStatus::Failed,
    }
}

/// A param value changed → recompile the shader (params bake in as constants)
/// and swap the new handle onto the live material in place.
fn recompile_material(
    mut preview: ResMut<MaterialPreview>,
    registry: Option<Res<ShaderBackendRegistry>>,
    mut cache: Option<ResMut<ShaderCache>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut code_materials: ResMut<Assets<CodeShaderMaterial>>,
) {
    if !preview.dirty {
        return;
    }
    preview.dirty = false;
    let (Some(registry), Some(cache)) = (registry, cache.as_mut()) else {
        return;
    };
    let (Some(source), Some(mat_handle)) = (preview.source.clone(), preview.mat_handle.clone())
    else {
        return;
    };
    if let Some(handle) = compile(&source, &preview.params, &registry, cache, &mut shaders) {
        if let Some(mut mat) = code_materials.get_mut(&mat_handle) {
            mat.shader_handle = handle;
        }
    }
}

// ── Controls (built into the overlay body) ─────────────────────────────────────

/// The material controls block: a shape selector + auto-generated param rows.
/// Shown only while a material preview is active.
pub(crate) fn build_material_controls(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(rgb(section_bg())),
        BorderColor::all(rgb(border())),
        ))
        .id();
    bind_display(commands, wrap, material_active);

    // Shape selector.
    let shape_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let shape_lbl = commands
        .spawn((Text::new("Shape"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())),
            Node { width: Val::Px(52.0), ..default() }))
        .id();
    commands.entity(shape_row).add_child(shape_lbl);
    for shape in MatShape::all() {
        let btn = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(rgb(card_bg())),
                Interaction::default(),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                ShapeBtn(shape),
                Name::new("material-shape-btn"),
            ))
            .id();
        let t = commands
            .spawn((Text::new(shape.label()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())),
                bevy::ui::FocusPolicy::Pass))
            .id();
        commands.entity(btn).add_child(t);
        commands.entity(shape_row).add_child(btn);
    }

    // Properties label + the (reactive) param rows.
    let props_lbl = commands
        .spawn((Text::new("Properties"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    let params = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, params, params_snapshot);

    commands.entity(wrap).add_children(&[shape_row, props_lbl, params]);
    wrap
}

/// One keyed row per extracted param. Keyed by `(name, version, type)` so the
/// rows rebuild when a new shader loads — but NOT on a slider value change (which
/// would recreate the slider mid-drag).
fn params_snapshot(world: &World) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let Some(p) = world.get_resource::<MaterialPreview>() else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    if !p.is_material {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    }
    let mut list: Vec<(String, ShaderParam)> =
        p.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    let ver = p.params_version;
    let items = list
        .iter()
        .enumerate()
        .map(|(i, (name, param))| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, ver, param.param_type as u8).hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    let data = list;
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, param) = &data[i];
            build_param_row(c, f, name, param)
        }),
    }
}

fn build_param_row(commands: &mut Commands, fonts: &EmberFonts, name: &str, param: &ShaderParam) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_primary())),
            Node { width: Val::Px(96.0), flex_shrink: 0.0, ..default() },
        ))
        .id();
    commands.entity(row).add_child(label);

    match param.param_type {
        ParamType::Float | ParamType::Int => {
            let (min, max) = param_range(param);
            let cur = match &param.default_value {
                ParamValue::Float(v) => *v,
                ParamValue::Int(v) => *v as f32,
                _ => min,
            };
            let norm = if max > min { ((cur - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
            let s = slider(commands, norm);
            // Grow to fill the row, but PRESERVE the slider's own layout fields
            // (18px hit-area height, relative positioning, centered) — a bare
            // `Node { flex_grow }` collapses the height and detaches the absolute
            // thumb from the track (the "doubled bars" glitch).
            commands.entity(s).insert((
                MatFloatParam { name: name.to_string(), min, max },
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    height: Val::Px(18.0),
                    position_type: PositionType::Relative,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            commands.entity(row).add_child(s);
        }
        ParamType::Color | ParamType::Vec3 | ParamType::Vec4 => {
            let rgb3 = match &param.default_value {
                ParamValue::Color(v) => [v[0], v[1], v[2]],
                ParamValue::Vec4(v) => [v[0], v[1], v[2]],
                ParamValue::Vec3(v) => *v,
                _ => [1.0, 1.0, 1.0],
            };
            let sliders = commands
                .spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
                .id();
            for (ch, v) in rgb3.iter().enumerate() {
                let s = slider(commands, v.clamp(0.0, 1.0));
                // Full-width, but keep the slider's own height/relative/centered
                // layout (see the float branch — a bare `Node` breaks the thumb).
                commands.entity(s).insert((
                    MatColorParam { name: name.to_string(), channel: ch },
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(18.0),
                        position_type: PositionType::Relative,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ));
                commands.entity(sliders).add_child(s);
            }
            commands.entity(row).add_child(sliders);
        }
        // Vec2 / Bool aren't given a control yet — just show the name so the user
        // still sees the param exists.
        _ => {
            let note = commands
                .spawn((Text::new("(read-only)"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
                .id();
            commands.entity(row).add_child(note);
        }
    }
    row
}

/// A sensible slider range for a float/int param: its declared `[min, max]`, or
/// a default `0..1` (widened to include the default value if it falls outside).
fn param_range(param: &ShaderParam) -> (f32, f32) {
    let cur = match &param.default_value {
        ParamValue::Float(v) => *v,
        ParamValue::Int(v) => *v as f32,
        _ => 0.0,
    };
    let mut min = param.min.unwrap_or(0.0);
    let mut max = param.max.unwrap_or_else(|| cur.max(1.0));
    if max <= min {
        max = min + 1.0;
    }
    min = min.min(cur);
    max = max.max(cur);
    (min, max)
}

// ── Controls systems ────────────────────────────────────────────────────────

fn shape_button_click(
    q: Query<(&Interaction, &ShapeBtn), Changed<Interaction>>,
    mut preview: ResMut<MaterialPreview>,
    rig: Option<Res<MatViewerRig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut subjects: Query<&mut Mesh3d, With<MaterialPreviewSubject>>,
) {
    let Some(rig) = rig else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed && preview.shape != btn.0 {
            preview.shape = btn.0;
            if let Ok(mut m) = subjects.get_mut(rig.subject) {
                m.0 = meshes.add(btn.0.mesh());
            }
        }
    }
}

fn sync_shape_buttons(
    preview: Res<MaterialPreview>,
    mut btns: Query<(&ShapeBtn, &mut BackgroundColor)>,
) {
    for (btn, mut bg) in &mut btns {
        let want = if btn.0 == preview.shape { rgb(accent()) } else { rgb(card_bg()) };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn read_float_params(
    q: Query<(&Bound<f32>, &MatFloatParam), Changed<Bound<f32>>>,
    mut preview: ResMut<MaterialPreview>,
) {
    for (b, p) in &q {
        let value = p.min + b.0.clamp(0.0, 1.0) * (p.max - p.min);
        if let Some(param) = preview.params.get_mut(&p.name) {
            param.default_value = match param.param_type {
                ParamType::Int => ParamValue::Int(value.round() as i32),
                _ => ParamValue::Float(value),
            };
            preview.dirty = true;
        }
    }
}

fn read_color_params(
    q: Query<(&Bound<f32>, &MatColorParam), Changed<Bound<f32>>>,
    mut preview: ResMut<MaterialPreview>,
) {
    for (b, p) in &q {
        let v = b.0.clamp(0.0, 1.0);
        if let Some(param) = preview.params.get_mut(&p.name) {
            let mut arr = match &param.default_value {
                ParamValue::Color(a) => *a,
                ParamValue::Vec4(a) => *a,
                ParamValue::Vec3(a) => [a[0], a[1], a[2], 1.0],
                _ => [1.0, 1.0, 1.0, 1.0],
            };
            if p.channel < 3 {
                arr[p.channel] = v;
            }
            param.default_value = match param.param_type {
                ParamType::Vec3 => ParamValue::Vec3([arr[0], arr[1], arr[2]]),
                ParamType::Vec4 => ParamValue::Vec4(arr),
                _ => ParamValue::Color(arr),
            };
            preview.dirty = true;
        }
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub(crate) struct MaterialViewerPlugin;

impl Plugin for MaterialViewerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, setup_material_viewer).add_systems(
            Update,
            (
                sync_material_camera_active,
                poll_material_download,
                shape_button_click,
                sync_shape_buttons,
                read_float_params,
                read_color_params,
                recompile_material,
                spin_subject,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}
