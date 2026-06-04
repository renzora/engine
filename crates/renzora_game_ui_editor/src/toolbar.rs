//! The canvas toolbar: zoom out / zoom% / zoom in, and grid + snap toggles.
//! (Align / distribute buttons land in a follow-up alongside selection, since
//! they need the widget-geometry snapshot + the selection set.)

use bevy::prelude::*;

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_text, bind_text_color};
use renzora_ember::theme::*;

use crate::NativeCanvasState;

#[derive(Component, Clone, Copy)]
pub(crate) enum CanvasTbBtn {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleGrid,
    ToggleSnap,
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, toolbar_click.run_if(in_state(SplashState::Editor)));
}

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(28.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("ui-canvas-toolbar"),
        ))
        .id();

    // Grid + snap toggles.
    let (grid, grid_ic) = icon_btn(commands, fonts, "grid-four", CanvasTbBtn::ToggleGrid);
    bind_text_color(commands, grid_ic, |w| toggle_color(w, |s| s.show_grid));
    let (snap, snap_ic) = icon_btn(commands, fonts, "magnet-straight", CanvasTbBtn::ToggleSnap);
    bind_text_color(commands, snap_ic, |w| toggle_color(w, |s| s.snap_enabled));

    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // Zoom cluster.
    let zoom_out = icon_btn(commands, fonts, "magnifying-glass-minus", CanvasTbBtn::ZoomOut).0;
    let zoom_lbl = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { min_width: Val::Px(40.0), justify_content: JustifyContent::Center, ..default() }, Interaction::default(), CanvasTbBtn::ZoomReset))
        .id();
    bind_text(commands, zoom_lbl, |w| format!("{:.0}%", w.get_resource::<NativeCanvasState>().map(|s| s.zoom).unwrap_or(1.0) * 100.0));
    let zoom_in = icon_btn(commands, fonts, "magnifying-glass-plus", CanvasTbBtn::ZoomIn).0;

    commands.entity(bar).add_children(&[grid, snap, spacer, zoom_out, zoom_lbl, zoom_in]);
    bar
}

fn icon_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, marker: CanvasTbBtn) -> (Entity, Entity) {
    let btn = commands
        .spawn((Node { width: Val::Px(24.0), height: Val::Px(20.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(btn).add_child(ic);
    (btn, ic)
}

fn toggle_color(w: &World, f: impl Fn(&NativeCanvasState) -> bool) -> Color {
    let on = w.get_resource::<NativeCanvasState>().is_some_and(f);
    rgb(if on { accent() } else { text_muted() })
}

fn toolbar_click(q: Query<(&Interaction, &CanvasTbBtn), Changed<Interaction>>, mut state: ResMut<NativeCanvasState>) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            CanvasTbBtn::ZoomIn => state.zoom = (state.zoom * 1.25).min(8.0),
            CanvasTbBtn::ZoomOut => state.zoom = (state.zoom * 0.8).max(0.1),
            CanvasTbBtn::ZoomReset => state.zoom = 1.0,
            CanvasTbBtn::ToggleGrid => state.show_grid = !state.show_grid,
            CanvasTbBtn::ToggleSnap => state.snap_enabled = !state.snap_enabled,
        }
    }
}
