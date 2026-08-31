//! The canvas toolbar: three draggable groups — align + distribute, the view
//! toggles (grid, the snap pill, backdrop), and zoom.
//!
//! Built from the shared strip in `renzora_ember::widgets::toolbar`, so it is
//! the same chrome the viewport's toolbar uses: same bar, same button metrics,
//! same pill for a toggle fused to the number it governs, and the same
//! grip-per-group that lets the user reorder them.

use bevy::prelude::*;

use renzora::{EditorSelection, SplashState};
use bevy::ui::FocusPolicy;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
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
    /// Flip the selection between laid out by its parent and placed by hand.
    ToggleFreePosition,
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
        // Flow ↔ free. Sits with align because it is the same question — where
        // does this node sit — asked at the level above.
        ("push-pin", CanvasTbBtn::ToggleFreePosition),
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
        .add_children(&[grid, snap.root, overlays]);
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

    // The scene backdrop, pinned to the right edge rather than placed in the
    // arrange row.
    //
    // It is not one of the editing tools — it says what is *behind* the canvas,
    // which is a property of the view, not of the UI you are building. And it is
    // absolutely positioned rather than pushed right by a spacer because a
    // spacer fights drag-to-arrange: a group dragged past it lands on the far
    // side and stays there.
    let icon = icon_text(commands, &fonts.phosphor, "video-camera", text_muted(), 14.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
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
    let backdrop_group = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                top: Val::Px(4.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("ui-backdrop-group"),
        ))
        .id();
    commands
        .entity(backdrop_group)
        .add_children(&[icon, backdrop]);
    commands.entity(bar).add_child(backdrop_group);
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

/// Flip a node between flow layout and free placement.
///
/// Free placement is `position: absolute` — the node leaves its parent's flow
/// and is placed by coordinates, which is also what switches the canvas drag
/// from reordering to moving. It was only reachable by typing the attribute,
/// which made "just put it where I want" the one thing the editor could not do.
///
/// Going free pins the node where it already sits, so the flip does not also
/// move it. Going back to flow clears the offsets, or the node keeps a
/// `left`/`top` its new layout does not expect.
fn set_free_position(world: &mut World, entity: Entity, was_flow: bool, x: f32, y: f32, parent: (f32, f32, f32, f32)) {
    let (px, py, pw, ph) = (parent.0, parent.1, parent.2.max(1.0), parent.3.max(1.0));
    if was_flow {
        let left = format!("{:.2}%", (x - px) / pw * 100.0);
        let top = format!("{:.2}%", (y - py) / ph * 100.0);
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            node.position_type = PositionType::Absolute;
            node.left = Val::Percent((x - px) / pw * 100.0);
            node.top = Val::Percent((y - py) / ph * 100.0);
        }
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, "position", "absolute");
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, "left", &left);
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, "top", &top);
    } else {
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            node.position_type = PositionType::Relative;
            node.left = Val::Auto;
            node.top = Val::Auto;
        }
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, "position", "relative");
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, "left", "auto");
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, "top", "auto");
    }
}

/// Align a node that its parent lays out, by writing the flexbox attribute that
/// expresses it — and writing it to the `.html`, so it survives the rebuild.
fn align_in_flow(world: &mut World, entity: Entity, action: AlignAction) {
    let Some(parent) = world.get::<ChildOf>(entity).map(|c| c.parent()) else {
        return;
    };
    let parent_is_row = world
        .get::<Node>(parent)
        .map(|n| {
            matches!(
                n.flex_direction,
                FlexDirection::Row | FlexDirection::RowReverse
            )
        })
        .unwrap_or(true);
    let (target, attr, value) = match action.flow_attr(parent_is_row) {
        crate::game_ui::align::FlowAlign::Own(a, v) => (entity, a, v),
        crate::game_ui::align::FlowAlign::Parent(a, v) => (parent, a, v),
    };
    // Live first so it reads immediately, then the file — the attribute
    // writeback deliberately does not rebuild, because it has already updated
    // the entity in place.
    if let Some(mut node) = world.get_mut::<Node>(target) {
        match (attr, value) {
            ("align_self", "start") => node.align_self = AlignSelf::FlexStart,
            ("align_self", "center") => node.align_self = AlignSelf::Center,
            ("align_self", "end") => node.align_self = AlignSelf::FlexEnd,
            ("justify_content", "start") => node.justify_content = JustifyContent::FlexStart,
            ("justify_content", "center") => node.justify_content = JustifyContent::Center,
            ("justify_content", "end") => node.justify_content = JustifyContent::FlexEnd,
            _ => {}
        }
    }
    // The markup spells these `flex_start` / `flex_end`; `start` and `end` are
    // different values in bevy's parser and not the ones flex layout wants here.
    let markup_value = match value {
        "start" => "flex_start",
        "end" => "flex_end",
        other => other,
    };
    renzora_ember::markup::writeback::write_attr_to_markup(world, target, attr, markup_value);
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
                // Two different operations wearing one button, because they are
                // the same *intent*. A free node aligns by moving; a node in
                // flow aligns by telling flexbox where it sits, since nudging
                // `left`/`top` on a flex child is simply ignored — which is why
                // these buttons did nothing on most of a template.
                let flow: Vec<Entity> = geoms
                    .iter()
                    .filter(|g| g.in_flow)
                    .map(|g| g.entity)
                    .collect();
                let free = compute_align(
                    &geoms.iter().filter(|g| !g.in_flow).cloned().collect::<Vec<_>>(),
                    *action,
                );
                for (e, nx, ny) in free {
                    commands.queue(move |w: &mut World| set_pos(w, e, Some(nx), Some(ny), rw, rh));
                }
                let act = *action;
                for e in flow {
                    commands.queue(move |w: &mut World| align_in_flow(w, e, act));
                }
            }
            CanvasTbBtn::ToggleFreePosition => {
                for g in selected_geoms(&state, &selection) {
                    let (e, was_flow) = (g.entity, g.in_flow);
                    // Pin at where it already is, so flipping to free placement
                    // does not also move it. Percentages of the parent, matching
                    // what the drag writes.
                    let p = crate::game_ui::interaction::parent_box(&state, e);
                    let (x, y) = (g.x, g.y);
                    commands.queue(move |w: &mut World| set_free_position(w, e, was_flow, x, y, p));
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
