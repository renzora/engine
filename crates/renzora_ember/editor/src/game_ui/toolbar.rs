//! The canvas toolbar: three draggable groups — align + distribute, the view
//! toggles (grid, the snap pill, backdrop), and zoom.
//!
//! Built from the shared strip in `renzora_ember::widgets::toolbar`, so it is
//! the same chrome the viewport's toolbar uses: same bar, same button metrics,
//! same pill for a toggle fused to the number it governs, and the same
//! grip-per-group that lets the user reorder them.

use bevy::prelude::*;

use renzora::{EditorSelection, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    arrange_row_items, icon_popup_trigger, popup_anchor, popup_panel, settings_check_row,
    settings_section, settings_separator, toggle_switch, toolbar_bar, toolbar_group,
    toolbar_icon_button, toolbar_pill, HoverTooltip,
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

/// Marks this panel's toolbar row, so `sync_toolbar_order` knows which
/// `ArrangeOrder` to save. There is one UI editor, but the marker keeps the
/// lookup honest if that ever stops being true.
#[derive(Component)]
struct UiToolbarRow;

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (toolbar_click, sync_toolbar_order).run_if(in_state(SplashState::Editor)),
    );
}

/// Mirror the row's live group order into the user's editor preferences, and
/// seed it from there on the first frame.
///
/// The drag itself is generic — `arrange_row` maintains `ArrangeOrder` — but
/// persisting it is the host panel's job, exactly as the viewport persists its
/// own into `ViewportSettings`. Without this the toolbar rearranges and then
/// forgets, which is worse than not being rearrangeable.
fn sync_toolbar_order(
    mut rows: Query<&mut renzora_ember::widgets::ArrangeOrder, With<UiToolbarRow>>,
    mut saved: Local<Option<Vec<String>>>,
) {
    let Ok(mut order) = rows.single_mut() else {
        return;
    };
    // An empty order means "not arranged yet" — take the saved one rather than
    // clobbering it with the default.
    if order.0.is_empty() {
        let disk = saved.get_or_insert_with(renzora::core::load_ui_toolbar_order);
        if !disk.is_empty() {
            order.0 = disk.clone();
        }
        return;
    }
    if saved.as_deref() != Some(order.0.as_slice()) {
        *saved = Some(order.0.clone());
        if let Err(err) = renzora::core::save_ui_toolbar_order(&order.0) {
            warn!("ui toolbar: could not save group order — {err}");
        }
    }
}

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Same strip the viewport builds — see `renzora_ember::widgets::toolbar`.
    // It used to be a fixed-height non-wrapping row on `header_bg` with 22×20
    // buttons, which was a second, quietly different toolbar design.
    let bar = toolbar_bar(commands, "ui-canvas-toolbar");
    commands.entity(bar).insert(UiToolbarRow);

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

    // Grid is a plain toggle; snap is a pill, because "snap" and "by how much"
    // are one idea. As a separate icon and a separate boxed field they read as
    // two unrelated widgets that happen to be adjacent — which is exactly how
    // this toolbar looked next to the viewport's snap pills.
    let (grid, grid_ic) = icon_btn(commands, fonts, "grid-four", CanvasTbBtn::ToggleGrid);
    bind_text_color(commands, grid_ic, |w| toggle_color(w, |s| s.show_grid));

    // The scene backdrop is a state, not an action, so it reads as a switch —
    // the same control the Overlays popup uses for every other "is this drawn"
    // question. As an icon button it was the odd one out, and its only "on"
    // signal was a tint you had to already know to look for.
    let backdrop = toggle_switch(commands, false);
    commands
        .entity(backdrop)
        .insert(HoverTooltip::new("Scene backdrop"));
    bind_2way(
        commands,
        backdrop,
        |w: &Rx| {
            w.get_resource::<UiCanvasPreviewEnabled>()
                .is_none_or(|r| r.0)
        },
        |w: &mut World, v: &bool| {
            if let Some(mut r) = w.get_resource_mut::<UiCanvasPreviewEnabled>() {
                r.0 = *v;
            }
        },
    );

    // `arrows-out-cardinal`, the glyph the viewport's translate-snap pill uses.
    // Both mean "snap movement to a step", so they should not be a magnet in one
    // panel and a move cursor in the other.
    let snap = toolbar_pill(commands, fonts, "arrows-out-cardinal", 1.0, 256.0, 1.0);
    commands.entity(snap.toggle).insert(CanvasTbBtn::ToggleSnap);
    commands
        .entity(snap.value)
        .insert(renzora_ember::widgets::DragSnap(1.0));
    // On, the pill fills with the accent — so the glyph has to become the colour
    // that reads *on* that fill, not the accent itself. Tinting it accent made
    // it vanish into the pill, so the icon looked missing exactly while the
    // control was doing something.
    bind_text_color(commands, snap.icon, |w| {
        let on = w
            .get_resource::<NativeCanvasState>()
            .is_some_and(|s| s.snap_enabled);
        if on {
            Color::WHITE
        } else {
            rgb(text_muted())
        }
    });
    // The pill fills when snapping is on, the same read as the viewport's.
    bind_bg(commands, snap.root, |w| {
        let on = w.get_resource::<NativeCanvasState>().is_some_and(|s| s.snap_enabled);
        rgb(if on { accent() } else { hover_bg() })
    });
    bind_2way(commands, snap.value, |w| w.get_resource::<NativeCanvasState>().map(|s| s.grid_size).unwrap_or(10.0), |w, v: &f32| {
        if let Some(mut s) = w.get_resource_mut::<NativeCanvasState>() {
            s.grid_size = v.max(1.0);
        }
    });

    let overlays = overlays_dropdown(commands, fonts);

    let view_group = toolbar_group(commands, "ui-view-group");
    commands
        .entity(view_group)
        .add_children(&[grid, snap.root, backdrop, overlays]);
    kids.push(view_group);

    // Zoom cluster, with the canvas resolution read out beside it.
    //
    // Left-aligned in the flow rather than pushed right by a spacer. A spacer
    // fights drag-to-arrange — a group you drag past it lands on the far side
    // and stays there — and the viewport's strip is fully left-aligned for the
    // same reason.
    let res = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { margin: UiRect::horizontal(Val::Px(4.0)), ..default() })).id();
    bind_text(commands, res, |w| {
        w.get_resource::<NativeCanvasState>().map(|s| format!("{} \u{d7} {}", s.canvas_width as i32, s.canvas_height as i32)).unwrap_or_default()
    });


    let zoom_out = icon_btn(commands, fonts, "magnifying-glass-minus", CanvasTbBtn::ZoomOut).0;
    let zoom_lbl = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { min_width: Val::Px(40.0), justify_content: JustifyContent::Center, ..default() }, Interaction::default(), CanvasTbBtn::ZoomReset))
        .id();
    bind_text(commands, zoom_lbl, |w| format!("{:.0}%", w.get_resource::<NativeCanvasState>().map(|s| s.zoom).unwrap_or(1.0) * 100.0));
    let zoom_in = icon_btn(commands, fonts, "magnifying-glass-plus", CanvasTbBtn::ZoomIn).0;
    let zoom_group = toolbar_group(commands, "ui-zoom-group");
    commands
        .entity(zoom_group)
        .add_children(&[res, zoom_out, zoom_lbl, zoom_in]);
    kids.push(zoom_group);

    // Each group gets a grip and a saved position, exactly like the viewport's.
    // The grips double as the dividers between groups, which is why there are no
    // explicit separators left in here.
    let keys = ["ui-align", "ui-view", "ui-zoom"];
    let entries: Vec<(Entity, &str)> = kids.iter().copied().zip(keys).collect();
    arrange_row_items(commands, fonts, bar, &entries);
    bar
}

/// The **Overlays** popup — one switch per thing the editor draws over the
/// canvas, in the same shape as the viewport's Gizmos dropdown.
///
/// A popup rather than a plain dropdown because these are independent switches,
/// not one choice among several: you can want the hover outline without the
/// container box, or the boxes without their names. A dropdown would have forced
/// them into a single ranked list of combinations.
fn overlays_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    macro_rules! canvas_switch {
        ($label:expr, $field:ident) => {
            settings_check_row(
                commands,
                fonts,
                $label,
                |w: &Rx| {
                    w.get_resource::<NativeCanvasState>()
                        .map(|s| s.$field)
                        .unwrap_or(false)
                },
                |w: &mut World, v: bool| {
                    if let Some(mut s) = w.get_resource_mut::<NativeCanvasState>() {
                        s.$field = v;
                    }
                },
            )
        };
    }

    let kids = vec![
        settings_section(commands, fonts, "Highlight"),
        canvas_switch!("Hovered node", hover_outline),
        canvas_switch!("Parent container", hover_group),
        settings_separator(commands),
        settings_section(commands, fonts, "Labels"),
        canvas_switch!("Node names", show_names),
        settings_separator(commands),
        settings_section(commands, fonts, "Guides"),
        canvas_switch!("Rulers", show_rulers),
    ];
    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "eye", panel);
    popup_anchor(commands, trigger, panel)
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
