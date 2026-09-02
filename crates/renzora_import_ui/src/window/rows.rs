//! What the window is looking at, and the one row builder every list uses.
//!
//! Almost every row in this window is a name plus muted detail, optionally
//! indented, with an expand caret, a selected state and — in the scene tree — an
//! include checkbox. [`list_row`] covers all of them, which is what keeps the
//! tree, the mesh list, the material list and the staged-file list visually the
//! same thing.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;

use crate::overlay::ImportOverlayState;

use super::widgets::hover_cursor;
use super::{ImportNav, ImportTab, TreeCheck, TreeItem};

/// Read the staged import, if the worker is waiting on a verdict.
pub(super) fn staged(w: &Rx) -> Option<crate::staged::StagedImport> {
    w.get_resource::<ImportOverlayState>()
        .and_then(|s| s.current().cloned())
}

/// True while a converted file is staged and awaiting a verdict.
pub(super) fn has_staged(w: &Rx) -> bool {
    w.get_resource::<ImportOverlayState>()
        .is_some_and(|s| !s.staged.is_empty())
}

/// True when the Materials tab has a selection, which is when the viewport
/// shows the material sphere instead of the model.
pub(super) fn showing_material(w: &Rx) -> bool {
    has_staged(w)
        && active_tab(w) == ImportTab::Materials
        && w.get_resource::<ImportNav>()
            .is_some_and(|n| n.sel_material.is_some())
}

pub(super) fn active_tab(w: &Rx) -> ImportTab {
    w.get_resource::<ImportNav>()
        .map(|n| n.tab)
        .unwrap_or(ImportTab::Files)
}

/// A scene-tree row's include-checkbox: what it draws, and what it toggles.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct RowCheck {
    /// The part of the model this box speaks for.
    pub(super) item: TreeItem,
    /// Is it going into the project?
    pub(super) checked: bool,
    /// False once an ancestor is unchecked — the row is coming out either way,
    /// so its own box is shown off and does not respond.
    pub(super) enabled: bool,
}

/// Most rows in this window are a name plus muted detail, optionally indented,
/// with an expand caret and a selected state. One builder covers all of them.
pub(super) struct RowSpec<'a> {
    pub(super) label: &'a str,
    pub(super) detail: &'a str,
    pub(super) icon: &'a str,
    pub(super) depth: usize,
    /// `Some(open)` draws a caret; `None` leaves the space blank.
    pub(super) caret: Option<bool>,
    pub(super) selected: bool,
    /// `Some` draws the include-checkbox that decides whether this part of the
    /// model is imported. `None` is a row that is only ever shown.
    pub(super) check: Option<RowCheck>,
    /// Draw the row as excluded without giving it a checkbox — for the mesh and
    /// material lists, which follow what the scene tree was told rather than
    /// being told anything themselves.
    pub(super) dim: bool,
}

impl RowSpec<'_> {
    /// A row with no checkbox — the shape every list other than the scene tree
    /// wants.
    pub(super) fn plain<'a>(label: &'a str, detail: &'a str, icon: &'a str) -> RowSpec<'a> {
        RowSpec { label, detail, icon, depth: 0, caret: None, selected: false, check: None, dim: false }
    }
}

pub(super) fn list_row(commands: &mut Commands, fonts: &EmberFonts, spec: RowSpec) -> Entity {
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
