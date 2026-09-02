//! The window's chrome: the scrim it floats on, the title bar, the tab bar, and
//! the splitters between the three regions.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown_compact, spinner, OverlaySurface};

use crate::overlay::{ImportOverlayState, ImportProgress};

use super::lifecycle::{import_title, Init};
use super::lists::hash_of;
use super::panes::{build_centre, build_left_pane, build_right_rail};
use super::rows::{active_tab, has_staged};
use super::widgets::{hover_cursor, txt};
use super::{
    CancelBtn, CommitBtn, DiscardAllBtn, ImportColumns, ImportRoot, ImportTab, Side, SkipBtn,
    Splitter, TabBtn,
};

/// Fraction of the screen the window occupies on each axis. It is a dialog, not
/// a workspace: at full bleed there was no visual cue that the editor was still
/// there behind it. The margin only has to read as one — the panes inside all
/// want the room, so it stays narrow.
const WINDOW_FRACTION: f32 = 90.0;

/// Build the import window: a centred panel with a tab bar, a left list pane, a
/// large centre viewport and a right properties rail, over a full-screen scrim.
///
/// The scrim, not the panel, is the [`ModalSurface`](renzora_ember::widgets::ModalSurface)
/// — it is what stops clicks reaching the editor around the panel's edges, and
/// the scroll and popup systems test for a modal *ancestor*, so it has to be the
/// root for the panel's contents to count as being inside one.
///
/// The layout is deliberately the same before and after conversion; only what
/// each region holds changes. Before, the left pane is the file queue, the
/// centre is a drop zone and the right rail is the import settings. After, the
/// left pane is the scene tree / mesh list / material list, the centre is the
/// staged model, and the right rail is the selected item's properties. Keeping
/// one frame means nothing jumps around when the conversion finishes.
pub(super) fn spawn_modal(
    commands: &mut Commands,
    fonts: &EmberFonts,
    init: &Init,
    has_project: bool,
) {
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
pub(super) fn splitter_drag(
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
    let discard = action_button(commands, fonts, "x-circle", "Discard all", super::RED);
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
