//! The viewport's options menu: one trigger whose panel holds a submenu each for
//! **Display**, **Gizmos**, **Snap** and **Camera**.
//!
//! These were four icon dropdowns side by side. Four triggers whose glyphs
//! (an eye, a box, a magnet, a cube) said little about what was behind them cost
//! the bar a lot of width for settings that are changed rarely, and it was the
//! first cluster to fold into the overflow caret. One menu with labelled rows
//! reads better and takes the room of one button.
//!
//! The rows themselves are unchanged and still built by [`super::display`] and
//! [`super::camera`]; every driver system finds them by component, so moving
//! them one level deeper needed no rewiring.

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::theme::text_muted;
use renzora_ember::widgets::{
    icon_popup_trigger, menu_submenu_parts, menu_submenu_parts_sized, popup_anchor, popup_panel,
};

use super::camera::{camera_rows, snap_rows};
use super::BTN_H;
use super::display::{display_rows, gizmos_rows, DisplayTrigger};

pub(super) fn build_options_menu(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let display = display_rows(commands, fonts);
    let gizmos = gizmos_rows(commands, fonts);
    let snap = snap_rows(commands, fonts);
    let camera = camera_rows(commands, fonts);
    let kids = [
        submenu(commands, fonts, "eye", &renzora::lang::t_or("viewport.menu.display", "Display"), display),
        submenu(commands, fonts, "bounding-box", &renzora::lang::t_or("viewport.gizmos", "Gizmos"), gizmos),
        submenu(commands, fonts, "magnet", &renzora::lang::t_or("viewport.snap", "Snap"), snap),
        submenu(commands, fonts, "video-camera", &renzora::lang::t_or("viewport.menu.camera", "Camera"), camera),
    ];

    let panel = popup_panel(commands, &kids);
    // The submenus carry the width; the panel only has to fit four labels.
    commands.entity(panel).entry::<Node>().and_modify(|mut n| {
        n.min_width = Val::Px(150.0);
        n.padding = UiRect::all(Val::Px(4.0));
        n.row_gap = Val::Px(0.0);
    });
    let trigger = icon_popup_trigger(commands, fonts, "sliders-horizontal", panel);
    // Same hover / open styling as the other icon dropdowns.
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

/// One row of the options menu, opening `kids` to its side on hover.
fn submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    kids: Vec<Entity>,
) -> Entity {
    let (row, content, panel) = menu_submenu_parts(commands, fonts, icon, label, text_muted());
    fill_panel(commands, content, panel, kids);
    row
}

/// A submenu row one level down, inside a panel of option rows (Display's
/// Virtual Geometry and SSIL): sized like those rows, 12px text in a
/// [`BTN_H`]-high row with their 8px inset, so it reads as one more entry with
/// an arrow rather than as a heading in a bigger font.
pub(super) fn compact_submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    kids: Vec<Entity>,
) -> Entity {
    let (row, content, panel) =
        menu_submenu_parts_sized(commands, fonts, icon, label, text_muted(), 12.0, 12.0);
    commands.entity(row).entry::<Node>().and_modify(|mut n| {
        n.height = Val::Px(BTN_H);
        n.padding = UiRect::new(Val::Px(8.0), Val::Px(6.0), Val::Px(0.0), Val::Px(0.0));
        n.column_gap = Val::Px(6.0);
    });
    fill_panel(commands, content, panel, kids);
    row
}

/// Restyle a submenu's floating panel to the popup surface its rows were laid
/// out for (8px padding, 3px between rows, 220px wide), and fill it. The
/// defaults are a context menu's, whose rows carry their own padding, and these
/// switches and drag fields do not.
fn fill_panel(commands: &mut Commands, content: Entity, panel: Entity, kids: Vec<Entity>) {
    commands.entity(panel).entry::<Node>().and_modify(|mut n| {
        n.min_width = Val::Px(220.0);
        n.padding = UiRect::all(Val::Px(8.0));
    });
    commands.entity(content).entry::<Node>().and_modify(|mut n| {
        n.row_gap = Val::Px(3.0);
    });
    commands.entity(content).add_children(&kids);
}
