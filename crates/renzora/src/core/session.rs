//! What kind of process this is, and the one-shot requests the editor's chrome
//! sends to whichever crate owns the answer.
//!
//! Almost everything here is a marker resource inserted for a frame and drained
//! by an observer somewhere else. It is the same seam the [shell
//! registries](super::shell) use, in its simplest form: the menu item that asks
//! for the export overlay does not link the exporter, it inserts
//! [`ExportRequested`].

use std::path::Path;

use bevy::prelude::*;

use super::project_config::{CurrentProject, ProjectConfig};

/// Marker resource: present when the runtime is running as a dedicated server
/// (`renzora-runtime --server`). Inserted before engine plugins build so they
/// can opt out of client/render-only setup. A dedicated server has no render
/// world (`backends: None`), so GPU-only plugins (e.g. `bevy_hanabi`) must
/// check this and skip their render-side initialization to avoid panicking on
/// the absent `RenderApp`. Networking uses it to skip client-side setup.
#[derive(Resource, Default)]
pub struct DedicatedServer;

/// Marker resource: present when the runtime is running as a host/listen-server
/// (`renzora-runtime --host`). Unlike [`DedicatedServer`] the host renders
/// normally (it has a local player), so it is *not* headless — it runs both the
/// client and server plugin sets in one process. Inserted before engine plugins
/// build so networking can wire host mode (client setup stays, the server plugin
/// owns the protocol/observers so they register exactly once).
#[derive(Resource, Default)]
pub struct HostServer;

/// Whether this process is an EDITOR session (the `renzora_editor` bundle dll
/// is present beside the exe) vs. a shipped game. Inserted by
/// `add_engine_plugins(is_editor)` before the engine plugins build. Lets the
/// dual-mode crates — compiled WITHOUT an `editor` cargo feature — still decide
/// editor-vs-game behaviour at RUNTIME, e.g. `RuntimePlugin` only runs the
/// rpak/project/scene game-startup when this is `false` (the editor's splash
/// drives loading otherwise). Defaults to `false` (a plain game) when absent.
#[derive(Resource, Clone, Copy, Default)]
pub struct EditorSession(pub bool);

impl EditorSession {
    /// True in an editor session (bundle present), false in a shipped game.
    pub fn is_editor(&self) -> bool {
        self.0
    }
}

/// Resource: request a scene load from scripts/blueprints.
/// The runtime system drains this each frame.
#[derive(Resource, Default)]
pub struct PendingSceneLoad {
    /// Scene name or relative path to load.
    pub requests: Vec<String>,
}

/// Marker resource requesting a scene save.
///
/// Insert this resource to trigger the scene save system next frame.
#[derive(Resource)]
pub struct SaveSceneRequested;

/// Request "Save As" — prompts user for a new scene name/path.
#[derive(Resource)]
pub struct SaveAsSceneRequested;

/// Request "New Scene" — clears the world and sets up a blank scene.
#[derive(Resource)]
pub struct NewSceneRequested;

/// Request "Open Scene" — prompts user to pick a scene file.
#[derive(Resource)]
pub struct OpenSceneRequested;

/// Request opening a *specific* scene file in its own document tab (loaded from
/// disk), e.g. double-clicking a `.bsn` in the asset browser or its "Open Scene"
/// context-menu item. Unlike [`OpenSceneRequested`] (which pops a file dialog),
/// this carries the path directly so the scene system can load it without a
/// prompt.
#[derive(Resource)]
pub struct OpenScenePathRequested(pub std::path::PathBuf);

/// Request a tab switch — serializes current scene, deserializes target.
#[derive(Resource)]
pub struct TabSwitchRequest {
    pub old_tab_id: u64,
    pub new_tab_id: u64,
}

/// In-memory snapshot of a scene tab's state (entities + camera).
pub struct TabSceneSnapshot {
    pub scene_ron: String,
    pub camera_focus: [f32; 3],
    pub camera_distance: f32,
    pub camera_yaw: f32,
    pub camera_pitch: f32,
}

/// Stores serialized scene data for each tab so switching tabs can serialize/deserialize.
#[derive(Resource, Default)]
pub struct SceneTabBuffers {
    pub buffers: std::collections::HashMap<u64, TabSceneSnapshot>,
}

/// Marker resource requesting the export overlay to open.
///
/// Insert this resource to trigger the export overlay next frame.
#[derive(Resource)]
pub struct ExportRequested;

/// Which OS picker an [`ImportRequested`] opens before showing the overlay.
///
/// No native file dialog can select files *and* folders in one pass — on
/// Windows a folder in a file dialog can only be navigated into — so the
/// request has to say up front which of the two it wants. The editor's Import
/// entry points ask the user with a two-row menu rather than guessing.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportPick {
    /// The multi-select file picker, filtered to every importable extension.
    #[default]
    Files,
    /// The folder picker; the chosen directory is walked for importable files
    /// and its subfolder tree is mirrored into the destination.
    Folder,
}

/// Marker resource requesting the import overlay to open.
///
/// Insert this resource to trigger the import overlay next frame. The payload
/// picks which OS dialog opens first — see [`ImportPick`].
#[derive(Resource, Default)]
pub struct ImportRequested(pub ImportPick);

/// Marker resource requesting the software-update dialog to open.
///
/// Inserted by Help ▸ Check for Updates and consumed by `renzora_update`. Here
/// rather than in that crate so the shell can ask for the dialog without linking
/// the updater — the same arrangement as [`ExportRequested`].
#[derive(Resource)]
pub struct UpdateRequested;

/// Present when the updater's background check found a newer engine; carries its
/// release tag.
///
/// The flow runs the other way from [`UpdateRequested`]: `renzora_update`
/// inserts this, and the shell reads it to label the Help menu item "Update to
/// r1-alpha8" instead of "Check for Updates". It is removed again if a later
/// check (say, after switching channel) finds nothing.
#[derive(Resource)]
pub struct UpdateAvailable(pub String);

/// Live mirror of the editor's developer-mode toggle.
///
/// [`load_dev_mode`](super::project_config::load_dev_mode) already answers "is
/// dev mode on" from disk, but reading a file is not something a system can do
/// every frame, and the flag has to be *watched*, not just read:
/// `renzora_update` re-runs its check the moment the toggle flips, because
/// nightlies are only offered in dev mode and the top bar's "update available"
/// chip must appear and disappear with it.
///
/// Written by `renzora_editor_framework` from `EditorSettings.dev_mode`, which
/// is the one place that knows about every way the toggle can change. Here in
/// the contract crate so a reader does not have to link the editor framework.
#[derive(Resource, Default)]
pub struct DevMode(pub bool);

/// Optional: carries the suggested target directory from the asset browser.
#[derive(Resource)]
pub struct ImportTargetDir(pub String);

/// The asset browser's current folder, project-relative and forward-slashed
/// (`""` = project root; `None` = no browser/project active). The browser
/// republishes it each frame so drag-and-drop imports land in the folder the
/// user is looking at, instead of the importer's default target. Read by the
/// importer's drop handler.
#[derive(Resource, Default)]
pub struct AssetBrowserCwd(pub Option<String>);

/// True while an OS file drag is hovering the editor window — set when a
/// `HoveredFile` event arrives and cleared on the matching drop or cancel. The
/// importer owns it (it already drains the file-drop events); the asset browser
/// reads it to render a "drop to import" highlight over its panel.
#[derive(Resource, Default)]
pub struct FileDragHovering(pub bool);

/// Set `true` by the importer when files are dropped onto the editor, so the
/// asset browser scrolls its grid to the freshly-imported items. The browser
/// resets it once consumed, then pins the grid to the bottom for a short window
/// (long enough for the ~0.5 s rescan to surface the new file and grow the grid).
#[derive(Resource, Default)]
pub struct AssetDropScrollRequest(pub bool);

/// Set true while the pointer is over a panel that owns the `Ctrl/Cmd+A`
/// shortcut for its own selection (currently the asset browser's file grid).
///
/// `Ctrl+A` is bound in several places — the hierarchy's "select all entities"
/// and the asset browser's "select all files" both listen for it. Without a
/// referee they'd fire together. So the panel under the pointer raises this flag
/// and the global entity select-all stands down for that frame, letting the
/// hovered panel handle the key. Absent/false → the entity select-all wins
/// (e.g. `Ctrl+A` over the viewport still selects every entity).
#[derive(Resource, Default)]
pub struct SelectAllClaimed(pub bool);

/// Set true while a focused panel is using the arrow keys to move through its
/// own contents, so the editor's *hover*-driven arrow-key behaviours stand down:
/// ember's keyboard scrolling and the 2D viewport's nudge.
///
/// Same referee problem as [`SelectAllClaimed`], with the twist that the two
/// claimants key off *different* things. Ember scrolls whichever view the cursor
/// rests over and the 2D nudge moves the selection while the viewport is hovered,
/// but a list panel walks its selection because it has *focus*. All of them fire
/// on the same keys, so with the cursor parked over the tree you'd move the
/// selection and scroll the view out from under it at once — and with the cursor
/// over the viewport you'd nudge the sprite while the selection walked off it.
///
/// Focus wins, which is the ordinary rule for keyboard input — a click elsewhere
/// hands the arrows straight back to the hovered consumer. The claim is published
/// from panel *state* (focused, not typing, something to move) rather than from
/// the keypress, so it is already settled by the time a key arrives and never
/// lands a frame late. The publisher must stay ungated by panel visibility, or a
/// backgrounded panel leaves the flag stuck true and swallows the lot.
#[derive(Resource, Default)]
pub struct ArrowKeysClaimed(pub bool);

/// One split static mesh referenced by an assembly `.prefab`: a display name
/// and the project-relative path to its `.glb`. The mesh's world transform is
/// baked into the `.glb` itself, so the assembly entity sits at identity.
#[derive(Clone, Debug)]
pub struct AssemblyMeshEntry {
    pub name: String,
    pub model_path: String,
}

/// A request to write an assembly `.prefab` for a freshly split model.
///
/// The import worker runs off-thread and can't touch the `World`, but writing a
/// prefab in the engine's scene format needs the type registry and the existing
/// `save_prefab_source` serializer. So the worker hands the mesh list to the
/// main thread via [`PendingAssemblyWrites`], where an engine system fulfills it.
#[derive(Clone, Debug)]
pub struct AssemblyWriteRequest {
    /// Absolute path of the `.prefab` to write.
    pub prefab_path: std::path::PathBuf,
    /// The split meshes the assembly references, in source order.
    pub entries: Vec<AssemblyMeshEntry>,
}

/// Queue of assembly `.prefab` files to write, drained by an engine system.
/// See [`AssemblyWriteRequest`].
#[derive(Resource, Default)]
pub struct PendingAssemblyWrites(pub Vec<AssemblyWriteRequest>);

/// Marker resource requesting the tutorial overlay to start.
#[derive(Resource)]
pub struct TutorialRequested;

/// One-shot: request to toggle the settings overlay.
#[derive(Resource)]
pub struct ToggleSettingsRequested;

/// One-shot: request to open the Create Node overlay in the hierarchy panel.
#[derive(Resource)]
pub struct CreateNodeRequested;

/// One-shot: request the code editor to open a file.
///
/// Inserted by the asset browser (or any plugin) so the code editor plugin
/// can observe it without a direct crate dependency.
#[derive(Resource)]
pub struct OpenCodeEditorFile {
    pub path: std::path::PathBuf,
}

/// One-shot: request the UI editor to open a `.html` template on a canvas.
///
/// The counterpart to [`OpenCodeEditorFile`] for the visual editor. Consumed by
/// `renzora_viewport`, which spawns (or re-selects) a `UiCanvas` carrying the
/// template and selects it — the same thing dropping the file on the viewport
/// does, which is what made this reachable at all before the UI workspace
/// existed.
#[derive(Resource)]
pub struct OpenUiTemplateFile {
    pub path: std::path::PathBuf,
}

/// One-shot: request the command palette to toggle open/closed.
///
/// Inserted by the title-bar search button; consumed by `renzora_command_palette`.
#[derive(Resource)]
pub struct ToggleCommandPaletteRequested;

/// One-shot: request a viewport camera operation from the View menu.
///
/// Consumed by the camera controller in `renzora_camera`.
#[derive(Resource, Clone, Copy, Debug)]
pub enum CameraViewRequest {
    ZoomIn,
    ZoomOut,
    ResetZoom,
    FrameAll,
}

/// Toggle: when active, only the selected entity (and its ancestors/descendants)
/// remain visible in the viewport. Toggled from the View menu.
#[derive(Resource, Default)]
pub struct IsolationMode {
    pub active: bool,
}

/// Tracks whether a UI text input has keyboard focus.
///
/// When `true`, keyboard shortcuts should not fire so typing is not interrupted.
/// Updated each frame by the viewport/editor systems from the bevy_ui (ember)
/// focus state.
#[derive(Resource, Default)]
pub struct InputFocusState {
    /// True when a UI text field (or an editing drag-value) has keyboard focus,
    /// so global editor shortcuts hold off while the user is typing.
    pub ui_wants_keyboard: bool,
    /// True when the pointer is over a floating UI panel/overlay (not the viewport).
    pub pointer_over_ui: bool,
    /// True when a panel is consuming the Delete key (e.g. the animation
    /// timeline with a keyframe selected). The entity-delete shortcut skips
    /// while this is set so Delete removes the keyframe, not the entity.
    pub suppress_entity_delete: bool,
}

/// HUD data for the modal transform overlay (written by gizmo crate, read by viewport).
///
/// When `active` is true the viewport panel draws the scale circle / axis info.
#[derive(Resource, Default)]
pub struct ModalTransformHud {
    /// Whether modal transform is active.
    pub active: bool,
    /// Mode name ("Grab", "Rotate", "Scale").
    pub mode: &'static str,
    /// Whether this is Scale mode (draws circle + line overlay).
    pub is_scale: bool,
    /// Screen-space pivot position (entity center projected).
    pub pivot: Option<[f32; 2]>,
    /// Current cursor screen position.
    pub cursor: [f32; 2],
    /// Axis constraint name ("", "X", "Y", "Z", "YZ", "XZ", "XY").
    pub axis_name: &'static str,
    /// Axis constraint color [r, g, b, a] in 0..=255.
    pub axis_color: [u8; 4],
    /// Numeric input display string.
    pub numeric_display: String,
    /// Scale-mode reference circle radius in screen px (cursor's distance from
    /// the pivot when the gesture started — when the cursor is back on this
    /// circle, the scale factor is exactly 1).
    pub ref_radius: f32,
    /// Scale-mode live scale factor (current cursor distance / start distance),
    /// shown as the readout when the user hasn't typed an explicit value.
    pub scale_factor: f32,
}

/// Holds the optional render target for the game camera.
///
/// - `Some(handle)` — camera renders to this image (editor mode).
/// - `None` — camera renders to the window (standalone mode).
#[derive(Resource, Default)]
pub struct ViewportRenderTarget {
    pub image: Option<Handle<Image>>,
}

/// Open an existing project from project.toml path
pub fn open_project(project_toml_path: &Path) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(project_toml_path)?;
    let config: ProjectConfig = toml::from_str(&content)?;

    let path = project_toml_path
        .parent()
        .ok_or("Invalid project path")?
        .to_path_buf();

    Ok(CurrentProject { path, config })
}
