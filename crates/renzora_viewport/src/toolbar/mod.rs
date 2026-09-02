//! The viewport's toolbar — the strip of chrome above the rendered scene.
//!
//! One bar, built from clusters. The registry-driven tool buttons
//! (Select / Move / Rotate / Scale, plus terrain and plugin tools), the view and
//! mode dropdowns, the shapes menu, the snap pills, camera speed, and the
//! Display / Gizmos / Snap / Camera dropdowns all live here; the session actions
//! (undo / redo / save) are built here and mounted by the editor shell in the top
//! bar, because they are session-wide rather than viewport-wide.
//!
//! Every driver system locates its widgets **by component**, never by where they
//! are parented. That is what has let these controls move — from a header strip,
//! to a shared toolbar host, to the viewport's own bar — without any of them
//! being rewired.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`actions`] | Undo / redo / save / maximize, and the dock swap maximize drives |
//! | [`view`] | View + Mode dropdowns, per-viewport view angle, World/Local, 2D/3D gating |
//! | [`display`] | The Display, Gizmos and 2D Overlays dropdowns, and the grid rows |
//! | [`camera`] | The Camera and Snap dropdowns, and every one-shot click they carry |
//! | [`snap`] | The inline snap pills and the camera-speed widget |
//! | [`rows`] | The row builders every dropdown is assembled from |
//! | [`tools`] | Filling the tool strip from `ToolbarRegistry` |
//! | [`shapes`] | The add-shape dropdown, filled from `ShapeRegistry` |

use bevy::prelude::*;

use renzora::core::viewport_types::ViewportSettings;
use renzora_ember::font::EmberFonts;

pub(crate) mod actions;
pub(crate) mod camera;
pub(crate) mod display;
pub(crate) mod rows;
pub(crate) mod shapes;
pub(crate) mod snap;
pub(crate) mod tools;
pub(crate) mod view;

use actions::{action_btn, HeaderAction, MaximizeSlot};
use snap::{set_snap, snap_pair, snap_val, SnapToggle};

use crate::tool_buttons::{SIDE_BTN, SIDE_ICON};

/// Height of the viewport header bar (matches the egui `HEADER_HEIGHT`).
pub const HEADER_HEIGHT: f32 = 28.0;

pub(super) const BTN_W: f32 = 26.0;
pub(super) const BTN_H: f32 = 22.0;

pub(super) fn col(c: renzora_theme::ThemeColor) -> Color {
    let [r, g, b, _a] = c.to_array();
    Color::srgb_u8(r, g, b)
}

/// Slugify an option label for the shared `opt.<slug>` translation namespace:
/// lowercase, each run of non-alphanumerics → one `_`, trimmed.
fn opt_slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_us = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_us && !out.is_empty() {
                out.push('_');
            }
            pending_us = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_us = true;
        }
    }
    out
}

/// Localize a dropdown OPTION label via the shared `opt.<slug>` namespace,
/// reusing a few `common.*` keys where the value already has one. The enum's
/// `label()` identity (used for index/state matching) is unchanged — only the
/// displayed string is translated.
pub(super) fn loc_opt(s: &str) -> String {
    match s {
        "None" => renzora::lang::t_or("common.none", s),
        "Disabled" => renzora::lang::t_or("common.disabled", s),
        "Default" => renzora::lang::t_or("common.default", s),
        "Always" => renzora::lang::t_or("common.always", s),
        _ => renzora::lang::t_or(&format!("opt.{}", opt_slug(s)), s),
    }
}

/// Tags toolbar widgets hidden in 2D view (rotate/scale snap, camera speed).
#[derive(Component)]
pub(super) struct ThreeDOnly;

/// Tags toolbar widgets shown ONLY in 2D view (the 2D Overlays dropdown).
#[derive(Component)]
pub(super) struct TwoDOnly;

/// The bar background (driven from `theme.surfaces.panel`).
#[derive(Component)]
pub(super) struct HeaderBg;

/// A pill background behind a widget (driven from `theme.widgets.inactive_bg`).
#[derive(Component)]
pub(super) struct WidgetBg;

fn gap(commands: &mut Commands, w: f32) -> Entity {
    commands
        .spawn((Node { width: Val::Px(w), ..default() }, Name::new("gap")))
        .id()
}

/// Build the toolbar controls, as a list of self-contained clusters.
///
/// This was child 0 of the viewport panel, then a full-width strip in the shared
/// toolbar host below the document tabs. It now lives back on the viewport's own
/// tool bar, beside Select / Move / Rotate / Scale — one bar of chrome instead of
/// two rows of it. The driver systems in [`register`] locate every widget by
/// component, so where it's parented has never mattered to them.
///
/// The return is a `Vec` of *groups* rather than one row because that bar folds
/// what it can't fit into a caret dropdown (see
/// [`renzora_ember::widgets::overflow_row`]), and a group is the unit that
/// folds. Grouping keeps clusters that are read together — the three snap pills,
/// the four display dropdowns — from being split across the fold.
pub(crate) fn header_groups(
    commands: &mut Commands,
    fonts: &EmberFonts,
) -> Vec<(Entity, &'static str)> {
    // One cluster of the toolbar: a row of widgets that moves — and wraps — as a
    // unit.
    fn group(commands: &mut Commands, name: &'static str, gap_px: f32, kids: &[Entity]) -> Entity {
        let g = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(gap_px),
                    flex_shrink: 0.0,
                    ..default()
                },
                bevy::ui::FocusPolicy::Pass,
                Name::new(name),
            ))
            .id();
        commands.entity(g).add_children(kids);
        g
    }

    // "Add shape" menu — a dropdown listing every registered shape (the same
    // ShapeRegistry the shape-library panel reads), grouped by category.
    // 3D-only: it spawns 3D primitives.
    let shapes_dd = shapes::build_shapes_dropdown(commands, fonts);
    commands.entity(shapes_dd).insert(ThreeDOnly);

    let gap3 = gap(commands, 8.0);
    let view_dd = view::view_dropdown(commands, fonts);
    let mode_dd = view::mode_dropdown(commands, fonts);

    // 2D-only Overlays dropdown — Grid (`show_grid_2d`, off by default; separate
    // from the 3D Display dropdown's "Grid", which drives the 3D floor grid),
    // Rulers (`show_rulers_2d`, on), and Gizmos (`show_gizmos_2d`, on: the light
    // markers + selection outlines). The 2D counterpart of the 3D Display
    // dropdown, which is itself 3D-only. Hidden in 3D/UI via `TwoDOnly`.
    let overlay_2d_dd = display::build_overlay_2d_dropdown(commands, fonts);
    commands.entity(overlay_2d_dd).insert(TwoDOnly);

    let gap5 = gap(commands, 6.0);
    let cam_speed = snap::cam_speed_widget(commands, fonts);
    commands.entity(cam_speed).insert(ThreeDOnly);

    // Inline snap pills (move / rotate / scale) — moved here from the per-viewport
    // strip. Translate doubles as the 2D grid snap; rotate / scale are 3D-only.
    let snap_gap = gap(commands, 8.0);
    let translate = snap_pair(
        commands, fonts, SnapToggle::Translate, "arrows-out-cardinal", 1.0, 100.0, 1.0,
        |w| snap_val(w, |s| s.translate_snap),
        |w, v| set_snap(w, |s| &mut s.translate_snap, v),
    );
    let rotate = snap_pair(
        commands, fonts, SnapToggle::Rotate, "arrow-clockwise", 1.0, 180.0, 1.0,
        |w| snap_val(w, |s| s.rotate_snap),
        |w, v| set_snap(w, |s| &mut s.rotate_snap, v),
    );
    let scale = snap_pair(
        commands, fonts, SnapToggle::Scale, "arrows-out", 1.0, 10.0, 1.0,
        |w| snap_val(w, |s| s.scale_snap),
        |w, v| set_snap(w, |s| &mut s.scale_snap, v),
    );
    commands.entity(rotate).insert(ThreeDOnly);
    commands.entity(scale).insert(ThreeDOnly);

    // Fixed gap (not a flex spacer) so the whole toolbar reads as one centered
    // cluster rather than splitting to the left/right edges.
    let center_gap = gap(commands, 12.0);
    let display_dd = display::build_display_dropdown(commands, fonts);
    // Per-gizmo visibility switches. Sits next to Display because the two are
    // read together ("why can't I see X?"); 3D-only like Display, since the 2D
    // view has its own single Gizmos switch in the Overlays dropdown.
    let gizmos_dd = display::build_gizmos_dropdown(commands, fonts);
    let snap_dd = camera::build_snap_dropdown(commands, fonts);
    let camera_dd = camera::build_camera_dropdown(commands, fonts);
    // The Display / Gizmos / Snap / Camera dropdowns are all 3D controls — hide in 2D.
    for e in [display_dd, gizmos_dd, snap_dd, camera_dd] {
        commands.entity(e).insert(ThreeDOnly);
    }
    // The old fixed gaps between clusters are gone: the toolbar row spaces its
    // children itself, and each group is announced by its grip.
    for e in [gap3, gap5, snap_gap, center_gap] {
        commands.entity(e).try_despawn();
    }
    // No "actions" group: undo / redo / save moved to the top bar, beside the
    // hamburger — see [`build_session_actions`].
    vec![
        (group(commands, "hdr-shapes", 2.0, &[shapes_dd]), "shapes"),
        (group(commands, "hdr-view", 2.0, &[view_dd, mode_dd]), "view"),
        (group(commands, "hdr-cam-speed", 2.0, &[cam_speed]), "cam_speed"),
        // Spaced like the view dropdowns beside them (`hdr-view`, gap 2), not
        // with the 4px spacers they used to carry between each pill: move,
        // rotate and scale snap are one setting read three ways, so they should
        // sit as tight together as `3D` and `Select` do.
        (group(commands, "hdr-snaps", 2.0, &[translate, rotate, scale]), "snaps"),
        (
            group(commands, "hdr-display", 2.0, &[display_dd, gizmos_dd, snap_dd, camera_dd]),
            "display",
        ),
        (group(commands, "hdr-overlay-2d", 2.0, &[overlay_2d_dd]), "overlay_2d"),
    ]
}

/// The Maximize toggle for one viewport slot.
///
/// Tagged with its slot so the click maximizes *that* viewport. Still public
/// from the spell when the editor shell mounted the primary one at the end of
/// the document-tab bar; the driver systems in [`register`] find it by
/// component, so where it's parented has never mattered to whether it works.
pub fn build_maximize(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
    let btn = action_btn(
        commands,
        fonts,
        HeaderAction::Maximize,
        "arrows-out",
        SIDE_BTN,
        SIDE_BTN,
        SIDE_ICON,
    );
    commands.entity(btn).insert(MaximizeSlot(slot));
    btn
}

/// The session-action cluster: undo / redo / save.
///
/// Built here but mounted by the **editor shell**, in the top bar beside the
/// hamburger. They're session-wide, not viewport-wide — you can undo and save
/// from a workspace that has no viewport in it at all — so the top bar is where
/// they belong, and it's the one place in the editor that's always on screen.
/// They've previously sat in the shared toolbar strip and in the viewport's own
/// tool bar; the driver systems in [`register`] find every widget by component
/// and aren't gated on the viewport panel, so where they're parented has never
/// mattered to whether they work.
pub fn build_session_actions(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let undo = action_btn(commands, fonts, HeaderAction::Undo, "arrow-u-up-left", BTN_W, BTN_H, SIDE_ICON);
    let redo = action_btn(commands, fonts, HeaderAction::Redo, "arrow-u-up-right", BTN_W, BTN_H, SIDE_ICON);
    let save = action_btn(commands, fonts, HeaderAction::Save, "floppy-disk", BTN_W, BTN_H, SIDE_ICON);
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                flex_shrink: 0.0,
                ..default()
            },
            // Structural — gaps between the buttons fall through to the top
            // bar's window-drag handle.
            bevy::ui::FocusPolicy::Pass,
            Name::new("session-actions"),
        ))
        .id();
    commands.entity(row).add_children(&[undo, redo, save]);
    row
}

/// Build the toolbar strip overlaid flush on the primary viewport's top edge —
/// the registry-driven tool buttons
/// (Select/Translate/… + terrain + plugin tools, filled by
/// [`tools::populate_tools`]), the inline snap pills (translate / rotate /
/// scale), and the maximize toggle pushed to the far right. These used to sit in
/// a separate header strip; the driver systems in [`register`] locate every
/// widget by component, so moving them into the viewport changes nothing about
/// how they behave.
pub(crate) fn build_side_toolbar(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
    // The shared editor toolbar strip — see `renzora_ember::widgets::toolbar`
    // for what it is and why the numbers live there rather than here. It sits
    // *above* the rendered scene rather than over it, so a two-line toolbar
    // costs the view nothing.
    let bar = renzora_ember::widgets::toolbar_bar(commands, "vp-toolbar");
    commands
        .entity(bar)
        .insert(Name::new("vp-side-toolbar"));

    // Registry-driven tool buttons (Select/Translate/... + terrain + plugins).
    // Populated from ToolbarRegistry by a deferred system (predicates need
    // World). The container stays visible in 2D — its 3D transform tools hide
    // themselves via a per-tool predicate, but 2D-relevant registry tools
    // (e.g. the tilemap Paint tool) must still show.
    let tools = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                ..default()
            },
            tools::ToolContainer,
            Name::new("vp-tools"),
        ))
        .id();

    // This viewport's own camera view-angle dropdown (Perspective / Front / Top /
    // …) — sets THIS slot's angle independently of the others. 3D-only.
    let view_angle = view::view_angle_menu(commands, fonts, slot);
    commands.entity(view_angle).insert(ThreeDOnly);

    // This viewport's own World/Local gizmo-space toggle (acts independently).
    let space_btn = view::space_toggle(commands, fonts, slot);
    commands.entity(space_btn).insert(ThreeDOnly);

    let view_group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Name::new("vp-view-controls"),
        ))
        .id();
    commands
        .entity(view_group)
        .add_children(&[view_angle, space_btn]);

    // The shell's Play control. Primary viewport only, so a 4-way split doesn't
    // sprout four Play buttons.
    let play = (slot == 0)
        .then(|| renzora_ember::toolbar::build_viewport_tool_trailing(commands, fonts))
        .filter(|t| !t.is_empty())
        .map(|trailing| {
            let holder = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        ..default()
                    },
                    bevy::ui::FocusPolicy::Pass,
                    Name::new("vp-play-controls"),
                ))
                .id();
            commands.entity(holder).add_children(&trailing);
            holder
        });

    // Every group in the bar, in its default order, each with the stable key its
    // position is saved under. Only the primary slot carries the header groups;
    // the secondary viewports keep just their own tools, as they always did.
    let mut groups: Vec<(Entity, &str)> = vec![(tools, "tools")];
    if slot == 0 {
        groups.extend(header_groups(commands, fonts));
        // Context bars mounted by crates the viewport can't depend on — the
        // terrain brush settings are the first. They're ordinary arrangement
        // groups, so they drag and persist like the built-in ones, and each
        // hides itself when its tool isn't active.
        groups.extend(renzora_ember::toolbar::build_viewport_tool_groups(
            commands, fonts,
        ));
    }
    let play_at = play.map(|p| {
        groups.push((p, "play"));
        groups.len() - 1
    });
    groups.push((view_group, "viewport"));
    let holders = renzora_ember::widgets::arrange_row_items(commands, fonts, bar, &groups);

    // Maximize floats to the right edge, outside the arrangement.
    //
    // `margin-left: auto` eats the free space on its line, so it sits hard
    // against the right of the bar however many groups are to its left — and on
    // the right of the last line when the bar wraps. It deliberately isn't one
    // of the arrangeable groups: those are ordered by the user and remembered,
    // and this one has a *position* as part of what it is. Being unkeyed is also
    // what keeps it there — `arrange_restore` rewrites the row's children with
    // the keyed holders in saved order and everything else appended after them.
    let maximize = build_maximize(commands, fonts, slot);
    // Patched onto the button's own `Node` rather than inserted as a new one:
    // `action_btn` sized and rounded it, and a fresh `Node` would drop all of it.
    commands.queue(move |world: &mut World| {
        if let Some(mut node) = world.get_mut::<Node>(maximize) {
            node.margin.left = Val::Auto;
            node.flex_shrink = 0.0;
        }
    });
    commands.entity(bar).add_children(&[maximize]);
    // Hidden with the rest of the toolbar while the game runs. It's a child of
    // the bar rather than of a holder, so the loop below doesn't cover it.
    renzora_ember::reactive::tracked::bind_display(commands, maximize, |w| {
        !w.get_resource::<renzora::core::PlayModeState>()
            .map(|p| p.is_in_play_mode())
            .unwrap_or(false)
    });
    // Only the primary bar's arrangement is saved: the secondary slots carry a
    // subset of the same groups, and letting each write the shared order would
    // have them overwrite each other with partial lists.
    if slot == 0 {
        commands.entity(bar).insert(PrimaryToolbarRow);
    }

    // Play is the one control that has to outlive the toolbar it sits on — it's
    // the Stop button. Everything else hides while the game runs. The bind goes
    // on the *holder* so a hidden group doesn't leave its grip behind.
    for (i, holder) in holders.iter().enumerate() {
        if Some(i) == play_at {
            continue;
        }
        renzora_ember::reactive::tracked::bind_display(commands, *holder, |w| {
            !w.get_resource::<renzora::core::PlayModeState>()
                .map(|p| p.is_in_play_mode())
                .unwrap_or(false)
        });
    }

    // Theme tracking for the strip's own background and closing rule now lives
    // in `toolbar_bar`, so the UI editor's toolbar gets it too.
    bar
}

/// Marks the primary viewport's toolbar row — the one whose arrangement is
/// saved. See [`sync_toolbar_order`].
#[derive(Component)]
struct PrimaryToolbarRow;

/// Keep the toolbar's arrangement and the persisted setting in step, both ways.
///
/// Dragging a group publishes a new [`ArrangeOrder`](renzora_ember::widgets::ArrangeOrder)
/// on the row; that goes into `ViewportSettings`, which
/// `persistence::save_on_change` writes to `project.toml`. Loading a project
/// applies the saved list back onto the row, which reorders itself to match. Both
/// directions compare before writing, so the two don't ping-pong through each
/// other's change detection.
fn sync_toolbar_order(
    mut settings: ResMut<ViewportSettings>,
    mut rows: Query<&mut renzora_ember::widgets::ArrangeOrder, With<PrimaryToolbarRow>>,
) {
    let Ok(mut order) = rows.single_mut() else {
        return;
    };
    // A freshly built row publishes nothing until its first drag, so an empty
    // order means "not arranged yet" — take the saved one rather than clobbering
    // it with the default.
    if order.0.is_empty() {
        if !settings.toolbar_order.is_empty() {
            order.0 = settings.toolbar_order.clone();
        }
        return;
    }
    if settings.toolbar_order != order.0 {
        settings.toolbar_order = order.0.clone();
    }
}

pub(crate) fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;

    app.init_resource::<display::ColliderGizmoMemory>();

    // The viewport toolbar no longer registers with the shared toolbar host: its
    // controls are built straight into the primary viewport's own tool bar by
    // [`build_side_toolbar`]. The shared strip is still there for the panels
    // that do use it (code editor, material graph, …).

    app.add_systems(
        Update,
        (
            actions::update_header_visuals,
            actions::header_action_click,
            actions::viewport_maximize_dock,
            view::update_mode_options,
            display::display_option_click,
            display::grid_div_click,
            sync_toolbar_order,
            display::update_display_visuals,
            snap::snap_toggle_click,
            snap::update_snap_toggles,
            view::update_three_d_only,
            snap::update_header_chrome,
            camera::header_click,
            rows::update_click_rows,
            rows::update_panel_buttons,
            camera::update_camera_snap_triggers,
            crate::tool_buttons::tool_button_click,
            // Nested tuple: keeps the top-level system count within Bevy's
            // 20-element tuple limit for `add_systems`.
            (
                shapes::shape_spawn_click,
                shapes::update_shape_menu,
                view::space_toggle_click,
                view::update_space_toggle,
                view::sanitize_mode_for_view,
                view::update_two_d_only,
            ),
        )
            .run_if(in_state(SplashState::Editor)),
    );
    // Exclusive (need `&World` for the registry predicates / shape registry).
    app.add_systems(
        Update,
        (
            tools::populate_tools,
            crate::tool_shelf::populate_shelf,
            crate::tool_buttons::update_tool_buttons,
            shapes::populate_shapes,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}
