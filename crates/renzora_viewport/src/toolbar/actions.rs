//! Undo / redo / save / maximize — the buttons whose state comes from the
//! session rather than from the viewport.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_editor_framework::EditorCommands;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_glyph, icon_text, EmberFonts};
use renzora_ember::theme::text_primary;
use renzora_theme::ThemeManager;

use super::col;

/// One of the fixed "session action" buttons.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeaderAction {
    Undo,
    Redo,
    Save,
    Maximize,
}

/// Tags a Maximize button with the viewport slot it belongs to, so clicking it
/// maximizes THAT viewport (each viewport carries its own Maximize button).
#[derive(Component, Clone, Copy)]
pub(super) struct MaximizeSlot(pub(super) usize);

/// Points a button at its child glyph `Text` entity so the visuals system can
/// re-glyph / re-color it without walking children.
#[derive(Component)]
pub(super) struct HeaderIcon(Entity);

pub(super) fn action_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    action: HeaderAction,
    icon: &str,
    w: f32,
    h: f32,
    icon_px: f32,
) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), icon_px);
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(w),
                height: Val::Px(h),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            action,
            HeaderIcon(glyph),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-hdr-action"),
        ))
        .id();
    commands.entity(btn).add_child(glyph);
    btn
}

/// Honor the viewport "maximize" toggle on the bevy_ui shell's ember dock: swap
/// the dock to a viewport-only leaf while maximized and restore the saved tree
/// when un-maximized (the egui dock handles this itself in renzora_editor_framework).
pub(super) fn viewport_maximize_dock(
    max: Option<Res<renzora_ui::ViewportMaximized>>,
    dock: Option<ResMut<renzora_ember::dock::Dock>>,
    dirty: Option<ResMut<renzora_ember::dock::DockDirty>>,
    mut saved: Local<Option<renzora_ember::dock::DockTree>>,
    mut last: Local<Option<usize>>,
) {
    let (Some(mut dock), Some(mut dirty)) = (dock, dirty) else {
        return;
    };
    let maximized = max.and_then(|m| m.0);
    if maximized == *last {
        return;
    }
    let was_maximized = last.is_some();
    *last = maximized;
    if let Some(slot) = maximized {
        // Save the layout the first time we maximize (not when switching which
        // viewport is maximized — the saved tree must stay the un-maximized one).
        if !was_maximized {
            *saved = Some(dock.tree.clone());
        }
        let panel = crate::native_viewport::PANEL_IDS
            .get(slot)
            .copied()
            .unwrap_or("viewport");
        dock.tree = renzora_ember::dock::DockTree::leaf(panel);
    } else if let Some(tree) = saved.take() {
        dock.tree = tree;
    }
    dirty.0 = true;
}

/// Resolved palette + the booleans that drive each button's glyph, color, and
/// hover/active background.
struct HeaderModel {
    can_undo: bool,
    can_redo: bool,
    can_save: bool,
    /// Which viewport slot is currently maximized (if any).
    maximized: Option<usize>,
    primary: Color,
    muted: Color,
    accent: Color,
    /// Amber. Only the Save button uses it — an unsaved tab has to be visible at
    /// a glance from across the top bar, and `primary` (the same color as every
    /// other enabled button) wasn't.
    warning: Color,
    hovered_bg: Color,
}

pub(super) fn update_header_visuals(
    actions: Query<(
        &HeaderAction,
        &HeaderIcon,
        &Interaction,
        Option<&MaximizeSlot>,
        &mut BackgroundColor,
    )>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    undo: Option<Res<renzora_undo::UndoStacks>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    maximized: Option<Res<renzora_ui::ViewportMaximized>>,
    theme: Option<Res<ThemeManager>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;

    let (can_undo, can_redo) = undo
        .map(|s| (s.can_undo(&s.active), s.can_redo(&s.active)))
        .unwrap_or((false, false));
    let can_save = tabs
        .and_then(|tabs| tabs.tabs.get(tabs.active_tab).map(|t| t.is_modified))
        .unwrap_or(false);

    let model = HeaderModel {
        can_undo,
        can_redo,
        can_save,
        maximized: maximized.and_then(|m| m.0),
        primary: col(t.text.primary),
        muted: col(t.text.muted),
        accent: col(t.semantic.accent),
        warning: col(t.semantic.warning),
        hovered_bg: col(t.widgets.hovered_bg),
    };

    for (action, icon, interaction, max_slot, mut bg) in actions {
        // This maximize button is "active" only if ITS viewport is the maximized
        // one (each viewport has its own button).
        let this_maximized = *action == HeaderAction::Maximize
            && model.maximized == Some(max_slot.map(|m| m.0).unwrap_or(0));
        let (glyph_name, color, enabled) = action_appearance(action, &model, this_maximized);

        if let Ok((mut text, mut tc)) = texts.get_mut(icon.0) {
            if let Some(ch) = icon_glyph(glyph_name) {
                let s = ch.to_string();
                if text.0 != s {
                    text.0 = s;
                }
            }
            if tc.0 != color {
                tc.0 = color;
            }
        }

        // Background: maximize shows the accent while active; the rest just
        // light up on hover. Disabled buttons never show a hover fill.
        let want = if this_maximized {
            model.accent
        } else if enabled && *interaction == Interaction::Hovered {
            model.hovered_bg
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Glyph name, color, and whether the button is clickable for the given action.
/// `this_maximized` is whether THIS specific maximize button's viewport is the
/// maximized one (ignored for the other actions).
fn action_appearance(
    action: &HeaderAction,
    m: &HeaderModel,
    this_maximized: bool,
) -> (&'static str, Color, bool) {
    match action {
        HeaderAction::Undo => (
            "arrow-u-up-left",
            if m.can_undo { m.primary } else { m.muted },
            m.can_undo,
        ),
        HeaderAction::Redo => (
            "arrow-u-up-right",
            if m.can_redo { m.primary } else { m.muted },
            m.can_redo,
        ),
        // Save is never disabled — only tinted.
        //
        // Amber when there is known unsaved work, muted otherwise, but always
        // clickable. `can_save` tracks the editor's own dirty flag, and plenty
        // of real edits do not reach it: a UI template edit writes an `.html`,
        // a script writes a `.rs`, and neither marks the *scene* dirty. The
        // button then looked broken — the user had visibly changed something
        // and the control that saves was greyed out. Saving when nothing
        // changed costs a file write; refusing to save when something did is
        // the failure worth avoiding.
        HeaderAction::Save => (
            "floppy-disk",
            if m.can_save { m.warning } else { m.muted },
            true,
        ),
        HeaderAction::Maximize => (
            if this_maximized { "arrows-in" } else { "arrows-out" },
            if this_maximized { m.primary } else { m.muted },
            true,
        ),
    }
}

pub(super) fn header_action_click(
    q: Query<(&Interaction, &HeaderAction, Option<&MaximizeSlot>), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, action, max_slot) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            HeaderAction::Undo => cmds.push(|w: &mut World| renzora_undo::undo_once(w)),
            HeaderAction::Redo => cmds.push(|w: &mut World| renzora_undo::redo_once(w)),
            HeaderAction::Save => cmds.push(|w: &mut World| {
                w.insert_resource(renzora::core::SaveSceneRequested);
            }),
            HeaderAction::Maximize => {
                let slot = max_slot.map(|m| m.0).unwrap_or(0);
                cmds.push(move |w: &mut World| {
                    let mut m =
                        w.get_resource_or_insert_with(renzora_ui::ViewportMaximized::default);
                    // Toggle: maximizing the already-maximized viewport restores;
                    // otherwise maximize this one (swapping straight from another).
                    m.0 = if m.0 == Some(slot) { None } else { Some(slot) };
                });
            }
        }
    }
}
