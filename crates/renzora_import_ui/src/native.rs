//! Bevy-native (ember) import overlay — the bevy_ui counterpart to the egui
//! `draw_import_overlay`. Two-pane modal: a left sidebar of sections (Files,
//! Settings, Extract, Optimize, Destination) and a right content pane showing
//! the active section. It edits the same [`ImportOverlayState`] and reuses the
//! worker (`run_import` / `poll_import_task`). Renders only under the BevyUi
//! backend; the egui overlay renders under Egui. The egui orchestration (file
//! drops, ImportRequested, auto-import) keeps running regardless.
//!
//! Why a sidebar instead of one long scroll: the import options span five
//! unrelated concerns. Stacked vertically they read as an undifferentiated wall
//! of checkboxes; split into named sections the user sees one focused panel at a
//! time and the modal stays a fixed, predictable size.

use std::path::PathBuf;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    checkbox, drag_value, dropdown, dropdown_compact, radio_group, scroll_view, spinner,
    OverlaySurface,
};

use renzora_import::settings::{SceneStructure, UpAxis};

use crate::overlay::{close_overlay, poll_import_task, run_import, ImportLayout, ImportOverlayState, ImportProgress};
use crate::staged::{human_bytes, thousands};

const GREEN: (u8, u8, u8) = (89, 191, 115);
const RED: (u8, u8, u8) = (239, 68, 68);
const AMBER: (u8, u8, u8) = (223, 165, 74);

/// Which tab the window's left pane is showing.
///
/// `Files` is the pre-conversion state — the queue and the drop targets. The
/// other three describe a *converted* model and only exist while one is staged,
/// which is why the tab bar hides them until then.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportTab {
    #[default]
    Files,
    Scene,
    Meshes,
    Materials,
    Destination,
}

/// One row of the scene tree. A node's mesh hangs under it as a child, and the
/// mesh's surfaces under that, which is how a DCC tool and Godot both present
/// it — the mesh is a *resource the node points at*, not the node itself, and
/// showing them as one row hides which nodes share geometry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TreeItem {
    Node(usize),
    Mesh(usize),
    /// `(mesh, primitive)` — one surface, i.e. one material's worth of it.
    Prim(usize, usize),
}

/// Everything the window remembers about what you are looking at: the tab, the
/// expanded tree rows, and the selection per tab.
///
/// Selection is kept per tab rather than as one shared index because the three
/// lists address different things — node 4, mesh 4 and material 4 are unrelated
/// — and switching tabs should not silently repoint the properties rail.
#[derive(Resource, Default)]
pub struct ImportNav {
    pub tab: ImportTab,
    pub expanded: std::collections::HashSet<TreeItem>,
    pub sel_item: Option<TreeItem>,
    pub sel_mesh: Option<usize>,
    pub sel_material: Option<usize>,
}

impl ImportNav {
    /// Clear everything tied to one staged file. Called when a verdict is given
    /// and when the next file stages, so indices from the previous model can
    /// never address the new one.
    pub(crate) fn reset_selection(&mut self) {
        self.expanded.clear();
        self.sel_item = None;
        self.sel_mesh = None;
        self.sel_material = None;
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ImportNav>()
        .init_resource::<ImportColumns>()
        // Split in two: a system tuple caps out at 20 elements.
        .add_systems(
            Update,
            (
                manage_import_modal,
                manage_import_toast,
                file_browse_click,
                folder_browse_click,
                dest_folder_click,
                tab_click,
                splitter_drag,
                staged_row_click,
                tree_row_click,
                tree_check_click,
                mesh_row_click,
                mat_row_click,
            ),
        )
        .add_systems(
            Update,
            (
                commit_click,
                skip_click,
                discard_all_click,
                settings_watch,
                crate::overlay::drive_reimport,
                auto_start_import,
                on_staged_changed,
                cancel_click,
                toast_dismiss_click,
                remove_file_click,
            ),
        );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ImportRoot;

/// The editor grid's visibility from before the window opened, restored on
/// close. The grid's render pass is not confined to the main viewport's layer,
/// so it draws through the preview's own camera and cuts a lattice across
/// whatever is being inspected.
#[derive(Resource)]
struct GridSuppressed(bool);
#[derive(Component)]
struct FileBrowseBtn;
#[derive(Component)]
struct FolderBrowseBtn;
/// A clickable sidebar nav row; switches the active pane on press.
#[derive(Component, Clone, Copy)]
struct TabBtn(ImportTab);

/// Which edge a splitter drags.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Component, Clone, Copy)]
struct Splitter(Side);

/// User-set column widths, in logical pixels. Lives outside the window so a
/// resize survives closing and reopening it.
#[derive(Resource)]
pub(crate) struct ImportColumns {
    left: f32,
    right: f32,
}

impl Default for ImportColumns {
    fn default() -> Self {
        Self {
            left: 310.0,
            right: 320.0,
        }
    }
}
/// A scene-tree row, carrying its node index.
#[derive(Component, Clone, Copy)]
struct TreeRow(TreeItem);
/// A row in the mesh list.
#[derive(Component, Clone, Copy)]
struct MeshRow(usize);
/// A row in the material list.
#[derive(Component, Clone, Copy)]
struct MatRow(usize);
/// Accept the staged file into the project.
#[derive(Component)]
struct CommitBtn;
/// Discard this staged file, continue the queue.
#[derive(Component)]
struct SkipBtn;
/// Discard this staged file and abandon the queue.
#[derive(Component)]
struct DiscardAllBtn;
/// A row in the destination folder tree. Holds the project-relative path it
/// targets (forward-slashed, `""` = project root).
#[derive(Component, Clone)]
struct DestFolderRow(String);
#[derive(Component)]
struct CancelBtn;
#[derive(Component, Clone)]
struct RemoveFileBtn(PathBuf);
/// A staged model in the Files list; clicking makes it the one on show.
#[derive(Component, Clone, Copy)]
struct StagedRow(usize);
/// The include-checkbox on a scene-tree row. Only ever inserted on a box the
/// user is allowed to click.
#[derive(Component, Clone, Copy)]
struct TreeCheck(TreeItem);
#[derive(Component)]
struct FilesContainer;
#[derive(Component)]
struct LogContainer;
/// Root of the corner progress toast shown after the modal closes on Import.
#[derive(Component)]
struct ToastRoot;
/// The toast's close/dismiss button.
#[derive(Component)]
struct ToastDismissBtn;

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_import_modal(world: &mut World) {
    let visible = world.get_resource::<ImportOverlayState>().is_some_and(|s| s.visible);
    if visible {
        poll_import_task(world); // keep progress flowing (egui draw is gated off)
    }

    let mut q = world.query_filtered::<Entity, With<ImportRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if visible && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let has_project = world.get_resource::<renzora::core::CurrentProject>().is_some();
        // Always open on Files — the first thing the user does is add files,
        // and a stale tab from a previous open would be confusing.
        {
            let mut nav = world.resource_mut::<ImportNav>();
            nav.tab = ImportTab::Files;
            nav.reset_selection();
        }
        // Last-ditch repair for a scale that can never be right. Both routes
        // that could write one are closed now — the unit probe rejects
        // non-positive values, and `enqueue` re-detects per queue instead of
        // inheriting the last one — but `ImportOverlayState` outlives the
        // window, and a scale of zero silently collapses every model to a
        // point, so it is worth refusing to open with one.
        {
            let mut s = world.resource_mut::<ImportOverlayState>();
            if !s.settings.scale.is_finite() || s.settings.scale <= 0.0 {
                warn!(
                    "[import] scale was {}; resetting to 1.0",
                    s.settings.scale
                );
                s.settings.scale = 1.0;
            }
        }
        if let Some(mut vp) = world.get_resource_mut::<renzora::core::viewport_types::ViewportSettings>() {
            let was = vp.show_grid;
            vp.show_grid = false;
            world.insert_resource(GridSuppressed(was));
        }
        let init = Init::read(&Rx::new(&*world));
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_modal(&mut commands, &fonts, &init, has_project);
        }
        queue.apply(world);
    } else if !visible && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
        if let Some(prev) = world.remove_resource::<GridSuppressed>() {
            if let Some(mut vp) = world.get_resource_mut::<renzora::core::viewport_types::ViewportSettings>() {
                vp.show_grid = prev.0;
            }
        }
    }
}

/// Initial widget values read once at spawn (the bindings keep them in sync after).
struct Init {
    scale: f32,
    up_axis: usize,
    layout: usize,
    structure: usize,
    /// Project directory tree for the destination picker: (rel_path, depth, name),
    /// `rel_path` forward-slashed and relative to the project root (`""` = root).
    dest_folders: Vec<(String, usize, String)>,
    /// Sibling texture sets offered for a geometry-only queue: (stem, roles).
    /// Empty when the queue has no such model, which hides the row entirely.
    texture_sets: Vec<(String, String)>,
    /// Index of the currently chosen set, offset by one for the "None" entry.
    texture_set: usize,
}
impl Init {
    fn read(world: &Rx) -> Self {
        let s = world.resource::<ImportOverlayState>();
        let dest_folders = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| scan_dest_dirs(&p.path))
            .unwrap_or_default();
        let texture_sets = queue_texture_sets(s);
        let texture_set = s
            .settings
            .texture_set
            .as_deref()
            .and_then(|want| texture_sets.iter().position(|(stem, _)| stem == want))
            .map_or(0, |i| i + 1);
        Self {
            texture_sets,
            texture_set,
            scale: s.settings.scale,
            up_axis: match s.settings.up_axis {
                UpAxis::Auto => 0,
                UpAxis::YUp => 1,
                UpAxis::ZUp => 2,
            },
            layout: match s.layout {
                ImportLayout::PerFileFolder => 0,
                ImportLayout::Combined => 1,
            },
            structure: match s.settings.structure {
                SceneStructure::Preserve => 0,
                SceneStructure::FlatPerMesh => 1,
                SceneStructure::Combined => 2,
            },
            dest_folders,
        }
    }
}

/// The sibling texture sets on offer for the queued files.
///
/// Read once when the window opens rather than per staged file: a queue is
/// almost always one folder, so every model in it sees the same sets, and a
/// dropdown that reshuffled as you clicked between files would be worse than
/// one that doesn't. Returns empty unless the queue holds a geometry-only
/// model — a format that names its own textures must not be overridden by a
/// folder full of guesses.
fn queue_texture_sets(s: &ImportOverlayState) -> Vec<(String, String)> {
    use renzora_import::sibling_textures;
    s.pending_files
        .iter()
        .map(|q| q.path.as_path())
        .chain(s.last_files.iter().map(|q| q.path.as_path()))
        .find(|p| sibling_textures::is_geometry_only(p))
        .map(|p| {
            sibling_textures::discover(p)
                .into_iter()
                .map(|set| (set.stem.clone(), set.role_summary()))
                .collect()
        })
        .unwrap_or_default()
}


// ── Window ───────────────────────────────────────────────────────────────────

/// Fraction of the screen the window occupies on each axis. It is a dialog, not
/// a workspace: at full bleed there was no visual cue that the editor was still
/// there behind it. The margin only has to read as one — the panes inside all
/// want the room, so it stays narrow.
const WINDOW_FRACTION: f32 = 90.0;

/// Build the import window: a centred panel with a tab bar, a left list pane, a
/// large centre viewport and a right properties rail, over a full-screen scrim.
///
/// The scrim, not the panel, is the [`ModalSurface`] — it is what stops clicks
/// reaching the editor around the panel's edges, and the scroll and popup
/// systems test for a modal *ancestor*, so it has to be the root for the panel's
/// contents to count as being inside one.
///
/// The layout is deliberately the same before and after conversion; only what
/// each region holds changes. Before, the left pane is the file queue, the
/// centre is a drop zone and the right rail is the import settings. After, the
/// left pane is the scene tree / mesh list / material list, the centre is the
/// staged model, and the right rail is the selected item's properties. Keeping
/// one frame means nothing jumps around when the conversion finishes.
fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, init: &Init, has_project: bool) {
    let scrim = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            GlobalZIndex(900),
            FocusPolicy::Block,
            OverlaySurface,
            renzora_ember::widgets::ModalSurface,
            bevy::ui::RelativeCursorPosition::default(),
            Interaction::default(),
            ImportRoot,
            Name::new("import-scrim"),
        ))
        .id();

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(WINDOW_FRACTION),
                height: Val::Percent(WINDOW_FRACTION),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                // Rounded corners only look rounded if what's behind them is
                // cut off: the title bar and the left pane both paint into them.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("import-window"),
        ))
        .id();
    commands.entity(scrim).add_child(root);

    let title = build_title_bar(commands, fonts);
    let tabs = build_tab_bar(commands, fonts);

    // Body: left list · centre viewport · right rail.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    let left = build_left_pane(commands, fonts, init, has_project);
    let split_l = splitter(commands, Side::Left);
    let centre = build_centre(commands, fonts);
    let split_r = splitter(commands, Side::Right);
    let right = build_right_rail(commands, fonts, init);
    commands
        .entity(body)
        .add_children(&[left, split_l, centre, split_r, right]);

    commands.entity(root).add_children(&[title, tabs, body]);
}

/// A drag handle between two columns.
///
/// The visible line is 2px; the hit area is 12px, because a hairline target is
/// unhittable in practice and this one is dragged, not just clicked.
fn splitter(commands: &mut Commands, side: Side) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(12.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            // Without this the press falls through to the 3D viewport behind
            // and starts a selection while you are dragging the column.
            FocusPolicy::Block,
            Splitter(side),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::ColResize),
        ))
        .id();
    let line = commands
        .spawn((
            Node {
                width: Val::Px(2.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
            FocusPolicy::Pass,
        ))
        .id();
    bind_bg(commands, line, move |w| {
        if matches!(
            w.get::<Interaction>(bar),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(accent())
        } else {
            rgb(border())
        }
    });
    commands.entity(bar).add_child(line);
    bar
}

/// Drag a splitter to resize its column. Latches on press so the drag survives
/// the cursor leaving the 7px strip, which it always does immediately.
fn splitter_drag(
    q: Query<(&Interaction, &Splitter, &bevy::ui::ComputedNode)>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mut columns: ResMut<ImportColumns>,
    mut held: Local<Option<(Side, f32)>>,
) {
    if held.is_none() && mouse.just_pressed(MouseButton::Left) {
        for (i, sp, cn) in &q {
            if *i == Interaction::Hovered || *i == Interaction::Pressed {
                // Mouse motion arrives in *physical* pixels while `Val::Px` is
                // logical, so on a scaled display the handle drifted away from
                // the cursor. Latch the node's conversion factor with the drag.
                *held = Some((sp.0, cn.inverse_scale_factor()));
                break;
            }
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        *held = None;
        return;
    }
    let Some((side, inv)) = *held else { return };
    let dx = motion.delta.x * inv;
    if dx == 0.0 {
        return;
    }
    match side {
        Side::Left => columns.left = (columns.left + dx).clamp(180.0, 720.0),
        // The right rail grows as the cursor moves *left*.
        Side::Right => columns.right = (columns.right - dx).clamp(200.0, 720.0),
    }
}

fn build_title_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(16.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "cube", accent(), 17.0);
    let title = commands
        .spawn((
            Text::new("Import".to_string()),
            ui_font(&fonts.ui, 15.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    // Subtitle tracks the staged file so the window says what you are looking at.
    let sub = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, sub, |w| {
        let Some(state) = w.get_resource::<ImportOverlayState>() else {
            return String::new();
        };
        // Which file is on screen is the switcher's job to say; this only
        // counts them. Before anything stages, the queue names itself.
        match state.staged.len() {
            0 => import_title(w),
            n => format!("— {} of {n} ready", state.active + 1),
        }
    });
    let switcher = build_model_switcher(commands);
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    // Progress and the verdict buttons live here rather than in a footer: the
    // decision belongs next to what it is about, and a full-height window has
    // no natural bottom edge to anchor a bar to.
    let progress = build_header_progress(commands, fonts);
    let actions = build_actions(commands, fonts);
    commands
        .entity(bar)
        .add_children(&[icon, title, sub, switcher, spacer, progress, actions]);
    bar
}

/// The header's model switcher: pick which staged file the window is showing.
///
/// A batch import stages every file and waits, so the window is always showing
/// one of several — and the only way to change which was to go back to the Files
/// tab, losing whichever tab you were working in. The dropdown moves that where
/// it belongs, next to the name of the thing it changes.
///
/// Wrapped in a one-item keyed list because a dropdown's options are fixed when
/// it is built, and this set changes as files finish converting and as they are
/// added or skipped. The list rebuilds the widget when the names (or the
/// selection) change, and does nothing on the frames where they don't.
fn build_model_switcher(commands: &mut Commands) -> Entity {
    let holder = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::left(Val::Px(4.0)),
            ..default()
        })
        .id();
    keyed_list(commands, holder, |w: &Rx| {
        let names: Vec<String> = w
            .get_resource::<ImportOverlayState>()
            .map(|s| s.staged.iter().map(|st| st.file_name.clone()).collect())
            .unwrap_or_default();
        let active = w
            .get_resource::<ImportOverlayState>()
            .map(|s| s.active)
            .unwrap_or(0);
        // One file is not a choice; the subtitle already names it.
        let items = if names.len() > 1 {
            vec![(0u64, hash_of((&names, active)))]
        } else {
            Vec::new()
        };
        KeyedSnapshot {
            items,
            build: Box::new(move |c, f, _| {
                let labels: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                let dd = dropdown_compact(c, f, &labels, active.min(labels.len() - 1), 210.0);
                bind_2way(
                    c,
                    dd,
                    |w| w.get_resource::<ImportOverlayState>().map(|s| s.active).unwrap_or(0),
                    |w, v: &usize| {
                        let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() else {
                            return;
                        };
                        if s.active != *v && *v < s.staged.len() {
                            s.active = *v;
                        }
                    },
                );
                dd
            }),
        }
    });
    holder
}

fn build_tab_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let mut kids = Vec::new();
    for (tab, label) in [
        (ImportTab::Files, "Files"),
        (ImportTab::Scene, "Scene"),
        (ImportTab::Meshes, "Meshes"),
        (ImportTab::Materials, "Materials"),
        (ImportTab::Destination, "Destination"),
    ] {
        let t = tab_button(commands, fonts, label, tab);
        // Scene / Meshes / Materials describe a converted model, so they only
        // exist once one has been staged. Files and Destination always apply.
        if !matches!(tab, ImportTab::Files | ImportTab::Destination) {
            bind_display(commands, t, has_staged);
        }
        kids.push(t);
    }
    commands.entity(bar).add_children(&kids);
    bar
}

fn tab_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, tab: ImportTab) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(14.0)),
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            Interaction::default(),
            TabBtn(tab),
            hover_cursor(),
        ))
        .id();
    // The active tab is marked by the underline rather than a fill, so the bar
    // stays quiet with four of them side by side.
    bind_with(
        commands,
        btn,
        move |w| active_tab(w) == tab,
        move |world, e, active| {
            let c = if *active { rgb(accent()) } else { Color::NONE };
            if let Some(mut b) = world.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(c);
            }
        },
    );
    let txt = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    bind_with(
        commands,
        txt,
        move |w| active_tab(w) == tab,
        move |world, e, active| {
            let c = if *active { text_primary() } else { text_muted() };
            if let Some(mut t) = world.get_mut::<TextColor>(e) {
                t.0 = rgb(c);
            }
        },
    );
    commands.entity(btn).add_child(txt);
    btn
}

// ── Left pane ────────────────────────────────────────────────────────────────

fn build_left_pane(commands: &mut Commands, fonts: &EmberFonts, init: &Init, has_project: bool) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(310.0),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    bind_column_width(commands, col, Side::Left);

    // Files — drop zone + queue.
    let files = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, files, |w| active_tab(w) == ImportTab::Files);
    let browse_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let b1 = pill_button(commands, fonts, "file", "Files");
    commands.entity(b1).insert(FileBrowseBtn);
    let b2 = pill_button(commands, fonts, "folder-open", "Folder");
    commands.entity(b2).insert(FolderBrowseBtn);
    commands.entity(browse_row).add_children(&[b1, b2]);
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FilesContainer,
        ))
        .id();
    keyed_list(commands, list, files_snapshot);
    let staged_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, staged_list, staged_snapshot);
    let stack = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    commands.entity(stack).add_children(&[staged_list, list]);
    let files_scroll = scroll_view(commands, stack);
    commands
        .entity(files)
        .add_children(&[browse_row, files_scroll]);

    // Scene — flattened tree with expand state.
    let scene = list_pane(commands, ImportTab::Scene, scene_snapshot);
    let meshes = list_pane(commands, ImportTab::Meshes, meshes_snapshot);
    let materials = list_pane(commands, ImportTab::Materials, materials_snapshot);

    // Destination — where a committed import lands.
    let dest = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, dest, |w| active_tab(w) == ImportTab::Destination);
    let mut dest_kids = Vec::new();
    if has_project {
        let tree = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .id();
        let mut rows = vec![dest_folder_row(commands, fonts, String::new(), 0, "assets")];
        for (rel, depth, name) in &init.dest_folders {
            rows.push(dest_folder_row(commands, fonts, rel.clone(), depth + 1, name));
        }
        commands.entity(tree).add_children(&rows);
        dest_kids.push(scroll_view(commands, tree));
    }
    let org = radio_group(
        commands,
        &fonts.ui,
        &["Folder per file", "All in one folder"],
        init.layout,
    );
    bind_2way(
        commands,
        org,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.layout) {
            Some(ImportLayout::Combined) => 1usize,
            _ => 0,
        },
        |w, v: &usize| {
            if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
                s.layout = if *v == 1 {
                    ImportLayout::Combined
                } else {
                    ImportLayout::PerFileFolder
                };
            }
        },
    );
    dest_kids.push(org);
    commands.entity(dest).add_children(&dest_kids);

    commands
        .entity(col)
        .add_children(&[files, scene, meshes, materials, dest]);
    col
}

/// A tab-gated scrolling keyed list, used for the tree and the two flat lists.
fn list_pane(commands: &mut Commands, tab: ImportTab, snapshot: fn(&Rx) -> KeyedSnapshot) -> Entity {
    let holder = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    bind_display(commands, holder, move |w| active_tab(w) == tab);
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, snapshot);
    let scroll = scroll_view(commands, list);
    commands.entity(holder).add_child(scroll);
    holder
}

// ── Centre ───────────────────────────────────────────────────────────────────

fn build_centre(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let centre = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                // No padding: the render fills the region edge to edge, so
                // there is no letterbox between it and the columns.
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
        ))
        .id();

    // The staged model, filling the region.
    let view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            Interaction::default(),
            // Blocks the press from reaching the editor viewport behind; the
            // orbit handler reads this node's own `Interaction`.
            FocusPolicy::Block,
            crate::preview3d::ImportPreviewViewport,
        ))
        .id();
    bind_display(commands, view, |w| has_staged(w) && !showing_material(w));
    bind_with(
        commands,
        view,
        |w| {
            w.get_resource::<crate::preview3d::ImportPreviewImage>()
                .map(|i| i.handle.id())
        },
        |world, entity, _| {
            let Some(handle) = crate::preview3d::preview_image(world) else {
                return;
            };
            if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                node.image = handle;
            }
        },
    );

    // The selected material, shown in the main viewport rather than a
    // thumbnail in the rail — a 190px square is not enough to judge a surface.
    let mat_view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            Interaction::default(),
            FocusPolicy::Block,
            crate::matpreview::MaterialPreviewViewport,
        ))
        .id();
    bind_display(commands, mat_view, showing_material);
    bind_with(
        commands,
        mat_view,
        |w| {
            w.get_resource::<crate::matpreview::MaterialPreviewImage>()
                .map(|i| i.handle.id())
        },
        |world, entity, _| {
            let Some(handle) = crate::matpreview::preview_image(world) else {
                return;
            };
            if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                node.image = handle;
            }
        },
    );

    // Before anything is staged the centre explains what the window is for.
    let placeholder = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, placeholder, |w| !has_staged(w));
    let ph_icon = icon_text(commands, &fonts.phosphor, "cube-transparent", text_muted(), 40.0);
    let ph_text = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, ph_text, |w| {
        let Some(s) = w.get_resource::<ImportOverlayState>() else {
            return String::new();
        };
        // Conversion starts on its own once files are chosen, so this reports
        // what is happening rather than asking for another click.
        if s.active_task.is_some() {
            return match &s.progress {
                ImportProgress::Working { label, .. } if !label.is_empty() => label.clone(),
                _ => "Converting…".to_string(),
            };
        }
        match s.pending_files.len() {
            0 => "Choose a model to import".to_string(),
            1 => "1 file queued".to_string(),
            n => format!("{n} files queued"),
        }
    });
    commands
        .entity(placeholder)
        .add_children(&[ph_icon, ph_text]);

    commands.entity(centre).add_children(&[view, mat_view, placeholder]);
    centre
}

// ── Right rail ───────────────────────────────────────────────────────────────

fn build_right_rail(commands: &mut Commands, fonts: &EmberFonts, init: &Init) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(320.0),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    bind_column_width(commands, col, Side::Right);

    let inner = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();

    // ── Selected-item properties (staged only) ──────────────────────────
    let props = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, props, has_staged);
    let props_head = group_label(commands, fonts, "Properties");
    let props_body = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.mono, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, props_body, selection_properties);
    commands
        .entity(props)
        .add_children(&[props_head, props_body]);

    // ── Findings (staged only) ──────────────────────────────────────────
    let findings = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, findings, has_staged);
    let f_head = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
        ))
        .id();
    bind_text(commands, f_head, |w| {
        staged(w)
            .map(|s| match s.problems() {
                0 => "FINDINGS — nothing looks wrong".to_string(),
                n => format!("FINDINGS — {n} to look at"),
            })
            .unwrap_or_default()
    });
    let f_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, f_list, findings_snapshot);
    commands.entity(findings).add_children(&[f_head, f_list]);

    // ── Import settings (before staging) ────────────────────────────────
    let settings = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let s_head = group_label(commands, fonts, "Import");

    let scale = drag_value(commands, &fonts.ui, "", text_primary(), init.scale, 0.01);
    bind_2way(
        commands,
        scale,
        |w| g_settings(w, |s| s.scale),
        |w, v: &f32| {
            s_settings(w, |s| s.scale = (*v).clamp(0.001, 1000.0));
            // Only reached when the widget's value differs from state, i.e. the
            // user scrubbed or typed — so it marks a deliberate choice and stops
            // the next queue auto-detecting over the top of it.
            if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
                s.scale_is_user_set = true;
            }
        },
    );
    let scale_row = field_row(commands, fonts, "Scale", scale);

    let axis = dropdown(
        commands,
        fonts,
        &["Auto", "Y-Up (GLTF/Bevy)", "Z-Up (Blender/CAD)"],
        init.up_axis,
    );
    bind_2way(
        commands,
        axis,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.up_axis) {
            Some(UpAxis::YUp) => 1usize,
            Some(UpAxis::ZUp) => 2,
            _ => 0,
        },
        |w, v: &usize| {
            s_settings(w, |s| {
                s.up_axis = match v {
                    1 => UpAxis::YUp,
                    2 => UpAxis::ZUp,
                    _ => UpAxis::Auto,
                }
            })
        },
    );
    let axis_row = field_row(commands, fonts, "Up axis", axis);

    // How the scene graph comes out. `Combined` is what the transcoders do
    // today; `One node per mesh` is the way to undo it and get pickable,
    // independently-culled objects back.
    let structure = dropdown(
        commands,
        fonts,
        &["As authored", "One node per mesh", "Combine meshes"],
        init.structure,
    );
    bind_2way(
        commands,
        structure,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.structure) {
            Some(SceneStructure::FlatPerMesh) => 1usize,
            Some(SceneStructure::Combined) => 2,
            _ => 0,
        },
        |w, v: &usize| {
            s_settings(w, |s| {
                s.structure = match v {
                    1 => SceneStructure::FlatPerMesh,
                    2 => SceneStructure::Combined,
                    _ => SceneStructure::Preserve,
                }
            })
        },
    );
    let structure_row = field_row(commands, fonts, "Hierarchy", structure);

    // Sibling texture sets, for a format that stores no materials of its own.
    // Only built when the queue actually offers some, so the row is absent
    // rather than empty for every other format.
    let texture_set_row = (!init.texture_sets.is_empty()).then(|| {
        let mut labels = vec!["None".to_string()];
        labels.extend(
            init.texture_sets
                .iter()
                .map(|(stem, roles)| format!("{stem}  ({roles})")),
        );
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let picker = dropdown(commands, fonts, &refs, init.texture_set);
        // The set list is captured rather than re-read: it is fixed for the
        // window's lifetime, and the binding stores the *name* so the choice
        // survives a reimport even if the folder gains a file.
        let stems: Vec<String> = init.texture_sets.iter().map(|(s, _)| s.clone()).collect();
        let get_stems = stems.clone();
        bind_2way(
            commands,
            picker,
            move |w| {
                w.get_resource::<ImportOverlayState>()
                    .and_then(|s| s.settings.texture_set.clone())
                    .and_then(|want| get_stems.iter().position(|s| *s == want))
                    .map_or(0usize, |i| i + 1)
            },
            move |w, v: &usize| {
                let chosen = v.checked_sub(1).and_then(|i| stems.get(i).cloned());
                s_settings(w, |s| s.texture_set = chosen);
            },
        );
        field_row(commands, fonts, "Textures", picker)
    });

    let flip = toggle_row(commands, fonts, "Flip UVs", |s| s.flip_uvs, |s, v| s.flip_uvs = v);
    let normals = toggle_row(
        commands,
        fonts,
        "Generate normals",
        |s| s.generate_normals,
        |s, v| s.generate_normals = v,
    );

    let e_head = group_label(commands, fonts, "Extract");
    let e1 = toggle_row(commands, fonts, "Skeleton + skin", |s| s.extract_skeleton, |s, v| s.extract_skeleton = v);
    let e2 = toggle_row(commands, fonts, "Animations", |s| s.extract_animations, |s, v| s.extract_animations = v);
    let e3 = toggle_row(commands, fonts, "Textures", |s| s.extract_textures, |s, v| s.extract_textures = v);
    let e4 = toggle_row(commands, fonts, "Materials", |s| s.extract_materials, |s, v| s.extract_materials = v);

    let o_head = group_label(commands, fonts, "Optimize");
    let o1 = toggle_row(commands, fonts, "Vertex cache", |s| s.optimize_vertex_cache, |s, v| s.optimize_vertex_cache = v);
    let o2 = toggle_row(commands, fonts, "Overdraw", |s| s.optimize_overdraw, |s, v| s.optimize_overdraw = v);
    let o3 = toggle_row(commands, fonts, "Vertex fetch", |s| s.optimize_vertex_fetch, |s, v| s.optimize_vertex_fetch = v);

    let mut kids = vec![s_head, scale_row, axis_row, structure_row];
    kids.extend(texture_set_row);
    kids.extend([
        flip, normals, e_head, e1, e2, e3, e4, o_head, o1, o2, o3,
    ]);
    // `add_children` takes a slice, and the settings column is past the
    // tuple-bundle limit, so build the vector and hand it over in one call.
    commands.entity(settings).add_children(&kids);

    // Per-file results from the last run. Hidden until something has been
    // logged, so the rail is not carrying an empty heading most of the time.
    let results = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    bind_display(commands, results, |w| {
        !has_staged(w)
            && w.get_resource::<ImportOverlayState>()
                .is_some_and(|s| !s.log_entries.is_empty())
    });
    let r_head = group_label(commands, fonts, "Results");
    let r_list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            LogContainer,
        ))
        .id();
    keyed_list(commands, r_list, log_snapshot);
    commands.entity(results).add_children(&[r_head, r_list]);

    commands
        .entity(inner)
        .add_children(&[props, findings, settings, results]);
    let scroll = scroll_view(commands, inner);
    commands.entity(col).add_child(scroll);
    col
}

/// Keep a column's width in step with [`ImportColumns`].
fn bind_column_width(commands: &mut Commands, target: Entity, side: Side) {
    bind_with(
        commands,
        target,
        move |w| {
            let c = w.get_resource::<ImportColumns>();
            let v = match side {
                Side::Left => c.map(|c| c.left).unwrap_or(310.0),
                Side::Right => c.map(|c| c.right).unwrap_or(320.0),
            };
            // Bindings compare by value, and f32 is not Eq — round to whole
            // pixels so this only fires when the width actually changes.
            v.round() as i32
        },
        |world, e, px| {
            if let Some(mut node) = world.get_mut::<Node>(e) {
                node.width = Val::Px(*px as f32);
            }
        },
    );
}

/// A small uppercase group heading for the right rail.
fn group_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id()
}

// ── Footer ───────────────────────────────────────────────────────────────────

/// The verdict buttons. Returns a row for the title bar to host.
fn build_actions(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    // Before anything has converted there is nothing to decide on, so the only
    // action is to give up on the window. Conversion itself needs no button:
    // choosing the files is the instruction, and `auto_start_import` acts on it.
    let cancel = action_button(commands, fonts, "x", "Cancel", text_primary());
    commands.entity(cancel).insert(CancelBtn);
    bind_display(commands, cancel, |w| !has_staged(w));
    bind_bg(commands, cancel, |_| rgb(section_bg()));

    // Verdict.
    let discard = action_button(commands, fonts, "x-circle", "Discard all", RED);
    commands.entity(discard).insert(DiscardAllBtn);
    bind_display(commands, discard, has_staged);
    bind_bg(commands, discard, |_| rgb(section_bg()));
    let skip = action_button(commands, fonts, "skip-forward", "Skip", text_primary());
    commands.entity(skip).insert(SkipBtn);
    bind_display(commands, skip, has_staged);
    bind_bg(commands, skip, |_| rgb(section_bg()));
    // The one action that writes anything into the project, and the only place
    // the word "import" would have been ambiguous — everything up to here has
    // happened in the project's cache directory.
    let commit = action_button(commands, fonts, "check-circle", "Add to project", (255, 255, 255));
    commands.entity(commit).insert(CommitBtn);
    bind_display(commands, commit, has_staged);
    bind_bg(commands, commit, |_| rgb(accent()));

    commands
        .entity(row)
        .add_children(&[cancel, discard, skip, commit]);
    row
}

fn action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    fg: (u8, u8, u8),
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                min_width: Val::Px(112.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            hover_cursor(),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, fg, 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let tx = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(fg)),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, tx]);
    btn
}

// ── Window state ─────────────────────────────────────────────────────────────

/// Read the staged import, if the worker is waiting on a verdict.
fn staged(w: &Rx) -> Option<crate::staged::StagedImport> {
    w.get_resource::<ImportOverlayState>()
        .and_then(|s| s.current().cloned())
}

/// True while a converted file is staged and awaiting a verdict.
fn has_staged(w: &Rx) -> bool {
    w.get_resource::<ImportOverlayState>()
        .is_some_and(|s| !s.staged.is_empty())
}

/// True when the Materials tab has a selection, which is when the viewport
/// shows the material sphere instead of the model.
fn showing_material(w: &Rx) -> bool {
    has_staged(w)
        && active_tab(w) == ImportTab::Materials
        && w.get_resource::<ImportNav>()
            .is_some_and(|n| n.sel_material.is_some())
}

fn active_tab(w: &Rx) -> ImportTab {
    w.get_resource::<ImportNav>()
        .map(|n| n.tab)
        .unwrap_or(ImportTab::Files)
}

/// A scene-tree row's include-checkbox: what it draws, and what it toggles.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RowCheck {
    /// The part of the model this box speaks for.
    item: TreeItem,
    /// Is it going into the project?
    checked: bool,
    /// False once an ancestor is unchecked — the row is coming out either way,
    /// so its own box is shown off and does not respond.
    enabled: bool,
}

/// Most rows in this window are a name plus muted detail, optionally indented,
/// with an expand caret and a selected state. One builder covers all of them.
struct RowSpec<'a> {
    label: &'a str,
    detail: &'a str,
    icon: &'a str,
    depth: usize,
    /// `Some(open)` draws a caret; `None` leaves the space blank.
    caret: Option<bool>,
    selected: bool,
    /// `Some` draws the include-checkbox that decides whether this part of the
    /// model is imported. `None` is a row that is only ever shown.
    check: Option<RowCheck>,
    /// Draw the row as excluded without giving it a checkbox — for the mesh and
    /// material lists, which follow what the scene tree was told rather than
    /// being told anything themselves.
    dim: bool,
}

impl RowSpec<'_> {
    /// A row with no checkbox — the shape every list other than the scene tree
    /// wants.
    fn plain<'a>(label: &'a str, detail: &'a str, icon: &'a str) -> RowSpec<'a> {
        RowSpec { label, detail, icon, depth: 0, caret: None, selected: false, check: None, dim: false }
    }
}

fn list_row(commands: &mut Commands, fonts: &EmberFonts, spec: RowSpec) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::left(Val::Px(4.0 + spec.depth as f32 * 13.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(if spec.selected {
                rgb(accent()).with_alpha(0.22)
            } else {
                Color::NONE
            }),
            Interaction::default(),
            // Rows are click targets; without blocking, the press also reaches
            // whatever is stacked behind the window.
            FocusPolicy::Block,
            hover_cursor(),
        ))
        .id();

    // Excluded rows read as struck-through-in-spirit: everything on them drops
    // to the muted colour, so a glance down the tree separates what is being
    // imported from what is not without having to read each checkbox.
    let included = !spec.dim && spec.check.is_none_or(|c| c.checked && c.enabled);
    let label_color = if included { text_primary() } else { text_muted() };

    let mut kids = Vec::new();
    if let Some(check) = spec.check {
        let box_e = row_checkbox(commands, fonts, check);
        // A disabled box carries no marker, which is what makes it inert: the
        // click handler only ever sees boxes that are allowed to be clicked.
        if check.enabled {
            commands.entity(box_e).insert(TreeCheck(check.item));
        }
        kids.push(box_e);
    }
    match spec.caret {
        Some(open) => {
            let c = icon_text(
                commands,
                &fonts.phosphor,
                if open { "caret-down" } else { "caret-right" },
                text_muted(),
                9.0,
            );
            // The caret is its own click target so expanding does not also
            // change the selection.
            commands.entity(c).insert((Interaction::default(), hover_cursor()));
            kids.push(c);
        }
        None => {
            kids.push(commands.spawn(Node { width: Val::Px(9.0), ..default() }).id());
        }
    }
    let ic = icon_text(commands, &fonts.phosphor, spec.icon, text_muted(), 11.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    kids.push(ic);
    let nm = commands
        .spawn((
            Text::new(spec.label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(label_color)),
            FocusPolicy::Pass,
        ))
        .id();
    kids.push(nm);
    if !spec.detail.is_empty() {
        let dt = commands
            .spawn((
                Text::new(spec.detail.to_string()),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_muted())),
                Node { flex_grow: 1.0, ..default() },
                FocusPolicy::Pass,
            ))
            .id();
        kids.push(dt);
    }
    commands.entity(row).add_children(&kids);
    row
}

/// The include-checkbox on a scene-tree row.
///
/// Hand-rolled rather than [`renzora_ember::widgets::checkbox`] because that one
/// owns its state in a `Bound<bool>` it flips on click, and these rows are
/// rebuilt from the exclusion set whenever it changes — two sources of truth for
/// the same tick, where the widget's would win the frame and then be overwritten.
/// This one only reports the press; what it draws comes from the rebuild.
fn row_checkbox(commands: &mut Commands, fonts: &EmberFonts, state: RowCheck) -> Entity {
    let on = state.checked && state.enabled;
    let fill = match (on, state.enabled) {
        (true, true) => rgb(accent()),
        (true, false) => rgb(accent()).with_alpha(0.35),
        _ => Color::NONE,
    };
    let box_e = commands
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(13.0),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(if state.enabled {
                rgb(border())
            } else {
                rgb(border()).with_alpha(0.4)
            }),
            Interaction::default(),
            // Without blocking, the press also lands on the row behind it and
            // toggling a node would change the selection at the same time.
            FocusPolicy::Block,
            hover_cursor(),
        ))
        .id();
    if on {
        let mark = icon_text(commands, &fonts.phosphor, "check", on_accent(), 9.0);
        commands.entity(mark).insert(FocusPolicy::Pass);
        commands.entity(box_e).add_child(mark);
    }
    box_e
}

// ── Scene tree ───────────────────────────────────────────────────────────────

/// Walk the scene graph depth-first, descending only into expanded rows, and
/// return what should be visible.
///
/// Flattened rather than built as nested widgets because a scene can carry well
/// over a thousand nodes and the reactive list rebuilds on every dirty frame —
/// nesting a thousand collapsible widgets to show twenty of them is the shape
/// that makes an ember panel drop frames.
/// Each row is `(item, depth, disabled)`, where `disabled` means an ancestor is
/// unchecked — the row is coming out of the import whatever its own box says.
fn visible_tree_rows(
    stats: &renzora_import::GlbStats,
    expanded: &std::collections::HashSet<TreeItem>,
    excluded: &renzora_import::PruneSpec,
) -> Vec<(TreeItem, usize, bool)> {
    /// Guard against a pathological expand — one node with thousands of
    /// children would otherwise build thousands of rows in a 310px pane.
    const MAX_ROWS: usize = 500;
    let mut out: Vec<(TreeItem, usize, bool)> = Vec::new();

    struct Walk<'a> {
        stats: &'a renzora_import::GlbStats,
        expanded: &'a std::collections::HashSet<TreeItem>,
        excluded: &'a renzora_import::PruneSpec,
    }

    fn walk(
        w: &Walk,
        idx: usize,
        depth: usize,
        disabled: bool,
        out: &mut Vec<(TreeItem, usize, bool)>,
        max: usize,
    ) {
        if out.len() >= max {
            return;
        }
        let Some(node) = w.stats.node_list.get(idx) else {
            return;
        };
        let item = TreeItem::Node(idx);
        out.push((item, depth, disabled));
        if !w.expanded.contains(&item) {
            return;
        }
        // Unchecking a node takes its whole subtree with it, so everything
        // below this point is disabled once it is excluded.
        let below = disabled || w.excluded.nodes.contains(&idx);
        // The mesh first, then child nodes — geometry belongs to this node,
        // children are separate objects.
        if let Some(mi) = node.mesh {
            if let Some(mesh) = w.stats.mesh_list.get(mi) {
                let m_item = TreeItem::Mesh(mi);
                out.push((m_item, depth + 1, below));
                if w.expanded.contains(&m_item) {
                    let prim_disabled = below || w.excluded.meshes.contains(&mi);
                    for k in 0..mesh.primitives.len() {
                        if out.len() >= max {
                            return;
                        }
                        out.push((TreeItem::Prim(mi, k), depth + 2, prim_disabled));
                    }
                }
            }
        }
        for &child in &node.children {
            walk(w, child, depth + 1, below, out, max);
        }
    }

    let w = Walk { stats, expanded, excluded };
    for &root in &stats.roots {
        walk(&w, root, 0, false, &mut out, MAX_ROWS);
    }
    out
}

/// What an import with the current checkboxes would actually contain:
/// `(meshes, materials)`, by glTF index.
///
/// The mesh and material lists use this to show what is on its way out. They
/// have no checkboxes of their own — a material is not a thing you can uncheck,
/// it is a thing that stops being used once nothing references it — so this is
/// the same reachability walk the prune does, run for display.
fn surviving(
    stats: &renzora_import::GlbStats,
    excluded: &renzora_import::PruneSpec,
) -> (std::collections::HashSet<usize>, std::collections::HashSet<usize>) {
    let mut meshes = std::collections::HashSet::new();
    let mut materials = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<usize> = stats.roots.clone();
    while let Some(n) = stack.pop() {
        if excluded.nodes.contains(&n) || !seen.insert(n) {
            continue;
        }
        let Some(node) = stats.node_list.get(n) else {
            continue;
        };
        stack.extend(node.children.iter().copied());
        let Some(mi) = node.mesh.filter(|mi| !excluded.meshes.contains(mi)) else {
            continue;
        };
        let Some(mesh) = stats.mesh_list.get(mi) else {
            continue;
        };
        let live: Vec<usize> = (0..mesh.primitives.len())
            .filter(|k| !excluded.prims.contains(&(mi, *k)))
            .collect();
        if live.is_empty() {
            continue;
        }
        meshes.insert(mi);
        materials.extend(live.iter().filter_map(|&k| mesh.primitives[k].material));
    }
    (meshes, materials)
}

/// Is this row's own box ticked, ignoring whether an ancestor overrules it?
fn item_included(excluded: &renzora_import::PruneSpec, item: TreeItem) -> bool {
    match item {
        TreeItem::Node(i) => !excluded.nodes.contains(&i),
        TreeItem::Mesh(mi) => !excluded.meshes.contains(&mi),
        TreeItem::Prim(mi, k) => !excluded.prims.contains(&(mi, k)),
    }
}

/// Whether a tree row can be opened, and the label/detail/icon it shows.
fn tree_row_parts(
    stats: &renzora_import::GlbStats,
    item: TreeItem,
) -> (String, String, &'static str, bool) {
    match item {
        TreeItem::Node(i) => {
            let Some(n) = stats.node_list.get(i) else {
                return (format!("Node {i}"), String::new(), "cube", false);
            };
            let expandable = !n.children.is_empty() || n.mesh.is_some();
            let detail = if n.children.is_empty() {
                String::new()
            } else {
                format!("{} children", n.children.len())
            };
            let icon = if n.mesh.is_some() { "cube" } else { "circles-three" };
            (n.name.clone(), detail, icon, expandable)
        }
        TreeItem::Mesh(mi) => {
            let Some(m) = stats.mesh_list.get(mi) else {
                return (format!("Mesh {mi}"), String::new(), "polygon", false);
            };
            (
                m.name.clone(),
                format!("{} tris", thousands(m.triangles())),
                "polygon",
                m.primitives.len() > 1,
            )
        }
        TreeItem::Prim(mi, k) => {
            let name = stats
                .mesh_list
                .get(mi)
                .and_then(|m| m.primitives.get(k))
                .and_then(|p| p.material)
                .and_then(|x| stats.material_names.get(x))
                .cloned()
                .unwrap_or_else(|| format!("Surface {k}"));
            let tris = stats
                .mesh_list
                .get(mi)
                .and_then(|m| m.primitives.get(k))
                .map(|p| p.triangles)
                .unwrap_or(0);
            (name, format!("{} tris", thousands(tris)), "circle-half-tilt", false)
        }
    }
}

fn scene_snapshot(world: &Rx) -> KeyedSnapshot {
    let empty = || KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|_, _, _| Entity::PLACEHOLDER),
    };
    let Some(st) = staged(world) else { return empty() };
    let Some(stats) = st.stats.clone() else { return empty() };
    let (expanded, selected) = world
        .get_resource::<ImportNav>()
        .map(|n| (n.expanded.clone(), n.sel_item))
        .unwrap_or_default();

    let built: Vec<TreeRowData> = visible_tree_rows(&stats, &expanded, &st.excluded)
        .into_iter()
        .map(|(item, depth, disabled)| {
            let (label, detail, _icon, expandable) = tree_row_parts(&stats, item);
            TreeRowData {
                item,
                depth,
                label,
                detail,
                caret: expandable.then(|| expanded.contains(&item)),
                selected: selected == Some(item),
                check: RowCheck {
                    item,
                    checked: item_included(&st.excluded, item),
                    enabled: !disabled,
                },
            }
        })
        .collect();

    let items: Vec<(u64, u64)> = built
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                i as u64,
                hash_of((r.item, r.depth, &r.label, &r.detail, r.caret, r.selected, r.check)),
            )
        })
        .collect();
    let stats_for_build = stats.clone();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let r = &built[i];
            let (_, _, icon, _) = tree_row_parts(&stats_for_build, r.item);
            let row = list_row(
                c,
                f,
                RowSpec {
                    label: &r.label,
                    detail: &r.detail,
                    icon,
                    depth: r.depth,
                    caret: r.caret,
                    selected: r.selected,
                    check: Some(r.check),
                    dim: false,
                },
            );
            c.entity(row).insert(TreeRow(r.item));
            row
        }),
    }
}

/// One built scene-tree row, ready to hash and to spawn.
struct TreeRowData {
    item: TreeItem,
    depth: usize,
    label: String,
    detail: String,
    caret: Option<bool>,
    selected: bool,
    check: RowCheck,
}

// ── Mesh + material lists ────────────────────────────────────────────────────

/// The staged models, so a multi-file import can be flipped through. Each row
/// carries its findings count, which is the thing worth comparing across a
/// batch — one bad file in twenty is easy to miss otherwise.
fn staged_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ImportOverlayState>() else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let active = state.active;
    let rows: Vec<(usize, String, String, bool)> = state
        .staged
        .iter()
        .enumerate()
        .map(|(i, st)| {
            let detail = match st.problems() {
                0 => human_bytes(st.glb_bytes as u64),
                n => format!("{n} to look at"),
            };
            (i, st.file_name.clone(), detail, i == active)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| (r.0 as u64, hash_of((r.0, &r.1, &r.2, r.3))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, detail, selected) = &rows[i];
            let row = list_row(
                c,
                f,
                RowSpec {
                    selected: *selected,
                    ..RowSpec::plain(name, detail, "cube")
                },
            );
            c.entity(row).insert(StagedRow(*idx));
            row
        }),
    }
}

/// Switch the window to another staged model. Selections are per-file, so they
/// reset — index 4 in one model is unrelated to index 4 in the next.
fn staged_row_click(
    q: Query<(&Interaction, &StagedRow), Changed<Interaction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
    mut nav: Option<ResMut<ImportNav>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, r) in &q {
        if *i == Interaction::Pressed && r.0 < state.staged.len() {
            let changed = state.active != r.0;
            state.active = r.0;
            if let Some(nav) = nav.as_mut() {
                if changed {
                    nav.reset_selection();
                }
                // Always move to Scene, so clicking a row that is already
                // active still does something visible rather than sitting dead.
                nav.tab = ImportTab::Scene;
            }
        }
    }
}

fn meshes_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(st) = staged(world) else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let Some(stats) = st.stats.clone() else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let selected = world.get_resource::<ImportNav>().and_then(|n| n.sel_mesh);
    let (live_meshes, _) = surviving(&stats, &st.excluded);
    let rows: Vec<(usize, String, String, bool, bool)> = stats
        .mesh_list
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let dim = !live_meshes.contains(&i);
            let detail = format!(
                "{} prims · {} tris{}",
                m.primitives.len(),
                thousands(m.triangles()),
                if dim { EXCLUDED_SUFFIX } else { "" }
            );
            (i, m.name.clone(), detail, selected == Some(i), dim)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| (r.0 as u64, hash_of((r.0, &r.1, &r.2, r.3, r.4))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, detail, selected, dim) = &rows[i];
            let row = list_row(
                c,
                f,
                RowSpec {
                    selected: *selected,
                    dim: *dim,
                    ..RowSpec::plain(name, detail, "polygon")
                },
            );
            c.entity(row).insert(MeshRow(*idx));
            row
        }),
    }
}

/// What a mesh or material row says when the scene tree has left it with
/// nothing referencing it.
const EXCLUDED_SUFFIX: &str = " · not imported";

fn materials_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(st) = staged(world) else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let selected = world.get_resource::<ImportNav>().and_then(|n| n.sel_material);
    // A material with nothing left using it is one the commit will drop, along
    // with its `.material` file and any texture only it read.
    let live_materials = st
        .stats
        .as_ref()
        .map(|stats| surviving(stats, &st.excluded).1);
    let rows: Vec<(usize, String, String, bool, bool)> = st
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let dim = live_materials.as_ref().is_some_and(|live| !live.contains(&i));
            let detail = format!(
                "{}{}{}",
                m.alpha_mode,
                if m.double_sided { " · 2-sided" } else { "" },
                if dim { EXCLUDED_SUFFIX } else { "" }
            );
            (i, m.name.clone(), detail, selected == Some(i), dim)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| (r.0 as u64, hash_of((r.0, &r.1, &r.2, r.3, r.4))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, detail, selected, dim) = &rows[i];
            let row = list_row(
                c,
                f,
                RowSpec {
                    selected: *selected,
                    dim: *dim,
                    ..RowSpec::plain(name, detail, "circle-half-tilt")
                },
            );
            c.entity(row).insert(MatRow(*idx));
            row
        }),
    }
}

fn findings_snapshot(world: &Rx) -> KeyedSnapshot {
    let rows: Vec<(bool, String)> = staged(world)
        .map(|s| {
            s.flags
                .iter()
                .map(|f| (f.level == crate::staged::FlagLevel::Problem, f.text.clone()))
                .collect()
        })
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (i as u64, hash_of((i, r.0, &r.1))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (problem, text) = &rows[i];
            let row = c
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(5.0),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ))
                .id();
            let colour = if *problem { AMBER } else { text_muted() };
            let ic = icon_text(
                c,
                &f.phosphor,
                if *problem { "warning" } else { "info" },
                colour,
                11.0,
            );
            c.entity(ic).insert(FocusPolicy::Pass);
            let tx = c
                .spawn((
                    Text::new(text.clone()),
                    ui_font(&f.ui, 10.5),
                    TextColor(rgb(if *problem { text_primary() } else { text_muted() })),
                    Node { flex_grow: 1.0, ..default() },
                    FocusPolicy::Pass,
                ))
                .id();
            c.entity(row).add_children(&[ic, tx]);
            row
        }),
    }
}

/// The right rail's properties block: whatever the active tab has selected,
/// falling back to a summary of the whole import when nothing is.
fn selection_properties(w: &Rx) -> String {
    let Some(st) = staged(w) else {
        return String::new();
    };
    let nav = w.get_resource::<ImportNav>();
    let stats = st.stats.as_ref();

    match nav.map(|n| n.tab) {
        Some(ImportTab::Scene) => {
            if let (Some(stats), Some(item)) = (stats, nav.and_then(|n| n.sel_item)) {
                match item {
                    TreeItem::Node(idx) => {
                        if let Some(node) = stats.node_list.get(idx) {
                            let mesh = node
                                .mesh
                                .and_then(|m| stats.mesh_list.get(m))
                                .map(|m| format!("mesh        {}", m.name))
                                .unwrap_or_else(|| "mesh        (none)".to_string());
                            return format!(
                                "node        {}\nchildren    {}\ntransform   {}\n{}",
                                node.name,
                                node.children.len(),
                                if node.has_transform { "yes" } else { "identity" },
                                mesh
                            );
                        }
                    }
                    TreeItem::Mesh(mi) => {
                        if let Some(m) = stats.mesh_list.get(mi) {
                            return format!(
                                "mesh        {}\nsurfaces    {}\ntriangles   {}\nvertices    {}",
                                m.name,
                                m.primitives.len(),
                                thousands(m.triangles()),
                                thousands(m.vertices())
                            );
                        }
                    }
                    TreeItem::Prim(mi, k) => {
                        if let Some(p) = stats.mesh_list.get(mi).and_then(|m| m.primitives.get(k)) {
                            let mat = p
                                .material
                                .and_then(|x| stats.material_names.get(x))
                                .cloned()
                                .unwrap_or_else(|| "(none)".into());
                            return format!(
                                "surface     {}\nmaterial    {}\ntriangles   {}\nvertices    {}\nattributes  {}",
                                k,
                                mat,
                                thousands(p.triangles),
                                thousands(p.vertices),
                                p.attributes.join(" ")
                            );
                        }
                    }
                }
            }
        }
        Some(ImportTab::Meshes) => {
            if let (Some(stats), Some(idx)) = (stats, nav.and_then(|n| n.sel_mesh)) {
                if let Some(m) = stats.mesh_list.get(idx) {
                    let mut out = format!(
                        "name        {}\nprimitives  {}\ntriangles   {}\nvertices    {}\n",
                        m.name,
                        m.primitives.len(),
                        thousands(m.triangles()),
                        thousands(m.vertices())
                    );
                    for (i, p) in m.primitives.iter().take(8).enumerate() {
                        let mat = p
                            .material
                            .and_then(|mi| stats.material_names.get(mi))
                            .cloned()
                            .unwrap_or_else(|| "(none)".into());
                        out.push_str(&format!(
                            "\n  [{}] {}\n      {} tris · {}",
                            i,
                            mat,
                            thousands(p.triangles),
                            p.attributes.join(" ")
                        ));
                    }
                    if m.primitives.len() > 8 {
                        out.push_str(&format!("\n  … {} more", m.primitives.len() - 8));
                    }
                    return out;
                }
            }
        }
        Some(ImportTab::Materials) => {
            if let Some(idx) = nav.and_then(|n| n.sel_material) {
                if let Some(m) = st.materials.get(idx) {
                    return format!(
                        "name        {}\nalpha       {}\ntwo-sided   {}\nmetallic    {:.3}\nroughness   {:.3}\nbase color  {:.2} {:.2} {:.2} {:.2}\ntextures    {}",
                        m.name,
                        m.alpha_mode,
                        if m.double_sided { "yes" } else { "no" },
                        m.metallic,
                        m.roughness,
                        m.base_color[0],
                        m.base_color[1],
                        m.base_color[2],
                        m.base_color[3],
                        if m.slots.is_empty() {
                            "none".to_string()
                        } else {
                            m.slots.join(", ")
                        }
                    );
                }
            }
        }
        _ => {}
    }

    // Nothing selected — describe the import as a whole, leading with where it
    // came from. Without the full path it is genuinely hard to tell two files
    // with the same stem apart, and a wrong pick reads as a broken importer.
    let source = st.source.display().to_string();
    let Some(s) = stats else {
        return format!("source
  {source}

No structure could be read from the converted model.");
    };
    format!(
        "source
  {source}

{}",
        format_args!(
        "nodes       {}\nmeshes      {}  ({} prims)\ntriangles   {}\nvertices    {}\nmaterials   {}\ntextures    {}  ({})\nanimations  {}\nskins       {}\nattributes  {}\nGLB         {}",
        thousands(s.nodes),
        thousands(s.meshes),
        thousands(s.primitives),
        thousands(s.triangles),
        thousands(s.vertices),
        thousands(s.materials),
        thousands(st.textures.len()),
        human_bytes(st.texture_bytes),
        thousands(st.animations.len()),
        thousands(s.skins),
        if s.attributes.is_empty() {
            "none".to_string()
        } else {
            s.attributes.join(" ")
        },
        human_bytes(st.glb_bytes as u64),
        )
    )
}

fn hash_of<T: std::hash::Hash>(v: T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

// ── Window interaction ───────────────────────────────────────────────────────

fn tab_click(q: Query<(&Interaction, &TabBtn), Changed<Interaction>>, mut nav: Option<ResMut<ImportNav>>) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, t) in &q {
        if *i == Interaction::Pressed {
            nav.tab = t.0;
        }
    }
}

/// Clicking a tree row selects it; clicking an openable one also toggles it,
/// which is what a single-column tree in a narrow pane wants — a 9px caret is
/// too small to be the only way to expand.
fn tree_row_click(
    q: Query<(&Interaction, &TreeRow), Changed<Interaction>>,
    mut nav: Option<ResMut<ImportNav>>,
    state: Option<Res<ImportOverlayState>>,
) {
    let (Some(nav), Some(state)) = (nav.as_mut(), state) else {
        return;
    };
    let stats = state.current().and_then(|s| s.stats.as_ref());
    for (i, r) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        nav.sel_item = Some(r.0);
        // Selecting a surface also points the Materials tab at its material, so
        // the two views agree about what you are looking at.
        if let (TreeItem::Prim(mi, k), Some(stats)) = (r.0, stats) {
            nav.sel_material = stats
                .mesh_list
                .get(mi)
                .and_then(|m| m.primitives.get(k))
                .and_then(|p| p.material);
        }
        if let (TreeItem::Mesh(mi), Some(_)) = (r.0, stats) {
            nav.sel_mesh = Some(mi);
        }
        let expandable = stats
            .map(|st| tree_row_parts(st, r.0).3)
            .unwrap_or(false);
        if expandable && !nav.expanded.insert(r.0) {
            nav.expanded.remove(&r.0);
        }
    }
}

/// Tick or untick a part of the model.
///
/// Unticking is a subtree operation: the node goes, and any entry its children
/// had of their own goes with it — they are implied now, and dropping them is
/// what lets ticking the parent again restore the whole branch in one click,
/// which is what "check the parent, check the children" has to mean for the box
/// to be worth having.
fn tree_check_click(
    q: Query<(&Interaction, &TreeCheck), Changed<Interaction>>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    let Some(item) = q
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| c.0)
    else {
        return;
    };
    let Some(stats) = state.current().and_then(|s| s.stats.clone()) else {
        return;
    };
    let active = state.active;
    let Some(staged) = state.staged.get_mut(active) else {
        return;
    };
    let ex = &mut staged.excluded;
    match item {
        TreeItem::Node(i) => {
            let subtree = subtree_of(&stats, i);
            let was_included = !ex.nodes.contains(&i);
            for &n in &subtree {
                ex.nodes.remove(&n);
                if let Some(mi) = stats.node_list.get(n).and_then(|n| n.mesh) {
                    ex.meshes.remove(&mi);
                    ex.prims.retain(|(m, _)| *m != mi);
                }
            }
            if was_included {
                ex.nodes.insert(i);
            }
        }
        TreeItem::Mesh(mi) => {
            let was_included = !ex.meshes.contains(&mi);
            ex.prims.retain(|(m, _)| *m != mi);
            if was_included {
                ex.meshes.insert(mi);
            } else {
                ex.meshes.remove(&mi);
            }
        }
        TreeItem::Prim(mi, k) => {
            if !ex.prims.remove(&(mi, k)) {
                ex.prims.insert((mi, k));
            }
        }
    }
}

/// A node and everything under it, guarded against a cycle in a malformed file.
fn subtree_of(stats: &renzora_import::GlbStats, root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if !seen.insert(n) {
            continue;
        }
        out.push(n);
        if let Some(node) = stats.node_list.get(n) {
            stack.extend(node.children.iter().copied());
        }
    }
    out
}

fn mesh_row_click(q: Query<(&Interaction, &MeshRow), Changed<Interaction>>, mut nav: Option<ResMut<ImportNav>>) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, r) in &q {
        if *i == Interaction::Pressed {
            nav.sel_mesh = Some(r.0);
        }
    }
}

fn mat_row_click(q: Query<(&Interaction, &MatRow), Changed<Interaction>>, mut nav: Option<ResMut<ImportNav>>) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, r) in &q {
        if *i == Interaction::Pressed {
            nav.sel_material = Some(r.0);
        }
    }
}

/// Answer the blocked worker and reset the window back to its file-picking
/// state, since the next staged file (if any) arrives fresh.
fn decide(world: &mut World, decision: crate::staged::PreviewDecision) {
    if world.resource::<ImportOverlayState>().staged.is_empty() {
        return;
    }
    crate::overlay::apply_decision(world, decision);
    if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
        nav.reset_selection();
    }
}

fn commit_click(q: Query<&Interaction, (With<CommitBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| decide(w, crate::staged::PreviewDecision::Commit));
    }
}

fn skip_click(q: Query<&Interaction, (With<SkipBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| decide(w, crate::staged::PreviewDecision::Skip));
    }
}

fn discard_all_click(
    q: Query<&Interaction, (With<DiscardAllBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| decide(w, crate::staged::PreviewDecision::CancelAll));
    }
}

/// When a file stages, open it: switch to the Scene tab, expand the roots so
/// the tree is not a single collapsed line, and point the 3D preview at the
/// staged GLB. When the verdict clears it, tear the preview down so its camera
/// stops rendering.
fn on_staged_changed(world: &mut World) {
    let path = world
        .get_resource::<ImportOverlayState>()
        .and_then(|s| s.current().map(|st| st.glb_path.clone()));
    let Some(path) = path else {
        crate::preview3d::clear(world);
        return;
    };

    let already = world
        .get_resource::<crate::preview3d::ImportPreview>()
        .and_then(|p| p.path.clone())
        .as_deref()
        == Some(path.as_path());
    if !already {
        let roots = world
            .get_resource::<ImportOverlayState>()
            .and_then(|s| s.current())
            .and_then(|s| s.stats.as_ref())
            .map(|st| st.roots.clone())
            .unwrap_or_default();
        if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
            nav.tab = ImportTab::Scene;
            nav.reset_selection();
            nav.expanded.extend(roots.into_iter().map(TreeItem::Node));
        }
        // The window has to be up for the user to answer; an inspecting import
        // must never hand off to the corner toast.
        let mut s = world.resource_mut::<ImportOverlayState>();
        s.visible = true;
        s.toast_active = false;
    }
    crate::preview3d::show(world, &path);
}





/// Header label reflecting the queue: uniform-kind queues get a specific title,
/// empty / mixed queues get the generic "Import Assets".
fn import_title(w: &Rx) -> String {
    use crate::kinds::{detect_kind, AssetKind};
    let Some(state) = w.get_resource::<ImportOverlayState>() else {
        return "Import Assets".to_string();
    };
    if state.pending_files.is_empty() {
        return "Import Assets".to_string();
    }
    let kinds: Vec<AssetKind> = state
        .pending_files
        .iter()
        .filter_map(|q| detect_kind(&q.path))
        .collect();
    let first = kinds.first().copied();
    let uniform = first.is_some_and(|k| kinds.iter().all(|&x| x == k));
    match first.filter(|_| uniform) {
        Some(AssetKind::Model) => "Import 3D Models",
        Some(AssetKind::Image) => "Import Images",
        Some(AssetKind::Audio) => "Import Audio",
        Some(AssetKind::Scene) => "Import Scenes",
        Some(AssetKind::Particle) => "Import Particles",
        Some(AssetKind::Material) => "Import Materials",
        Some(AssetKind::Font) => "Import Fonts",
        Some(AssetKind::Script) => "Import Scripts",
        Some(AssetKind::GaussianSplat) => "Import Gaussian Splats",
        None => "Import Assets",
    }
    .to_string()
}



// ── Panes ────────────────────────────────────────────────────────────────────









/// One selectable row in the destination folder tree. `rel` is the
/// project-relative target path (`""` = project root); selection highlights the
/// row whose path matches `ImportOverlayState::target_directory`.
fn dest_folder_row(commands: &mut Commands, fonts: &EmberFonts, rel: String, depth: usize, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(8.0 + depth as f32 * 14.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DestFolderRow(rel.clone()),
            hover_cursor(),
        ))
        .id();
    let p = rel.clone();
    bind_bg(commands, row, move |w| {
        let selected = w.get_resource::<ImportOverlayState>().map(|s| s.target_directory == p).unwrap_or(false);
        if selected {
            rgb(accent()).with_alpha(0.20)
        } else if matches!(w.get::<Interaction>(row), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "folder", text_muted(), 12.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    commands.entity(row).add_children(&[icon, lbl]);
    row
}

/// Recursively list the project's directories (two levels deep) as
/// project-relative forward-slashed paths, skipping hidden / build / dependency
/// folders. Mirrors the marketplace install picker's `scan_dirs`.
fn scan_dest_dirs(root: &std::path::Path) -> Vec<(String, usize, String)> {
    fn rec(root: &std::path::Path, dir: &std::path::Path, depth: usize, max: usize, out: &mut Vec<(String, usize, String)>) {
        if depth > max || out.len() > 300 {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            let rel = path.strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            out.push((rel, depth, name));
            rec(root, &path, depth + 1, max, out);
        }
    }
    let mut out = Vec::new();
    rec(root, root, 0, 1, &mut out);
    out
}

/// A compact spinner + label for the title bar. Shows only while a conversion
/// is actually running; the verdict buttons beside it carry the rest.
fn build_header_progress(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(7.0),
            margin: UiRect::right(Val::Px(10.0)),
            ..default()
        })
        .id();
    bind_display(commands, row, |w| {
        w.get_resource::<ImportOverlayState>()
            .is_some_and(|s| matches!(s.progress, ImportProgress::Working { .. }))
    });
    let spin = spinner(commands);
    let label = txt(commands, fonts, "", 11.5, text_muted());
    bind_text(commands, label, |w| {
        match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
            Some(ImportProgress::Working { current, total, label }) => {
                if label.is_empty() {
                    format!("[{current}/{total}]")
                } else {
                    format!("[{current}/{total}] {label}")
                }
            }
            _ => String::new(),
        }
    });
    commands.entity(row).add_children(&[spin, label]);
    row
}


// ── Keyed list (files) ─────────────────────────────────────────────────────────

fn files_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    use crate::kinds::QueuedAsset;
    let files: Vec<QueuedAsset> = world
        .get_resource::<ImportOverlayState>()
        .map(|s| s.pending_files.clone())
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = files
        .iter()
        .map(|q| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&q.path, &q.relative_dir).hash(&mut h);
            (h.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| file_row(c, f, &files[i])),
    }
}

/// Row label for a queued asset: the bare filename for a flat pick, or the
/// mirrored `sub/dir/file.png` path for a folder import.
///
/// Deep pack paths are elided in the middle (`Pack/…/textures/a.png`). The row
/// is a fixed 26px, so an un-elided path wraps out of it — and the filename is
/// the half worth keeping, which a plain right-clip would be the half to lose.
fn queued_label(asset: &crate::kinds::QueuedAsset) -> String {
    let file = asset.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    if asset.relative_dir.is_empty() {
        return file.to_string();
    }
    const MAX: usize = 52;
    let full = format!("{}/{}", asset.relative_dir, file);
    if full.chars().count() <= MAX {
        return full;
    }
    // Keep the root folder (which pack this is) and the tail (where in it).
    let segs: Vec<&str> = asset.relative_dir.split('/').collect();
    let root = segs.first().copied().unwrap_or("");
    let tail = segs.last().copied().unwrap_or("");
    if segs.len() > 2 {
        format!("{}/…/{}/{}", root, tail, file)
    } else {
        format!("{}/…/{}", root, file)
    }
}

fn file_row(commands: &mut Commands, fonts: &EmberFonts, asset: &crate::kinds::QueuedAsset) -> Entity {
    let path = &asset.path;
    let name = queued_label(asset);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(26.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), padding: UiRect::axes(Val::Px(7.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            FocusPolicy::Pass,
        ))
        .id();
    let (glyph, color) = crate::kinds::kind_icon(path);
    let icon = icon_text(commands, &fonts.phosphor, glyph, color, 12.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() })).id();
    let ex = commands.spawn((Text::new(ext), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    let rm = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), RemoveFileBtn(path.to_path_buf()), hover_cursor())).id();
    let rmx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    commands.entity(rmx).insert(FocusPolicy::Pass);
    commands.entity(rm).add_child(rmx);
    commands.entity(row).add_children(&[icon, nm, ex, rm]);
    row
}

fn log_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let entries: Vec<(String, bool, String)> = world.get_resource::<ImportOverlayState>().map(|s| s.log_entries.iter().map(|e| (e.file_name.clone(), e.success, e.message.clone())).collect()).unwrap_or_default();
    let items: Vec<(u64, u64)> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (i, &e.0, e.1, &e.2).hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| log_row(c, f, &entries[i])) }
}

fn log_row(commands: &mut Commands, fonts: &EmberFonts, e: &(String, bool, String)) -> Entity {
    let (name, ok, msg) = e;
    let row = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, FocusPolicy::Pass)).id();
    let icon = icon_text(commands, &fonts.phosphor, if *ok { "check-circle" } else { "warning" }, if *ok { GREEN } else { RED }, 11.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    let mc = if *ok { text_muted() } else { RED };
    let mg = commands.spawn((Text::new(msg.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(mc)), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() })).id();
    commands.entity(row).add_children(&[icon, nm, mg]);
    row
}

// ── Interaction ──────────────────────────────────────────────────────────────

#[derive(PartialEq)]
struct OrderedF32(f32);
fn progress_fraction(w: &Rx) -> OrderedF32 {
    let f = match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, .. }) if total > 0 => current as f32 / total as f32,
        _ => 0.0,
    };
    OrderedF32(f)
}

/// Convert whatever is queued, as soon as it is queued.
///
/// There used to be an Import button whose only job was to start the conversion
/// the user had already asked for by choosing the files, and it was misnamed
/// besides: nothing it did touched the project. Every model converts into the
/// project's cache and waits there, so starting early costs nothing and buys the
/// user a preview by the time they have finished looking at the queue. The
/// decision that matters is Add to project, at the other end.
///
/// Files added to an open window join the ones already staged rather than
/// replacing them, which is what makes dropping a second batch mid-inspection
/// work.
fn auto_start_import(world: &mut World) {
    let ready = {
        let Some(s) = world.get_resource::<ImportOverlayState>() else {
            return;
        };
        s.visible
            && !s.pending_files.is_empty()
            && s.active_task.is_none()
            // A queued reconvert owns the next run; starting one here would
            // race it into the same staging directories.
            && !s.reimport_requested
            // `Error` holds until something new is queued — `enqueue` clears it
            // — so a file that cannot convert doesn't retry forever.
            && matches!(s.progress, ImportProgress::Idle | ImportProgress::Done(_))
    };
    // The worker writes into the project's cache directory, so there has to be
    // a project.
    if !ready || world.get_resource::<renzora::core::CurrentProject>().is_none() {
        return;
    }
    run_import(world);
}

/// How long the settings have to stop changing before the window reconverts.
/// Long enough to drag a scale field across its range as one edit rather than
/// forty.
const SETTINGS_SETTLE_SECS: f64 = 0.9;

/// Reconvert when the import settings change under a staged model.
///
/// Without this the settings rail would be dead controls after the first
/// conversion: the model on screen was built with the old values, and the only
/// thing that could rebuild it was the Reimport button this replaces. Making it
/// automatic is what lets the window be "it converts, you adjust, you add".
///
/// The destination counts as a setting here — the worker bakes the final paths
/// into each staged import and into the `.material` writes it is holding, so
/// pointing the window at another folder has to rebuild them too.
fn settings_watch(
    world: &mut World,
    mut seen: Local<Option<crate::overlay::ConvertedWith>>,
    mut due: Local<Option<f64>>,
) {
    let Some(state) = world.get_resource::<ImportOverlayState>() else {
        return;
    };
    if !state.visible {
        *seen = None;
        *due = None;
        return;
    }
    let now = crate::overlay::ConvertedWith {
        settings: state.settings.clone(),
        target_directory: state.target_directory.clone(),
        layout: state.layout,
    };
    let changed_this_frame = seen.as_ref().is_some_and(|prev| *prev != now);
    let differs = state.converted_with.as_ref().is_some_and(|c| *c != now);
    let idle = state.active_task.is_none() && !state.reimport_requested;
    let staged = !state.staged.is_empty();
    *seen = Some(now);

    let elapsed = world
        .get_resource::<Time>()
        .map(|t| t.elapsed_secs_f64())
        .unwrap_or(0.0);
    if !differs {
        // Back to what is already on disk — including a value edited away and
        // then edited back, which needs no work at all.
        *due = None;
        return;
    }
    // Push the deadline out on every keystroke or drag tick, so a value being
    // scrubbed reconverts once, when it settles.
    if changed_this_frame || due.is_none() {
        *due = Some(elapsed + SETTINGS_SETTLE_SECS);
    }
    let Some(at) = *due else { return };
    // A change made *during* a conversion stays armed rather than being
    // dropped: the run in flight is building the model with the old value, so
    // the reconvert is still owed once it finishes.
    if elapsed < at || !idle || !staged {
        return;
    }
    *due = None;
    crate::overlay::request_reimport(world);
    if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
        nav.reset_selection();
    }
}

fn cancel_click(q: Query<&Interaction, (With<CancelBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| close_overlay(w));
    }
}

fn remove_file_click(q: Query<(&Interaction, &RemoveFileBtn), Changed<Interaction>>, mut state: Option<ResMut<ImportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, rm) in &q {
        if *i == Interaction::Pressed {
            state.pending_files.retain(|q| q.path != rm.0);
        }
    }
}

fn file_browse_click(q: Query<&Interaction, (With<FileBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| { pick_and_queue_files(w); });
    }
}

fn folder_browse_click(q: Query<&Interaction, (With<FolderBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| { pick_and_queue_folder(w); });
    }
}

/// Open the OS file picker (filtered to every importable kind) and append the
/// chosen files to the queue. Returns `true` if at least one new file was added.
/// Shared by the asset-browser Import trigger (`lib.rs`) and the overlay's own
/// **Browse files** button, so both honour the same filter and de-dup rules.
pub(crate) fn pick_and_queue_files(world: &mut World) -> bool {
    let Some(paths) = crate::kinds::pick_importable_files() else {
        return false;
    };
    let assets: Vec<_> = paths.into_iter().map(crate::kinds::QueuedAsset::flat).collect();
    world.resource_mut::<ImportOverlayState>().enqueue(&assets)
}

/// Open the OS folder picker, expand it (mirroring the source tree), and
/// append to the queue. A folder with nothing importable in it reports that in
/// the overlay's message line instead of leaving the button looking dead.
pub(crate) fn pick_and_queue_folder(world: &mut World) -> bool {
    let Some((dir, assets)) = crate::kinds::pick_importable_folder() else {
        return false;
    };
    if assets.is_empty() {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("that folder")
            .to_string();
        world.resource_mut::<ImportOverlayState>().progress =
            ImportProgress::Error(format!("No importable files in {}", name));
        return false;
    }
    world.resource_mut::<ImportOverlayState>().enqueue(&assets)
}

/// Click a destination folder row → it becomes the import target directory.
fn dest_folder_click(q: Query<(&Interaction, &DestFolderRow), Changed<Interaction>>, mut state: Option<ResMut<ImportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, row) in &q {
        if *i == Interaction::Pressed && state.target_directory != row.0 {
            state.target_directory = row.0.clone();
        }
    }
}

// ── Corner progress toast ──────────────────────────────────────────────────────

/// Owns the corner progress toast: polls the running import, spawns/despawns the
/// toast entity, and auto-dismisses a few seconds after the import finishes.
fn manage_import_toast(world: &mut World) {
    let active = world.resource::<ImportOverlayState>().toast_active;
    if active {
        poll_import_task(world); // keep the bar moving while the modal is closed
    }

    // Once the import reaches a terminal state, arm a short auto-dismiss timer
    // so the success/error toast lingers briefly before clearing itself.
    if active {
        let terminal = matches!(
            world.resource::<ImportOverlayState>().progress,
            ImportProgress::Done(_) | ImportProgress::Error(_)
        );
        if terminal {
            let now = world.resource::<Time>().elapsed_secs_f64();
            let dismiss_at = world.resource::<ImportOverlayState>().toast_dismiss_at;
            match dismiss_at {
                None => world.resource_mut::<ImportOverlayState>().toast_dismiss_at = Some(now + 5.0),
                Some(t) if now >= t => {
                    let mut s = world.resource_mut::<ImportOverlayState>();
                    s.toast_active = false;
                    s.toast_dismiss_at = None;
                    s.progress = ImportProgress::Idle;
                    s.pending_files.clear();
                    s.log_entries.clear();
                }
                _ => {}
            }
        }
    }

    let want = world.resource::<ImportOverlayState>().toast_active;
    let mut q = world.query_filtered::<Entity, With<ToastRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();
    if want && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_toast(&mut commands, &fonts);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_toast(commands: &mut Commands, fonts: &EmberFonts) {
    // Fixed bottom-right card, above the viewport chrome.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(9200),
            OverlaySurface,
            ToastRoot,
            Name::new("import-toast"),
        ))
        .id();

    // Header: title + dismiss ×.
    let header = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let title = icon_label(commands, fonts, "download-simple", "Importing assets", text_primary(), 12.0);
    let close = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), ToastDismissBtn, hover_cursor())).id();
    let close_x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 13.0);
    commands.entity(close_x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(close_x);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(root).add_child(header);

    // Working: label + progress bar.
    let working = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }).id();
    let toprow = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let spin = spinner(commands);
    let plabel = txt(commands, fonts, "", 11.0, text_muted());
    bind_text(commands, plabel, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, label }) => format!("[{current}/{total}] {label}"),
        _ => "Starting…".to_string(),
    });
    commands.entity(toprow).add_children(&[spin, plabel]);
    let track = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(rgb(section_bg())))).id();
    let fill = commands.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(accent())))).id();
    bind_with(commands, fill, progress_fraction, |w, target, v: &OrderedF32| { if let Some(mut n) = w.get_mut::<Node>(target) { n.width = Val::Percent((v.0 * 100.0).clamp(0.0, 100.0)); } });
    commands.entity(track).add_child(fill);
    commands.entity(working).add_children(&[toprow, track]);
    bind_display(commands, working, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Working { .. }) | Some(ImportProgress::Idle)));
    commands.entity(root).add_child(working);

    // Done / Error result lines.
    let (done, done_msg) = icon_msg(commands, fonts, "check-circle", GREEN);
    bind_text(commands, done_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Done(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, done, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Done(_))));
    commands.entity(root).add_child(done);

    let (err, err_msg) = icon_msg(commands, fonts, "warning", RED);
    bind_text(commands, err_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Error(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, err, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Error(_))));
    commands.entity(root).add_child(err);
}

fn toast_dismiss_click(q: Query<&Interaction, (With<ToastDismissBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ImportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        // Hide the toast. A still-running import keeps writing in the background;
        // dismissing only removes the notification.
        state.toast_active = false;
        state.toast_dismiss_at = None;
        if matches!(state.progress, ImportProgress::Done(_) | ImportProgress::Error(_)) {
            state.progress = ImportProgress::Idle;
            state.pending_files.clear();
            state.log_entries.clear();
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn hover_cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}


fn icon_label(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, color: (u8, u8, u8), size: f32) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, size);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

/// An icon + a bindable message text. Returns `(row, message_text_entity)`.
fn icon_msg(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8)) -> (Entity, Entity) {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let t = commands.spawn((Text::new(String::new()), ui_font(&fonts.ui, 12.0), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    (row, t)
}

/// A settings row: a left-aligned label and a right-aligned control.
fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: Val::Px(12.0), min_height: Val::Px(26.0), ..default() }).id();
    let t = txt(commands, fonts, label, 12.0, text_primary());
    commands.entity(row).add_children(&[t, control]);
    row
}

/// A boolean settings row: label on the left, checkbox on the right.
fn toggle_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&renzora_import::settings::ImportSettings) -> bool, set: fn(&mut renzora_import::settings::ImportSettings, bool)) -> Entity {
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, move |w| g_settings(w, get), move |w, v: &bool| s_settings(w, |s| set(s, *v)));
    field_row(commands, fonts, label, cb)
}

fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(accent())), Interaction::default(), hover_cursor()))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, (255, 255, 255), 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}


fn g_settings<T>(w: &Rx, get: impl Fn(&renzora_import::settings::ImportSettings) -> T) -> T
where
    T: Default,
{
    w.get_resource::<ImportOverlayState>().map(|s| get(&s.settings)).unwrap_or_default()
}
fn s_settings(w: &mut World, set: impl FnOnce(&mut renzora_import::settings::ImportSettings)) {
    if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
        set(&mut s.settings);
    }
}
