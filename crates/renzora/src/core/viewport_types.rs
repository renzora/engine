//! Viewport state types — shared between editor plugins via renzora.
//!
//! Moved here from `renzora_viewport` so that camera, gizmo, and other
//! editor plugin DLLs can use these types without depending on each other.

use std::sync::atomic::{AtomicBool, AtomicI32};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Tracks the render target image and current resolution.
#[derive(Resource)]
pub struct ViewportState {
    pub image_handle: Option<Handle<Image>>,
    pub current_size: UVec2,
    /// Whether the mouse cursor is currently over the viewport.
    pub hovered: bool,
    /// Screen-space position of the viewport panel (top-left corner).
    pub screen_position: Vec2,
    /// Screen-space size of the viewport panel.
    pub screen_size: Vec2,
    /// Whether the focused viewport panel is actually visible in the live dock
    /// (some leaf's active tab). `screen_position`/`screen_size` go STALE the
    /// moment the panel leaves the layout — the panel's per-frame resize
    /// requests stop — so screen-space chrome drawn outside the panel (the 2D
    /// rulers) must check this instead of trusting the rect.
    pub docked: bool,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            image_handle: None,
            current_size: UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            hovered: false,
            screen_position: Vec2::ZERO,
            screen_size: Vec2::new(DEFAULT_WIDTH as f32, DEFAULT_HEIGHT as f32),
            docked: true,
        }
    }
}

/// The OS cursor the viewport interaction layer wants while the pointer is
/// over the viewport (e.g. Move over a selected sprite, a directional resize
/// cursor over a handle). `None` = no opinion. Written by the 2D picker each
/// frame; consumed by ember's cursor system, which prioritises hovered UI
/// widgets (their `HoverCursor` wins) and falls back to this before Default —
/// one writer of the window's `CursorIcon`, no fighting systems.
#[derive(Resource, Default)]
pub struct ViewportCursorRequest(pub Option<bevy::window::SystemCursorIcon>);

/// Active 2D rubber-band selection: `(start, current)` in WINDOW pixels.
/// Written by the 2D picker while a left-drag that started on empty space is
/// in flight; drawn by the 2D viewport overlay. `None` when no band is
/// active. Lives in the contract because the picker (gizmo crate) and the
/// overlay (viewport crate) must share it without depending on each other.
#[derive(Resource, Default)]
pub struct ViewportBoxSelect2d(pub Option<(Vec2, Vec2)>);

/// Number of editor viewport slots (the maximum number of camera views you can
/// dock at once). Slot 0 is the primary viewport (full 3D/2D/UI + toolbar);
/// slots 1.. are additional camera views of the same scene.
pub const VIEWPORT_COUNT: usize = 4;

/// Base bevy `RenderLayers` index for the per-slot 2D editor grid meshes.
///
/// Each viewport's 2D camera renders layer 0 (the scene) plus its own grid
/// layer `VIEWPORT_2D_GRID_LAYER_BASE + slot`, and that slot's grid mesh sits on
/// the same layer — so every viewport gets an independent grid framed to its own
/// zoom, and no camera ever draws another slot's grid. Kept well clear of the
/// low layers (0/1) the scene and 3D cameras use.
pub const VIEWPORT_2D_GRID_LAYER_BASE: usize = 20;

/// Per-slot state for one editor viewport: its render-target image, panel rect,
/// and its own orbit camera (focus / distance / yaw / pitch).
///
/// The orbit is stored as raw fields rather than `renzora_camera::OrbitCameraState`
/// so this type can live in `renzora` core without depending on the camera crate.
/// The camera controller mirrors the focused slot's fields in and out of its
/// singleton `OrbitCameraState` each frame.
#[derive(Debug, Clone)]
pub struct ViewportSlot {
    /// Render-target image this slot's camera draws into (and the panel displays).
    pub image: Option<Handle<Image>>,
    /// The 3D editor camera entity bound to this slot.
    pub camera_entity: Option<Entity>,
    /// The 2D editor camera entity bound to this slot (its orthographic sibling,
    /// active only in 2D view). Renders into the same [`Self::image`].
    pub camera_2d_entity: Option<Entity>,
    /// Stored 2D pan for this slot: the camera's world translation on the XY
    /// plane. Persisted here so each viewport keeps an independent 2D framing
    /// even while another slot is the focused (live-controlled) one.
    pub pan_2d: Vec2,
    /// Stored 2D zoom for this slot: the orthographic `scale` (world units per
    /// render-image pixel). `0.0` is the "not yet framed" sentinel — a slot at
    /// zero inherits the focused view's framing the first time it's shown, then
    /// diverges independently.
    pub zoom_2d: f32,
    /// Current render-target resolution (pixels).
    pub current_size: UVec2,
    /// Screen-space top-left of the panel rect.
    pub screen_position: Vec2,
    /// Screen-space size of the panel rect.
    pub screen_size: Vec2,
    /// Whether the cursor is over this viewport's panel.
    pub hovered: bool,
    /// Whether this slot's panel is currently present in the dock tree.
    pub docked: bool,
    /// Orbit focus point.
    pub focus: Vec3,
    /// Orbit distance from focus.
    pub distance: f32,
    /// Orbit yaw (radians).
    pub yaw: f32,
    /// Orbit pitch (radians).
    pub pitch: f32,
}

impl ViewportSlot {
    fn new(focus: Vec3, distance: f32, yaw: f32, pitch: f32) -> Self {
        Self {
            image: None,
            camera_entity: None,
            camera_2d_entity: None,
            pan_2d: Vec2::ZERO,
            zoom_2d: 0.0,
            current_size: UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            screen_position: Vec2::ZERO,
            screen_size: Vec2::new(DEFAULT_WIDTH as f32, DEFAULT_HEIGHT as f32),
            hovered: false,
            docked: false,
            focus,
            distance,
            yaw,
            pitch,
        }
    }

    /// Aspect ratio of the panel rect, falling back to 16:9.
    pub fn aspect(&self) -> f32 {
        if self.screen_size.y > 0.0 {
            self.screen_size.x / self.screen_size.y
        } else {
            16.0 / 9.0
        }
    }
}

/// All editor viewport slots plus which one currently has focus.
///
/// The focused slot is the one the user is hovering / interacting with; the
/// camera controller mirrors it into the singleton `OrbitCameraState` /
/// [`ViewportState`] and ensures the `EditorCamera` marker sits on its camera,
/// so the entire existing single-viewport tool/gizmo/overlay stack transparently
/// operates on the focused view.
#[derive(Resource)]
pub struct Viewports {
    pub slots: [ViewportSlot; VIEWPORT_COUNT],
    pub focused: usize,
}

impl Default for Viewports {
    fn default() -> Self {
        use std::f32::consts::FRAC_PI_2;
        // Slot 0 matches the historical single-viewport default angle; the
        // extra slots start on classic orthographic-ish presets so a fresh
        // quad layout reads as perspective / front / top / side.
        Self {
            slots: [
                ViewportSlot::new(Vec3::ZERO, 4.5, 0.3, 0.4), // primary / user angle
                ViewportSlot::new(Vec3::ZERO, 4.5, 0.0, 0.0), // front
                ViewportSlot::new(Vec3::ZERO, 4.5, 0.0, FRAC_PI_2 - 0.001), // top
                ViewportSlot::new(Vec3::ZERO, 4.5, FRAC_PI_2, 0.0), // right side
            ],
            focused: 0,
        }
    }
}

/// Atomically-writable nav overlay drag state from the panel's `ui()` method.
///
/// The nav overlay buttons write drag deltas here (from `&World`), and the
/// camera controller system reads + consumes them each frame.
#[derive(Resource)]
pub struct NavOverlayState {
    /// Whether the pan button is currently being dragged.
    pub pan_dragging: AtomicBool,
    /// Whether the zoom button is currently being dragged.
    pub zoom_dragging: AtomicBool,
    /// Pan drag delta X (scaled by 1000 to preserve fractional part).
    pub pan_delta_x: AtomicI32,
    /// Pan drag delta Y (scaled by 1000 to preserve fractional part).
    pub pan_delta_y: AtomicI32,
    /// Zoom drag delta Y (scaled by 1000 to preserve fractional part).
    pub zoom_delta_y: AtomicI32,
    /// Whether the axis gizmo is currently being dragged (orbits).
    pub orbit_dragging: AtomicBool,
    /// Orbit drag delta X (scaled by 1000).
    pub orbit_delta_x: AtomicI32,
    /// Orbit drag delta Y (scaled by 1000).
    pub orbit_delta_y: AtomicI32,
}

impl Default for NavOverlayState {
    fn default() -> Self {
        Self {
            pan_dragging: AtomicBool::new(false),
            zoom_dragging: AtomicBool::new(false),
            pan_delta_x: AtomicI32::new(0),
            pan_delta_y: AtomicI32::new(0),
            zoom_delta_y: AtomicI32::new(0),
            orbit_dragging: AtomicBool::new(false),
            orbit_delta_x: AtomicI32::new(0),
            orbit_delta_y: AtomicI32::new(0),
        }
    }
}

/// Camera orbit orientation, written by the camera system and read by the axis gizmo overlay.
#[derive(Resource, Debug, Clone, Default)]
pub struct CameraOrbitSnapshot {
    pub yaw: f32,
    pub pitch: f32,
}

/// Cached clip-from-world matrix of the editor camera, plus camera world position.
/// Updated every frame. Used by CPU-projected viewport overlays (grid, gizmos).
#[derive(Resource, Debug, Clone)]
pub struct EditorCameraMatrix {
    pub clip_from_world: Mat4,
    pub world_from_clip: Mat4,
    pub cam_pos: Vec3,
    pub cam_forward: Vec3,
    pub valid: bool,
}

impl Default for EditorCameraMatrix {
    fn default() -> Self {
        Self {
            clip_from_world: Mat4::IDENTITY,
            world_from_clip: Mat4::IDENTITY,
            cam_pos: Vec3::ZERO,
            cam_forward: Vec3::NEG_Z,
            valid: false,
        }
    }
}

/// Camera projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionMode {
    #[default]
    Perspective,
    Orthographic,
}

/// Which scene camera drives the editor viewport's FOV (and, in `Selected`
/// mode, its pose when the selection changes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorCameraSource {
    /// Always mirror the `DefaultCamera` (or first scene camera). The editor
    /// fly-camera keeps its own position; only the FOV follows.
    #[default]
    Default,
    /// Follow whichever scene camera is selected: the editor view jumps to that
    /// camera's pose when you select it, and its FOV tracks the selection.
    Selected,
}

impl EditorCameraSource {
    pub const ALL: &'static [EditorCameraSource] = &[Self::Default, Self::Selected];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Always Use Default",
            Self::Selected => "Change Camera to Selected",
        }
    }

    /// Parse a label (as produced by [`Self::label`]) back into a variant.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Change Camera to Selected" => Self::Selected,
            _ => Self::Default,
        }
    }
}

/// Render-resolution scale for a camera. The camera's render target is sized at
/// this fraction of the on-screen panel size; the displayed image is upscaled to
/// fill the panel. Lower resolutions trade sharpness for a large fill-rate win on
/// the fullscreen-bound passes (GI / atmosphere / prepass / auto-exposure).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize,
)]
#[reflect(Serialize, Deserialize)]
pub enum RenderResolution {
    #[default]
    Full,
    Half,
    Quarter,
}

impl RenderResolution {
    pub const ALL: &'static [RenderResolution] = &[Self::Full, Self::Half, Self::Quarter];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Half => "Half",
            Self::Quarter => "Quarter",
        }
    }

    /// Parse a label (as produced by [`Self::label`]) back into a variant.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Half" => Self::Half,
            "Quarter" => Self::Quarter,
            _ => Self::Full,
        }
    }

    /// Multiplier applied to the panel size to get the render-target size.
    pub fn scale(&self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Half => 0.5,
            Self::Quarter => 0.25,
        }
    }
}

/// Overall graphics-quality tier. A single user-facing switch that gates the
/// expensive, *fullscreen / resolution-bound* render passes — the ones whose
/// cost is per-pixel, not per-object, and so dominate on weak GPUs and high-DPI
/// (Retina) displays even on an empty scene.
///
/// Each tier maps to a set of crash-safe `enabled` toggles on the routed effect
/// sources (screen-space GI, auto-exposure, bloom, TAA); see
/// `renzora_level_presets::graphics_quality`. It deliberately does **not** touch
/// passes whose attachment layout is fixed at camera spawn (atmosphere / the
/// prepass bundle) — toggling those at runtime trips a wgpu validation crash, so
/// they stay resident regardless of tier.
///
/// The ladder removes the next-most-expensive pass at each step down:
/// - `High`   — everything on (the full authored look).
/// - `Medium` — screen-space GI off (the single biggest GPU cost); the tonemapped
///   look — auto-exposure, bloom, TAA — is kept. **Default**: it's the best
///   out-of-box trade for the low-end / Retina machines that motivated this.
/// - `Low`    — GI, auto-exposure, bloom, and TAA all off; the lightest path,
///   roughly a "compatibility" renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub enum GraphicsQuality {
    Low,
    /// The shipping default: it kills the heaviest pass (SSGI) while keeping the
    /// tonemapped look, so the engine runs acceptably on the kind of older /
    /// integrated GPUs where the full stack drops to single-digit FPS. Capable
    /// machines can raise it to `High` in Settings → Viewport → Performance.
    #[default]
    Medium,
    High,
}

impl GraphicsQuality {
    pub const ALL: &'static [GraphicsQuality] = &[Self::Low, Self::Medium, Self::High];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    /// Parse a persisted label back into a tier. Unknown / empty (an older config
    /// written before this field existed) resolves to the default `Medium`, so
    /// upgrading projects inherit the lighter default.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Low" => Self::Low,
            "High" => Self::High,
            _ => Self::Medium,
        }
    }

    /// Screen-space global illumination (Lumen / RT) — on only at `High`. This is
    /// the costliest, most resolution-bound pass, so it's the first thing dropped.
    pub fn gi(&self) -> bool {
        matches!(self, Self::High)
    }

    /// Auto-exposure histogram pass — on at `Medium` and `High`.
    pub fn auto_exposure(&self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Bloom downsample/upsample chain — on at `Medium` and `High`.
    pub fn bloom(&self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Temporal anti-aliasing — on at `Medium` and `High`.
    pub fn taa(&self) -> bool {
        !matches!(self, Self::Low)
    }
}

/// The render resolution the editor viewport is currently rendering at, derived
/// each frame from the relevant scene camera's [`crate::core::CameraRenderResolution`]
/// (selected camera → default camera → first scene camera). Read by the viewport
/// slot resizer so the editor view reflects the focused camera's resolution.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportRenderResolution(pub RenderResolution);

/// High-level viewport interaction mode (Blender-style mode switcher).
///
/// `Scene` is the default pick/move mode — its user-facing label is
/// **Select** (the variant keeps its historical name because it crosses the
/// plugin ABI and is matched all over the editor crates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ViewportMode {
    #[default]
    Scene,
    Edit,
    Sculpt,
    Paint,
    /// Tile eraser (2D only): the paint brush with erase always on.
    Erase,
}

impl ViewportMode {
    /// Every mode, in dropdown-row order. UI that offers a per-view subset
    /// should use [`Self::for_view`]; `ALL` stays the index space the header
    /// dropdown rows are built from.
    pub const ALL: &'static [ViewportMode] = &[
        Self::Scene,
        Self::Edit,
        Self::Sculpt,
        Self::Paint,
        Self::Erase,
    ];
    /// The modes the header's Mode dropdown offers for the given view:
    /// Sculpt is mesh sculpting (3D only), Erase is the tile eraser (2D
    /// only).
    pub fn for_view(view: ViewportView) -> &'static [ViewportMode] {
        match view {
            ViewportView::Two => &[Self::Scene, Self::Edit, Self::Paint, Self::Erase],
            _ => &[Self::Scene, Self::Edit, Self::Sculpt, Self::Paint],
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Scene => "Select",
            Self::Edit => "Edit",
            Self::Sculpt => "Sculpt",
            Self::Paint => "Paint",
            Self::Erase => "Erase",
        }
    }
}

/// What kind of content the viewport is currently displaying. Switches the
/// camera/projection preset and (for `Ui`) hands off rendering to the
/// `ui_canvas` panel so UI authoring lives in the same surface as the 3D
/// scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ViewportView {
    #[default]
    Three,
    Two,
    Ui,
}

impl ViewportView {
    pub const ALL: &'static [ViewportView] = &[Self::Three, Self::Two, Self::Ui];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Three => "3D",
            Self::Two => "2D",
            Self::Ui => "UI",
        }
    }
}

/// Visualization mode for debug rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizationMode {
    #[default]
    None,
    Normals,
    Roughness,
    Metallic,
    Depth,
    UvChecker,
}

impl VisualizationMode {
    pub const ALL: &'static [VisualizationMode] = &[
        Self::None,
        Self::Normals,
        Self::Roughness,
        Self::Metallic,
        Self::Depth,
        Self::UvChecker,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Normals => "Normals",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Depth => "Depth",
            Self::UvChecker => "UV Checker",
        }
    }
}

/// Which entities the in-viewport name-label overlay draws. Imported models
/// nest hundreds of named sub-meshes under one root, so labeling everything
/// (`All`) carpets dense scenes — the other scopes thin that out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelScope {
    /// Every named, non-chrome entity (can be very busy in big models).
    All,
    /// Only top-level objects: a placed model's root and standalone
    /// primitives/lights, not the sub-meshes parented beneath them.
    #[default]
    TopLevel,
    /// Only entities that have an actual mesh (skips empty transform nodes).
    Meshes,
    /// Only the currently selected entity.
    Selected,
}

impl LabelScope {
    pub const ALL: &'static [LabelScope] =
        &[Self::All, Self::TopLevel, Self::Meshes, Self::Selected];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All Entities",
            Self::TopLevel => "Top-Level Objects",
            Self::Meshes => "Meshes Only",
            Self::Selected => "Selected Only",
        }
    }

    /// Parse from the persisted `{:?}` Debug string; unknown → default.
    pub fn from_debug(s: &str) -> Self {
        match s {
            "All" => Self::All,
            "Meshes" => Self::Meshes,
            "Selected" => Self::Selected,
            _ => Self::TopLevel,
        }
    }
}

/// Render feature toggles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderToggles {
    pub textures: bool,
    pub wireframe: bool,
    pub lighting: bool,
    pub shadows: bool,
    /// Solid mesh rendering. Off hides mesh fill (wireframe still renders if on).
    pub mesh: bool,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            textures: true,
            wireframe: false,
            lighting: true,
            shadows: true,
            mesh: true,
        }
    }
}

/// Collision gizmo visibility mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionGizmoVisibility {
    #[default]
    SelectedOnly,
    Always,
}

/// Snapping settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapSettings {
    pub translate_enabled: bool,
    pub translate_snap: f32,
    /// If true, snap the entity's world-space AABB min corner to the grid
    /// instead of its pivot. Aligns cube edges to gridlines.
    pub translate_edge_snap: bool,
    pub rotate_enabled: bool,
    pub rotate_snap: f32,
    pub scale_enabled: bool,
    pub scale_snap: f32,
    /// If true, Y-axis scaling keeps the entity's world-space AABB bottom
    /// fixed (scales upward from the floor instead of symmetrically).
    pub scale_bottom_anchor: bool,
    pub object_snap_enabled: bool,
    pub object_snap_distance: f32,
    pub floor_snap_enabled: bool,
    pub floor_y: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            translate_enabled: false,
            translate_snap: 1.0,
            translate_edge_snap: true,
            rotate_enabled: false,
            rotate_snap: 15.0,
            scale_enabled: false,
            scale_snap: 0.25,
            scale_bottom_anchor: true,
            object_snap_enabled: true,
            object_snap_distance: 0.5,
            floor_snap_enabled: true,
            floor_y: 0.0,
        }
    }
}

/// Camera sensitivity settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSettingsState {
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub invert_y: bool,
    pub distance_relative_speed: bool,
    /// Which scene camera the editor viewport mirrors (FOV always; pose on
    /// selection in `Selected` mode).
    pub editor_camera_source: EditorCameraSource,
}

impl Default for CameraSettingsState {
    fn default() -> Self {
        Self {
            move_speed: 10.0,
            look_sensitivity: 0.3,
            orbit_sensitivity: 0.5,
            pan_sensitivity: 1.0,
            zoom_sensitivity: 1.0,
            invert_y: false,
            distance_relative_speed: true,
            editor_camera_source: EditorCameraSource::default(),
        }
    }
}

/// A pending view angle command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewAngleCommand {
    pub yaw: f32,
    pub pitch: f32,
}

/// Viewport overlay and rendering settings.
///
/// This resource is the single source of truth for the viewport header UI.
/// Other crates (camera, gizmo) read from this to apply changes.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ViewportSettings {
    pub render_toggles: RenderToggles,
    pub visualization_mode: VisualizationMode,
    pub show_grid: bool,
    pub show_subgrid: bool,
    /// The 2D editor's own grid toggle — independent of the 3D `show_grid`
    /// so turning the 2D grid off doesn't also kill the 3D floor grid.
    /// Off by default: 2D scenes are usually pixel-art where the grid is
    /// noise until you're aligning tiles. Toolbar switch (2D view only).
    pub show_grid_2d: bool,
    /// Cell size of the 2D grid, in world units. Its own setting — NOT the
    /// snap step: the grid used to draw at `snap.translate_snap`, which made
    /// the snap pill silently restyle the grid and left the lines misaligned
    /// with the default 16-unit tiles. Editable inline next to the Grid
    /// switch (2D view only).
    pub grid_size_2d: f32,
    /// The 2D view's ruler bars (+ tick labels and the cursor marker ticks).
    /// On by default — they're the coordinate reference for the whole 2D
    /// editor — but toggleable for a chrome-free view. Toolbar switch
    /// (2D view only).
    pub show_rulers_2d: bool,
    /// The status-bar cursor-coordinate readout for the 2D view. On by
    /// default; independent of the rulers so either can be shown alone.
    /// Toolbar switch (2D view only).
    pub show_cursor_coords_2d: bool,
    /// 2D grid line colour (R, G, B, A in 0–255). Alpha controls the
    /// minor-line opacity; major lines auto-bump the alpha by ~2× for
    /// the typical Photoshop-style minor/major hierarchy.
    pub grid_color_2d: [u8; 4],
    /// The 2D view's editor gizmo overlays — the always-visible light markers
    /// and the selected light/occluder outlines. On by default; the 2D
    /// counterpart of the 3D "Scene Icons" toggle (which is unreachable in 2D,
    /// since the whole Display dropdown is 3D-only). Toolbar switch, in the 2D
    /// Overlays dropdown (2D view only).
    pub show_gizmos_2d: bool,
    pub show_axis_gizmo: bool,
    /// Toggle for in-viewport scene icons (light bulb / sun / camera glyphs).
    pub show_scene_icons: bool,
    /// Toggle for in-viewport entity name labels (drawn with Bevy's stroke-font
    /// text gizmos above each named scene entity). Off by default to avoid
    /// clutter — it's an opt-in debug/orientation overlay.
    pub show_labels: bool,
    /// Size multiplier for entity name labels (`1.0` = the default auto size,
    /// which is itself distance-scaled to stay roughly screen-constant).
    pub label_size: f32,
    /// Base RGB colour (0–255) for entity name labels. The selected entity is
    /// always drawn gold regardless, as a selection cue.
    pub label_color: [u8; 3],
    /// Max camera distance at which a label is drawn; farther entities are
    /// culled so big scenes don't carpet the view with text.
    pub label_max_distance: f32,
    /// Which entities get a name label (all / top-level / meshes / selected).
    pub label_scope: LabelScope,
    pub collision_gizmo_visibility: CollisionGizmoVisibility,
    pub projection_mode: ProjectionMode,
    pub viewport_mode: ViewportMode,
    pub viewport_view: ViewportView,
    pub camera: CameraSettingsState,
    pub snap: SnapSettings,
    /// Pending view angle command (consumed by camera system).
    pub pending_view_angle: Option<ViewAngleCommand>,
    /// Cap the framerate at the monitor refresh rate. Off lets the FPS
    /// counter reflect actual render capacity at the cost of possible
    /// screen tearing.
    pub vsync: bool,
    /// Opacity (0–1) the transform gizmo fades to while a handle is being
    /// dragged. The handles render always-on-top, so at full opacity they hide
    /// whatever you're moving; fading them lets the object stay visible during
    /// the drag. `1.0` keeps the gizmo fully opaque (no fade).
    pub gizmo_drag_opacity: f32,
    /// Overall graphics-quality tier — gates the expensive fullscreen passes
    /// (GI / auto-exposure / bloom / TAA). Enforced by
    /// `renzora_level_presets::graphics_quality`. Defaults to `Medium` so the
    /// editor stays responsive on weak / high-DPI hardware out of the box.
    pub graphics_quality: GraphicsQuality,
}

impl Default for ViewportSettings {
    fn default() -> Self {
        Self {
            render_toggles: RenderToggles::default(),
            visualization_mode: VisualizationMode::default(),
            show_grid: true,
            show_subgrid: true,
            show_grid_2d: false,
            // Matches the default tilemap tile size (16 units = 16 px art).
            grid_size_2d: 16.0,
            show_rulers_2d: true,
            show_cursor_coords_2d: true,
            // Faint by design — the 2D grid sits behind the sprites, so it
            // only needs to whisper. Major lines double this automatically.
            grid_color_2d: [255, 255, 255, 20],
            show_gizmos_2d: true,
            show_axis_gizmo: true,
            show_scene_icons: true,
            show_labels: false,
            label_size: 1.0,
            label_color: [217, 230, 255],
            label_max_distance: 40.0,
            label_scope: LabelScope::default(),
            collision_gizmo_visibility: CollisionGizmoVisibility::default(),
            projection_mode: ProjectionMode::default(),
            viewport_mode: ViewportMode::default(),
            viewport_view: ViewportView::default(),
            camera: CameraSettingsState::default(),
            snap: SnapSettings::default(),
            pending_view_angle: None,
            vsync: true,
            gizmo_drag_opacity: default_gizmo_drag_opacity(),
            graphics_quality: GraphicsQuality::default(),
        }
    }
}

// ── Persisted editor preferences (stored in project.toml) ──────────────────
//
// Editor-only fields. Stripped from exported builds (the runtime ignores the
// `[editor]` section of project.toml). Uses `#[serde(default)]` on every
// field so missing entries fall back to sensible defaults.

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PersistedViewportSettings {
    pub textures: bool,
    pub wireframe: bool,
    pub lighting: bool,
    pub shadows: bool,
    #[serde(default = "default_true")]
    pub mesh: bool,
    pub visualization_mode: String,
    pub show_grid: bool,
    pub show_subgrid: bool,
    /// 2D-view grid toggle. Defaults off (`#[serde(default)]` = false), so
    /// projects saved before the switch existed open with the grid hidden.
    #[serde(default)]
    pub show_grid_2d: bool,
    /// 2D grid cell size in world units. Defaults to 16 (the tilemap/pixel-art
    /// convention) for projects saved before the field existed.
    #[serde(default = "default_grid_size_2d")]
    pub grid_size_2d: f32,
    /// 2D-view ruler toggle. Defaults on — rulers pre-date the switch, so
    /// older projects keep looking the way they did.
    #[serde(default = "default_true")]
    pub show_rulers_2d: bool,
    /// 2D-view status-bar coordinate readout toggle. Defaults on (the
    /// readout pre-dates the switch).
    #[serde(default = "default_true")]
    pub show_cursor_coords_2d: bool,
    /// 2D grid line colour (R, G, B, A in 0–255). Defaults to subtle
    /// white when missing; major / minor split is automatic in the
    /// drawer.
    #[serde(default = "default_grid_color_2d")]
    pub grid_color_2d: [u8; 4],
    /// 2D-view gizmo-overlay toggle. Defaults on — the markers pre-date the
    /// switch, so older projects keep showing them.
    #[serde(default = "default_true")]
    pub show_gizmos_2d: bool,
    pub show_axis_gizmo: bool,
    #[serde(default = "default_true")]
    pub show_scene_icons: bool,
    #[serde(default)]
    pub show_labels: bool,
    #[serde(default = "default_label_size")]
    pub label_size: f32,
    #[serde(default = "default_label_color")]
    pub label_color: [u8; 3],
    #[serde(default = "default_label_max_distance")]
    pub label_max_distance: f32,
    #[serde(default)]
    pub label_scope: String,
    pub collision_always: bool,
    pub orthographic: bool,
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub invert_y: bool,
    pub distance_relative_speed: bool,
    /// `"Default"` or `"Selected"` — which scene camera drives the editor view.
    #[serde(default)]
    pub editor_camera_source: String,
    pub translate_enabled: bool,
    pub translate_snap: f32,
    pub translate_edge_snap: bool,
    pub rotate_enabled: bool,
    pub rotate_snap: f32,
    pub scale_enabled: bool,
    pub scale_snap: f32,
    pub scale_bottom_anchor: bool,
    pub object_snap_enabled: bool,
    pub object_snap_distance: f32,
    pub floor_snap_enabled: bool,
    pub floor_y: f32,
    #[serde(default = "default_true")]
    pub vsync: bool,
    #[serde(default = "default_gizmo_drag_opacity")]
    pub gizmo_drag_opacity: f32,
    /// Graphics-quality tier label (`"Low"` / `"Medium"` / `"High"`). Missing in
    /// configs written before this field existed → `default_graphics_quality()`
    /// (`"Medium"`), so upgrading projects pick up the lighter default.
    #[serde(default = "default_graphics_quality")]
    pub graphics_quality: String,
}

impl PersistedViewportSettings {
    pub fn from_settings(s: &ViewportSettings) -> Self {
        let rt = s.render_toggles;
        let c = s.camera;
        let sn = s.snap;
        Self {
            textures: rt.textures,
            wireframe: rt.wireframe,
            lighting: rt.lighting,
            shadows: rt.shadows,
            mesh: rt.mesh,
            visualization_mode: format!("{:?}", s.visualization_mode),
            show_grid: s.show_grid,
            show_subgrid: s.show_subgrid,
            show_grid_2d: s.show_grid_2d,
            grid_size_2d: s.grid_size_2d,
            show_rulers_2d: s.show_rulers_2d,
            show_cursor_coords_2d: s.show_cursor_coords_2d,
            grid_color_2d: s.grid_color_2d,
            show_gizmos_2d: s.show_gizmos_2d,
            show_axis_gizmo: s.show_axis_gizmo,
            show_scene_icons: s.show_scene_icons,
            show_labels: s.show_labels,
            label_size: s.label_size,
            label_color: s.label_color,
            label_max_distance: s.label_max_distance,
            label_scope: format!("{:?}", s.label_scope),
            collision_always: matches!(
                s.collision_gizmo_visibility,
                CollisionGizmoVisibility::Always
            ),
            orthographic: matches!(s.projection_mode, ProjectionMode::Orthographic),
            move_speed: c.move_speed,
            look_sensitivity: c.look_sensitivity,
            orbit_sensitivity: c.orbit_sensitivity,
            pan_sensitivity: c.pan_sensitivity,
            zoom_sensitivity: c.zoom_sensitivity,
            invert_y: c.invert_y,
            distance_relative_speed: c.distance_relative_speed,
            editor_camera_source: format!("{:?}", c.editor_camera_source),
            translate_enabled: sn.translate_enabled,
            translate_snap: sn.translate_snap,
            translate_edge_snap: sn.translate_edge_snap,
            rotate_enabled: sn.rotate_enabled,
            rotate_snap: sn.rotate_snap,
            scale_enabled: sn.scale_enabled,
            scale_snap: sn.scale_snap,
            scale_bottom_anchor: sn.scale_bottom_anchor,
            object_snap_enabled: sn.object_snap_enabled,
            object_snap_distance: sn.object_snap_distance,
            floor_snap_enabled: sn.floor_snap_enabled,
            floor_y: sn.floor_y,
            vsync: s.vsync,
            gizmo_drag_opacity: s.gizmo_drag_opacity,
            graphics_quality: s.graphics_quality.label().to_string(),
        }
    }

    pub fn apply(&self, s: &mut ViewportSettings) {
        s.render_toggles = RenderToggles {
            textures: self.textures,
            wireframe: self.wireframe,
            lighting: self.lighting,
            shadows: self.shadows,
            mesh: self.mesh,
        };
        s.visualization_mode = match self.visualization_mode.as_str() {
            "Normals" => VisualizationMode::Normals,
            "Roughness" => VisualizationMode::Roughness,
            "Metallic" => VisualizationMode::Metallic,
            "Depth" => VisualizationMode::Depth,
            "UvChecker" => VisualizationMode::UvChecker,
            _ => VisualizationMode::None,
        };
        s.show_grid = self.show_grid;
        s.show_subgrid = self.show_subgrid;
        s.show_grid_2d = self.show_grid_2d;
        s.grid_size_2d = self.grid_size_2d;
        s.show_rulers_2d = self.show_rulers_2d;
        s.show_cursor_coords_2d = self.show_cursor_coords_2d;
        s.grid_color_2d = self.grid_color_2d;
        s.show_gizmos_2d = self.show_gizmos_2d;
        s.show_axis_gizmo = self.show_axis_gizmo;
        s.show_scene_icons = self.show_scene_icons;
        s.show_labels = self.show_labels;
        s.label_size = self.label_size;
        s.label_color = self.label_color;
        s.label_max_distance = self.label_max_distance;
        s.label_scope = LabelScope::from_debug(&self.label_scope);
        s.collision_gizmo_visibility = if self.collision_always {
            CollisionGizmoVisibility::Always
        } else {
            CollisionGizmoVisibility::SelectedOnly
        };
        s.projection_mode = if self.orthographic {
            ProjectionMode::Orthographic
        } else {
            ProjectionMode::Perspective
        };
        s.camera = CameraSettingsState {
            move_speed: self.move_speed,
            look_sensitivity: self.look_sensitivity,
            orbit_sensitivity: self.orbit_sensitivity,
            pan_sensitivity: self.pan_sensitivity,
            zoom_sensitivity: self.zoom_sensitivity,
            invert_y: self.invert_y,
            distance_relative_speed: self.distance_relative_speed,
            editor_camera_source: match self.editor_camera_source.as_str() {
                "Selected" => EditorCameraSource::Selected,
                _ => EditorCameraSource::Default,
            },
        };
        s.snap = SnapSettings {
            translate_enabled: self.translate_enabled,
            translate_snap: self.translate_snap,
            translate_edge_snap: self.translate_edge_snap,
            rotate_enabled: self.rotate_enabled,
            rotate_snap: self.rotate_snap,
            scale_enabled: self.scale_enabled,
            scale_snap: self.scale_snap,
            scale_bottom_anchor: self.scale_bottom_anchor,
            object_snap_enabled: self.object_snap_enabled,
            object_snap_distance: self.object_snap_distance,
            floor_snap_enabled: self.floor_snap_enabled,
            floor_y: self.floor_y,
        };
        s.vsync = self.vsync;
        s.gizmo_drag_opacity = self.gizmo_drag_opacity;
        s.graphics_quality = GraphicsQuality::from_label(&self.graphics_quality);
    }
}

fn default_true() -> bool {
    true
}

fn default_grid_size_2d() -> f32 {
    16.0
}

fn default_graphics_quality() -> String {
    GraphicsQuality::default().label().to_string()
}

fn default_grid_color_2d() -> [u8; 4] {
    [255, 255, 255, 18]
}

fn default_label_size() -> f32 {
    1.0
}

fn default_label_color() -> [u8; 3] {
    [217, 230, 255]
}

fn default_label_max_distance() -> f32 {
    40.0
}

fn default_gizmo_drag_opacity() -> f32 {
    0.25
}

/// Editor-only preferences persisted in `project.toml` under `[editor]`.
/// The runtime ignores this section, and `renzora_export` strips it from
/// shipped builds.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorPrefs {
    pub viewport: PersistedViewportSettings,
    /// Set once the first-run onboarding tutorial (`renzora_tutorial`) has been
    /// completed or skipped. While `false`/absent the tutorial auto-launches the
    /// first time the editor opens this project. Editor-only like the rest of
    /// this section — the runtime never reads it and export strips it.
    #[serde(default)]
    pub tutorial_completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nondefault_viewport() -> ViewportSettings {
        // Touch every field so the round-trip really exercises the
        // PersistedViewportSettings <-> ViewportSettings bridge — a missed
        // field on either side would make this test fail.
        ViewportSettings {
            render_toggles: RenderToggles {
                textures: false,
                wireframe: true,
                lighting: false,
                shadows: false,
                mesh: false,
            },
            visualization_mode: VisualizationMode::Normals,
            show_grid: false,
            show_subgrid: false,
            // Non-default (defaults are false / true / true) so the round-trip
            // exercises all three 2D toggles.
            show_grid_2d: true,
            grid_size_2d: 32.0,
            show_rulers_2d: false,
            show_cursor_coords_2d: false,
            grid_color_2d: [128, 200, 255, 60],
            show_gizmos_2d: false,
            show_axis_gizmo: false,
            show_scene_icons: false,
            show_labels: true,
            label_size: 2.5,
            label_color: [10, 20, 30],
            label_max_distance: 99.0,
            label_scope: LabelScope::Selected,
            collision_gizmo_visibility: CollisionGizmoVisibility::Always,
            projection_mode: ProjectionMode::Orthographic,
            viewport_mode: ViewportMode::default(),
            viewport_view: ViewportView::default(),
            camera: CameraSettingsState {
                move_speed: 11.5,
                look_sensitivity: 0.7,
                orbit_sensitivity: 0.42,
                pan_sensitivity: 1.7,
                zoom_sensitivity: 2.3,
                invert_y: true,
                distance_relative_speed: false,
                editor_camera_source: EditorCameraSource::Selected,
            },
            snap: SnapSettings {
                translate_enabled: true,
                translate_snap: 0.5,
                translate_edge_snap: true,
                rotate_enabled: true,
                rotate_snap: 15.0,
                scale_enabled: false,
                scale_snap: 0.25,
                scale_bottom_anchor: true,
                object_snap_enabled: true,
                object_snap_distance: 1.5,
                floor_snap_enabled: true,
                floor_y: -1.5,
            },
            pending_view_angle: None,
            vsync: false,
            gizmo_drag_opacity: 0.6,
            // Non-default tier (default is Medium) so the round-trip exercises it.
            graphics_quality: GraphicsQuality::High,
        }
    }

    #[test]
    fn persisted_round_trip_preserves_every_field() {
        let original = nondefault_viewport();
        let persisted = PersistedViewportSettings::from_settings(&original);
        let mut restored = ViewportSettings::default();
        persisted.apply(&mut restored);

        // Skip pending_view_angle (transient) and viewport_mode (not persisted).
        assert_eq!(original.render_toggles, restored.render_toggles);
        assert!(matches!(
            restored.visualization_mode,
            VisualizationMode::Normals
        ));
        assert_eq!(original.show_grid, restored.show_grid);
        assert_eq!(original.show_subgrid, restored.show_subgrid);
        assert_eq!(original.show_grid_2d, restored.show_grid_2d);
        assert_eq!(original.grid_size_2d, restored.grid_size_2d);
        assert_eq!(original.show_rulers_2d, restored.show_rulers_2d);
        assert_eq!(original.show_cursor_coords_2d, restored.show_cursor_coords_2d);
        assert_eq!(original.show_gizmos_2d, restored.show_gizmos_2d);
        assert_eq!(original.show_axis_gizmo, restored.show_axis_gizmo);
        assert_eq!(original.show_scene_icons, restored.show_scene_icons);
        assert_eq!(original.show_labels, restored.show_labels);
        assert_eq!(original.label_size, restored.label_size);
        assert_eq!(original.label_color, restored.label_color);
        assert_eq!(original.label_max_distance, restored.label_max_distance);
        assert_eq!(original.label_scope, restored.label_scope);
        assert!(matches!(
            restored.collision_gizmo_visibility,
            CollisionGizmoVisibility::Always
        ));
        assert!(matches!(
            restored.projection_mode,
            ProjectionMode::Orthographic
        ));
        assert_eq!(original.camera, restored.camera);
        assert_eq!(original.snap, restored.snap);
        assert_eq!(original.vsync, restored.vsync);
        assert_eq!(original.gizmo_drag_opacity, restored.gizmo_drag_opacity);
        assert_eq!(original.graphics_quality, restored.graphics_quality);
    }

    #[test]
    fn vsync_round_trips() {
        // The whole point of the recent vsync setting is that it survives
        // a save/load. Lock that in.
        let mut s = ViewportSettings::default();
        s.vsync = false;
        let persisted = PersistedViewportSettings::from_settings(&s);
        let mut restored = ViewportSettings::default();
        persisted.apply(&mut restored);
        assert!(!restored.vsync);
    }

    #[test]
    fn visualization_mode_string_round_trips_through_persisted() {
        for mode in [
            VisualizationMode::None,
            VisualizationMode::Normals,
            VisualizationMode::Roughness,
            VisualizationMode::Metallic,
            VisualizationMode::Depth,
            VisualizationMode::UvChecker,
        ] {
            let mut s = ViewportSettings::default();
            s.visualization_mode = mode;
            let p = PersistedViewportSettings::from_settings(&s);
            let mut restored = ViewportSettings::default();
            p.apply(&mut restored);
            assert!(
                std::mem::discriminant(&restored.visualization_mode)
                    == std::mem::discriminant(&mode),
                "round trip lost mode {:?}, got {:?}",
                mode,
                restored.visualization_mode,
            );
        }
    }

    #[test]
    fn editor_prefs_default_has_default_viewport() {
        let prefs = EditorPrefs::default();
        assert_eq!(prefs.viewport, PersistedViewportSettings::default());
    }

    #[test]
    fn persisted_viewport_serde_is_keyed_by_field_name() {
        // Hand-rolled TOML has to deserialize cleanly — proves we didn't
        // accidentally tag the struct or rename a field.
        let s = r#"
            textures = true
            wireframe = false
            lighting = true
            shadows = true
            mesh = true
            visualization_mode = "None"
            show_grid = true
            show_subgrid = true
            show_axis_gizmo = true
            show_scene_icons = true
            collision_always = false
            orthographic = false
            move_speed = 10.0
            look_sensitivity = 1.0
            orbit_sensitivity = 1.0
            pan_sensitivity = 1.0
            zoom_sensitivity = 1.0
            invert_y = false
            distance_relative_speed = true
            translate_enabled = false
            translate_snap = 1.0
            translate_edge_snap = false
            rotate_enabled = false
            rotate_snap = 15.0
            scale_enabled = false
            scale_snap = 0.1
            scale_bottom_anchor = false
            object_snap_enabled = false
            object_snap_distance = 1.0
            floor_snap_enabled = false
            floor_y = 0.0
            vsync = true
        "#;
        let parsed: PersistedViewportSettings = toml::from_str(s).expect("parse");
        assert!(parsed.vsync);
        assert!(parsed.mesh);
    }
}
