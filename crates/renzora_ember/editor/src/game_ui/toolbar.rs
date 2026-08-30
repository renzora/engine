//! The canvas toolbar: align + distribute (left), grid + snap toggles, and the
//! zoom cluster (right).

use bevy::prelude::*;

use renzora::{EditorSelection, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    drag_value, toolbar_bar, toolbar_group, toolbar_icon_button, toolbar_separator, DragRange,
};

use crate::game_ui::align::{compute_align, compute_distribute_h, compute_distribute_v, AlignAction};
use crate::game_ui::canvas::UiCanvasPreviewEnabled;
use crate::game_ui::geometry::WidgetGeom;
use crate::game_ui::NativeCanvasState;

#[derive(Component, Clone, Copy)]
pub(crate) enum CanvasTbBtn {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleGrid,
    ToggleSnap,
    ToggleBackdrop,
    Align(AlignAction),
    DistH,
    DistV,
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, toolbar_click.run_if(in_state(SplashState::Editor)));
}

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Same strip the viewport builds — see `renzora_ember::widgets::toolbar`.
    // It used to be a fixed-height non-wrapping row on `header_bg` with 22×20
    // buttons, which was a second, quietly different toolbar design.
    let bar = toolbar_bar(commands, "ui-canvas-toolbar");

    // Align + distribute.
    let aligns = [
        ("align-left", CanvasTbBtn::Align(AlignAction::Left)),
        ("align-center-horizontal", CanvasTbBtn::Align(AlignAction::CenterH)),
        ("align-right", CanvasTbBtn::Align(AlignAction::Right)),
        ("align-top", CanvasTbBtn::Align(AlignAction::Top)),
        ("align-center-vertical", CanvasTbBtn::Align(AlignAction::CenterV)),
        ("align-bottom", CanvasTbBtn::Align(AlignAction::Bottom)),
        ("arrows-out-line-horizontal", CanvasTbBtn::DistH),
        ("arrows-out-line-vertical", CanvasTbBtn::DistV),
    ];
    // Grouped, not flat: the six align buttons plus the two distribute buttons
    // are one control, and a group also never gets split down the middle when
    // the bar wraps to a second line.
    let align_group = toolbar_group(commands, "ui-align-group");
    let align_kids: Vec<Entity> = aligns
        .iter()
        .map(|(icon, btn)| icon_btn(commands, fonts, icon, *btn).0)
        .collect();
    commands.entity(align_group).add_children(&align_kids);
    let mut kids: Vec<Entity> = vec![align_group];

    kids.push(toolbar_separator(commands));

    // Grid + snap toggles.
    let (grid, grid_ic) = icon_btn(commands, fonts, "grid-four", CanvasTbBtn::ToggleGrid);
    bind_text_color(commands, grid_ic, |w| toggle_color(w, |s| s.show_grid));
    let (snap, snap_ic) = icon_btn(commands, fonts, "magnet-straight", CanvasTbBtn::ToggleSnap);
    bind_text_color(commands, snap_ic, |w| toggle_color(w, |s| s.snap_enabled));
    let (backdrop, backdrop_ic) = icon_btn(commands, fonts, "image", CanvasTbBtn::ToggleBackdrop);
    bind_text_color(commands, backdrop_ic, |w| {
        let on = w.get_resource::<UiCanvasPreviewEnabled>().is_none_or(|r| r.0);
        rgb(if on { accent() } else { text_muted() })
    });
    // Snap-amount (grid size) scrub field.
    let snap_amt = drag_value(commands, &fonts.ui, "", value_text(), 10.0, 1.0);
    commands.entity(snap_amt).insert(DragRange { min: 1.0, max: 256.0 });
    bind_2way(commands, snap_amt, |w| w.get_resource::<NativeCanvasState>().map(|s| s.grid_size).unwrap_or(10.0), |w, v: &f32| {
        if let Some(mut s) = w.get_resource_mut::<NativeCanvasState>() {
            s.grid_size = v.max(1.0);
        }
    });
    let view_group = toolbar_group(commands, "ui-view-group");
    commands
        .entity(view_group)
        .add_children(&[grid, snap, snap_amt, backdrop]);
    kids.push(view_group);

    kids.push(commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(8.0), ..default() }).id());

    // Resolution readout (left of the zoom cluster).
    let res = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, res, |w| {
        w.get_resource::<NativeCanvasState>().map(|s| format!("{} \u{d7} {}", s.canvas_width as i32, s.canvas_height as i32)).unwrap_or_default()
    });
    kids.push(res);
    kids.push(toolbar_separator(commands));

    // Zoom cluster.
    let zoom_out = icon_btn(commands, fonts, "magnifying-glass-minus", CanvasTbBtn::ZoomOut).0;
    let zoom_lbl = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { min_width: Val::Px(40.0), justify_content: JustifyContent::Center, ..default() }, Interaction::default(), CanvasTbBtn::ZoomReset))
        .id();
    bind_text(commands, zoom_lbl, |w| format!("{:.0}%", w.get_resource::<NativeCanvasState>().map(|s| s.zoom).unwrap_or(1.0) * 100.0));
    let zoom_in = icon_btn(commands, fonts, "magnifying-glass-plus", CanvasTbBtn::ZoomIn).0;
    let zoom_group = toolbar_group(commands, "ui-zoom-group");
    commands
        .entity(zoom_group)
        .add_children(&[zoom_out, zoom_lbl, zoom_in]);
    kids.push(zoom_group);

    commands.entity(bar).add_children(&kids);
    bar
}

/// A toolbar button carrying the marker that says what pressing it does. The
/// chrome — size, radius, icon scale — is the shared one, so this only adds the
/// behaviour. Icons start muted; the toggles rebind their colour below.
fn icon_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, marker: CanvasTbBtn) -> (Entity, Entity) {
    let (btn, ic) = toolbar_icon_button(commands, fonts, icon);
    commands.entity(btn).insert(marker);
    commands.entity(ic).insert(TextColor(rgb(text_muted())));
    (btn, ic)
}

fn toggle_color(w: &Rx, f: impl Fn(&NativeCanvasState) -> bool) -> Color {
    let on = w.get_resource::<NativeCanvasState>().is_some_and(f);
    rgb(if on { accent() } else { text_muted() })
}

fn toolbar_click(
    q: Query<(&Interaction, &CanvasTbBtn), Changed<Interaction>>,
    mut state: ResMut<NativeCanvasState>,
    backdrop: Option<ResMut<UiCanvasPreviewEnabled>>,
    selection: Option<Res<EditorSelection>>,
    mut commands: Commands,
) {
    let mut backdrop = backdrop;
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
            CanvasTbBtn::ToggleBackdrop => {
                if let Some(b) = backdrop.as_mut() {
                    b.0 = !b.0;
                }
            }
            CanvasTbBtn::Align(action) => {
                let geoms = selected_geoms(&state, &selection);
                let (rw, rh) = (state.canvas_width.max(1.0), state.canvas_height.max(1.0));
                for (e, nx, ny) in compute_align(&geoms, *action) {
                    commands.queue(move |w: &mut World| set_pos(w, e, Some(nx), Some(ny), rw, rh));
                }
            }
            CanvasTbBtn::DistH => {
                let geoms = selected_geoms(&state, &selection);
                let rw = state.canvas_width.max(1.0);
                for (e, nx) in compute_distribute_h(&geoms) {
                    commands.queue(move |w: &mut World| set_pos(w, e, Some(nx), None, rw, 1.0));
                }
            }
            CanvasTbBtn::DistV => {
                let geoms = selected_geoms(&state, &selection);
                let rh = state.canvas_height.max(1.0);
                for (e, ny) in compute_distribute_v(&geoms) {
                    commands.queue(move |w: &mut World| set_pos(w, e, None, Some(ny), 1.0, rh));
                }
            }
        }
    }
}

fn selected_geoms(state: &NativeCanvasState, selection: &Option<Res<EditorSelection>>) -> Vec<WidgetGeom> {
    let sel = selection.as_ref().map(|s| s.get_all()).unwrap_or_default();
    state.widgets.iter().filter(|g| sel.contains(&g.entity)).cloned().collect()
}

fn set_pos(world: &mut World, entity: Entity, nx: Option<f32>, ny: Option<f32>, rw: f32, rh: f32) {
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if let Some(mut node) = em.get_mut::<Node>() {
            if let Some(nx) = nx {
                node.left = Val::Percent(nx / rw * 100.0);
            }
            if let Some(ny) = ny {
                node.top = Val::Percent(ny / rh * 100.0);
            }
            node.position_type = PositionType::Absolute;
        }
    }
}
