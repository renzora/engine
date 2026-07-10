//! Icon menu button — a compact icon-only trigger that opens a floating,
//! scrolling option menu.
//!
//! WHY not a child-anchored dropdown menu: bevy_ui clips absolutely-positioned
//! children by every scrolling/clipping ancestor (see the tooltip module), so a
//! menu wider than the panel hosting the button — e.g. a list of script paths
//! in the inspector — gets cut off at the panel edge, and `GlobalZIndex` can't
//! fix that. The menu here is a [`screen_menu_flip`] root overlay instead:
//! never clipped, clamped on-screen, height-capped with a scrollbar, dismissed
//! by outside-click, and despawned after a pick — all via the shared
//! screen-menu systems.

use std::sync::Arc;

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

/// Marks an [`icon_menu_button`] trigger and carries its menu content.
#[derive(Component)]
pub(crate) struct IconMenuButton {
    /// Phosphor icon shown on every option row.
    option_icon: String,
    options: Vec<String>,
    /// Shared across the spawned rows; called with the picked option's index.
    on_pick: Arc<dyn Fn(&mut World, usize) + Send + Sync>,
}

/// A compact icon button that opens a floating menu of `options` on press,
/// aimed right-aligned under the button. `on_pick` runs (deferred, with
/// `&mut World`) with the picked option's index; the menu closes itself after
/// a pick or an outside click. Every pick fires, including the same option
/// twice — the menu holds no selection state.
pub fn icon_menu_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    option_icon: &str,
    options: &[&str],
    on_pick: impl Fn(&mut World, usize) + Send + Sync + 'static,
) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            // Anchors the menu to the button rect (cursor − normalized offset),
            // the same scheme the asset browser's Add button uses.
            bevy::ui::RelativeCursorPosition::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            IconMenuButton {
                option_icon: option_icon.to_string(),
                options: options.iter().map(|s| s.to_string()).collect(),
                on_pick: Arc::new(on_pick),
            },
            Name::new("icon-menu-button"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    commands.entity(box_e).add_child(glyph);
    box_e
}

/// Press an [`IconMenuButton`] → spawn its screen menu at the button's
/// bottom-right corner (right-aligned for the menu's min width;
/// `screen_menu_clamp` pulls a wider menu back on-screen). The button rect is
/// recovered from the cursor + `RelativeCursorPosition` — the same scheme as
/// the asset browser's Add button. Pressing while a menu is open reads as a
/// re-open: `screen_menu_dismiss` reaps the old menu on the same press.
pub(crate) fn icon_menu_button_open(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    buttons: Query<
        (
            &Interaction,
            &IconMenuButton,
            &bevy::ui::RelativeCursorPosition,
            &ComputedNode,
        ),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, btn, rcp, cn)) = buttons.iter().find(|(i, ..)| **i == Interaction::Pressed)
    else {
        return;
    };
    let Some((cursor, win_h)) = windows
        .iter()
        .find_map(|w| w.cursor_position().map(|c| (c, w.height())))
    else {
        return;
    };
    // Button rect from the cursor and its normalized position inside the button.
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let x = (top_left.x + size.x - 184.0).max(0.0);
    let y = top_left.y + size.y + 2.0;
    info!(
        "icon-menu: open at ({x:.0},{y:.0}) win_h={win_h:.0} options={}",
        btn.options.len()
    );
    let content = super::popup::screen_menu_flip(&mut commands, x, y, win_h);
    for (i, opt) in btn.options.iter().enumerate() {
        let pick = btn.on_pick.clone();
        let item = super::popup::menu_item(
            &mut commands,
            &fonts,
            &btn.option_icon,
            opt,
            move |w| pick(w, i),
        );
        commands.entity(content).add_child(item);
    }
}
