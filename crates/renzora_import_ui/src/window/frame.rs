//! The window's chrome: the scrim it floats on, the title bar, the tab bar, and
//! the splitters between the three regions.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_with};
use renzora_ember::theme::*;
use renzora_ember::widgets::OverlaySurface;

use super::lifecycle::Init;
use super::panes::{build_centre, build_left_pane, build_right_rail};
use super::rows::{active_tab, has_staged};
use super::widgets::hover_cursor;
use super::{CancelBtn, ImportColumns, ImportRoot, ImportTab, Side, Splitter, TabBtn};

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
pub(super) fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, init: &Init) {
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
    let left = build_left_pane(commands, fonts, init);
    let split_l = splitter(commands, Side::Left);
    let centre = build_centre(commands, fonts);
    let split_r = splitter(commands, Side::Right);
    let right = build_right_rail(commands, fonts, init);
    commands
        .entity(body)
        .add_children(&[left, split_l, centre, split_r, right]);

    commands.entity(root).add_children(&[title, body]);
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
/// the cursor leaving the 12px strip, which it always does immediately.
///
/// The width is computed from where the cursor **is**, not from how far the
/// mouse has moved. Accumulated device motion is not the same quantity as
/// cursor travel — the compositor applies pointer acceleration between the two
/// — so summing deltas drifted a little further from the handle with every
/// flick, and the line ended up visibly lagging behind the cursor that was
/// dragging it. Latching the cursor position and the width at press makes the
/// handle sit exactly under the pointer for the whole drag, however fast it
/// moves, and it self-corrects rather than accumulating error.
pub(super) fn splitter_drag(
    q: Query<(&Interaction, &Splitter)>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut columns: ResMut<ImportColumns>,
    mut held: Local<Option<Drag>>,
) {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    if held.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(pos) = cursor {
            for (i, sp) in &q {
                if *i == Interaction::Hovered || *i == Interaction::Pressed {
                    *held = Some(Drag {
                        side: sp.0,
                        start_x: pos.x,
                        start_width: match sp.0 {
                            Side::Left => columns.left,
                            Side::Right => columns.right,
                        },
                    });
                    break;
                }
            }
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        *held = None;
        return;
    }
    let (Some(drag), Some(pos)) = (*held, cursor) else {
        return;
    };
    let dx = pos.x - drag.start_x;
    match drag.side {
        Side::Left => columns.left = (drag.start_width + dx).clamp(180.0, 720.0),
        // The right rail grows as the cursor moves *left*.
        Side::Right => columns.right = (drag.start_width - dx).clamp(220.0, 720.0),
    }
}

/// A splitter drag in progress: which edge, and the two numbers the new width
/// is measured against.
#[derive(Clone, Copy)]
pub(super) struct Drag {
    side: Side,
    /// Cursor x, in the same logical pixels `Val::Px` uses.
    start_x: f32,
    start_width: f32,
}

fn build_title_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(52.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
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
    // The tabs live in this bar rather than in a strip of their own below it,
    // and Import lives here too. It is the shape the editor's own top bar
    // already has — identity on the left, navigation in the middle, actions on
    // the right — and it buys the preview a whole row of height back. A lone
    // Import button at the bottom of the narrow settings rail read as an
    // afterthought; up here it is where the window's other verdict already was.
    //
    // The header used to name the staged file too, with a count and a dropdown
    // to switch between them. Both are gone: the Files tab is that list, with
    // the selection, the per-file findings and the trash on each row, so the
    // header was a second, worse copy of it that pushed the tabs off centre by
    // however long the current file name happened to be.
    let left = group(commands, &[icon, title], 10.0);
    let tabs = build_tabs(commands, fonts);
    let commit = build_header_commit(commands, fonts);
    // Close sits where a window's close sits, at the far right.
    let close = build_header_close(commands, fonts);
    let right = group(commands, &[commit, close], 8.0);
    // Both spacers grow, so the tabs sit centred in whatever is left over.
    let spacer_l = commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(8.0), ..default() }).id();
    let spacer_r = commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(8.0), ..default() }).id();
    commands
        .entity(bar)
        .add_children(&[left, spacer_l, tabs, spacer_r, right]);
    bar
}

/// A horizontal, vertically-centred run of children. The header's three
/// regions each need one, and stretching them instead would make every text
/// node in the bar full height.
fn group(commands: &mut Commands, kids: &[Entity], gap: f32) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(gap),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    commands.entity(row).add_children(kids);
    row
}

/// The one action that writes anything into the project.
fn build_header_commit(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let commit = action_button(commands, fonts, "check-circle", "Import", (255, 255, 255), 0.0);
    commands.entity(commit).insert(super::CommitBtn);
    bind_display(commands, commit, has_staged);
    bind_bg(commands, commit, |_| rgb(super::GREEN));
    commit
}

/// The window's close control: an × in the title bar's right corner.
fn build_header_close(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            CancelBtn,
            hover_cursor(),
            renzora_ember::widgets::HoverTooltip::new("Close without importing"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(super::RED).with_alpha(0.25)
        } else {
            Color::NONE
        }
    });
    let x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 15.0);
    commands.entity(x).insert(FocusPolicy::Pass);
    commands.entity(btn).add_child(x);
    btn
}

fn build_tabs(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn(Node {
            height: Val::Percent(100.0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let mut kids = Vec::new();
    for (tab, icon, label) in [
        (ImportTab::Files, "files", "Files"),
        (ImportTab::Scene, "tree-structure", "Scene"),
        (ImportTab::Meshes, "polygon", "Meshes"),
        (ImportTab::Materials, "circle-half-tilt", "Materials"),
        (ImportTab::Destination, "folder-open", "Destination"),
    ] {
        let t = tab_button(commands, fonts, icon, label, tab);
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

fn tab_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    tab: ImportTab,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(7.0),
                // Tighter than a tab bar of its own would be: five of these now
                // share the header with the title, the progress readout and two
                // buttons, and the bar has to survive a narrow window.
                padding: UiRect::horizontal(Val::Px(13.0)),
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
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 15.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    bind_with(
        commands,
        ic,
        move |w| active_tab(w) == tab,
        move |world, e, active| {
            let c = if *active { accent() } else { text_muted() };
            if let Some(mut t) = world.get_mut::<TextColor>(e) {
                t.0 = rgb(c);
            }
        },
    );
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
            ui_font(&fonts.ui, 14.0),
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
    commands.entity(btn).add_children(&[ic, txt]);
    btn
}

pub(super) fn action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    fg: (u8, u8, u8),
    grow: f32,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                // A growing button must be allowed to shrink below its content,
                // or the flex line refuses to divide at narrow rail widths.
                min_width: if grow > 0.0 { Val::Px(0.0) } else { Val::Px(112.0) },
                flex_grow: grow,
                height: Val::Px(34.0),
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
