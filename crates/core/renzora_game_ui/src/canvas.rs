//! UI Canvas panel — 2D visual editor for laying out bevy_ui widgets.
//!
//! Renders an egui canvas that mirrors the bevy_ui hierarchy. Each UiWidget
//! entity is drawn as a rectangle that can be selected, moved, and resized.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use egui_phosphor::regular;
use renzora_editor::{AssetDragPayload, EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::*;
use crate::palette::WidgetDragPayload;
use crate::shapes::*;

// ── Canvas Preview (render selected camera behind UI canvas) ─────────────────

const CANVAS_PREVIEW_WIDTH: u32 = 1280;
const CANVAS_PREVIEW_HEIGHT: u32 = 720;

/// Resource holding the canvas preview render target and camera.
#[derive(Resource)]
pub struct UiCanvasPreview {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<egui::TextureId>,
    /// The preview camera entity we spawned.
    pub camera_entity: Option<Entity>,
    /// The scene camera entity we're currently previewing.
    pub previewing: Option<Entity>,
}

use renzora_core::UiCanvasPreviewCamera;

/// Sets up the canvas preview render target. Called once from GameUiPlugin build.
pub fn setup_canvas_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
) {
    let size = Extent3d {
        width: CANVAS_PREVIEW_WIDTH,
        height: CANVAS_PREVIEW_HEIGHT,
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

    let image_handle = images.add(image);
    user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));
    let texture_id = user_textures.image_id(image_handle.id());

    commands.insert_resource(UiCanvasPreview {
        image_handle,
        texture_id,
        camera_entity: None,
        previewing: None,
    });
}

/// Updates the canvas preview camera to match the selected/default scene camera.
///
/// Priority: selected Camera3d in hierarchy → DefaultCamera → first scene Camera3d → nothing.
pub fn update_canvas_preview(
    mut commands: Commands,
    selection: Res<EditorSelection>,
    mut preview: ResMut<UiCanvasPreview>,
    scene_cameras: Query<
        (Entity, &GlobalTransform, &Projection, Option<&renzora_core::DefaultCamera>),
        (With<Camera3d>, Without<UiCanvasPreviewCamera>, Without<renzora_core::EditorCamera>),
    >,
    mut preview_cameras: Query<
        (Entity, &mut Transform, &mut Projection),
        With<UiCanvasPreviewCamera>,
    >,
    editor_cameras: Query<
        (Option<&bevy::core_pipeline::Skybox>, &Camera),
        (With<renzora_core::EditorCamera>, Without<UiCanvasPreviewCamera>),
    >,
) {
    let selected = selection.get();

    // Pick target camera: selected Camera3d → DefaultCamera → first scene Camera3d
    let target = selected
        .and_then(|e| scene_cameras.get(e).ok())
        .map(|(e, gt, p, _)| (e, gt, p))
        .or_else(|| {
            scene_cameras
                .iter()
                .find(|(_, _, _, dc)| dc.is_some())
                .map(|(e, gt, p, _)| (e, gt, p))
        })
        .or_else(|| {
            scene_cameras
                .iter()
                .next()
                .map(|(e, gt, p, _)| (e, gt, p))
        });

    let existing = preview_cameras.iter_mut().next();

    let (editor_skybox, editor_clear) = editor_cameras
        .iter()
        .next()
        .map(|(skybox, cam)| (skybox.cloned(), cam.clear_color.clone()))
        .unwrap_or((None, ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12))));

    if let Some((cam_entity, cam_gt, cam_proj)) = target {
        preview.previewing = Some(cam_entity);
        let (scale, rotation, translation) = cam_gt.to_scale_rotation_translation();
        let cam_transform = Transform { translation, rotation, scale };

        match existing {
            Some((entity, mut t, mut p)) => {
                *t = cam_transform;
                *p = cam_proj.clone();
                if let Some(ref skybox) = editor_skybox {
                    commands.entity(entity).try_insert(skybox.clone());
                } else {
                    commands.entity(entity).remove::<bevy::core_pipeline::Skybox>();
                }
            }
            None => {
                let mut ecmds = commands.spawn((
                    Camera3d::default(),
                    Msaa::Off,
                    Camera {
                        clear_color: editor_clear,
                        order: -3,
                        is_active: false,
                        ..default()
                    },
                    RenderTarget::Image(preview.image_handle.clone().into()),
                    cam_proj.clone(),
                    cam_transform,
                    UiCanvasPreviewCamera,
                    renzora_core::IsolatedCamera,
                    renzora_core::HideInHierarchy,
                    renzora_core::EditorLocked,
                    Name::new("UI Canvas Preview Camera"),
                ));
                if let Some(skybox) = editor_skybox {
                    ecmds.insert(skybox);
                }
                preview.camera_entity = Some(ecmds.id());
            }
        }
    } else {
        preview.previewing = None;
        if let Some((entity, _, _)) = existing {
            commands.entity(entity).despawn();
            preview.camera_entity = None;
        }
    }
}


/// Image file extensions accepted for drag-and-drop onto the canvas.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "tga", "webp"];

// ── Snapshot types ────────────────────────────────────────────────────────────

/// A snapshot of one UI widget taken from the ECS each frame.
#[derive(Clone, Debug)]
struct WidgetSnapshot {
    entity: Entity,
    name: String,
    widget_type: UiWidgetType,
    locked: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    parent: Option<Entity>,
    has_bg: bool,
    bg_color: [f32; 4],
    has_border: bool,
    border_color: [f32; 4],
    /// Egui texture id for Image widgets (looked up from ImageNode handle).
    image_texture_id: Option<egui::TextureId>,

    // ── Style data (from individual style components) ─────────────────
    border_radius: [f32; 4],
    stroke_width: f32,
    opacity: f32,
    shadow: Option<[f32; 6]>, // [r, g, b, a, blur, spread] (offset baked into rect)

    // ── Text content ────────────────────────────────────────────────
    text_content: Option<String>,
    text_size: f32,
    text_color: [f32; 4],
    text_bold: bool,

    // ── Per-widget-type data ────────────────────────────────────────
    widget_data: WidgetDataSnapshot,
}

/// Per-widget-type visual data needed for faithful preview rendering.
#[derive(Clone, Debug, Default)]
enum WidgetDataSnapshot {
    #[default]
    None,
    Slider {
        value: f32,
        min: f32,
        max: f32,
        track_color: [f32; 4],
        fill_color: [f32; 4],
        thumb_color: [f32; 4],
    },
    ProgressBar {
        value: f32,
        max: f32,
        fill_color: [f32; 4],
    },
    HealthBar {
        current: f32,
        max: f32,
        low_threshold: f32,
        fill_color: [f32; 4],
        low_color: [f32; 4],
    },
    Checkbox {
        checked: bool,
        label: String,
        check_color: [f32; 4],
        box_color: [f32; 4],
    },
    Toggle {
        on: bool,
        label: String,
        on_color: [f32; 4],
        off_color: [f32; 4],
        knob_color: [f32; 4],
    },
    Dropdown {
        selected_text: String,
        open: bool,
    },
    TextInput {
        text: String,
        placeholder: String,
    },
    TabBar {
        tabs: Vec<String>,
        active: usize,
        tab_color: [f32; 4],
        active_color: [f32; 4],
    },
    Spinner {
        color: [f32; 4],
    },
    RadioButton {
        selected: bool,
        label: String,
        active_color: [f32; 4],
    },
    Modal {
        title: String,
    },
    DraggableWindow {
        title: String,
        title_bar_color: [f32; 4],
    },
    // ── HUD ──
    Crosshair {
        style: String,  // "Cross", "Dot", "CircleDot", "CrossDot"
        color: [f32; 4],
        size: f32,
        thickness: f32,
    },
    AmmoCounter {
        current: u32,
        max: u32,
        color: [f32; 4],
        low_color: [f32; 4],
        low_threshold: u32,
    },
    Compass {
        heading: f32,
        color: [f32; 4],
    },
    StatusEffectBar {
        effect_count: usize,
        color: [f32; 4],
    },
    NotificationFeed {
        count: usize,
        color: [f32; 4],
    },
    RadialMenu {
        item_count: usize,
        color: [f32; 4],
    },
    Minimap {
        shape: String,  // "Circle" or "Square"
        bg_color: [f32; 4],
        border_color: [f32; 4],
    },
    // ── Shapes ──
    ShapeCircle {
        color: [f32; 4],
        stroke_color: [f32; 4],
        stroke_width: f32,
    },
    ShapeArc {
        color: [f32; 4],
        start_angle: f32,
        end_angle: f32,
    },
    ShapeTriangle {
        color: [f32; 4],
        stroke_color: [f32; 4],
    },
    ShapeLine {
        color: [f32; 4],
        thickness: f32,
    },
    ShapePolygon {
        color: [f32; 4],
        stroke_color: [f32; 4],
        sides: u32,
    },
    ShapeWedge {
        color: [f32; 4],
        start_angle: f32,
        end_angle: f32,
    },
    ShapeRadialProgress {
        color: [f32; 4],
        track_color: [f32; 4],
        value: f32,
    },
    // ── Menu ──
    InventoryGrid {
        columns: u32,
        rows: u32,
        slot_size: f32,
        slot_bg_color: [f32; 4],
        slot_border_color: [f32; 4],
    },
    DialogBox {
        speaker: String,
        text: String,
        bg_color: [f32; 4],
        speaker_color: [f32; 4],
    },
    ObjectiveTracker {
        title: String,
        objective_count: usize,
        title_color: [f32; 4],
    },
    LoadingScreen {
        progress: f32,
        message: String,
        bar_color: [f32; 4],
        bg_color: [f32; 4],
    },
    KeybindRow {
        action: String,
        binding: String,
        key_bg_color: [f32; 4],
    },
    SettingsRow {
        label: String,
        value: String,
    },
    // ── Extra ──
    Separator {
        horizontal: bool,
        color: [f32; 4],
        thickness: f32,
    },
    NumberInput {
        value: f64,
        precision: u32,
        bg_color: [f32; 4],
        button_color: [f32; 4],
    },
    VerticalSlider {
        value: f32,
        min: f32,
        max: f32,
        track_color: [f32; 4],
        fill_color: [f32; 4],
        thumb_color: [f32; 4],
    },
    Scrollbar {
        vertical: bool,
        viewport_fraction: f32,
        position: f32,
        track_color: [f32; 4],
        thumb_color: [f32; 4],
    },
    ListWidget {
        item_count: usize,
        bg_color: [f32; 4],
        selected_bg_color: [f32; 4],
        item_height: f32,
    },
}

/// Canvas editor state.
struct CanvasState {
    /// Pan offset in canvas pixels.
    pan: Vec2,
    /// Zoom level (1.0 = 100%).
    zoom: f32,
    /// Snapped widget data from the ECS.
    widgets: Vec<WidgetSnapshot>,
    /// Canvas entities.
    canvases: Vec<(Entity, String)>,
    /// Active canvas entity being edited.
    active_canvas: Option<Entity>,
    /// Drag state for moving widgets.
    dragging: Option<DragState>,
    /// Drag state for resizing.
    resizing: Option<ResizeState>,
    /// Box-select state.
    box_select: Option<BoxSelectState>,
    /// Canvas background resolution (logical game window size).
    canvas_width: f32,
    canvas_height: f32,
    /// Multi-select: entities currently selected (in addition to EditorSelection).
    multi_selection: Vec<Entity>,
    /// Snap-to-grid enabled.
    snap_enabled: bool,
    /// Grid spacing in logical UI pixels.
    grid_size: f32,
    /// Show grid lines on canvas.
    show_grid: bool,
    /// Show the game camera render behind the canvas.
    show_preview: bool,
    /// Clipboard for copy/paste (widget type + offset from first widget).
    clipboard: Vec<ClipboardEntry>,
}

#[derive(Clone)]
struct ClipboardEntry {
    widget_type: UiWidgetType,
    name: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    has_bg: bool,
    bg_color: [f32; 4],
    has_border: bool,
    border_color: [f32; 4],
}

#[derive(Clone)]
struct DragState {
    entities: Vec<Entity>,
    start_pos: Pos2,
    /// Original positions for each entity (same order as entities).
    originals: Vec<(f32, f32)>,
}

#[derive(Clone)]
struct ResizeState {
    entity: Entity,
    start_pos: Pos2,
    original_w: f32,
    original_h: f32,
    original_x: f32,
    original_y: f32,
    handle: ResizeHandle,
}

#[derive(Clone)]
struct BoxSelectState {
    start: Pos2,
    current: Pos2,
}

#[derive(Clone, Copy, PartialEq)]
enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl CanvasState {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            canvas_width: 1280.0,
            canvas_height: 720.0,
            pan: Vec2::ZERO,
            widgets: Vec::new(),
            canvases: Vec::new(),
            active_canvas: None,
            dragging: None,
            resizing: None,
            box_select: None,
            multi_selection: Vec::new(),
            snap_enabled: true,
            grid_size: 10.0,
            show_grid: true,
            show_preview: false,
            clipboard: Vec::new(),
        }
    }

    /// Returns all selected entities (primary + multi).
    fn all_selected(&self, primary: Option<Entity>) -> Vec<Entity> {
        let mut all = self.multi_selection.clone();
        if let Some(e) = primary {
            if !all.contains(&e) {
                all.push(e);
            }
        }
        all
    }

    fn toggle_multi(&mut self, entity: Entity) {
        if let Some(pos) = self.multi_selection.iter().position(|e| *e == entity) {
            self.multi_selection.remove(pos);
        } else {
            self.multi_selection.push(entity);
        }
    }
}

/// Snap a value to the nearest grid line.
fn snap(v: f32, grid: f32) -> f32 {
    (v / grid).round() * grid
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct UiCanvasPanel {
    state: RwLock<CanvasState>,
}

impl Default for UiCanvasPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(CanvasState::new()),
        }
    }
}

/// Convert a Val to design-space pixels given a reference dimension.
///
/// Handles `Val::Percent` (converting back using reference) and `Val::Px`
/// (for backwards compatibility with older scenes).
fn val_to_design_px(v: bevy::ui::Val, reference: f32) -> f32 {
    match v {
        bevy::ui::Val::Percent(p) => p * reference / 100.0,
        bevy::ui::Val::Px(px) => px,
        _ => 0.0,
    }
}

/// Convert a widget snapshot position to screen rect given canvas_rect and zoom.
fn ws_screen_rect(ws: &WidgetSnapshot, canvas_rect: Rect, z: f32) -> Rect {
    let x = canvas_rect.min.x + ws.x * z;
    let y = canvas_rect.min.y + ws.y * z;
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(ws.width * z, ws.height * z))
}

impl EditorPanel for UiCanvasPanel {
    fn id(&self) -> &str {
        "ui_canvas"
    }

    fn title(&self) -> &str {
        "UI Canvas"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FRAME_CORNERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let commands = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let selection = match world.get_resource::<EditorSelection>() {
            Some(s) => s,
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // ── Snapshot canvases ────────────────────────────────────────────
        state.canvases.clear();
        for archetype in world.archetypes().iter() {
            for arch_entity in archetype.entities() {
                let entity = arch_entity.id();
                if world.get::<UiCanvas>(entity).is_some() {
                    let name = world
                        .get::<Name>(entity)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_else(|| "Unnamed Canvas".to_string());
                    state.canvases.push((entity, name));
                }
            }
        }

        // Auto-select first canvas if none active
        if state.active_canvas.is_none()
            || !state
                .canvases
                .iter()
                .any(|(e, _)| Some(*e) == state.active_canvas)
        {
            state.active_canvas = state.canvases.first().map(|(e, _)| *e);
        }

        // Sync canvas size from active canvas component's reference resolution
        if let Some(active) = state.active_canvas {
            if let Some(canvas) = world.get::<UiCanvas>(active) {
                state.canvas_width = canvas.reference_width;
                state.canvas_height = canvas.reference_height;
            }
        }

        // Reference resolution for px↔percent conversion in closures
        let ref_w = state.canvas_width;
        let ref_h = state.canvas_height;

        // ── Toolbar ─────────────────────────────────────────────────────
        let text_muted = theme.text.muted.to_color32();
        let accent = theme.semantic.accent.to_color32();

        let selected_entity = selection.get();
        let all_sel = state.all_selected(selected_entity);

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Alignment buttons (dim when nothing selected)
            let has_sel = !all_sel.is_empty();
            let btn_color = if has_sel { text_muted } else { Color32::from_white_alpha(30) };

            let align_buttons: &[(&str, &str, AlignAction)] = &[
                (regular::ALIGN_LEFT, "Align left", AlignAction::Left),
                (regular::ALIGN_CENTER_HORIZONTAL, "Align center H", AlignAction::CenterH),
                (regular::ALIGN_RIGHT, "Align right", AlignAction::Right),
                (regular::ALIGN_TOP, "Align top", AlignAction::Top),
                (regular::ALIGN_CENTER_VERTICAL, "Align center V", AlignAction::CenterV),
                (regular::ALIGN_BOTTOM, "Align bottom", AlignAction::Bottom),
            ];
            for (icon, tooltip, action) in align_buttons {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(*icon).size(13.0).color(btn_color),
                        )
                        .fill(Color32::TRANSPARENT),
                    )
                    .on_hover_text(*tooltip)
                    .clicked()
                    && has_sel
                {
                    let snapshots: Vec<_> = state
                        .widgets
                        .iter()
                        .filter(|w| all_sel.contains(&w.entity))
                        .cloned()
                        .collect();
                    let moves = compute_align(&snapshots, *action);
                    for (entity, new_x, new_y) in moves {
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut node) = em.get_mut::<Node>() {
                                    node.left = bevy::ui::Val::Percent(new_x / ref_w * 100.0);
                                    node.top = bevy::ui::Val::Percent(new_y / ref_h * 100.0);
                                    node.position_type = bevy::ui::PositionType::Absolute;
                                }
                            }
                        });
                    }
                }
            }

            ui.separator();

            // Distribute (dim when < 3 selected)
            let dist_color = if all_sel.len() >= 3 { text_muted } else { Color32::from_white_alpha(30) };

            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(regular::ARROWS_OUT_LINE_HORIZONTAL)
                            .size(13.0)
                            .color(dist_color),
                    )
                    .fill(Color32::TRANSPARENT),
                )
                .on_hover_text("Distribute horizontally")
                .clicked()
                && all_sel.len() >= 3
            {
                let snapshots: Vec<_> = state
                    .widgets
                    .iter()
                    .filter(|w| all_sel.contains(&w.entity))
                    .cloned()
                    .collect();
                let moves = compute_distribute_h(&snapshots);
                for (entity, new_x) in moves {
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut node) = em.get_mut::<Node>() {
                                node.left = bevy::ui::Val::Percent(new_x / ref_w * 100.0);
                                node.position_type = bevy::ui::PositionType::Absolute;
                            }
                        }
                    });
                }
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(regular::ARROWS_OUT_LINE_VERTICAL)
                            .size(13.0)
                            .color(dist_color),
                    )
                    .fill(Color32::TRANSPARENT),
                )
                .on_hover_text("Distribute vertically")
                .clicked()
                && all_sel.len() >= 3
            {
                let snapshots: Vec<_> = state
                    .widgets
                    .iter()
                    .filter(|w| all_sel.contains(&w.entity))
                    .cloned()
                    .collect();
                let moves = compute_distribute_v(&snapshots);
                for (entity, new_y) in moves {
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut node) = em.get_mut::<Node>() {
                                node.top = bevy::ui::Val::Percent(new_y / ref_h * 100.0);
                                node.position_type = bevy::ui::PositionType::Absolute;
                            }
                        }
                    });
                }
            }

            // Right side: selection info, grid, snap, zoom
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                // Zoom label
                ui.label(
                    egui::RichText::new(format!("{:.0}%", state.zoom * 100.0))
                        .size(10.0)
                        .color(text_muted),
                );

                ui.separator();

                // Grid size
                let mut gs = state.grid_size;
                ui.add(egui::DragValue::new(&mut gs).range(2.0..=100.0).speed(1.0).prefix("grid: "));
                state.grid_size = gs;

                // Snap toggle
                let snap_color = if state.snap_enabled { accent } else { text_muted };
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::DOTS_NINE).size(14.0).color(snap_color),
                    ).fill(Color32::TRANSPARENT))
                    .on_hover_text("Toggle snap to grid")
                    .clicked()
                {
                    state.snap_enabled = !state.snap_enabled;
                }

                // Grid toggle
                let grid_color = if state.show_grid { accent } else { text_muted };
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::GRID_FOUR).size(14.0).color(grid_color),
                    ).fill(Color32::TRANSPARENT))
                    .on_hover_text("Toggle grid")
                    .clicked()
                {
                    state.show_grid = !state.show_grid;
                }

                // Preview toggle (show viewport render behind canvas)
                let preview_color = if state.show_preview { accent } else { text_muted };
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(regular::MONITOR).size(14.0).color(preview_color),
                    ).fill(Color32::TRANSPARENT))
                    .on_hover_text("Toggle game viewport preview")
                    .clicked()
                {
                    state.show_preview = !state.show_preview;
                }

                // Selection count
                if !all_sel.is_empty() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!("{} selected", all_sel.len()))
                            .size(10.0)
                            .color(text_muted),
                    );
                }
            });
        });

        ui.separator();

        // ── Snapshot widgets for active canvas ───────────────────────────
        let user_textures = world.get_resource::<EguiUserTextures>();
        state.widgets.clear();
        if let Some(active_canvas) = state.active_canvas {
            for archetype in world.archetypes().iter() {
                for arch_entity in archetype.entities() {
                    let entity = arch_entity.id();
                    let Some(widget) = world.get::<UiWidget>(entity) else {
                        continue;
                    };
                    if !is_descendant_of(world, entity, active_canvas) {
                        continue;
                    }
                    let Some(node) = world.get::<Node>(entity) else {
                        continue;
                    };

                    let name = world
                        .get::<Name>(entity)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_default();
                    let bg = world.get::<BackgroundColor>(entity);
                    let border = world.get::<BorderColor>(entity);
                    let parent = world.get::<ChildOf>(entity).map(|c| c.parent());

                    // Look up egui texture for Image widgets
                    let image_texture_id = world
                        .get::<ImageNode>(entity)
                        .and_then(|img| {
                            user_textures.and_then(|ut| ut.image_id(img.image.id()))
                        });

                    // Read individual style components
                    let border_radius_comp = world.get::<UiBorderRadius>(entity);
                    let stroke_comp = world.get::<UiStroke>(entity);
                    let opacity_comp = world.get::<UiOpacity>(entity);
                    let shadow_comp = world.get::<UiBoxShadow>(entity);
                    let text_style_comp = world.get::<UiTextStyle>(entity);

                    let border_radius = border_radius_comp
                        .map(|s| [s.top_left, s.top_right, s.bottom_right, s.bottom_left])
                        .unwrap_or([0.0; 4]);
                    let stroke_width = stroke_comp.map(|s| s.width).unwrap_or(0.0);
                    let opacity = opacity_comp.map(|s| s.0).unwrap_or(1.0);
                    let shadow = shadow_comp.map(|sh| {
                        let c = sh.color.to_srgba().to_f32_array();
                        [c[0], c[1], c[2], c[3], sh.blur, sh.spread]
                    });

                    // Read text content
                    let text_content = world
                        .get::<bevy::ui::widget::Text>(entity)
                        .map(|t| t.0.clone());
                    let text_font = world.get::<TextFont>(entity);
                    let text_color_comp = world.get::<TextColor>(entity);
                    let text_size = text_style_comp.map(|s| s.size)
                        .or_else(|| text_font.map(|f| f.font_size))
                        .unwrap_or(14.0);
                    let text_color = text_style_comp
                        .map(|s| s.color.to_srgba().to_f32_array())
                        .or_else(|| text_color_comp.map(|c| c.0.to_srgba().to_f32_array()))
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                    let text_bold = text_style_comp.map(|s| s.bold).unwrap_or(false);

                    // Read per-widget-type data
                    let widget_data = snapshot_widget_data(world, entity, &widget.widget_type);

                    state.widgets.push(WidgetSnapshot {
                        entity,
                        name,
                        widget_type: widget.widget_type.clone(),
                        locked: widget.locked,
                        x: val_to_design_px(node.left, ref_w),
                        y: val_to_design_px(node.top, ref_h),
                        width: val_to_design_px(node.width, ref_w),
                        height: val_to_design_px(node.height, ref_h),
                        parent,
                        has_bg: bg.is_some(),
                        bg_color: bg
                            .map(|b| b.0.to_srgba().to_f32_array())
                            .unwrap_or([0.0; 4]),
                        has_border: border.is_some(),
                        border_color: border
                            .map(|b| b.top.to_srgba().to_f32_array())
                            .unwrap_or([0.0; 4]),
                        image_texture_id,
                        border_radius,
                        stroke_width,
                        opacity,
                        shadow,
                        text_content,
                        text_size,
                        text_color,
                        text_bold,
                        widget_data,
                    });
                }
            }
        }

        // Sort widgets by sibling order (reversed): last child in hierarchy
        // draws first so that the top item in the hierarchy renders on top.
        if let Some(active_canvas) = state.active_canvas {
            if let Some(children) = world.get::<Children>(active_canvas) {
                let order_map: std::collections::HashMap<Entity, usize> = children
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (e, i))
                    .collect();
                // Reverse sort: higher sibling index first → drawn first → behind.
                // Lower sibling index (top of hierarchy) drawn last → on top.
                state.widgets.sort_by(|a, b| {
                    let oa = order_map.get(&a.entity).copied().unwrap_or(usize::MAX);
                    let ob = order_map.get(&b.entity).copied().unwrap_or(usize::MAX);
                    ob.cmp(&oa)
                });
            }
        }

        // ── Vertical toolbar + Canvas area ──────────────────────────────
        let full_available = ui.available_rect_before_wrap();
        let toolbar_width = 40.0;
        let surface = theme.surfaces.panel.to_color32();
        let text_primary = theme.text.primary.to_color32();

        // Toolbar strip on the left
        let toolbar_rect = Rect::from_min_size(
            full_available.min,
            Vec2::new(toolbar_width, full_available.height()),
        );
        let tb_response = ui.allocate_rect(toolbar_rect, egui::Sense::hover());
        let tb_painter = ui.painter_at(toolbar_rect);

        // Toolbar background
        tb_painter.rect_filled(toolbar_rect, 0.0, Color32::from_rgb(35, 35, 40));
        // Right edge separator
        tb_painter.line_segment(
            [
                Pos2::new(toolbar_rect.max.x, toolbar_rect.min.y),
                Pos2::new(toolbar_rect.max.x, toolbar_rect.max.y),
            ],
            Stroke::new(1.0, Color32::from_rgb(50, 50, 55)),
        );

        // Toolbar widget buttons (categorized)
        let active_canvas = state.active_canvas;
        {
            const ICON_SIZE: f32 = 18.0;
            const BTN_SIZE: f32 = 32.0;
            const BTN_PAD: f32 = 4.0;

            // Widget types grouped by category with separators
            let tool_groups: &[&[UiWidgetType]] = &[
                // Layout
                &[UiWidgetType::Container, UiWidgetType::Panel, UiWidgetType::ScrollView],
                // Basic
                &[UiWidgetType::Text, UiWidgetType::Image, UiWidgetType::Button],
                // Input
                &[UiWidgetType::Slider, UiWidgetType::Checkbox, UiWidgetType::Toggle, UiWidgetType::Dropdown, UiWidgetType::TextInput],
                // Display
                &[UiWidgetType::ProgressBar, UiWidgetType::HealthBar],
                // Overlay
                &[UiWidgetType::Modal, UiWidgetType::DraggableWindow],
            ];

            let mut y_offset = toolbar_rect.min.y + BTN_PAD;
            let btn_x = toolbar_rect.min.x + (toolbar_width - BTN_SIZE) / 2.0;

            for (gi, group) in tool_groups.iter().enumerate() {
                for wtype in *group {
                    let btn_rect = Rect::from_min_size(
                        Pos2::new(btn_x, y_offset),
                        Vec2::new(BTN_SIZE, BTN_SIZE),
                    );

                    // Hover detection
                    let hovered = tb_response.hovered()
                        && ui.ctx().pointer_latest_pos().map_or(false, |p| btn_rect.contains(p));
                    let bg = if hovered { surface } else { Color32::TRANSPARENT };
                    tb_painter.rect_filled(btn_rect, 3.0, bg);

                    // Icon
                    let icon_color = if hovered { text_primary } else { text_muted };
                    tb_painter.text(
                        btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        wtype.icon(),
                        egui::FontId::proportional(ICON_SIZE),
                        icon_color,
                    );

                    // Tooltip (painted to the right of the button)
                    if hovered {
                        let tip_pos = Pos2::new(toolbar_rect.max.x + 6.0, btn_rect.center().y);
                        let tip_text = wtype.label();
                        let tip_galley = tb_painter.layout_no_wrap(
                            tip_text.to_string(),
                            egui::FontId::proportional(11.0),
                            text_primary,
                        );
                        let tip_rect = Rect::from_min_size(
                            Pos2::new(tip_pos.x - 4.0, tip_pos.y - tip_galley.size().y / 2.0 - 3.0),
                            Vec2::new(tip_galley.size().x + 8.0, tip_galley.size().y + 6.0),
                        );
                        // Use a foreground painter so it draws on top of everything
                        let fg = ui.ctx().layer_painter(egui::LayerId::new(
                            egui::Order::Tooltip,
                            ui.id().with("vtoolbar_tip"),
                        ));
                        fg.rect_filled(tip_rect, 4.0, Color32::from_rgb(50, 50, 55));
                        fg.text(
                            Pos2::new(tip_pos.x, tip_pos.y),
                            egui::Align2::LEFT_CENTER,
                            tip_text,
                            egui::FontId::proportional(11.0),
                            text_primary,
                        );
                    }

                    // Click to add widget
                    if hovered && ui.ctx().input(|i| i.pointer.any_click()) {
                        let wt = wtype.clone();
                        commands.push(move |world: &mut World| {
                            crate::spawn::spawn_widget(world, &wt, active_canvas);
                        });
                    }

                    y_offset += BTN_SIZE + 2.0;
                }

                // Separator between groups (except after the last)
                if gi < tool_groups.len() - 1 {
                    y_offset += 2.0;
                    let sep_y = y_offset;
                    tb_painter.line_segment(
                        [
                            Pos2::new(toolbar_rect.min.x + 6.0, sep_y),
                            Pos2::new(toolbar_rect.max.x - 6.0, sep_y),
                        ],
                        Stroke::new(1.0, Color32::from_rgb(55, 55, 60)),
                    );
                    y_offset += 6.0;
                }
            }
        }

        // Canvas area (right of the toolbar)
        let available = Rect::from_min_max(
            Pos2::new(full_available.min.x + toolbar_width, full_available.min.y),
            full_available.max,
        );
        let response = ui.allocate_rect(available, egui::Sense::click_and_drag());
        let painter = ui.painter_at(available);

        // Background
        painter.rect_filled(available, 0.0, Color32::from_rgb(30, 30, 35));

        // Handle pan (middle mouse / right drag)
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            state.pan += response.drag_delta();
        }

        // Handle zoom (scroll)
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 && response.hovered() {
            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
            state.zoom = (state.zoom * factor).clamp(0.1, 5.0);
        }

        // Canvas origin: center of the panel, offset by pan
        let origin = Pos2::new(
            available.center().x + state.pan.x,
            available.center().y + state.pan.y,
        );
        let z = state.zoom;

        // Draw canvas background (game window area)
        let cw = state.canvas_width * z;
        let ch = state.canvas_height * z;
        let canvas_rect = Rect::from_min_size(
            Pos2::new(origin.x - cw / 2.0, origin.y - ch / 2.0),
            Vec2::new(cw, ch),
        );
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(20, 20, 24));

        // ── Camera preview (game render behind the canvas) ─────────────
        if state.show_preview {
            // Activate the preview camera
            if let Some(preview) = world.get_resource::<UiCanvasPreview>() {
                if preview.previewing.is_some() {
                    // Use the preview texture
                    let tex_id = preview.texture_id;
                    if let Some(tex_id) = tex_id {
                        let uv = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                        painter.image(tex_id, canvas_rect, uv, Color32::WHITE);
                    }
                    // Ensure camera is active
                    if let Some(cam_entity) = preview.camera_entity {
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(cam_entity) {
                                if let Some(mut cam) = em.get_mut::<Camera>() {
                                    if !cam.is_active {
                                        cam.is_active = true;
                                    }
                                }
                            }
                        });
                    }
                }
            }
        } else {
            // Deactivate the preview camera when not showing
            if let Some(preview) = world.get_resource::<UiCanvasPreview>() {
                if let Some(cam_entity) = preview.camera_entity {
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(cam_entity) {
                            if let Some(mut cam) = em.get_mut::<Camera>() {
                                if cam.is_active {
                                    cam.is_active = false;
                                }
                            }
                        }
                    });
                }
            }
        }

        painter.rect_stroke(
            canvas_rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
            egui::StrokeKind::Outside,
        );

        // ── Grid lines ───────────────────────────────────────────────────
        if state.show_grid {
            let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
            let gs = state.grid_size * z;
            if gs > 3.0 {
                // Vertical lines
                let mut x = canvas_rect.min.x;
                while x <= canvas_rect.max.x {
                    painter.line_segment(
                        [Pos2::new(x, canvas_rect.min.y), Pos2::new(x, canvas_rect.max.y)],
                        Stroke::new(0.5, grid_color),
                    );
                    x += gs;
                }
                // Horizontal lines
                let mut y = canvas_rect.min.y;
                while y <= canvas_rect.max.y {
                    painter.line_segment(
                        [Pos2::new(canvas_rect.min.x, y), Pos2::new(canvas_rect.max.x, y)],
                        Stroke::new(0.5, grid_color),
                    );
                    y += gs;
                }
            }
        }

        // Size label
        painter.text(
            Pos2::new(canvas_rect.center().x, canvas_rect.max.y + 14.0),
            egui::Align2::CENTER_CENTER,
            format!(
                "{}x{}",
                state.canvas_width as u32, state.canvas_height as u32
            ),
            egui::FontId::proportional(10.0),
            text_muted,
        );

        // Recalculate all_sel after widgets are snapshot (for drawing)
        let all_sel = state.all_selected(selected_entity);

        // ── Draw widgets ─────────────────────────────────────────────────
        let widget_snapshots = state.widgets.clone();
        for ws in &widget_snapshots {
            let rect = ws_screen_rect(ws, canvas_rect, z);
            let is_sel = all_sel.contains(&ws.entity);

            paint_widget_preview(&painter, &ws, rect, z, is_sel, accent, text_muted);

            // Resize handles for selected widget (single selection only)
            if is_sel && all_sel.len() == 1 && !ws.locked {
                let hs = 6.0;
                for (hx, hy) in [
                    (rect.min.x, rect.min.y),
                    (rect.max.x, rect.min.y),
                    (rect.min.x, rect.max.y),
                    (rect.max.x, rect.max.y),
                ] {
                    painter.rect_filled(
                        Rect::from_center_size(Pos2::new(hx, hy), Vec2::splat(hs)),
                        1.0,
                        accent,
                    );
                }
            }
        }

        // ── Box-select rendering ─────────────────────────────────────────
        if let Some(bs) = &state.box_select {
            let sel_rect = Rect::from_two_pos(bs.start, bs.current);
            painter.rect_filled(
                sel_rect,
                0.0,
                Color32::from_rgba_unmultiplied(100, 150, 255, 25),
            );
            painter.rect_stroke(
                sel_rect,
                0.0,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 150, 255, 120)),
                egui::StrokeKind::Outside,
            );
        }

        // ── Asset drag-and-drop (images from asset browser) ─────────────
        if let Some(payload) = world.get_resource::<AssetDragPayload>() {
            if payload.is_detached && payload.matches_extensions(IMAGE_EXTENSIONS) {
                let pointer_pos = ui.ctx().pointer_hover_pos();
                let pointer_in_canvas = pointer_pos.map_or(false, |p| canvas_rect.contains(p));

                if pointer_in_canvas {
                    // Draw drop-zone highlight on the canvas
                    painter.rect_filled(
                        canvas_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(100, 200, 100, 15),
                    );
                    painter.rect_stroke(
                        canvas_rect,
                        0.0,
                        Stroke::new(2.0, Color32::from_rgba_unmultiplied(100, 200, 100, 180)),
                        egui::StrokeKind::Inside,
                    );

                    // Show "Drop to add Image" text at pointer
                    if let Some(pos) = pointer_pos {
                        painter.text(
                            Pos2::new(pos.x, pos.y - 16.0),
                            egui::Align2::CENTER_BOTTOM,
                            format!("{} Drop to add image", regular::IMAGE),
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(100, 200, 100),
                        );
                    }

                    // Detect drop (pointer released)
                    if !ui.ctx().input(|i| i.pointer.any_down()) {
                        if let Some(pos) = pointer_pos {
                            // Convert screen position to canvas logical coordinates
                            let lx = (pos.x - canvas_rect.min.x) / z;
                            let ly = (pos.y - canvas_rect.min.y) / z;

                            let asset_path = payload.path.clone();
                            let active_canvas = state.active_canvas;
                            let snap_on = state.snap_enabled;
                            let grid = state.grid_size;

                            commands.push(move |world: &mut World| {
                                crate::spawn::spawn_image_at(
                                    world,
                                    &asset_path,
                                    lx, ly,
                                    snap_on, grid,
                                    active_canvas,
                                );
                            });
                        }
                    }
                }
            }
        }

        // ── Keyboard shortcuts ───────────────────────────────────────────
        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
        let shift = ui.input(|i| i.modifiers.shift);

        // Delete selected widgets
        if response.has_focus() || response.hovered() {
            if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                let to_delete = all_sel.clone();
                if !to_delete.is_empty() {
                    state.multi_selection.clear();
                    commands.push(move |world: &mut World| {
                        for entity in &to_delete {
                            if world.get_entity(*entity).is_ok() {
                                world.despawn(*entity);
                            }
                        }
                        if let Some(sel) = world.get_resource::<EditorSelection>() {
                            sel.set(None);
                        }
                    });
                }
            }

            // Arrow key nudge
            let nudge = if shift { 10.0 } else { 1.0 };
            let nudge = if state.snap_enabled && !shift {
                state.grid_size
            } else {
                nudge
            };

            let mut dx = 0.0_f32;
            let mut dy = 0.0_f32;
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                dx = -nudge;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                dx = nudge;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                dy = -nudge;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                dy = nudge;
            }

            if dx != 0.0 || dy != 0.0 {
                let entities_to_nudge = all_sel.clone();
                let snap_on = state.snap_enabled && !shift;
                let grid = state.grid_size;
                // Read current positions from snapshots
                let positions: Vec<_> = entities_to_nudge
                    .iter()
                    .filter_map(|e| {
                        widget_snapshots.iter().find(|w| w.entity == *e).map(|w| (*e, w.x, w.y))
                    })
                    .collect();
                for (entity, wx, wy) in positions {
                    let mut nx = wx + dx;
                    let mut ny = wy + dy;
                    if snap_on {
                        nx = snap(nx, grid);
                        ny = snap(ny, grid);
                    }
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut node) = em.get_mut::<Node>() {
                                node.left = bevy::ui::Val::Percent(nx / ref_w * 100.0);
                                node.top = bevy::ui::Val::Percent(ny / ref_h * 100.0);
                                node.position_type = bevy::ui::PositionType::Absolute;
                            }
                        }
                    });
                }
            }

            // Copy (Ctrl+C)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::C)) {
                state.clipboard.clear();
                let sel_widgets: Vec<_> = widget_snapshots
                    .iter()
                    .filter(|w| all_sel.contains(&w.entity))
                    .collect();
                if !sel_widgets.is_empty() {
                    let base_x = sel_widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
                    let base_y = sel_widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
                    for w in &sel_widgets {
                        state.clipboard.push(ClipboardEntry {
                            widget_type: w.widget_type.clone(),
                            name: w.name.clone(),
                            x: w.x - base_x,
                            y: w.y - base_y,
                            width: w.width,
                            height: w.height,
                            has_bg: w.has_bg,
                            bg_color: w.bg_color,
                            has_border: w.has_border,
                            border_color: w.border_color,
                        });
                    }
                }
            }

            // Paste (Ctrl+V)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::V)) {
                let entries = state.clipboard.clone();
                let active_canvas = state.active_canvas;
                if !entries.is_empty() {
                    commands.push(move |world: &mut World| {
                        for entry in &entries {
                            let offset_x = entry.x + 20.0; // Paste offset
                            let offset_y = entry.y + 20.0;
                            let wt = entry.widget_type.clone();
                            crate::spawn::spawn_widget(world, &wt, active_canvas);
                            // After spawn, update position/size on the last spawned entity
                            // spawn_widget selects the entity, so we can find it via selection
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                if let Some(entity) = sel.get() {
                                    if let Ok(mut em) = world.get_entity_mut(entity) {
                                        if let Some(mut name) = em.get_mut::<Name>() {
                                            name.set(format!("{} (copy)", entry.name));
                                        }
                                        if let Some(mut node) = em.get_mut::<Node>() {
                                            node.left = bevy::ui::Val::Percent(offset_x / ref_w * 100.0);
                                            node.top = bevy::ui::Val::Percent(offset_y / ref_h * 100.0);
                                            node.width = bevy::ui::Val::Percent(entry.width / ref_w * 100.0);
                                            node.height = bevy::ui::Val::Percent(entry.height / ref_h * 100.0);
                                            node.position_type = bevy::ui::PositionType::Absolute;
                                        }
                                        if entry.has_bg {
                                            let c = entry.bg_color;
                                            em.insert(BackgroundColor(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                        if entry.has_border {
                                            let c = entry.border_color;
                                            em.insert(BorderColor::all(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }

            // Duplicate (Ctrl+D)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::D)) {
                let sel_widgets: Vec<ClipboardEntry> = widget_snapshots
                    .iter()
                    .filter(|w| all_sel.contains(&w.entity))
                    .map(|w| ClipboardEntry {
                        widget_type: w.widget_type.clone(),
                        name: w.name.clone(),
                        x: w.x + 20.0,
                        y: w.y + 20.0,
                        width: w.width,
                        height: w.height,
                        has_bg: w.has_bg,
                        bg_color: w.bg_color,
                        has_border: w.has_border,
                        border_color: w.border_color,
                    })
                    .collect();
                let active_canvas = state.active_canvas;
                if !sel_widgets.is_empty() {
                    commands.push(move |world: &mut World| {
                        for entry in &sel_widgets {
                            let wt = entry.widget_type.clone();
                            crate::spawn::spawn_widget(world, &wt, active_canvas);
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                if let Some(entity) = sel.get() {
                                    if let Ok(mut em) = world.get_entity_mut(entity) {
                                        if let Some(mut name) = em.get_mut::<Name>() {
                                            name.set(format!("{} (copy)", entry.name));
                                        }
                                        if let Some(mut node) = em.get_mut::<Node>() {
                                            node.left = bevy::ui::Val::Percent(entry.x / ref_w * 100.0);
                                            node.top = bevy::ui::Val::Percent(entry.y / ref_h * 100.0);
                                            node.width = bevy::ui::Val::Percent(entry.width / ref_w * 100.0);
                                            node.height = bevy::ui::Val::Percent(entry.height / ref_h * 100.0);
                                            node.position_type = bevy::ui::PositionType::Absolute;
                                        }
                                        if entry.has_bg {
                                            let c = entry.bg_color;
                                            em.insert(BackgroundColor(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                        if entry.has_border {
                                            let c = entry.border_color;
                                            em.insert(BorderColor::all(Color::srgba(
                                                c[0], c[1], c[2], c[3],
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }

            // Select all (Ctrl+A)
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::A)) {
                state.multi_selection = widget_snapshots.iter().map(|w| w.entity).collect();
            }
        }

        // ── Interaction: click, shift-click, box-select, drag ────────────

        // Click: select or shift-toggle
        if response.clicked() {
            let pointer = response.interact_pointer_pos().unwrap_or_default();
            let mut clicked_entity = None;

            for ws in widget_snapshots.iter().rev() {
                let rect = ws_screen_rect(ws, canvas_rect, z);
                if rect.contains(pointer) {
                    clicked_entity = Some(ws.entity);
                    break;
                }
            }

            if shift {
                // Shift+click: toggle in multi-selection
                if let Some(e) = clicked_entity {
                    state.toggle_multi(e);
                }
            } else {
                // Normal click: clear multi, set primary
                state.multi_selection.clear();
                let entity = clicked_entity;
                commands.push(move |world: &mut World| {
                    if let Some(sel) = world.get_resource::<EditorSelection>() {
                        sel.set(entity);
                    }
                });
            }
        }

        // Drag start
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                let all_sel = state.all_selected(selected_entity);

                // Check resize handles (single selection only)
                if all_sel.len() == 1 {
                    if let Some(ws) = widget_snapshots.iter().find(|w| w.entity == all_sel[0]) {
                        let rect = ws_screen_rect(ws, canvas_rect, z);
                        let handles = [
                            (Pos2::new(rect.min.x, rect.min.y), ResizeHandle::TopLeft),
                            (Pos2::new(rect.max.x, rect.min.y), ResizeHandle::TopRight),
                            (Pos2::new(rect.min.x, rect.max.y), ResizeHandle::BottomLeft),
                            (Pos2::new(rect.max.x, rect.max.y), ResizeHandle::BottomRight),
                        ];
                        let mut found_handle = false;
                        for (pos, handle) in &handles {
                            let hr = Rect::from_center_size(*pos, Vec2::splat(8.0));
                            if hr.contains(pointer) && !ws.locked {
                                state.resizing = Some(ResizeState {
                                    entity: ws.entity,
                                    start_pos: pointer,
                                    original_w: ws.width,
                                    original_h: ws.height,
                                    original_x: ws.x,
                                    original_y: ws.y,
                                    handle: *handle,
                                });
                                found_handle = true;
                                break;
                            }
                        }
                        if found_handle {
                            // handled below
                        } else if rect.contains(pointer) && !ws.locked {
                            // Single widget drag
                            state.dragging = Some(DragState {
                                entities: all_sel.clone(),
                                start_pos: pointer,
                                originals: all_sel
                                    .iter()
                                    .filter_map(|e| {
                                        widget_snapshots
                                            .iter()
                                            .find(|w| w.entity == *e)
                                            .map(|w| (w.x, w.y))
                                    })
                                    .collect(),
                            });
                        } else {
                            // Start box-select
                            state.box_select = Some(BoxSelectState {
                                start: pointer,
                                current: pointer,
                            });
                        }
                    } else {
                        state.box_select = Some(BoxSelectState {
                            start: pointer,
                            current: pointer,
                        });
                    }
                } else if !all_sel.is_empty() {
                    // Multi-drag: check if pointer is on any selected widget
                    let on_selected = widget_snapshots.iter().rev().any(|ws| {
                        all_sel.contains(&ws.entity)
                            && ws_screen_rect(ws, canvas_rect, z).contains(pointer)
                    });
                    if on_selected {
                        state.dragging = Some(DragState {
                            entities: all_sel.clone(),
                            start_pos: pointer,
                            originals: all_sel
                                .iter()
                                .filter_map(|e| {
                                    widget_snapshots
                                        .iter()
                                        .find(|w| w.entity == *e)
                                        .map(|w| (w.x, w.y))
                                })
                                .collect(),
                        });
                    } else {
                        state.box_select = Some(BoxSelectState {
                            start: pointer,
                            current: pointer,
                        });
                    }
                } else {
                    // Nothing selected, start box select
                    state.box_select = Some(BoxSelectState {
                        start: pointer,
                        current: pointer,
                    });
                }
            }
        }

        // Apply drag movement (multi-widget)
        if let Some(drag) = &state.dragging {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    let dx = (pointer.x - drag.start_pos.x) / z;
                    let dy = (pointer.y - drag.start_pos.y) / z;
                    let snap_on = state.snap_enabled;
                    let grid = state.grid_size;

                    for (i, entity) in drag.entities.iter().enumerate() {
                        if let Some(&(ox, oy)) = drag.originals.get(i) {
                            let mut nx = ox + dx;
                            let mut ny = oy + dy;
                            if snap_on {
                                nx = snap(nx, grid);
                                ny = snap(ny, grid);
                            }
                            let e = *entity;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(e) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.left = bevy::ui::Val::Percent(nx / ref_w * 100.0);
                                        node.top = bevy::ui::Val::Percent(ny / ref_h * 100.0);
                                        node.position_type = bevy::ui::PositionType::Absolute;
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }

        // Apply resize (with snap)
        if let Some(resize) = &state.resizing {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    let dx = (pointer.x - resize.start_pos.x) / z;
                    let dy = (pointer.y - resize.start_pos.y) / z;
                    let snap_on = state.snap_enabled;
                    let grid = state.grid_size;

                    let (mut nw, mut nh, mut nx, mut ny) = match resize.handle {
                        ResizeHandle::BottomRight => (
                            (resize.original_w + dx).max(10.0),
                            (resize.original_h + dy).max(10.0),
                            resize.original_x,
                            resize.original_y,
                        ),
                        ResizeHandle::TopRight => (
                            (resize.original_w + dx).max(10.0),
                            (resize.original_h - dy).max(10.0),
                            resize.original_x,
                            resize.original_y + dy,
                        ),
                        ResizeHandle::BottomLeft => (
                            (resize.original_w - dx).max(10.0),
                            (resize.original_h + dy).max(10.0),
                            resize.original_x + dx,
                            resize.original_y,
                        ),
                        ResizeHandle::TopLeft => (
                            (resize.original_w - dx).max(10.0),
                            (resize.original_h - dy).max(10.0),
                            resize.original_x + dx,
                            resize.original_y + dy,
                        ),
                    };

                    if snap_on {
                        nw = snap(nw, grid).max(grid);
                        nh = snap(nh, grid).max(grid);
                        nx = snap(nx, grid);
                        ny = snap(ny, grid);
                    }

                    let entity = resize.entity;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut node) = em.get_mut::<Node>() {
                                node.width = bevy::ui::Val::Percent(nw / ref_w * 100.0);
                                node.height = bevy::ui::Val::Percent(nh / ref_h * 100.0);
                                node.left = bevy::ui::Val::Percent(nx / ref_w * 100.0);
                                node.top = bevy::ui::Val::Percent(ny / ref_h * 100.0);
                                node.position_type = bevy::ui::PositionType::Absolute;
                            }
                        }
                    });
                }
            }
        }

        // Update box-select
        if let Some(bs) = &mut state.box_select {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    bs.current = pointer;
                }
            }
        }

        // End drag/resize/box-select on release
        if !ui.ctx().input(|i| i.pointer.any_down()) {
            state.dragging = None;
            state.resizing = None;

            // Finalize box-select
            if let Some(bs) = state.box_select.take() {
                let sel_rect = Rect::from_two_pos(bs.start, bs.current);
                if sel_rect.area() > 16.0 {
                    // Select all widgets that intersect the box
                    state.multi_selection.clear();
                    for ws in &widget_snapshots {
                        let rect = ws_screen_rect(ws, canvas_rect, z);
                        if sel_rect.intersects(rect) {
                            state.multi_selection.push(ws.entity);
                        }
                    }
                    // Set primary selection to first
                    if let Some(&first) = state.multi_selection.first() {
                        commands.push(move |world: &mut World| {
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                sel.set(Some(first));
                            }
                        });
                    }
                }
            }
        }

        // ── Widget palette drag-and-drop ────────────────────────────────
        if let Some(widget_drag) = world.get_resource::<WidgetDragPayload>() {
            let pointer = ui.ctx().pointer_latest_pos();

            // Update detach state
            if !widget_drag.is_detached {
                if let Some(pos) = pointer {
                    if pos.distance(widget_drag.origin) > 5.0 {
                        commands.push(|world: &mut World| {
                            if let Some(mut drag) = world.get_resource_mut::<WidgetDragPayload>() {
                                drag.is_detached = true;
                            }
                        });
                    }
                }
            }

            if widget_drag.is_detached {
                // Draw ghost
                if let Some(pos) = pointer {
                    crate::palette::draw_widget_drag_ghost(ui.ctx(), widget_drag, pos, &theme);

                    // Highlight canvas drop zone
                    if canvas_rect.contains(pos) {
                        painter.rect_stroke(
                            canvas_rect,
                            0.0,
                            Stroke::new(2.0, accent),
                            egui::StrokeKind::Inside,
                        );
                    }
                }

                // Drop on pointer release
                if !ui.ctx().input(|i| i.pointer.any_down()) {
                    let over_canvas = pointer.map_or(false, |p| canvas_rect.contains(p));
                    if over_canvas && state.active_canvas.is_some() {
                        // Convert screen position to canvas-local coordinates
                        let pos = pointer.unwrap();
                        let mut canvas_x = (pos.x - canvas_rect.min.x) / z;
                        let mut canvas_y = (pos.y - canvas_rect.min.y) / z;
                        if state.snap_enabled {
                            canvas_x = snap(canvas_x, state.grid_size);
                            canvas_y = snap(canvas_y, state.grid_size);
                        }
                        let wt = widget_drag.widget_type.clone();
                        let active = state.active_canvas;
                        commands.push(move |world: &mut World| {
                            crate::spawn::spawn_widget(world, &wt, active);
                            // Set position on the newly spawned widget
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                if let Some(entity) = sel.get() {
                                    if let Ok(mut em) = world.get_entity_mut(entity) {
                                        if let Some(mut node) = em.get_mut::<Node>() {
                                            node.left = bevy::ui::Val::Px(canvas_x);
                                            node.top = bevy::ui::Val::Px(canvas_y);
                                            node.position_type =
                                                bevy::ui::PositionType::Absolute;
                                        }
                                    }
                                }
                            }
                            world.remove_resource::<WidgetDragPayload>();
                        });
                    } else {
                        // Released outside canvas — cancel
                        commands.push(|world: &mut World| {
                            world.remove_resource::<WidgetDragPayload>();
                        });
                    }
                }
            }

            // Cancel on Escape
            if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                commands.push(|world: &mut World| {
                    world.remove_resource::<WidgetDragPayload>();
                });
            }
        }

        // ── Empty state ──────────────────────────────────────────────────
        if state.canvases.is_empty() {
            painter.text(
                available.center(),
                egui::Align2::CENTER_CENTER,
                "Add a UI Canvas from the Widget palette to get started",
                egui::FontId::proportional(13.0),
                text_muted,
            );
        } else if widget_snapshots.is_empty() && world.get_resource::<WidgetDragPayload>().is_none() {
            painter.text(
                canvas_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drag widgets from the palette or click to add them here",
                egui::FontId::proportional(12.0),
                text_muted,
            );
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Check if `entity` is a descendant of `ancestor` by walking up the parent chain.
fn is_descendant_of(world: &World, entity: Entity, ancestor: Entity) -> bool {
    let mut current = entity;
    for _ in 0..32 {
        if current == ancestor {
            return true;
        }
        match world.get::<ChildOf>(current) {
            Some(child_of) => current = child_of.parent(),
            None => return false,
        }
    }
    false
}

// ── Alignment ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum AlignAction {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

fn compute_align(widgets: &[WidgetSnapshot], action: AlignAction) -> Vec<(Entity, f32, f32)> {
    if widgets.is_empty() {
        return vec![];
    }
    match action {
        AlignAction::Left => {
            let min_x = widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
            widgets.iter().map(|w| (w.entity, min_x, w.y)).collect()
        }
        AlignAction::Right => {
            let max_right = widgets
                .iter()
                .map(|w| w.x + w.width)
                .fold(f32::MIN, f32::max);
            widgets
                .iter()
                .map(|w| (w.entity, max_right - w.width, w.y))
                .collect()
        }
        AlignAction::CenterH => {
            let min_x = widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
            let max_right = widgets
                .iter()
                .map(|w| w.x + w.width)
                .fold(f32::MIN, f32::max);
            let center = (min_x + max_right) / 2.0;
            widgets
                .iter()
                .map(|w| (w.entity, center - w.width / 2.0, w.y))
                .collect()
        }
        AlignAction::Top => {
            let min_y = widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            widgets.iter().map(|w| (w.entity, w.x, min_y)).collect()
        }
        AlignAction::Bottom => {
            let max_bottom = widgets
                .iter()
                .map(|w| w.y + w.height)
                .fold(f32::MIN, f32::max);
            widgets
                .iter()
                .map(|w| (w.entity, w.x, max_bottom - w.height))
                .collect()
        }
        AlignAction::CenterV => {
            let min_y = widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            let max_bottom = widgets
                .iter()
                .map(|w| w.y + w.height)
                .fold(f32::MIN, f32::max);
            let center = (min_y + max_bottom) / 2.0;
            widgets
                .iter()
                .map(|w| (w.entity, w.x, center - w.height / 2.0))
                .collect()
        }
    }
}

fn compute_distribute_h(widgets: &[WidgetSnapshot]) -> Vec<(Entity, f32)> {
    if widgets.len() < 3 {
        return vec![];
    }
    let mut sorted: Vec<_> = widgets.to_vec();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    let first_x = sorted.first().unwrap().x;
    let last_x = sorted.last().unwrap().x;
    let step = (last_x - first_x) / (sorted.len() - 1) as f32;
    sorted
        .iter()
        .enumerate()
        .map(|(i, w)| (w.entity, first_x + step * i as f32))
        .collect()
}

fn compute_distribute_v(widgets: &[WidgetSnapshot]) -> Vec<(Entity, f32)> {
    if widgets.len() < 3 {
        return vec![];
    }
    let mut sorted: Vec<_> = widgets.to_vec();
    sorted.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
    let first_y = sorted.first().unwrap().y;
    let last_y = sorted.last().unwrap().y;
    let step = (last_y - first_y) / (sorted.len() - 1) as f32;
    sorted
        .iter()
        .enumerate()
        .map(|(i, w)| (w.entity, first_y + step * i as f32))
        .collect()
}

// ── Widget data snapshot extraction ─────────────────────────────────────────

fn c2a(c: Color) -> [f32; 4] {
    c.to_srgba().to_f32_array()
}

fn snapshot_widget_data(world: &World, entity: Entity, wtype: &UiWidgetType) -> WidgetDataSnapshot {
    match wtype {
        UiWidgetType::Slider => {
            if let Some(d) = world.get::<SliderData>(entity) {
                WidgetDataSnapshot::Slider {
                    value: d.value, min: d.min, max: d.max,
                    track_color: c2a(d.track_color),
                    fill_color: c2a(d.fill_color),
                    thumb_color: c2a(d.thumb_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::ProgressBar => {
            if let Some(d) = world.get::<ProgressBarData>(entity) {
                WidgetDataSnapshot::ProgressBar {
                    value: d.value, max: d.max, fill_color: c2a(d.fill_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::HealthBar => {
            if let Some(d) = world.get::<HealthBarData>(entity) {
                WidgetDataSnapshot::HealthBar {
                    current: d.current, max: d.max, low_threshold: d.low_threshold,
                    fill_color: c2a(d.fill_color), low_color: c2a(d.low_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Checkbox => {
            if let Some(d) = world.get::<CheckboxData>(entity) {
                WidgetDataSnapshot::Checkbox {
                    checked: d.checked, label: d.label.clone(),
                    check_color: c2a(d.check_color), box_color: c2a(d.box_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Toggle => {
            if let Some(d) = world.get::<ToggleData>(entity) {
                WidgetDataSnapshot::Toggle {
                    on: d.on, label: d.label.clone(),
                    on_color: c2a(d.on_color), off_color: c2a(d.off_color),
                    knob_color: c2a(d.knob_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Dropdown => {
            if let Some(d) = world.get::<DropdownData>(entity) {
                let text = if d.selected >= 0 && (d.selected as usize) < d.options.len() {
                    d.options[d.selected as usize].clone()
                } else {
                    d.placeholder.clone()
                };
                WidgetDataSnapshot::Dropdown { selected_text: text, open: d.open }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::TextInput => {
            if let Some(d) = world.get::<TextInputData>(entity) {
                WidgetDataSnapshot::TextInput {
                    text: d.text.clone(), placeholder: d.placeholder.clone(),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::TabBar => {
            if let Some(d) = world.get::<TabBarData>(entity) {
                WidgetDataSnapshot::TabBar {
                    tabs: d.tabs.clone(), active: d.active,
                    tab_color: c2a(d.tab_color), active_color: c2a(d.active_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Spinner => {
            if let Some(d) = world.get::<SpinnerData>(entity) {
                WidgetDataSnapshot::Spinner { color: c2a(d.color) }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::RadioButton => {
            if let Some(d) = world.get::<RadioButtonData>(entity) {
                WidgetDataSnapshot::RadioButton {
                    selected: d.selected, label: d.label.clone(),
                    active_color: c2a(d.active_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Modal => {
            if let Some(d) = world.get::<ModalData>(entity) {
                WidgetDataSnapshot::Modal { title: d.title.clone() }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::DraggableWindow => {
            if let Some(d) = world.get::<DraggableWindowData>(entity) {
                WidgetDataSnapshot::DraggableWindow {
                    title: d.title.clone(), title_bar_color: c2a(d.title_bar_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        // ── HUD ──
        UiWidgetType::Crosshair => {
            if let Some(d) = world.get::<CrosshairData>(entity) {
                WidgetDataSnapshot::Crosshair {
                    style: format!("{:?}", d.style),
                    color: c2a(d.color),
                    size: d.size,
                    thickness: d.thickness,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::AmmoCounter => {
            if let Some(d) = world.get::<AmmoCounterData>(entity) {
                WidgetDataSnapshot::AmmoCounter {
                    current: d.current, max: d.max,
                    color: c2a(d.color), low_color: c2a(d.low_color),
                    low_threshold: d.low_threshold,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Compass => {
            if let Some(d) = world.get::<CompassData>(entity) {
                WidgetDataSnapshot::Compass {
                    heading: d.heading,
                    color: c2a(d.text_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::StatusEffectBar => {
            if let Some(d) = world.get::<StatusEffectBarData>(entity) {
                WidgetDataSnapshot::StatusEffectBar {
                    effect_count: d.effects.len(),
                    color: [0.3, 0.7, 1.0, 1.0],
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::NotificationFeed => {
            if let Some(d) = world.get::<NotificationFeedData>(entity) {
                WidgetDataSnapshot::NotificationFeed {
                    count: d.max_visible,
                    color: [0.9, 0.9, 0.9, 1.0],
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::RadialMenu => {
            if let Some(d) = world.get::<RadialMenuData>(entity) {
                WidgetDataSnapshot::RadialMenu {
                    item_count: d.items.len().max(1),
                    color: c2a(d.bg_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Minimap => {
            if let Some(d) = world.get::<MinimapData>(entity) {
                WidgetDataSnapshot::Minimap {
                    shape: format!("{:?}", d.shape),
                    bg_color: c2a(d.bg_color),
                    border_color: c2a(d.border_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        // ── Shapes ──
        UiWidgetType::Circle => {
            if let Some(d) = world.get::<CircleShape>(entity) {
                WidgetDataSnapshot::ShapeCircle {
                    color: c2a(d.color),
                    stroke_color: c2a(d.stroke_color),
                    stroke_width: d.stroke_width,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Arc => {
            if let Some(d) = world.get::<ArcShape>(entity) {
                WidgetDataSnapshot::ShapeArc {
                    color: c2a(d.color),
                    start_angle: d.start_angle,
                    end_angle: d.end_angle,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Triangle => {
            if let Some(d) = world.get::<TriangleShape>(entity) {
                WidgetDataSnapshot::ShapeTriangle {
                    color: c2a(d.color),
                    stroke_color: c2a(d.stroke_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Line => {
            if let Some(d) = world.get::<LineShape>(entity) {
                WidgetDataSnapshot::ShapeLine {
                    color: c2a(d.color),
                    thickness: d.thickness,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Polygon => {
            if let Some(d) = world.get::<PolygonShape>(entity) {
                WidgetDataSnapshot::ShapePolygon {
                    color: c2a(d.color),
                    stroke_color: c2a(d.stroke_color),
                    sides: d.sides,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Wedge => {
            if let Some(d) = world.get::<WedgeShape>(entity) {
                WidgetDataSnapshot::ShapeWedge {
                    color: c2a(d.color),
                    start_angle: d.start_angle,
                    end_angle: d.end_angle,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::RadialProgress => {
            if let Some(d) = world.get::<RadialProgressShape>(entity) {
                WidgetDataSnapshot::ShapeRadialProgress {
                    color: c2a(d.color),
                    track_color: c2a(d.bg_color),
                    value: d.value,
                }
            } else { WidgetDataSnapshot::None }
        }
        // ── Menu ──
        UiWidgetType::InventoryGrid => {
            if let Some(d) = world.get::<InventoryGridData>(entity) {
                WidgetDataSnapshot::InventoryGrid {
                    columns: d.columns, rows: d.rows, slot_size: d.slot_size,
                    slot_bg_color: c2a(d.slot_bg_color),
                    slot_border_color: c2a(d.slot_border_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::DialogBox => {
            if let Some(d) = world.get::<DialogBoxData>(entity) {
                WidgetDataSnapshot::DialogBox {
                    speaker: d.speaker.clone(), text: d.text.clone(),
                    bg_color: c2a(d.bg_color), speaker_color: c2a(d.speaker_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::ObjectiveTracker => {
            if let Some(d) = world.get::<ObjectiveTrackerData>(entity) {
                WidgetDataSnapshot::ObjectiveTracker {
                    title: d.title.clone(),
                    objective_count: d.objectives.len(),
                    title_color: c2a(d.title_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::LoadingScreen => {
            if let Some(d) = world.get::<LoadingScreenData>(entity) {
                WidgetDataSnapshot::LoadingScreen {
                    progress: d.progress, message: d.message.clone(),
                    bar_color: c2a(d.bar_color), bg_color: c2a(d.bg_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::KeybindRow => {
            if let Some(d) = world.get::<KeybindRowData>(entity) {
                WidgetDataSnapshot::KeybindRow {
                    action: d.action.clone(), binding: d.binding.clone(),
                    key_bg_color: c2a(d.key_bg_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::SettingsRow => {
            if let Some(d) = world.get::<SettingsRowData>(entity) {
                WidgetDataSnapshot::SettingsRow {
                    label: d.label.clone(), value: d.value.clone(),
                }
            } else { WidgetDataSnapshot::None }
        }
        // ── Extra ──
        UiWidgetType::Separator => {
            if let Some(d) = world.get::<SeparatorData>(entity) {
                WidgetDataSnapshot::Separator {
                    horizontal: matches!(d.direction, SeparatorDirection::Horizontal),
                    color: c2a(d.color), thickness: d.thickness,
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::NumberInput => {
            if let Some(d) = world.get::<NumberInputData>(entity) {
                WidgetDataSnapshot::NumberInput {
                    value: d.value, precision: d.precision,
                    bg_color: c2a(d.bg_color), button_color: c2a(d.button_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::VerticalSlider => {
            if let Some(d) = world.get::<VerticalSliderData>(entity) {
                WidgetDataSnapshot::VerticalSlider {
                    value: d.value, min: d.min, max: d.max,
                    track_color: c2a(d.track_color),
                    fill_color: c2a(d.fill_color),
                    thumb_color: c2a(d.thumb_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::Scrollbar => {
            if let Some(d) = world.get::<ScrollbarData>(entity) {
                WidgetDataSnapshot::Scrollbar {
                    vertical: matches!(d.orientation, ScrollbarOrientation::Vertical),
                    viewport_fraction: d.viewport_fraction,
                    position: d.position,
                    track_color: c2a(d.track_color),
                    thumb_color: c2a(d.thumb_color),
                }
            } else { WidgetDataSnapshot::None }
        }
        UiWidgetType::List => {
            if let Some(d) = world.get::<ListData>(entity) {
                WidgetDataSnapshot::ListWidget {
                    item_count: d.items.len(),
                    bg_color: c2a(d.bg_color),
                    selected_bg_color: c2a(d.selected_bg_color),
                    item_height: d.item_height,
                }
            } else { WidgetDataSnapshot::None }
        }
        _ => WidgetDataSnapshot::None,
    }
}

// ── Per-widget-type painting ────────────────────────────────────────────────

fn arr_to_c32(c: &[f32; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}

/// Per-corner border radius scaled by zoom, converted for egui.
fn avg_radius(r: &[f32; 4], z: f32) -> egui::Rounding {
    egui::Rounding {
        nw: (r[0] * z) as u8,
        ne: (r[1] * z) as u8,
        se: (r[2] * z) as u8,
        sw: (r[3] * z) as u8,
    }
}

fn round_f(v: f32) -> egui::Rounding {
    egui::Rounding::same(v as u8)
}

/// Paint a widget preview on the canvas. Called instead of the old flat-rect code.
fn paint_widget_preview(
    painter: &egui::Painter,
    ws: &WidgetSnapshot,
    rect: Rect,
    z: f32,
    is_sel: bool,
    accent: Color32,
    text_muted: Color32,
) {
    let rounding = avg_radius(&ws.border_radius, z);

    // ── Drop shadow ──────────────────────────────────────────────────
    if let Some(ref sh) = ws.shadow {
        let [r, g, b, a, blur, _spread] = *sh;
        // Approximate shadow with a larger, semi-transparent rect behind the widget
        let expand = blur * z * 0.5;
        let shadow_rect = rect.expand(expand);
        let shadow_color = Color32::from_rgba_premultiplied(
            (r * a * 80.0) as u8,
            (g * a * 80.0) as u8,
            (b * a * 80.0) as u8,
            (a * 80.0) as u8,
        );
        painter.rect_filled(shadow_rect, rounding, shadow_color);
    }

    // Dispatch to per-type painter, or fall back to generic
    match &ws.widget_data {
        WidgetDataSnapshot::Slider { value, min, max, track_color, fill_color, thumb_color } => {
            paint_slider(painter, rect, z, rounding, *value, *min, *max, track_color, fill_color, thumb_color);
        }
        WidgetDataSnapshot::ProgressBar { value, max, fill_color } => {
            paint_progress_bar(painter, ws, rect, z, rounding, *value, *max, fill_color);
        }
        WidgetDataSnapshot::HealthBar { current, max, low_threshold, fill_color, low_color } => {
            let ratio = if *max > 0.0 { *current / *max } else { 0.0 };
            let color = if ratio < *low_threshold { low_color } else { fill_color };
            paint_progress_bar(painter, ws, rect, z, rounding, *current, *max, color);
        }
        WidgetDataSnapshot::Checkbox { checked, label, check_color, box_color } => {
            paint_checkbox(painter, ws, rect, z, *checked, label, check_color, box_color);
        }
        WidgetDataSnapshot::Toggle { on, label, on_color, off_color, knob_color } => {
            paint_toggle(painter, ws, rect, z, *on, label, on_color, off_color, knob_color);
        }
        WidgetDataSnapshot::Dropdown { selected_text, .. } => {
            paint_dropdown(painter, ws, rect, z, rounding, selected_text);
        }
        WidgetDataSnapshot::TextInput { text, placeholder } => {
            paint_text_input(painter, ws, rect, z, rounding, text, placeholder);
        }
        WidgetDataSnapshot::TabBar { tabs, active, tab_color, active_color } => {
            paint_tab_bar(painter, ws, rect, z, tabs, *active, tab_color, active_color);
        }
        WidgetDataSnapshot::Spinner { color } => {
            paint_spinner(painter, rect, z, color);
        }
        WidgetDataSnapshot::RadioButton { selected, label, active_color } => {
            paint_radio_button(painter, ws, rect, z, *selected, label, active_color);
        }
        WidgetDataSnapshot::Modal { title } => {
            paint_window_like(painter, ws, rect, z, rounding, title, &ws.bg_color);
        }
        WidgetDataSnapshot::DraggableWindow { title, title_bar_color } => {
            paint_window_like(painter, ws, rect, z, rounding, title, title_bar_color);
        }
        // ── HUD ──
        WidgetDataSnapshot::Crosshair { style, color, size, thickness } => {
            paint_crosshair(painter, rect, z, style, color, *size, *thickness);
        }
        WidgetDataSnapshot::AmmoCounter { current, max, color, low_color, low_threshold } => {
            paint_ammo_counter(painter, rect, z, *current, *max, color, low_color, *low_threshold);
        }
        WidgetDataSnapshot::Compass { heading, color } => {
            paint_compass(painter, rect, z, *heading, color);
        }
        WidgetDataSnapshot::StatusEffectBar { effect_count, color } => {
            paint_status_effect_bar(painter, rect, z, *effect_count, color);
        }
        WidgetDataSnapshot::NotificationFeed { count, color } => {
            paint_notification_feed(painter, rect, z, *count, color);
        }
        WidgetDataSnapshot::RadialMenu { item_count, color } => {
            paint_radial_menu(painter, rect, z, *item_count, color);
        }
        WidgetDataSnapshot::Minimap { shape, bg_color, border_color } => {
            paint_minimap(painter, rect, z, shape, bg_color, border_color);
        }
        // ── Shapes ──
        WidgetDataSnapshot::ShapeCircle { color, stroke_color, stroke_width } => {
            paint_shape_circle(painter, rect, z, color, stroke_color, *stroke_width);
        }
        WidgetDataSnapshot::ShapeArc { color, start_angle, end_angle } => {
            paint_shape_arc(painter, rect, z, color, *start_angle, *end_angle);
        }
        WidgetDataSnapshot::ShapeTriangle { color, stroke_color } => {
            paint_shape_triangle(painter, rect, z, color, stroke_color);
        }
        WidgetDataSnapshot::ShapeLine { color, thickness } => {
            paint_shape_line(painter, rect, z, color, *thickness);
        }
        WidgetDataSnapshot::ShapePolygon { color, stroke_color, sides } => {
            paint_shape_polygon(painter, rect, z, color, stroke_color, *sides);
        }
        WidgetDataSnapshot::ShapeWedge { color, start_angle, end_angle } => {
            paint_shape_wedge(painter, rect, z, color, *start_angle, *end_angle);
        }
        WidgetDataSnapshot::ShapeRadialProgress { color, track_color, value } => {
            paint_shape_radial_progress(painter, rect, z, color, track_color, *value);
        }
        // ── Menu ──
        WidgetDataSnapshot::InventoryGrid { columns, rows, slot_size, slot_bg_color, slot_border_color } => {
            paint_inventory_grid(painter, ws, rect, z, *columns, *rows, *slot_size, slot_bg_color, slot_border_color);
        }
        WidgetDataSnapshot::DialogBox { speaker, text, bg_color, speaker_color } => {
            paint_dialog_box(painter, ws, rect, z, rounding, speaker, text, bg_color, speaker_color);
        }
        WidgetDataSnapshot::ObjectiveTracker { title, objective_count, title_color } => {
            paint_objective_tracker(painter, ws, rect, z, rounding, title, *objective_count, title_color);
        }
        WidgetDataSnapshot::LoadingScreen { progress, message, bar_color, bg_color } => {
            paint_loading_screen(painter, rect, z, *progress, message, bar_color, bg_color);
        }
        WidgetDataSnapshot::KeybindRow { action, binding, key_bg_color } => {
            paint_keybind_row(painter, ws, rect, z, action, binding, key_bg_color);
        }
        WidgetDataSnapshot::SettingsRow { label, value } => {
            paint_settings_row(painter, ws, rect, z, label, value);
        }
        // ── Extra ──
        WidgetDataSnapshot::Separator { horizontal, color, thickness } => {
            paint_separator(painter, rect, z, *horizontal, color, *thickness);
        }
        WidgetDataSnapshot::NumberInput { value, precision, bg_color, button_color } => {
            paint_number_input(painter, ws, rect, z, rounding, *value, *precision, bg_color, button_color);
        }
        WidgetDataSnapshot::VerticalSlider { value, min, max, track_color, fill_color, thumb_color } => {
            paint_vertical_slider(painter, rect, z, *value, *min, *max, track_color, fill_color, thumb_color);
        }
        WidgetDataSnapshot::Scrollbar { vertical, viewport_fraction, position, track_color, thumb_color } => {
            paint_scrollbar(painter, rect, z, *vertical, *viewport_fraction, *position, track_color, thumb_color);
        }
        WidgetDataSnapshot::ListWidget { item_count, bg_color, selected_bg_color, item_height } => {
            paint_list_widget(painter, ws, rect, z, rounding, *item_count, bg_color, selected_bg_color, *item_height);
        }
        WidgetDataSnapshot::None => {
            // Generic: text widget, container, panel, image
            paint_generic(painter, ws, rect, z, rounding);
        }
    }

    // ── Selection highlight (always on top) ──────────────────────────
    if is_sel {
        painter.rect_stroke(rect, rounding, Stroke::new(2.0, accent), egui::StrokeKind::Outside);
    } else if ws.stroke_width > 0.0 && ws.has_border {
        let sc = arr_to_c32(&ws.border_color);
        painter.rect_stroke(rect, rounding, Stroke::new(ws.stroke_width * z, sc), egui::StrokeKind::Outside);
    }

    // ── Resize handles ──────────────────────────────────────────────
    // (handled by the caller)
}

// ── Individual widget painters ──────────────────────────────────────────────

fn paint_generic(painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding) {
    // Image widget with texture
    if let Some(tex_id) = ws.image_texture_id {
        painter.image(
            tex_id, rect,
            egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        return;
    }

    // Background fill
    let bg = if ws.has_bg {
        arr_to_c32(&ws.bg_color)
    } else {
        Color32::from_rgba_unmultiplied(50, 50, 60, 40)
    };
    painter.rect_filled(rect, rounding, bg);

    // Text content (for Text / Button widgets)
    if let Some(ref text) = ws.text_content {
        if !text.is_empty() && rect.width() > 10.0 && rect.height() > 8.0 {
            let tc = arr_to_c32(&ws.text_color);
            let font_size = ws.text_size * z;
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(font_size.clamp(6.0, 40.0)),
                tc,
            );
        }
    } else if rect.height() > 16.0 && rect.width() > 30.0 {
        // Fallback: icon + name label for containers/panels
        let label = format!("{} {}", ws.widget_type.icon(), ws.name);
        painter.text(
            Pos2::new(rect.min.x + 4.0 * z, rect.min.y + 2.0 * z),
            egui::Align2::LEFT_TOP,
            &label,
            egui::FontId::proportional(10.0 * z.min(1.5)),
            Color32::from_rgba_unmultiplied(180, 180, 180, 160),
        );
    }
}

fn paint_slider(
    painter: &egui::Painter, rect: Rect, z: f32, _rounding: egui::Rounding,
    value: f32, min: f32, max: f32,
    track_color: &[f32; 4], fill_color: &[f32; 4], thumb_color: &[f32; 4],
) {
    let track_h = (6.0 * z).max(2.0);
    let track_y = rect.center().y - track_h / 2.0;
    let track_rect = Rect::from_min_size(
        Pos2::new(rect.min.x, track_y),
        Vec2::new(rect.width(), track_h),
    );
    let track_round = round_f(track_h / 2.0);

    // Track background
    painter.rect_filled(track_rect, track_round, arr_to_c32(track_color));

    // Fill
    let ratio = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
    let fill_w = rect.width() * ratio;
    if fill_w > 0.5 {
        let fill_rect = Rect::from_min_size(track_rect.min, Vec2::new(fill_w, track_h));
        painter.rect_filled(fill_rect, track_round, arr_to_c32(fill_color));
    }

    // Thumb
    let thumb_r = (8.0 * z).max(3.0);
    let thumb_x = rect.min.x + fill_w;
    painter.circle_filled(
        Pos2::new(thumb_x, rect.center().y),
        thumb_r,
        arr_to_c32(thumb_color),
    );
}

fn paint_progress_bar(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    value: f32, max: f32, fill_color: &[f32; 4],
) {
    // Background
    let bg = if ws.has_bg { arr_to_c32(&ws.bg_color) } else { Color32::from_rgb(40, 40, 45) };
    painter.rect_filled(rect, rounding, bg);

    // Fill bar
    let ratio = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
    let fill_w = rect.width() * ratio;
    if fill_w > 0.5 {
        let fill_rect = Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        painter.rect_filled(fill_rect, rounding, arr_to_c32(fill_color));
    }
}

fn paint_checkbox(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    checked: bool, label: &str, check_color: &[f32; 4], box_color: &[f32; 4],
) {
    let box_size = (18.0 * z).max(8.0);
    let box_y = rect.center().y - box_size / 2.0;
    let box_rect = Rect::from_min_size(Pos2::new(rect.min.x, box_y), Vec2::splat(box_size));
    let box_round = round_f(3.0 * z);

    // Box
    painter.rect_filled(box_rect, box_round, arr_to_c32(box_color));
    painter.rect_stroke(box_rect, box_round, Stroke::new(1.5 * z, Color32::from_rgb(120, 120, 130)), egui::StrokeKind::Outside);

    // Checkmark
    if checked {
        let cc = arr_to_c32(check_color);
        let inner = box_rect.shrink(4.0 * z);
        painter.rect_filled(inner, round_f(2.0 * z), cc);
    }

    // Label
    if !label.is_empty() {
        let tc = arr_to_c32(&ws.text_color);
        painter.text(
            Pos2::new(box_rect.max.x + 6.0 * z, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional((ws.text_size * z).clamp(6.0, 30.0)),
            tc,
        );
    }
}

fn paint_toggle(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    on: bool, label: &str,
    on_color: &[f32; 4], off_color: &[f32; 4], knob_color: &[f32; 4],
) {
    let track_w = (44.0 * z).max(20.0);
    let track_h = (24.0 * z).max(12.0);
    let track_y = rect.center().y - track_h / 2.0;
    let track_rect = Rect::from_min_size(Pos2::new(rect.min.x, track_y), Vec2::new(track_w, track_h));
    let track_round = round_f(track_h / 2.0);

    let track_color = if on { arr_to_c32(on_color) } else { arr_to_c32(off_color) };
    painter.rect_filled(track_rect, track_round, track_color);

    // Knob
    let knob_r = (track_h - 4.0 * z) / 2.0;
    let knob_x = if on {
        track_rect.max.x - knob_r - 2.0 * z
    } else {
        track_rect.min.x + knob_r + 2.0 * z
    };
    painter.circle_filled(Pos2::new(knob_x, rect.center().y), knob_r, arr_to_c32(knob_color));

    // Label
    if !label.is_empty() {
        let tc = arr_to_c32(&ws.text_color);
        painter.text(
            Pos2::new(track_rect.max.x + 8.0 * z, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional((ws.text_size * z).clamp(6.0, 30.0)),
            tc,
        );
    }
}

fn paint_dropdown(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    selected_text: &str,
) {
    let bg = if ws.has_bg { arr_to_c32(&ws.bg_color) } else { Color32::from_rgb(45, 45, 50) };
    painter.rect_filled(rect, rounding, bg);
    painter.rect_stroke(rect, rounding, Stroke::new(z, Color32::from_rgb(80, 80, 90)), egui::StrokeKind::Outside);

    // Text
    let tc = arr_to_c32(&ws.text_color);
    let pad = 8.0 * z;
    painter.text(
        Pos2::new(rect.min.x + pad, rect.center().y),
        egui::Align2::LEFT_CENTER,
        selected_text,
        egui::FontId::proportional((ws.text_size * z).clamp(6.0, 30.0)),
        tc,
    );

    // Down arrow
    let arrow_x = rect.max.x - pad - 6.0 * z;
    painter.text(
        Pos2::new(arrow_x, rect.center().y),
        egui::Align2::CENTER_CENTER,
        egui_phosphor::regular::CARET_DOWN,
        egui::FontId::proportional(12.0 * z),
        tc,
    );
}

fn paint_text_input(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    text: &str, placeholder: &str,
) {
    let bg = if ws.has_bg { arr_to_c32(&ws.bg_color) } else { Color32::from_rgb(35, 35, 40) };
    painter.rect_filled(rect, rounding, bg);
    painter.rect_stroke(rect, rounding, Stroke::new(z, Color32::from_rgb(80, 80, 90)), egui::StrokeKind::Outside);

    let pad = 8.0 * z;
    let (display_text, color) = if text.is_empty() {
        (placeholder, Color32::from_rgb(120, 120, 130))
    } else {
        (text.as_ref(), arr_to_c32(&ws.text_color))
    };
    painter.text(
        Pos2::new(rect.min.x + pad, rect.center().y),
        egui::Align2::LEFT_CENTER,
        display_text,
        egui::FontId::proportional((ws.text_size * z).clamp(6.0, 30.0)),
        color,
    );

    // Cursor line
    if !text.is_empty() {
        let galley = painter.layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional((ws.text_size * z).clamp(6.0, 30.0)),
            arr_to_c32(&ws.text_color),
        );
        let cursor_x = (rect.min.x + pad + galley.size().x).min(rect.max.x - 2.0);
        painter.line_segment(
            [
                Pos2::new(cursor_x, rect.min.y + 4.0 * z),
                Pos2::new(cursor_x, rect.max.y - 4.0 * z),
            ],
            Stroke::new(z, Color32::from_rgb(180, 180, 200)),
        );
    }
}

fn paint_tab_bar(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    tabs: &[String], active: usize, tab_color: &[f32; 4], active_color: &[f32; 4],
) {
    if tabs.is_empty() { return; }
    let tab_w = rect.width() / tabs.len() as f32;
    let tc = arr_to_c32(&ws.text_color);

    for (i, tab) in tabs.iter().enumerate() {
        let x = rect.min.x + tab_w * i as f32;
        let tab_rect = Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(tab_w, rect.height()));
        let color = if i == active { arr_to_c32(active_color) } else { arr_to_c32(tab_color) };
        painter.rect_filled(tab_rect, 0.0, color);

        painter.text(
            tab_rect.center(),
            egui::Align2::CENTER_CENTER,
            tab,
            egui::FontId::proportional((11.0 * z).clamp(6.0, 24.0)),
            tc,
        );
    }
}

fn paint_spinner(painter: &egui::Painter, rect: Rect, z: f32, color: &[f32; 4]) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0 * z;
    let sw = (3.0 * z).max(1.0);
    let c = arr_to_c32(color);

    // Draw 3/4 of a circle arc (approximated with line segments)
    let segments = 24;
    let start_angle = 0.0_f32;
    let sweep = std::f32::consts::PI * 1.5;
    for i in 0..segments {
        let a0 = start_angle + sweep * (i as f32 / segments as f32);
        let a1 = start_angle + sweep * ((i + 1) as f32 / segments as f32);
        let p0 = Pos2::new(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        painter.line_segment([p0, p1], Stroke::new(sw, c));
    }
}

fn paint_radio_button(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    selected: bool, label: &str, active_color: &[f32; 4],
) {
    let circle_r = (9.0 * z).max(4.0);
    let cx = rect.min.x + circle_r;
    let cy = rect.center().y;

    // Outer circle
    painter.circle_stroke(
        Pos2::new(cx, cy), circle_r,
        Stroke::new(1.5 * z, Color32::from_rgb(120, 120, 130)),
    );

    // Inner dot if selected
    if selected {
        painter.circle_filled(Pos2::new(cx, cy), circle_r * 0.5, arr_to_c32(active_color));
    }

    // Label
    if !label.is_empty() {
        let tc = arr_to_c32(&ws.text_color);
        painter.text(
            Pos2::new(cx + circle_r + 6.0 * z, cy),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional((ws.text_size * z).clamp(6.0, 30.0)),
            tc,
        );
    }
}

fn paint_window_like(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    title: &str, title_bar_color: &[f32; 4],
) {
    // Body
    let bg = if ws.has_bg { arr_to_c32(&ws.bg_color) } else { Color32::from_rgb(40, 40, 48) };
    painter.rect_filled(rect, rounding, bg);

    // Title bar
    let tb_h = (28.0 * z).max(12.0);
    let tb_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), tb_h));
    let tb_round = egui::Rounding { nw: rounding.nw, ne: rounding.ne, se: 0, sw: 0 };
    painter.rect_filled(tb_rect, tb_round, arr_to_c32(title_bar_color));

    // Title text
    painter.text(
        Pos2::new(tb_rect.min.x + 8.0 * z, tb_rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional((12.0 * z).clamp(6.0, 24.0)),
        Color32::WHITE,
    );

    // Border
    painter.rect_stroke(rect, rounding, Stroke::new(z, Color32::from_rgb(70, 70, 80)), egui::StrokeKind::Outside);
}

// ── HUD widget painters ────────────────────────────────────────────────────

fn paint_crosshair(
    painter: &egui::Painter, rect: Rect, z: f32,
    style: &str, color: &[f32; 4], _size: f32, thickness: f32,
) {
    let center = rect.center();
    let c = arr_to_c32(color);
    let sw = (thickness * z).max(1.0);
    let arm = rect.width().min(rect.height()) / 2.0 - 4.0 * z;

    match style {
        "Dot" => {
            painter.circle_filled(center, (3.0 * z).max(1.5), c);
        }
        "CircleDot" => {
            painter.circle_stroke(center, arm * 0.6, Stroke::new(sw, c));
            painter.circle_filled(center, (2.0 * z).max(1.0), c);
        }
        "CrossDot" => {
            // Cross lines
            painter.line_segment(
                [Pos2::new(center.x - arm, center.y), Pos2::new(center.x + arm, center.y)],
                Stroke::new(sw, c),
            );
            painter.line_segment(
                [Pos2::new(center.x, center.y - arm), Pos2::new(center.x, center.y + arm)],
                Stroke::new(sw, c),
            );
            painter.circle_filled(center, (2.0 * z).max(1.0), c);
        }
        _ => {
            // "Cross" (default)
            painter.line_segment(
                [Pos2::new(center.x - arm, center.y), Pos2::new(center.x + arm, center.y)],
                Stroke::new(sw, c),
            );
            painter.line_segment(
                [Pos2::new(center.x, center.y - arm), Pos2::new(center.x, center.y + arm)],
                Stroke::new(sw, c),
            );
        }
    }
}

fn paint_ammo_counter(
    painter: &egui::Painter, rect: Rect, z: f32,
    current: u32, max: u32, color: &[f32; 4], low_color: &[f32; 4], low_threshold: u32,
) {
    // Background
    painter.rect_filled(rect, round_f(4.0 * z), Color32::from_rgba_unmultiplied(20, 20, 25, 200));

    let c = if current <= low_threshold { arr_to_c32(low_color) } else { arr_to_c32(color) };
    let text = format!("{}/{}", current, max);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &text,
        egui::FontId::proportional((16.0 * z).clamp(8.0, 36.0)),
        c,
    );
}

fn paint_compass(
    painter: &egui::Painter, rect: Rect, z: f32,
    heading: f32, color: &[f32; 4],
) {
    // Background bar
    painter.rect_filled(rect, round_f(2.0 * z), Color32::from_rgba_unmultiplied(20, 20, 25, 180));

    let c = arr_to_c32(color);
    let tick_c = Color32::from_rgba_unmultiplied(100, 100, 110, 200);
    let dirs = ["N", "E", "S", "W"];
    let center_x = rect.center().x;
    let w = rect.width();

    for (i, dir) in dirs.iter().enumerate() {
        let deg = i as f32 * 90.0;
        // Offset from heading, wrapped to -180..180
        let mut diff = deg - heading;
        while diff > 180.0 { diff -= 360.0; }
        while diff < -180.0 { diff += 360.0; }
        let frac = diff / 180.0; // -1..1
        let x = center_x + frac * w * 0.5;
        if x >= rect.min.x && x <= rect.max.x {
            // Tick line
            painter.line_segment(
                [Pos2::new(x, rect.max.y - 4.0 * z), Pos2::new(x, rect.max.y)],
                Stroke::new(z, tick_c),
            );
            // Direction label
            painter.text(
                Pos2::new(x, rect.center().y),
                egui::Align2::CENTER_CENTER,
                *dir,
                egui::FontId::proportional((12.0 * z).clamp(6.0, 24.0)),
                c,
            );
        }
    }

    // Center indicator triangle
    painter.line_segment(
        [Pos2::new(center_x, rect.min.y), Pos2::new(center_x, rect.min.y + 4.0 * z)],
        Stroke::new(2.0 * z, c),
    );
}

fn paint_status_effect_bar(
    painter: &egui::Painter, rect: Rect, z: f32,
    effect_count: usize, color: &[f32; 4],
) {
    let count = effect_count.max(3); // show at least 3 placeholder slots
    let slot_size = (rect.height() * 0.8).min(rect.width() / count as f32 - 2.0 * z).max(8.0);
    let spacing = 2.0 * z;
    let total_w = count as f32 * (slot_size + spacing) - spacing;
    let start_x = rect.center().x - total_w / 2.0;
    let y = rect.center().y - slot_size / 2.0;

    let c = arr_to_c32(color);
    let empty_c = Color32::from_rgba_unmultiplied(50, 50, 60, 120);

    for i in 0..count {
        let x = start_x + i as f32 * (slot_size + spacing);
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(slot_size));
        let fill = if i < effect_count { c } else { empty_c };
        painter.rect_filled(r, round_f(3.0 * z), fill);
    }
}

fn paint_notification_feed(
    painter: &egui::Painter, rect: Rect, z: f32,
    count: usize, color: &[f32; 4],
) {
    let n = count.max(1).min(5);
    let card_h = ((rect.height() - (n as f32 - 1.0) * 2.0 * z) / n as f32).max(12.0);
    let c = arr_to_c32(color);
    let pad = 4.0 * z;

    for i in 0..n {
        let y = rect.min.y + i as f32 * (card_h + 2.0 * z);
        let card_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, y),
            Vec2::new(rect.width(), card_h),
        );
        let alpha = (255 - (i as u16 * 40).min(180)) as u8;
        let bg = Color32::from_rgba_unmultiplied(40, 42, 48, alpha);
        painter.rect_filled(card_rect, round_f(3.0 * z), bg);

        // Placeholder text line
        let line_w = rect.width() * 0.6;
        let line_h = (3.0 * z).max(1.0);
        let line_rect = Rect::from_min_size(
            Pos2::new(card_rect.min.x + pad, card_rect.center().y - line_h / 2.0),
            Vec2::new(line_w, line_h),
        );
        painter.rect_filled(line_rect, round_f(1.0), Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha));
    }
}

fn paint_radial_menu(
    painter: &egui::Painter, rect: Rect, z: f32,
    item_count: usize, color: &[f32; 4],
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0 * z;
    let c = arr_to_c32(color);
    let n = item_count.max(1);

    // Background circle
    painter.circle_filled(center, radius, c);

    // Divider lines
    let line_c = Color32::from_rgba_unmultiplied(200, 200, 210, 150);
    let angle_step = std::f32::consts::TAU / n as f32;
    for i in 0..n {
        let angle = angle_step * i as f32 - std::f32::consts::FRAC_PI_2;
        let outer = Pos2::new(center.x + radius * angle.cos(), center.y + radius * angle.sin());
        let inner_r = radius * 0.3;
        let inner = Pos2::new(center.x + inner_r * angle.cos(), center.y + inner_r * angle.sin());
        painter.line_segment([inner, outer], Stroke::new(z, line_c));
    }

    // Inner circle
    painter.circle_filled(center, radius * 0.3, Color32::from_rgba_unmultiplied(30, 30, 35, 220));
}

fn paint_minimap(
    painter: &egui::Painter, rect: Rect, z: f32,
    shape: &str, bg_color: &[f32; 4], border_color: &[f32; 4],
) {
    let center = rect.center();
    let size = rect.width().min(rect.height());
    let radius = size / 2.0 - 2.0 * z;
    let bg = arr_to_c32(bg_color);
    let border = arr_to_c32(border_color);
    let grid_c = Color32::from_rgba_unmultiplied(60, 65, 60, 80);

    if shape == "Square" {
        let sq = Rect::from_center_size(center, Vec2::splat(size - 4.0 * z));
        painter.rect_filled(sq, round_f(2.0 * z), bg);
        // Grid lines
        let step = (size - 4.0 * z) / 4.0;
        for i in 1..4 {
            let offset = step * i as f32;
            painter.line_segment(
                [Pos2::new(sq.min.x + offset, sq.min.y), Pos2::new(sq.min.x + offset, sq.max.y)],
                Stroke::new(z * 0.5, grid_c),
            );
            painter.line_segment(
                [Pos2::new(sq.min.x, sq.min.y + offset), Pos2::new(sq.max.x, sq.min.y + offset)],
                Stroke::new(z * 0.5, grid_c),
            );
        }
        painter.rect_stroke(sq, round_f(2.0 * z), Stroke::new(2.0 * z, border), egui::StrokeKind::Outside);
    } else {
        // Circle
        painter.circle_filled(center, radius, bg);
        // Grid lines (horizontal + vertical through center)
        let step = radius / 2.0;
        for i in [-1.0_f32, 0.0, 1.0] {
            let offset = step * i;
            // Approximate chord-clipped lines
            let half_chord = (radius * radius - offset * offset).max(0.0).sqrt();
            painter.line_segment(
                [Pos2::new(center.x - half_chord, center.y + offset), Pos2::new(center.x + half_chord, center.y + offset)],
                Stroke::new(z * 0.5, grid_c),
            );
            painter.line_segment(
                [Pos2::new(center.x + offset, center.y - half_chord), Pos2::new(center.x + offset, center.y + half_chord)],
                Stroke::new(z * 0.5, grid_c),
            );
        }
        painter.circle_stroke(center, radius, Stroke::new(2.0 * z, border));
    }

    // Player indicator dot at center
    painter.circle_filled(center, (3.0 * z).max(1.5), Color32::from_rgb(60, 180, 255));
}

// ── Shape widget painters ──────────────────────────────────────────────────

fn paint_shape_circle(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], stroke_color: &[f32; 4], stroke_width: f32,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 1.0;
    let c = arr_to_c32(color);

    painter.circle_filled(center, radius, c);

    if stroke_width > 0.0 {
        let sc = arr_to_c32(stroke_color);
        painter.circle_stroke(center, radius, Stroke::new(stroke_width * z, sc));
    }
}

fn paint_shape_arc(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], start_angle: f32, end_angle: f32,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0 * z;
    let c = arr_to_c32(color);
    let sw = (3.0 * z).max(1.0);

    let start_rad = start_angle.to_radians();
    let end_rad = end_angle.to_radians();
    let segments = 32;
    let sweep = end_rad - start_rad;

    for i in 0..segments {
        let a0 = start_rad + sweep * (i as f32 / segments as f32);
        let a1 = start_rad + sweep * ((i + 1) as f32 / segments as f32);
        let p0 = Pos2::new(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        painter.line_segment([p0, p1], Stroke::new(sw, c));
    }
}

fn paint_shape_triangle(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], stroke_color: &[f32; 4],
) {
    let center = rect.center();
    let half_w = rect.width() / 2.0 - 2.0 * z;
    let half_h = rect.height() / 2.0 - 2.0 * z;
    let c = arr_to_c32(color);

    let top = Pos2::new(center.x, center.y - half_h);
    let bl = Pos2::new(center.x - half_w, center.y + half_h);
    let br = Pos2::new(center.x + half_w, center.y + half_h);

    // Filled triangle via PathShape
    let path = egui::epaint::PathShape::convex_polygon(vec![top, bl, br], c, Stroke::NONE);
    painter.add(path);

    let sc = arr_to_c32(stroke_color);
    if sc.a() > 0 {
        painter.line_segment([top, bl], Stroke::new(z, sc));
        painter.line_segment([bl, br], Stroke::new(z, sc));
        painter.line_segment([br, top], Stroke::new(z, sc));
    }
}

fn paint_shape_line(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], thickness: f32,
) {
    let c = arr_to_c32(color);
    let sw = (thickness * z).max(1.0);
    // Diagonal from top-left to bottom-right
    painter.line_segment(
        [rect.left_top(), rect.right_bottom()],
        Stroke::new(sw, c),
    );
}

fn paint_shape_polygon(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], stroke_color: &[f32; 4], sides: u32,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0 * z;
    let c = arr_to_c32(color);
    let n = sides.max(3) as usize;

    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let angle = std::f32::consts::TAU * i as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
        points.push(Pos2::new(center.x + radius * angle.cos(), center.y + radius * angle.sin()));
    }

    let path = egui::epaint::PathShape::convex_polygon(points.clone(), c, Stroke::NONE);
    painter.add(path);

    let sc = arr_to_c32(stroke_color);
    if sc.a() > 0 {
        for i in 0..n {
            painter.line_segment([points[i], points[(i + 1) % n]], Stroke::new(z, sc));
        }
    }
}

fn paint_shape_wedge(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], start_angle: f32, end_angle: f32,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0 * z;
    let c = arr_to_c32(color);

    let start_rad = start_angle.to_radians();
    let end_rad = end_angle.to_radians();
    let segments = 24;
    let sweep = end_rad - start_rad;

    let mut points = Vec::with_capacity(segments + 2);
    points.push(center);
    for i in 0..=segments {
        let a = start_rad + sweep * (i as f32 / segments as f32);
        points.push(Pos2::new(center.x + radius * a.cos(), center.y + radius * a.sin()));
    }

    let path = egui::epaint::PathShape::convex_polygon(points, c, Stroke::NONE);
    painter.add(path);
}

fn paint_shape_radial_progress(
    painter: &egui::Painter, rect: Rect, z: f32,
    color: &[f32; 4], track_color: &[f32; 4], value: f32,
) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 2.0 * z;
    let sw = (4.0 * z).max(2.0);

    // Track (full circle)
    let tc = arr_to_c32(track_color);
    painter.circle_stroke(center, radius, Stroke::new(sw, tc));

    // Filled arc
    let c = arr_to_c32(color);
    let start_angle = -std::f32::consts::FRAC_PI_2; // top
    let sweep = std::f32::consts::TAU * value.clamp(0.0, 1.0);
    let segments = 32;

    for i in 0..segments {
        let frac0 = i as f32 / segments as f32;
        let frac1 = (i + 1) as f32 / segments as f32;
        if frac0 >= value { break; }
        let a0 = start_angle + sweep * frac0;
        let a1 = start_angle + sweep * (frac1.min(value));
        let p0 = Pos2::new(center.x + radius * a0.cos(), center.y + radius * a0.sin());
        let p1 = Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        painter.line_segment([p0, p1], Stroke::new(sw, c));
    }
}

// ── Menu widget painters ────────────────────────────────────────────────────

fn paint_inventory_grid(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    columns: u32, rows: u32, slot_size: f32,
    slot_bg_color: &[f32; 4], slot_border_color: &[f32; 4],
) {
    let bg = if ws.has_bg { arr_to_c32(&ws.bg_color) } else { Color32::from_rgb(30, 30, 35) };
    painter.rect_filled(rect, round_f(4.0 * z), bg);

    let slot = (slot_size * z).max(8.0);
    let gap = (4.0 * z).max(1.0);
    let pad = 6.0 * z;
    let slot_bg = arr_to_c32(slot_bg_color);
    let slot_border = arr_to_c32(slot_border_color);
    let slot_round = round_f(3.0 * z);

    for row in 0..rows {
        for col in 0..columns {
            let x = rect.min.x + pad + (slot + gap) * col as f32;
            let y = rect.min.y + pad + (slot + gap) * row as f32;
            if x + slot > rect.max.x || y + slot > rect.max.y { continue; }
            let slot_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(slot));
            painter.rect_filled(slot_rect, slot_round, slot_bg);
            painter.rect_stroke(slot_rect, slot_round, Stroke::new(z, slot_border), egui::StrokeKind::Outside);
        }
    }
}

fn paint_dialog_box(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    speaker: &str, text: &str,
    bg_color: &[f32; 4], speaker_color: &[f32; 4],
) {
    painter.rect_filled(rect, rounding, arr_to_c32(bg_color));
    painter.rect_stroke(rect, rounding, Stroke::new(z, Color32::from_rgb(70, 70, 80)), egui::StrokeKind::Outside);

    let pad = 10.0 * z;

    if !speaker.is_empty() {
        painter.text(
            Pos2::new(rect.min.x + pad, rect.min.y + pad),
            egui::Align2::LEFT_TOP,
            speaker,
            egui::FontId::proportional((13.0 * z).clamp(6.0, 26.0)),
            arr_to_c32(speaker_color),
        );
    }

    if !text.is_empty() {
        let text_y = rect.min.y + pad + 18.0 * z;
        let tc = arr_to_c32(&ws.text_color);
        painter.text(
            Pos2::new(rect.min.x + pad, text_y),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional((ws.text_size * z).clamp(6.0, 28.0)),
            tc,
        );
    }
}

fn paint_objective_tracker(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    title: &str, objective_count: usize, title_color: &[f32; 4],
) {
    let bg = if ws.has_bg { arr_to_c32(&ws.bg_color) } else { Color32::from_rgba_unmultiplied(20, 20, 25, 180) };
    painter.rect_filled(rect, rounding, bg);

    let pad = 8.0 * z;

    painter.text(
        Pos2::new(rect.min.x + pad, rect.min.y + pad),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional((13.0 * z).clamp(6.0, 26.0)),
        arr_to_c32(title_color),
    );

    let line_h = (16.0 * z).max(8.0);
    let bullet_y_start = rect.min.y + pad + 20.0 * z;
    let tc = arr_to_c32(&ws.text_color);
    let display_count = objective_count.min(6);
    for i in 0..display_count {
        let y = bullet_y_start + line_h * i as f32;
        if y + line_h > rect.max.y { break; }
        let bullet_r = (2.5 * z).max(1.0);
        painter.circle_filled(
            Pos2::new(rect.min.x + pad + bullet_r, y + line_h / 2.0),
            bullet_r, tc,
        );
        painter.text(
            Pos2::new(rect.min.x + pad + bullet_r * 2.0 + 6.0 * z, y + line_h / 2.0),
            egui::Align2::LEFT_CENTER,
            &format!("Objective {}", i + 1),
            egui::FontId::proportional((11.0 * z).clamp(6.0, 22.0)),
            tc,
        );
    }
}

fn paint_loading_screen(
    painter: &egui::Painter, rect: Rect, z: f32,
    progress: f32, message: &str,
    bar_color: &[f32; 4], bg_color: &[f32; 4],
) {
    painter.rect_filled(rect, 0.0, arr_to_c32(bg_color));

    let center = rect.center();

    if !message.is_empty() {
        painter.text(
            Pos2::new(center.x, center.y - 20.0 * z),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional((14.0 * z).clamp(6.0, 28.0)),
            Color32::WHITE,
        );
    }

    let bar_w = (rect.width() * 0.6).max(40.0);
    let bar_h = (8.0 * z).max(3.0);
    let bar_x = center.x - bar_w / 2.0;
    let bar_y = center.y + 4.0 * z;
    let bar_rect = Rect::from_min_size(Pos2::new(bar_x, bar_y), Vec2::new(bar_w, bar_h));
    let bar_round = round_f(bar_h / 2.0);

    painter.rect_filled(bar_rect, bar_round, Color32::from_rgb(50, 50, 55));

    let fill_w = bar_w * progress.clamp(0.0, 1.0);
    if fill_w > 0.5 {
        let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, bar_h));
        painter.rect_filled(fill_rect, bar_round, arr_to_c32(bar_color));
    }
}

fn paint_keybind_row(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    action: &str, binding: &str, key_bg_color: &[f32; 4],
) {
    let pad = 8.0 * z;
    let tc = arr_to_c32(&ws.text_color);

    painter.text(
        Pos2::new(rect.min.x + pad, rect.center().y),
        egui::Align2::LEFT_CENTER,
        action,
        egui::FontId::proportional((ws.text_size * z).clamp(6.0, 28.0)),
        tc,
    );

    let key_font = egui::FontId::proportional((11.0 * z).clamp(6.0, 22.0));
    let key_galley = painter.layout_no_wrap(binding.to_string(), key_font.clone(), tc);
    let key_w = key_galley.size().x + 12.0 * z;
    let key_h = (22.0 * z).max(12.0);
    let key_x = rect.max.x - pad - key_w;
    let key_y = rect.center().y - key_h / 2.0;
    let key_rect = Rect::from_min_size(Pos2::new(key_x, key_y), Vec2::new(key_w, key_h));
    let key_round = round_f(4.0 * z);

    painter.rect_filled(key_rect, key_round, arr_to_c32(key_bg_color));
    painter.rect_stroke(key_rect, key_round, Stroke::new(z, Color32::from_rgb(90, 90, 100)), egui::StrokeKind::Outside);
    painter.text(
        key_rect.center(),
        egui::Align2::CENTER_CENTER,
        binding,
        key_font,
        tc,
    );
}

fn paint_settings_row(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32,
    label: &str, value: &str,
) {
    let pad = 8.0 * z;
    let tc = arr_to_c32(&ws.text_color);

    painter.text(
        Pos2::new(rect.min.x + pad, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional((ws.text_size * z).clamp(6.0, 28.0)),
        tc,
    );

    painter.text(
        Pos2::new(rect.max.x - pad, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        value,
        egui::FontId::proportional((ws.text_size * z).clamp(6.0, 28.0)),
        Color32::from_rgb(160, 160, 170),
    );
}

// ── Extra widget painters ───────────────────────────────────────────────────

fn paint_separator(
    painter: &egui::Painter, rect: Rect, z: f32,
    horizontal: bool, color: &[f32; 4], thickness: f32,
) {
    let c = arr_to_c32(color);
    let t = (thickness * z).max(1.0);

    if horizontal {
        let y = rect.center().y;
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            Stroke::new(t, c),
        );
    } else {
        let x = rect.center().x;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(t, c),
        );
    }
}

fn paint_number_input(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    value: f64, precision: u32,
    bg_color: &[f32; 4], button_color: &[f32; 4],
) {
    painter.rect_filled(rect, rounding, arr_to_c32(bg_color));
    painter.rect_stroke(rect, rounding, Stroke::new(z, Color32::from_rgb(80, 80, 90)), egui::StrokeKind::Outside);

    let btn_w = (28.0 * z).max(14.0);
    let btn_c = arr_to_c32(button_color);
    let tc = arr_to_c32(&ws.text_color);
    let btn_font = egui::FontId::proportional((14.0 * z).clamp(6.0, 28.0));
    let val_font = egui::FontId::proportional((ws.text_size * z).clamp(6.0, 28.0));

    let left_rect = Rect::from_min_size(rect.min, Vec2::new(btn_w, rect.height()));
    let left_round = egui::Rounding { nw: rounding.nw, ne: 0, se: 0, sw: rounding.sw };
    painter.rect_filled(left_rect, left_round, btn_c);
    painter.text(left_rect.center(), egui::Align2::CENTER_CENTER, "\u{2212}", btn_font.clone(), tc);

    let right_rect = Rect::from_min_max(
        Pos2::new(rect.max.x - btn_w, rect.min.y),
        rect.max,
    );
    let right_round = egui::Rounding { nw: 0, ne: rounding.ne, se: rounding.se, sw: 0 };
    painter.rect_filled(right_rect, right_round, btn_c);
    painter.text(right_rect.center(), egui::Align2::CENTER_CENTER, "+", btn_font, tc);

    let value_text = format!("{:.prec$}", value, prec = precision as usize);
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, &value_text, val_font, tc);
}

fn paint_vertical_slider(
    painter: &egui::Painter, rect: Rect, z: f32,
    value: f32, min: f32, max: f32,
    track_color: &[f32; 4], fill_color: &[f32; 4], thumb_color: &[f32; 4],
) {
    let track_w = (6.0 * z).max(2.0);
    let track_x = rect.center().x - track_w / 2.0;
    let track_rect = Rect::from_min_size(
        Pos2::new(track_x, rect.min.y),
        Vec2::new(track_w, rect.height()),
    );
    let track_round = round_f(track_w / 2.0);

    painter.rect_filled(track_rect, track_round, arr_to_c32(track_color));

    let ratio = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
    let fill_h = rect.height() * ratio;
    if fill_h > 0.5 {
        let fill_rect = Rect::from_min_size(
            Pos2::new(track_x, rect.max.y - fill_h),
            Vec2::new(track_w, fill_h),
        );
        painter.rect_filled(fill_rect, track_round, arr_to_c32(fill_color));
    }

    let thumb_r = (8.0 * z).max(3.0);
    let thumb_y = rect.max.y - fill_h;
    painter.circle_filled(
        Pos2::new(rect.center().x, thumb_y),
        thumb_r,
        arr_to_c32(thumb_color),
    );
}

fn paint_scrollbar(
    painter: &egui::Painter, rect: Rect, z: f32,
    vertical: bool, viewport_fraction: f32, position: f32,
    track_color: &[f32; 4], thumb_color: &[f32; 4],
) {
    let track_round = round_f(3.0 * z);

    painter.rect_filled(rect, track_round, arr_to_c32(track_color));

    let vf = viewport_fraction.clamp(0.05, 1.0);
    let pos = position.clamp(0.0, 1.0);

    if vertical {
        let thumb_h = rect.height() * vf;
        let available = rect.height() - thumb_h;
        let thumb_y = rect.min.y + available * pos;
        let thumb_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, thumb_y),
            Vec2::new(rect.width(), thumb_h),
        );
        painter.rect_filled(thumb_rect, track_round, arr_to_c32(thumb_color));
    } else {
        let thumb_w = rect.width() * vf;
        let available = rect.width() - thumb_w;
        let thumb_x = rect.min.x + available * pos;
        let thumb_rect = Rect::from_min_size(
            Pos2::new(thumb_x, rect.min.y),
            Vec2::new(thumb_w, rect.height()),
        );
        painter.rect_filled(thumb_rect, track_round, arr_to_c32(thumb_color));
    }
}

fn paint_list_widget(
    painter: &egui::Painter, ws: &WidgetSnapshot, rect: Rect, z: f32, rounding: egui::Rounding,
    item_count: usize, bg_color: &[f32; 4], selected_bg_color: &[f32; 4], item_height: f32,
) {
    painter.rect_filled(rect, rounding, arr_to_c32(bg_color));

    let row_h = (item_height * z).max(10.0);
    let pad = 8.0 * z;
    let tc = arr_to_c32(&ws.text_color);
    let sel_bg = arr_to_c32(selected_bg_color);
    let display_count = item_count.min(20);

    for i in 0..display_count {
        let y = rect.min.y + row_h * i as f32;
        if y + row_h > rect.max.y { break; }
        let row_rect = Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(rect.width(), row_h));

        if i == 0 {
            painter.rect_filled(row_rect, 0.0, sel_bg);
        }

        painter.text(
            Pos2::new(row_rect.min.x + pad, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &format!("Item {}", i + 1),
            egui::FontId::proportional((11.0 * z).clamp(6.0, 22.0)),
            tc,
        );
    }

    painter.rect_stroke(rect, rounding, Stroke::new(z, Color32::from_rgb(70, 70, 80)), egui::StrokeKind::Outside);
}
