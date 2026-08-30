//! Bevy-native (ember) viewport header bar — the chrome above the 3D image —
//! plus the floating vertical toolbar overlaid on the viewport itself.
//!
//! Mirrors `header.rs` (the egui version) but built from bevy_ui nodes so it
//! runs in the native editor. The header strip keeps the view/mode dropdowns,
//! the shapes menu, World/Local, camera speed, and the Display/Camera/Snap
//! dropdowns; the session actions (undo / redo / save), the registry-driven
//! tool buttons, the inline snap pills, and maximize live in
//! [`build_side_toolbar`], a horizontal strip parented into the primary
//! viewport's content node. Every driver system locates its widgets by
//! component, so the two strips share one set of systems.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::core::viewport_types::{
    CameraSettingsState, CollisionGizmoVisibility, ProjectionMode, SnapSettings, ViewAngleCommand,
    ViewportMode, ViewportSettings, ViewportView, VisualizationMode,
};
use renzora::core::ShapeRegistry;
use renzora_undo::{execute, SpawnShapeCmd, UndoContext};
use bevy::ecs::world::CommandQueue;

use renzora_editor_framework::{EditorCommands, GizmoSpace, ToolSection, ToolbarRegistry};
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{
    drag_value, drag_value_flat, dropdown_compact, icon_popup_trigger, popup_anchor, popup_panel,
    popup_panel_aligned, scroll_area, toggle_switch, DragRange, EmberDropdownOption, Popup,
    PopupAlign,
};
use renzora_ember::theme::{
    border, hover_bg, rgb, tab_active, text_muted, text_primary, value_text,
};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_theme::ThemeManager;

use crate::tool_buttons::{
    tool_button, tool_separator, ToolSepVis, ToolsPopulated, SIDE_BTN, SIDE_ICON,
};

/// Height of the viewport header bar (matches the egui `HEADER_HEIGHT`).
pub const HEADER_HEIGHT: f32 = 28.0;

const BTN_W: f32 = 26.0;
const BTN_H: f32 = 22.0;

/// One of the fixed "session action" buttons in the header's left strip.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HeaderAction {
    Undo,
    Redo,
    Save,
    Maximize,
}

/// Tags a Maximize button with the viewport slot it belongs to, so clicking it
/// maximizes THAT viewport (each viewport carries its own Maximize button).
#[derive(Component, Clone, Copy)]
struct MaximizeSlot(usize);

/// Points a button at its child glyph `Text` entity so the visuals system can
/// re-glyph / re-color it without walking children.
#[derive(Component)]
struct HeaderIcon(Entity);

fn col(c: renzora_theme::ThemeColor) -> Color {
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
fn loc_opt(s: &str) -> String {
    match s {
        "None" => renzora::lang::t_or("common.none", s),
        "Disabled" => renzora::lang::t_or("common.disabled", s),
        "Default" => renzora::lang::t_or("common.default", s),
        "Always" => renzora::lang::t_or("common.always", s),
        _ => renzora::lang::t_or(&format!("opt.{}", opt_slug(s)), s),
    }
}

/// Build the header controls, as a list of self-contained clusters.
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
    // One cluster of the header: a row of widgets that moves — and wraps — as a
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
    let shapes_dd = build_shapes_dropdown(commands, fonts);
    commands.entity(shapes_dd).insert(ThreeDOnly);

    let gap3 = gap(commands, 8.0);
    let view_dd = view_dropdown(commands, fonts);
    let mode_dd = mode_dropdown(commands, fonts);

    // 2D-only Overlays dropdown — Grid (`show_grid_2d`, off by default; separate
    // from the 3D Display dropdown's "Grid", which drives the 3D floor grid),
    // Rulers (`show_rulers_2d`, on), and Gizmos (`show_gizmos_2d`, on: the light
    // markers + selection outlines). The 2D counterpart of the 3D Display
    // dropdown, which is itself 3D-only. Hidden in 3D/UI via `TwoDOnly`.
    let overlay_2d_dd = build_overlay_2d_dropdown(commands, fonts);
    commands.entity(overlay_2d_dd).insert(TwoDOnly);

    let gap5 = gap(commands, 6.0);
    let cam_speed = cam_speed_widget(commands, fonts);
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
    let display_dd = build_display_dropdown(commands, fonts);
    // Per-gizmo visibility switches. Sits next to Display because the two are
    // read together ("why can't I see X?"); 3D-only like Display, since the 2D
    // view has its own single Gizmos switch in the Overlays dropdown.
    let gizmos_dd = build_gizmos_dropdown(commands, fonts);
    let snap_dd = build_snap_dropdown(commands, fonts);
    let camera_dd = build_camera_dropdown(commands, fonts);
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
/// (Select/Translate/… + terrain + plugin tools, filled by [`populate_tools`]),
/// the inline snap pills (translate / rotate / scale), and the maximize toggle
/// pushed to the far right. These used to sit in the header strip; the driver
/// systems in [`register`] locate every widget by component, so moving them
/// into the viewport changes nothing about how they behave. The cluster is an
/// [`OverlaySurface`] so hovering it suppresses viewport hover (a click on a
/// tool never bleeds into picking / box-select underneath).
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
            ToolContainer,
            Name::new("vp-tools"),
        ))
        .id();

    // This viewport's own camera view-angle dropdown (Perspective / Front / Top /
    // …) — sets THIS slot's angle independently of the others. 3D-only.
    let view_angle = view_angle_menu(commands, fonts, slot);
    commands.entity(view_angle).insert(ThreeDOnly);

    // This viewport's own World/Local gizmo-space toggle (acts independently).
    let space_btn = space_toggle(commands, fonts, slot);
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
/// Dragging a group publishes a new [`ArrangeOrder`] on the row; that goes into
/// `ViewportSettings`, which `persistence::save_on_change` writes to
/// `project.toml`. Loading a project applies the saved list back onto the row,
/// which reorders itself to match. Both directions compare before writing, so
/// the two don't ping-pong through each other's change detection.
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


/// The Grid row of the 2D Overlays dropdown: label, the cell-size input, then
/// the on/off switch. Unlike the other rows (plain `toggle_row!`), the grid
/// carries its size setting inline — a boxed ember [`drag_value`] editing
/// `ViewportSettings.grid_size_2d` in whole world units — sitting to the LEFT of
/// the switch. Its own setting, deliberately NOT the translate-snap step (tying
/// them together made the snap pill silently restyle the grid).
fn grid_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // On/off switch → `show_grid_2d`.
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.show_grid_2d)
                .unwrap_or(false)
        },
        |w: &mut World, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.show_grid_2d = *v;
            }
        },
    );

    // Cell-size input — a boxed ember numeric field (click to type, drag to
    // scrub). Whole world units: a fractional grid size is never what a tile
    // artist wants, and int snapping keeps the readout stable.
    let dv = drag_value(commands, &fonts.ui, "", value_text(), 16.0, 0.2);
    commands.entity(dv).insert((
        DragRange { min: 1.0, max: 1024.0 },
        renzora_ember::widgets::DragSnap(1.0),
    ));
    bind_2way(
        commands,
        dv,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.grid_size_2d)
                .unwrap_or(16.0)
        },
        |w: &mut World, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                let want = v.round().max(1.0);
                if s.grid_size_2d != want {
                    s.grid_size_2d = want;
                }
            }
        },
    );

    let lbl = commands
        .spawn((
            Text::new(renzora::lang::t("viewport.grid")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-2d-grid-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, dv, sw]);
    row
}

// ── World / Local gizmo-space toggle ─────────────────────────────────────────

/// Tags a space-toggle button with the viewport slot it controls (each viewport
/// has its own World/Local toggle, acting independently — see
/// `renzora::core::viewport_types::ViewportGizmoSpace`).
#[derive(Component, Clone, Copy)]
struct SpaceToggleSlot(usize);

/// Points a space-toggle button at its child glyph `Text` entity.
#[derive(Component)]
struct SpaceToggleGlyphRef(Entity);

/// Phosphor icons for the two gizmo spaces (globe = World, cube = Local).
fn space_icon(space: GizmoSpace) -> &'static str {
    match space {
        GizmoSpace::World => "globe",
        GizmoSpace::Local => "cube-focus",
    }
}

fn space_label(space: GizmoSpace) -> String {
    match space {
        GizmoSpace::World => renzora::lang::t("viewport.gizmo.world"),
        GizmoSpace::Local => renzora::lang::t("viewport.gizmo.local"),
    }
}

fn space_for(local: bool) -> GizmoSpace {
    if local {
        GizmoSpace::Local
    } else {
        GizmoSpace::World
    }
}

/// An icon button that flips THIS viewport's transform gizmo between World and
/// Local space (globe / cube glyph; the tooltip names the active space).
fn space_toggle(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
    let glyph = icon_text(
        commands,
        &fonts.phosphor,
        space_icon(GizmoSpace::World),
        text_primary(),
        13.0,
    );
    commands
        .entity(glyph)
        .insert(bevy::picking::Pickable::IGNORE);
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(BTN_W),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(space_label(GizmoSpace::World)),
            SpaceToggleSlot(slot),
            SpaceToggleGlyphRef(glyph),
            Name::new("vp-space-toggle"),
        ))
        .id();
    commands.entity(btn).add_child(glyph);
    btn
}

/// Click flips THIS viewport's space (World ↔ Local) in `ViewportGizmoSpace`.
fn space_toggle_click(
    q: Query<(&Interaction, &SpaceToggleSlot), Changed<Interaction>>,
    space: Option<ResMut<renzora::core::viewport_types::ViewportGizmoSpace>>,
) {
    let Some(mut space) = space else { return };
    for (i, slot) in &q {
        if *i == Interaction::Pressed {
            if let Some(local) = space.local.get_mut(slot.0) {
                *local = !*local;
            }
        }
    }
}

/// Keep each viewport's space-toggle glyph + tooltip in sync with its own space.
fn update_space_toggle(
    space: Option<Res<renzora::core::viewport_types::ViewportGizmoSpace>>,
    mut buttons: Query<(
        &SpaceToggleSlot,
        &SpaceToggleGlyphRef,
        &mut renzora_ember::widgets::HoverTooltip,
    )>,
    mut texts: Query<&mut Text>,
) {
    let Some(space) = space else { return };
    if !space.is_changed() {
        return;
    }
    for (slot, glyph, mut tip) in &mut buttons {
        let s = space_for(space.local.get(slot.0).copied().unwrap_or(false));
        if let Some(g) = icon_glyph(space_icon(s)) {
            if let Ok(mut t) = texts.get_mut(glyph.0) {
                t.0 = g.to_string();
            }
        }
        tip.0 = space_label(s);
    }
}

fn gap(commands: &mut Commands, w: f32) -> Entity {
    commands
        .spawn((Node { width: Val::Px(w), ..default() }, Name::new("gap")))
        .id()
}

fn action_btn(
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

pub(crate) fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;

    app.init_resource::<ColliderGizmoMemory>();

    // The viewport header no longer registers with the shared toolbar host: its
    // controls are built straight into the primary viewport's own tool bar by
    // [`build_side_toolbar`]. The shared strip is still there for the panels
    // that do use it (code editor, material graph, …).

    app.add_systems(
        Update,
        (
            update_header_visuals,
            header_action_click,
            viewport_maximize_dock,
            update_mode_options,
            display_option_click,
            grid_div_click,
            sync_toolbar_order,
            update_display_visuals,
            snap_toggle_click,
            update_snap_toggles,
            update_three_d_only,
            update_header_chrome,
            header_click,
            update_click_rows,
            update_panel_buttons,
            update_camera_snap_triggers,
            crate::tool_buttons::tool_button_click,
            // Nested tuple: keeps the top-level system count within Bevy's
            // 20-element tuple limit for `add_systems`.
            (
                shape_spawn_click,
                update_shape_menu,
                space_toggle_click,
                update_space_toggle,
                sanitize_mode_for_view,
                update_two_d_only,
            ),
        )
            .run_if(in_state(SplashState::Editor)),
    );
    // Exclusive (need `&World` for the registry predicates / shape registry).
    app.add_systems(
        Update,
        (
            populate_tools,
            crate::native_tool_shelf::populate_shelf,
            crate::tool_buttons::update_tool_buttons,
            populate_shapes,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── View / Mode / view-angle controls (A2) ───────────────────────────────────
//
// These were three hand-rolled comboboxes (trigger + popup + four driver
// systems). They are now ember widgets: the two real selections are
// `dropdown_compact` bound to `ViewportSettings` with `bind_2way`, and the
// per-viewport view angle — which is a list of *actions*, not a selection — is
// an ember `Popup` of click rows. Besides deleting the duplicated open/close/
// dismiss logic, this is what gives them `OverlaySurface` pointer-blocking and
// the flip-up-when-clipped positioning for free.

/// Per-viewport view-angle presets: (label, yaw, pitch). "Perspective" is the
/// default free 3/4 angle; the rest are the orthographic-style snaps.
const VIEW_ANGLE_OPTIONS: &[(&str, f32, f32)] = {
    use std::f32::consts::{FRAC_PI_2, PI};
    &[
        ("Perspective", 0.3, 0.4),
        ("Front", 0.0, 0.0),
        ("Back", PI, 0.0),
        ("Left", -FRAC_PI_2, 0.0),
        ("Right", FRAC_PI_2, 0.0),
        ("Top", 0.0, FRAC_PI_2),
        ("Bottom", 0.0, -FRAC_PI_2),
    ]
};

/// Marks the Mode combobox so [`update_mode_options`] can find its option rows.
#[derive(Component)]
struct ModeDropdown;

/// A viewport's view-angle menu trigger: which slot it drives, and the `Text`
/// entity showing the current pick. Picking a row writes that label and closes
/// the menu — ember's `popup_dismiss` deliberately leaves a popup open when the
/// click lands inside it (the Display/Snap/Camera panels want that, since you
/// flip several switches in a row), but a one-shot action list should close.
#[derive(Component)]
struct ViewAngleTrigger {
    slot: usize,
    label: Entity,
}

/// The shared **View** combobox (3D / 2D / UI), bound to
/// `ViewportSettings::viewport_view`.
fn view_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let labels: Vec<String> = ViewportView::ALL.iter().map(|v| loc_opt(v.label())).collect();
    let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let dd = dropdown_compact(commands, fonts, &refs, 0, 56.0);
    bind_2way(
        commands,
        dd,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .and_then(|s| ViewportView::ALL.iter().position(|v| *v == s.viewport_view))
                .unwrap_or(0)
        },
        |w: &mut World, i: &usize| {
            if let (Some(mut s), Some(v)) = (
                w.get_resource_mut::<ViewportSettings>(),
                ViewportView::ALL.get(*i).copied(),
            ) {
                if s.viewport_view != v {
                    s.viewport_view = v;
                }
            }
        },
    );
    dd
}

/// The shared **Mode** combobox, bound to `ViewportSettings::viewport_mode`.
/// Built from the full `ViewportMode::ALL` list; [`update_mode_options`] hides
/// the rows that don't apply to the current view.
fn mode_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let labels: Vec<String> = ViewportMode::ALL.iter().map(|m| loc_opt(m.label())).collect();
    let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let dd = dropdown_compact(commands, fonts, &refs, 0, 80.0);
    commands.entity(dd).insert(ModeDropdown);
    bind_2way(
        commands,
        dd,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .and_then(|s| ViewportMode::ALL.iter().position(|m| *m == s.viewport_mode))
                .unwrap_or(0)
        },
        |w: &mut World, i: &usize| {
            if let (Some(mut s), Some(m)) = (
                w.get_resource_mut::<ViewportSettings>(),
                ViewportMode::ALL.get(*i).copied(),
            ) {
                if s.viewport_mode != m {
                    s.viewport_mode = m;
                }
            }
        },
    );
    dd
}

/// The Mode list offers a per-view subset (no Sculpt in 2D, no Erase in 3D).
/// Rows are built once from `ALL` and hidden per view, so
/// `EmberDropdownOption::value` stays a stable index into `ALL`.
fn update_mode_options(
    settings: Option<Res<ViewportSettings>>,
    mode_boxes: Query<Entity, With<ModeDropdown>>,
    mut options: Query<(&EmberDropdownOption, &mut Node)>,
) {
    let Some(settings) = settings else { return };
    let allowed = ViewportMode::for_view(settings.viewport_view);
    for (opt, mut node) in &mut options {
        if !mode_boxes.contains(opt.dropdown) {
            continue;
        }
        let ok = ViewportMode::ALL.get(opt.value).is_some_and(|m| allowed.contains(m));
        let want = if ok { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
}

/// A viewport's own **view-angle** menu (Perspective / Front / Top / …).
///
/// An ember [`Popup`] of click rows rather than a combobox, because these are
/// actions: picking the angle you are already "on" must re-snap the camera
/// (you have orbited away since), and a selection widget would swallow that as
/// a no-op. The trigger still shows the last pick so it reads like a dropdown.
fn view_angle_menu(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
    let kids: Vec<Entity> = VIEW_ANGLE_OPTIONS
        .iter()
        .enumerate()
        .map(|(index, (label, yaw, pitch))| {
            click_row(
                commands,
                fonts,
                &loc_opt(label),
                HeaderClick::SlotViewAngle {
                    slot,
                    index,
                    yaw: *yaw,
                    pitch: *pitch,
                },
            )
        })
        .collect();
    let panel = popup_panel(commands, &kids);

    let label_e = commands
        .spawn((
            Text::new(loc_opt(VIEW_ANGLE_OPTIONS[0].0)),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 10.0);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Px(96.0),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            Popup::new(panel),
            DisplayTrigger,
            ViewAngleTrigger { slot, label: label_e },
            Name::new("vp-view-angle"),
        ))
        .id();
    commands.entity(trigger).add_children(&[label_e, caret]);
    popup_anchor(commands, trigger, panel)
}

/// Honor the viewport "maximize" toggle on the bevy_ui shell's ember dock: swap
/// the dock to a viewport-only leaf while maximized and restore the saved tree
/// when un-maximized (the egui dock handles this itself in renzora_editor_framework).
fn viewport_maximize_dock(
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

/// Resolved header palette + the booleans that drive each button's glyph,
/// color, and hover/active background.
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

fn update_header_visuals(
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
        // Save is the one action whose enabled state means "you have work you
        // could lose", so it gets amber rather than the neutral `primary` the
        // other enabled buttons use — the unsaved tab is the thing to notice.
        HeaderAction::Save => (
            "floppy-disk",
            if m.can_save { m.warning } else { m.muted },
            m.can_save,
        ),
        HeaderAction::Maximize => (
            if this_maximized { "arrows-in" } else { "arrows-out" },
            if this_maximized { m.primary } else { m.muted },
            true,
        ),
    }
}

fn header_action_click(
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

// ── Display dropdown (A3): visualization + render flags + overlays + collision ─

/// Marks the Display dropdown's icon trigger (for hover / open background).
#[derive(Component)]
struct DisplayTrigger;

/// A click-to-select option inside the Display popup.
#[derive(Component, Clone, Copy)]
enum DisplayOption {
    /// Visualization mode by index into `VisualizationMode::ALL`.
    Viz(usize),
    /// Collision gizmo visibility — `true` = Selected Only, `false` = Always.
    Collision(bool),
}

/// Builds a [`check_row`] bound to `ViewportSettings.<field-path>`. Defined
/// before its first use (macro_rules is textually scoped).
macro_rules! toggle_row {
    ($c:expr, $f:expr, $label:expr, $($field:tt)+) => {
        check_row(
            $c,
            $f,
            $label,
            |w: &Rx| {
                w.get_resource::<ViewportSettings>()
                    .map(|s| s.$($field)+)
                    .unwrap_or(false)
            },
            |w: &mut World, v: bool| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.$($field)+ = v;
                }
            },
        )
    };
}

fn build_display_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.visualization")));
    for (i, m) in VisualizationMode::ALL.iter().enumerate() {
        kids.push(option_row(commands, fonts, DisplayOption::Viz(i), &loc_opt(m.label())));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.render")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.mesh"), render_toggles.mesh));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.textures"), render_toggles.textures));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.shading.wireframe"), render_toggles.wireframe));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.lighting"), render_toggles.lighting));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.shadows"), render_toggles.shadows));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.overlays")));
    kids.push(grid_divisions_row(commands, fonts));
    // Subgrid's switch used to sit here, but it never touched this grid — the
    // flag only splits the *2D* editor's grid into major/minor lines. Dividing
    // the 3D grid is what the -/+ above does, and the 2D flag keeps its home in
    // Settings → Viewport.
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.axis_gizmo"), show_axis_gizmo));
    // Sits with the axis gizmo rather than in Gizmos: both are corner chrome
    // that belongs to the *view*, where everything in Gizmos is drawn per
    // scene object.
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.stats"), show_stats));
    // Scene Icons / Labels and the whole collision-gizmo picker used to live
    // here; they moved to the Gizmos dropdown (`build_gizmos_dropdown`) so all
    // the "what's drawn over the scene" switches sit in one place. Display keeps
    // what the renderer produces — viz modes, render flags, and the grid.

    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "eye", panel);
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

/// The **Gizmos** dropdown — one switch per editor overlay drawn *over* the
/// scene, grouped by what it belongs to (selection / scene objects / rigging /
/// physics).
///
/// Separate from Display on purpose: Display decides what the renderer
/// produces (visualization mode, mesh/texture/lighting/shadow flags, the
/// grid), while these decide what the *editor* draws on top of it. Several of
/// these gizmos had no switch at all before — the skeleton bones, the light
/// falloff wireframes and the camera frustum were unconditional — so this is
/// the first way to get a dense rig or a wall of collider boxes out of the
/// view without deselecting.
///
/// Lives in the shared header rather than a viewport's own side toolbar
/// because `ViewportSettings` is one global resource: a per-slot placement
/// would promise per-viewport control that doesn't exist.
fn build_gizmos_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.selection")));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.bounding_box"),
        show_selection_box
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.scene")));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.lights"),
        show_light_gizmos
    ));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.cameras"),
        show_camera_gizmos
    ));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.scene_icons"), show_scene_icons));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.labels"), show_labels));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.rigging")));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.skeleton"),
        show_skeleton_gizmos
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.physics")));
    // The on/off half of `collision_gizmo_visibility`. Turning it back on
    // restores the remembered Selected Only / Always choice rather than always
    // snapping to Selected Only.
    kids.push(check_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.gizmos.colliders"),
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.collision_gizmo_visibility != CollisionGizmoVisibility::Off)
                .unwrap_or(false)
        },
        |w: &mut World, v: bool| {
            let restore = w
                .get_resource::<ColliderGizmoMemory>()
                .map(|m| m.0)
                .unwrap_or(CollisionGizmoVisibility::SelectedOnly);
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.collision_gizmo_visibility = if v {
                    restore
                } else {
                    CollisionGizmoVisibility::Off
                };
            }
        },
    ));
    // The two mode rows also act as an "on" — picking either while the switch
    // is off turns colliders back on in that mode, which is what a click on a
    // visible row is expected to do.
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(true),
        &renzora::lang::t("viewport.display.selected_only"),
    ));
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(false),
        &renzora::lang::t("common.always"),
    ));

    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "bounding-box", panel);
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

/// Remembers the last non-`Off` collider-gizmo mode so the Physics →
/// "Colliders" switch can return to Selected Only *or* Always — whichever was
/// in use. Editor UI state only: never persisted (a fresh session starts from
/// whatever `collision_gizmo_visibility` loaded as) and never crosses the
/// plugin boundary, so it stays here instead of in `ViewportSettings`.
#[derive(Resource)]
struct ColliderGizmoMemory(CollisionGizmoVisibility);

impl Default for ColliderGizmoMemory {
    fn default() -> Self {
        Self(CollisionGizmoVisibility::SelectedOnly)
    }
}

/// The 2D **Overlays** dropdown — the 2D counterpart of the 3D Display
/// dropdown. Three switches bound to the 2D `ViewportSettings` flags: the
/// behind-sprites Grid, the ruler bars, and the editor Gizmos (the light
/// markers + selected-light/occluder outlines). The icon trigger reuses
/// `DisplayTrigger` so it themes identically to the other icon dropdowns and
/// needs no extra visuals system; the caller adds `TwoDOnly` so it shows only
/// in 2D view (the 3D Display dropdown is the mirror-image `ThreeDOnly`).
fn build_overlay_2d_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let kids = vec![
        section_label(commands, fonts, &renzora::lang::t("viewport.display.overlays")),
        grid_row(commands, fonts),
        toggle_row!(commands, fonts, &renzora::lang::t("viewport.rulers"), show_rulers_2d),
        toggle_row!(
            commands,
            fonts,
            &renzora::lang::t_or("viewport.gizmos", "Gizmos"),
            show_gizmos_2d
        ),
    ];
    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "eye", panel);
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Name::new("vp-section-label"),
        ))
        .id()
}

fn separator_row(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("vp-separator"),
        ))
        .id()
}

/// A label + click-to-select row (for the visualization / collision pickers).
fn option_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    opt: DisplayOption,
    label: &str,
) -> Entity {
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            opt,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-display-option"),
        ))
        .id();
    commands.entity(row).add_child(txt);
    row
}

/// A label + two-way switch row, bound to a `ViewportSettings` field.
fn check_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    get: impl Fn(&Rx) -> bool + Send + Sync + 'static,
    set: impl Fn(&mut World, bool) + Send + Sync + 'static,
) -> Entity {
    let cb = toggle_switch(commands, false);
    bind_2way(commands, cb, get, move |w, v: &bool| set(w, *v));
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-check-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, cb]);
    row
}

/// The Display dropdown's **Grid** row: the on/off switch, plus `-` / `+` that
/// subdivide the floor grid.
///
/// Each press halves (or doubles) the squares — the grid is infinite and
/// unitless, so a subdivision count is the only thing that means anything here;
/// a cell size in world units would be a number with nothing to measure against.
/// The readout is the divisor: `1` is the base grid, `4` is sixteenth-squares.
fn grid_divisions_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.show_grid)
                .unwrap_or(false)
        },
        |w: &mut World, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.show_grid = *v;
            }
        },
    );

    let lbl = commands
        .spawn((
            Text::new(renzora::lang::t("viewport.grid")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();

    let minus = grid_div_btn(commands, fonts, "minus", false);
    let count = commands
        .spawn((
            Text::new("1"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Name::new("vp-grid-divisions"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_text(commands, count, |w| {
        w.get_resource::<ViewportSettings>()
            .map(|s| s.grid_divisions.to_string())
            .unwrap_or_else(|| "1".into())
    });
    let plus = grid_div_btn(commands, fonts, "plus", true);

    let stepper = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-grid-stepper"),
        ))
        .id();
    commands.entity(stepper).add_children(&[minus, count, plus]);

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-grid-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, stepper, sw]);
    row
}

/// One end of the grid stepper. `up` doubles the divisions, otherwise halves —
/// powers of two, so the finer lines always land on the coarser ones.
#[derive(Component, Clone, Copy)]
struct GridDivBtn(bool);

fn grid_div_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, up: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            GridDivBtn(up),
            Name::new(if up { "vp-grid-div-up" } else { "vp-grid-div-down" }),
        ))
        .id();
    let g = icon_text(commands, &fonts.phosphor, icon, text_muted(), 10.0);
    commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(btn).add_child(g);
    renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
        match w.get::<Interaction>(btn) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
            _ => rgb(tab_active()),
        }
    });
    btn
}

/// `-` / `+` on the grid row → halve or double the subdivision, clamped to a
/// range where the lines stay distinguishable at a sane camera height.
fn grid_div_click(
    buttons: Query<(&Interaction, &GridDivBtn), Changed<Interaction>>,
    settings: Option<ResMut<ViewportSettings>>,
) {
    let Some(mut settings) = settings else { return };
    for (interaction, btn) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let want = if btn.0 {
            settings.grid_divisions.saturating_mul(2)
        } else {
            settings.grid_divisions / 2
        }
        .clamp(1, 64);
        if settings.grid_divisions != want {
            settings.grid_divisions = want;
        }
    }
}

fn display_option_click(
    options: Query<(&Interaction, &DisplayOption), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *opt {
            DisplayOption::Viz(i) => cmds.push(move |w: &mut World| {
                if let (Some(mut s), Some(m)) = (
                    w.get_resource_mut::<ViewportSettings>(),
                    VisualizationMode::ALL.get(i).copied(),
                ) {
                    s.visualization_mode = m;
                }
            }),
            // Picking a mode also turns colliders on if they were `Off` — the
            // row is right there under the switch, so a click on it meaning
            // "show me this" is the only sensible reading.
            DisplayOption::Collision(selected_only) => cmds.push(move |w: &mut World| {
                let mode = if selected_only {
                    CollisionGizmoVisibility::SelectedOnly
                } else {
                    CollisionGizmoVisibility::Always
                };
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.collision_gizmo_visibility = mode;
                }
            }),
        }
    }
}

fn update_display_visuals(
    settings: Option<Res<ViewportSettings>>,
    mut collider_memory: ResMut<ColliderGizmoMemory>,
    theme: Option<Res<ThemeManager>>,
    triggers: Query<(&Interaction, &Popup, &mut BackgroundColor), With<DisplayTrigger>>,
    options: Query<(&DisplayOption, &Interaction, &mut BackgroundColor), Without<DisplayTrigger>>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in triggers {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }

    let viz_idx = VisualizationMode::ALL
        .iter()
        .position(|m| *m == settings.visualization_mode);
    // Keep the "turn colliders back on into what?" memory current. Doing it
    // here (rather than in the switch's setter) also catches changes made from
    // the Settings panel's Colliders dropdown.
    let collision = settings.collision_gizmo_visibility;
    if collision != CollisionGizmoVisibility::Off && collider_memory.0 != collision {
        collider_memory.0 = collision;
    }

    for (opt, interaction, mut bg) in options {
        let is_current = match *opt {
            DisplayOption::Viz(i) => viz_idx == Some(i),
            // Neither mode row is "current" while colliders are off — the
            // switch above them is the thing that reads as off.
            DisplayOption::Collision(sel) => match collision {
                CollisionGizmoVisibility::Off => false,
                CollisionGizmoVisibility::SelectedOnly => sel,
                CollisionGizmoVisibility::Always => !sel,
            },
        };
        let want = if is_current {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

// ── Inline snapping + camera speed (A5) ──────────────────────────────────────

/// Which snap the icon toggle in a snap-pair enables/disables.
#[derive(Component, Clone, Copy)]
enum SnapToggle {
    Translate,
    Rotate,
    Scale,
}

/// The snap-pair pill, tagged with which snap it represents so its *whole*
/// background fills accent when that snap is enabled.
#[derive(Component, Clone, Copy)]
struct SnapPillOf(SnapToggle);

/// Tags header widgets hidden in 2D view (rotate/scale snap, camera speed).
#[derive(Component)]
struct ThreeDOnly;

/// Tags header widgets shown ONLY in 2D view (the 2D Overlays dropdown).
#[derive(Component)]
struct TwoDOnly;

/// The header bar background (driven from `theme.surfaces.panel`).
#[derive(Component)]
struct HeaderBg;

/// A pill background behind a header widget (driven from `theme.widgets.inactive_bg`).
#[derive(Component)]
struct WidgetBg;

/// Keep the header bar + widget pills matched to the active egui theme so the
/// native toolbar reads identically to the egui one.
fn update_header_chrome(
    theme: Option<Res<ThemeManager>>,
    mut panels: Query<&mut BackgroundColor, (With<HeaderBg>, Without<WidgetBg>)>,
    mut widgets: Query<&mut BackgroundColor, (With<WidgetBg>, Without<HeaderBg>)>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let panel = col(t.surfaces.panel);
    let widget = col(t.widgets.inactive_bg);
    for mut bg in &mut panels {
        if bg.0 != panel {
            bg.0 = panel;
        }
    }
    for mut bg in &mut widgets {
        if bg.0 != widget {
            bg.0 = widget;
        }
    }
}

fn snap_val(w: &Rx, f: impl Fn(&SnapSettings) -> f32) -> f32 {
    w.get_resource::<ViewportSettings>()
        .map(|s| f(&s.snap))
        .unwrap_or(0.0)
}

fn set_snap(w: &mut World, f: impl Fn(&mut SnapSettings) -> &mut f32, v: f32) {
    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
        *f(&mut s.snap) = v;
    }
}

/// An icon toggle (enable/disable) + a scrubbable snap amount, in a pill.
#[allow(clippy::too_many_arguments)]
fn snap_pair(
    commands: &mut Commands,
    fonts: &EmberFonts,
    which: SnapToggle,
    icon: &str,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&Rx) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, value_text(), 13.0);
    let toggle = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            which,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-snap-toggle"),
        ))
        .id();
    commands.entity(toggle).add_child(glyph);

    // Divider between the toggle and the number, so the two halves of the pill
    // read as separate hit areas.
    let divider = commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(14.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("vp-snap-divider"),
        ))
        .id();

    let dv = drag_value_flat(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert((
        DragRange { min, max },
        // Whole-number steps: the model quantizes to 1, so the readout never
        // shows decimals and every scrub/wheel/typed value lands on an integer.
        renzora_ember::widgets::DragSnap(1.0),
    ));
    // Narrower number cell than the widget's 44px default — these live in the
    // viewport toolbar where width is precious and the values are 1–3 digits.
    commands
        .entity(dv)
        .entry::<Node>()
        .and_modify(|mut n| n.min_width = Val::Px(32.0));
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            SnapPillOf(which),
            Name::new("vp-snap-pair"),
        ))
        .id();
    commands.entity(row).add_children(&[toggle, divider, dv]);
    row
}

/// A camera icon + scrubbable move-speed (3D fly-cam).
fn cam_speed_widget(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, "video-camera", text_primary(), 13.0);
    let iconbox = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Name::new("vp-cam-icon"),
        ))
        .id();
    commands.entity(iconbox).add_child(glyph);

    let dv = drag_value_flat(commands, &fonts.ui, "", value_text(), 1.0, 0.5);
    commands.entity(dv).insert(DragRange {
        min: 0.1,
        max: 100.0,
    });
    bind_2way(
        commands,
        dv,
        |w| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.camera.move_speed)
                .unwrap_or(1.0)
        },
        |w, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.camera.move_speed = *v;
            }
        },
    );

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            WidgetBg,
            Name::new("vp-cam-speed"),
        ))
        .id();
    commands.entity(row).add_children(&[iconbox, dv]);
    row
}

fn snap_toggle_click(
    q: Query<(&Interaction, &SnapToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, which) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let which = *which;
        cmds.push(move |w: &mut World| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                let flag = match which {
                    SnapToggle::Translate => &mut s.snap.translate_enabled,
                    SnapToggle::Rotate => &mut s.snap.rotate_enabled,
                    SnapToggle::Scale => &mut s.snap.scale_enabled,
                };
                *flag = !*flag;
            }
        });
    }
}

fn update_snap_toggles(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    mut pills: Query<(&SnapPillOf, &mut BackgroundColor)>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);

    for (pill, mut bg) in &mut pills {
        let enabled = match pill.0 {
            SnapToggle::Translate => settings.snap.translate_enabled,
            SnapToggle::Rotate => settings.snap.rotate_enabled,
            SnapToggle::Scale => settings.snap.scale_enabled,
        };
        // The whole pill fills accent when the snap is on, so the widget reads as
        // one cohesive filled background (not a bright box beside the number).
        let want = if enabled { accent } else { inactive };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn update_three_d_only(
    settings: Option<Res<ViewportSettings>>,
    mut q: Query<&mut Node, With<ThreeDOnly>>,
) {
    let Some(settings) = settings else { return };
    let show = settings.viewport_view != ViewportView::Two;
    for mut n in &mut q {
        let want = if show { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
}

/// Sibling of [`update_three_d_only`]: shows `TwoDOnly` widgets only in 2D view.
/// Keep the interaction mode legal for the active view: switching views
/// while in a view-specific mode (Sculpt is 3D-only, Erase is 2D-only)
/// falls back to Select, matching what the Mode dropdown offers. Covers
/// every entry path — the dropdown, Tab shortcuts, and panels that set the
/// mode directly.
fn sanitize_mode_for_view(settings: Option<ResMut<ViewportSettings>>) {
    let Some(mut s) = settings else { return };
    if !ViewportMode::for_view(s.viewport_view).contains(&s.viewport_mode) {
        s.viewport_mode = ViewportMode::Scene;
    }
}

fn update_two_d_only(
    settings: Option<Res<ViewportSettings>>,
    mut q: Query<&mut Node, With<TwoDOnly>>,
) {
    let Some(settings) = settings else { return };
    let show = settings.viewport_view == ViewportView::Two;
    for mut n in &mut q {
        let want = if show { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
}

// ── Camera + Snap dropdowns (A4) ─────────────────────────────────────────────

/// A discrete one-shot click action inside a header dropdown.
#[derive(Component, Clone, Copy)]
enum HeaderClick {
    Projection(ProjectionMode),
    ViewAngle { yaw: f32, pitch: f32 },
    /// A per-viewport view-angle pick: snaps THIS slot's camera (the shared
    /// `ViewAngle` above writes the global channel) and relabels its trigger.
    /// `index` is into [`VIEW_ANGLE_OPTIONS`], for the label.
    SlotViewAngle {
        slot: usize,
        index: usize,
        yaw: f32,
        pitch: f32,
    },
    CamReset,
    ToggleObjectSnap,
    ToggleFloorSnap,
}

/// Tags a projection row so it highlights when that projection is current.
#[derive(Component, Clone, Copy)]
struct ProjOption(ProjectionMode);

/// Object/Floor snap toggle buttons (accent fill when enabled).
#[derive(Component, Clone, Copy)]
enum SnapBtnKind {
    Object,
    Floor,
}

/// The Camera dropdown's icon trigger.
#[derive(Component)]
struct CameraTrigger;

/// The Snap dropdown's icon trigger (magnet — accent when any snap is active).
#[derive(Component)]
struct SnapTrigger;

/// View-angle presets: (label, shortcut, yaw, pitch). Mirrors egui `ViewAngle`.
const VIEW_ANGLES: &[(&str, &str, f32, f32)] = {
    use std::f32::consts::{FRAC_PI_2, PI};
    &[
        ("Front", "Num1", 0.0, 0.0),
        ("Back", "Ctrl+Num1", PI, 0.0),
        ("Left", "Ctrl+Num3", -FRAC_PI_2, 0.0),
        ("Right", "Num3", FRAC_PI_2, 0.0),
        ("Top", "Num7", 0.0, FRAC_PI_2),
        ("Bottom", "Ctrl+Num7", 0.0, -FRAC_PI_2),
    ]
};

/// Builds a label + boxed [`drag_value`] row bound to `ViewportSettings.<path>`.
macro_rules! drag_row {
    ($c:expr, $f:expr, $label:expr, $min:expr, $max:expr, $step:expr, $($field:tt)+) => {
        drag_row_build(
            $c, $f, $label, $min, $max, $step,
            |w: &Rx| {
                w.get_resource::<ViewportSettings>()
                    .map(|s| s.$($field)+)
                    .unwrap_or($min)
            },
            |w: &mut World, v: f32| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.$($field)+ = v;
                }
            },
        )
    };
}

/// A label + click-to-fire row (view angles, reset).
fn click_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, click: HeaderClick) -> Entity {
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            click,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-click-row"),
        ))
        .id();
    commands.entity(row).add_child(txt);
    row
}

/// A projection-mode row (highlights when current).
fn proj_row(commands: &mut Commands, fonts: &EmberFonts, mode: ProjectionMode, label: &str) -> Entity {
    let row = click_row(commands, fonts, label, HeaderClick::Projection(mode));
    commands.entity(row).insert(ProjOption(mode));
    row
}

/// A toggle button (Objects / Floor) that fills accent when its snap is on.
fn snap_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    kind: SnapBtnKind,
    click: HeaderClick,
) -> Entity {
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                min_width: Val::Px(70.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            kind,
            click,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-snap-button"),
        ))
        .id();
    commands.entity(btn).add_child(txt);
    btn
}

/// A label + (flex spacer) + boxed drag_value row, bound two-way.
fn drag_row_build(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&Rx) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-drag-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, dv]);
    row
}

#[allow(clippy::vec_init_then_push)]
fn build_camera_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.projection")));
    kids.push(proj_row(commands, fonts, ProjectionMode::Perspective, &renzora::lang::t("viewport.camera.perspective")));
    kids.push(proj_row(commands, fonts, ProjectionMode::Orthographic, &renzora::lang::t("viewport.camera.orthographic")));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.view_angles")));
    for (label, sc, yaw, pitch) in VIEW_ANGLES {
        let lbl = match *label {
            "Front" => renzora::lang::t("viewport.camera.front"),
            "Back" => renzora::lang::t("viewport.camera.back"),
            "Left" => renzora::lang::t("viewport.camera.left"),
            "Right" => renzora::lang::t("viewport.camera.right"),
            "Top" => renzora::lang::t("viewport.camera.top"),
            "Bottom" => renzora::lang::t("viewport.camera.bottom"),
            other => other.to_string(),
        };
        kids.push(click_row(
            commands,
            fonts,
            &format!("{lbl}  ({sc})"),
            HeaderClick::ViewAngle {
                yaw: *yaw,
                pitch: *pitch,
            },
        ));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.sensitivities")));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.look"), 0.05, 2.0, 0.05, camera.look_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.orbit"), 0.05, 2.0, 0.05, camera.orbit_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.pan"), 0.1, 5.0, 0.1, camera.pan_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.zoom"), 0.1, 5.0, 0.1, camera.zoom_sensitivity));

    kids.push(separator_row(commands));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.camera.invert_y"), camera.invert_y));
    kids.push(toggle_row!(
        commands,
        fonts,
        &renzora::lang::t("viewport.camera.distance_relative_speed"),
        camera.distance_relative_speed
    ));
    kids.push(click_row(commands, fonts, &renzora::lang::t("inspector.component.reset"), HeaderClick::CamReset));

    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "cube", panel);
    commands.entity(trigger).insert(CameraTrigger);
    popup_anchor(commands, trigger, panel)
}

#[allow(clippy::vec_init_then_push)]
fn build_snap_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.snap.object_snapping")));
    kids.push(snap_dist_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.objects"),
        SnapBtnKind::Object,
        HeaderClick::ToggleObjectSnap,
        0.1,
        10.0,
        0.1,
        |w| snap_val(w, |s| s.object_snap_distance),
        |w, v| set_snap(w, |s| &mut s.object_snap_distance, v),
    ));
    kids.push(snap_dist_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.floor"),
        SnapBtnKind::Floor,
        HeaderClick::ToggleFloorSnap,
        -1000.0,
        1000.0,
        0.1,
        |w| snap_val(w, |s| s.floor_y),
        |w, v| set_snap(w, |s| &mut s.floor_y, v),
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.snap.transform_aids")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.snap.edge_snap"), snap.translate_edge_snap));
    kids.push(toggle_row!(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.scale_from_bottom"),
        snap.scale_bottom_anchor
    ));

    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "magnet", panel);
    commands.entity(trigger).insert(SnapTrigger);
    popup_anchor(commands, trigger, panel)
}

/// A snap toggle button + its bound distance/offset drag value, in one row.
#[allow(clippy::too_many_arguments)]
fn snap_dist_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    kind: SnapBtnKind,
    click: HeaderClick,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&Rx) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let btn = snap_button(commands, fonts, label, kind, click);
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-snap-dist-row"),
        ))
        .id();
    commands.entity(row).add_children(&[btn, dv]);
    row
}

fn header_click(
    q: Query<(&Interaction, &HeaderClick), Changed<Interaction>>,
    mut angle_triggers: Query<(&ViewAngleTrigger, &mut Popup)>,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, click) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *click {
            HeaderClick::SlotViewAngle {
                slot,
                index,
                yaw,
                pitch,
            } => {
                // Per-slot channel, consumed by `renzora_camera::apply_per_slot_view_angle`.
                cmds.push(move |w: &mut World| {
                    if let Some(mut vps) =
                        w.get_resource_mut::<renzora::core::viewport_types::Viewports>()
                    {
                        if let Some(s) = vps.slots.get_mut(slot) {
                            s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                        }
                    }
                });
                // Reflect the pick on this viewport's trigger label, and close
                // the menu (a one-shot action, unlike the switch panels).
                let name = VIEW_ANGLE_OPTIONS.get(index).map(|(l, ..)| loc_opt(l));
                for (tag, mut popup) in &mut angle_triggers {
                    if tag.slot != slot {
                        continue;
                    }
                    if let (Some(name), Ok(mut text)) = (name.as_ref(), texts.get_mut(tag.label)) {
                        if text.0 != *name {
                            text.0 = name.clone();
                        }
                    }
                    popup.open = false;
                    if let Ok(mut n) = nodes.get_mut(popup.panel) {
                        n.display = Display::None;
                    }
                }
            }
            HeaderClick::Projection(mode) => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.projection_mode = mode;
                }
            }),
            HeaderClick::ViewAngle { yaw, pitch } => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                }
            }),
            HeaderClick::CamReset => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.camera = CameraSettingsState::default();
                }
            }),
            HeaderClick::ToggleObjectSnap => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.snap.object_snap_enabled = !s.snap.object_snap_enabled;
                }
            }),
            HeaderClick::ToggleFloorSnap => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.snap.floor_snap_enabled = !s.snap.floor_snap_enabled;
                }
            }),
        }
    }
}

/// Hover highlight for plain click rows (view angles, reset) — projection rows
/// and snap buttons are handled by [`update_panel_buttons`].
fn update_click_rows(
    theme: Option<Res<ThemeManager>>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor),
        (With<HeaderClick>, Without<ProjOption>, Without<SnapBtnKind>),
    >,
) {
    let Some(theme) = theme else { return };
    let hovered = col(theme.active_theme.widgets.hovered_bg);
    for (interaction, mut bg) in &mut q {
        let want = if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn update_panel_buttons(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    mut proj: Query<(&ProjOption, &Interaction, &mut BackgroundColor), Without<SnapBtnKind>>,
    mut snapbtns: Query<(&SnapBtnKind, &Interaction, &mut BackgroundColor), Without<ProjOption>>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (opt, interaction, mut bg) in &mut proj {
        let want = if settings.projection_mode == opt.0 {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    for (kind, interaction, mut bg) in &mut snapbtns {
        let on = match kind {
            SnapBtnKind::Object => settings.snap.object_snap_enabled,
            SnapBtnKind::Floor => settings.snap.floor_snap_enabled,
        };
        let want = if on {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn update_camera_snap_triggers(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    mut cam: Query<
        (&Interaction, &Popup, &mut BackgroundColor),
        (With<CameraTrigger>, Without<SnapTrigger>),
    >,
    mut snap: Query<
        (&Interaction, &Popup, &mut BackgroundColor),
        (With<SnapTrigger>, Without<CameraTrigger>),
    >,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in &mut cam {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    let s = &settings.snap;
    let any_snap = s.object_snap_enabled
        || s.floor_snap_enabled
        || s.translate_edge_snap
        || s.scale_bottom_anchor;
    for (interaction, toggle, mut bg) in &mut snap {
        let want = if any_snap {
            accent
        } else if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

// ── Registry-driven tool buttons (A6) ────────────────────────────────────────

/// The header's tool-button strip; filled from `ToolbarRegistry` once it exists.
#[derive(Component)]
struct ToolContainer;

/// Fill an empty `ToolContainer` from the registry (Transform / Terrain / custom
/// sections with separators). Exclusive because the visibility/active predicates
/// take `&World`; runs until the registry is populated and the container exists.
fn populate_tools(world: &mut World) {
    let Some(registry) = world.get_resource::<ToolbarRegistry>().cloned() else {
        return;
    };
    if registry.entries().is_empty() {
        return; // tools not registered yet
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut cq = world.query_filtered::<Entity, (With<ToolContainer>, Without<ToolsPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    // Build the ordered section list: Transform, Terrain, then custom sections.
    // These are the *mode* buttons — the ones that say what the viewport is set
    // to do. What each mode opens (brushes, select modes, ops) renders on the
    // shelf instead; see `native_tool_shelf`.
    let mut sections: Vec<Vec<renzora_editor_framework::ToolEntry>> = Vec::new();
    let by_section = |sec| {
        let mut v: Vec<_> = registry
            .entries()
            .iter()
            .filter(|e| e.section == sec)
            .cloned()
            .collect();
        v.sort_by_key(|e| e.order);
        v
    };
    let transform = by_section(ToolSection::Transform);
    if !transform.is_empty() {
        sections.push(transform);
    }
    let terrain = by_section(ToolSection::Terrain);
    if !terrain.is_empty() {
        sections.push(terrain);
    }
    for id in registry.custom_sections() {
        let v = by_section(ToolSection::Custom(id));
        if !v.is_empty() {
            sections.push(v);
        }
    }

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        // Buttons first, per section, so each separator can be tagged with the
        // buttons on either side of it (that's what drives its visibility).
        let section_buttons: Vec<Vec<Entity>> = sections
            .iter()
            .map(|section| {
                section
                    .iter()
                    .map(|entry| tool_button(&mut commands, &fonts, entry))
                    .collect()
            })
            .collect();
        let mut children: Vec<Entity> = Vec::new();
        for (si, btns) in section_buttons.iter().enumerate() {
            if si > 0 {
                let sep = tool_separator(&mut commands);
                commands.entity(sep).insert(ToolSepVis {
                    before: section_buttons[..si].concat(),
                    after: btns.clone(),
                });
                children.push(sep);
            }
            children.extend(btns.iter().copied());
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ToolsPopulated);
    }
    queue.apply(world);
}


// ── Add-shape dropdown ───────────────────────────────────────────────────────
//
// A dropdown that spawns any registered shape from the toolbar. It reads the
// same `ShapeRegistry` the shape-library panel uses (so the two never drift),
// grouped by category. Population is deferred to an exclusive system because the
// registry is filled by the shape crate's plugin at startup, after the header is
// built; the list is filled once the registry is non-empty.

/// The shapes-menu trigger button (for hover / open background tinting).
#[derive(Component)]
struct ShapeMenuTrigger;

/// The (initially empty) column inside the shapes popup that `populate_shapes`
/// fills from the registry.
#[derive(Component)]
struct ShapeMenuContainer;

/// Marks a shapes list that's already been filled, so it isn't refilled.
#[derive(Component)]
struct ShapesPopulated;

/// A selectable shape row — carries the registry id so the click handler can
/// look the rest up (name + default color) at spawn time.
#[derive(Component, Clone)]
struct ShapeSpawn {
    id: String,
}

fn build_shapes_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Empty column that `populate_shapes` fills; wrapped in a capped scroll area
    // since the registry holds ~30 shapes across several categories.
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            ShapeMenuContainer,
            Name::new("vp-shapes-list"),
        ))
        .id();
    let scroll = scroll_area(commands, container, 360.0);

    // Left-aligned: this is the leftmost toolbar control, so a right-aligned
    // panel would grow off the left edge of the window.
    let panel = popup_panel_aligned(commands, &[scroll], PopupAlign::Left);
    let trigger = icon_popup_trigger(commands, fonts, "shapes", panel);
    commands.entity(trigger).insert(ShapeMenuTrigger);
    popup_anchor(commands, trigger, panel)
}

/// A label + icon row that spawns shape `id` when clicked.
fn shape_row(commands: &mut Commands, fonts: &EmberFonts, icon: &str, name: &str, id: &str) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 14.0);
    let label = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ShapeSpawn { id: id.to_string() },
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-shape-row"),
        ))
        .id();
    commands.entity(row).add_children(&[glyph, label]);
    row
}

/// Fill an empty `ShapeMenuContainer` from `ShapeRegistry`, grouped by category
/// (a section label whenever the category changes, separators between groups).
/// Exclusive so it can spawn rows from the registry's `&World` data; runs until
/// the registry is populated and the container exists.
fn populate_shapes(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Snapshot (icon, name, id, category) so the borrow of the registry ends
    // before we open a `Commands` over the world.
    let shapes: Vec<(String, String, String, String)> = {
        let Some(reg) = world.get_resource::<ShapeRegistry>() else {
            return;
        };
        reg.iter()
            .map(|e| {
                (
                    e.icon.to_string(),
                    e.name.to_string(),
                    e.id.to_string(),
                    e.category.to_string(),
                )
            })
            .collect()
    };
    if shapes.is_empty() {
        return; // shapes not registered yet
    }
    let mut cq = world.query_filtered::<Entity, (With<ShapeMenuContainer>, Without<ShapesPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let mut children: Vec<Entity> = Vec::new();
        let mut last_cat: Option<&str> = None;
        for (icon, name, id, category) in &shapes {
            if last_cat != Some(category.as_str()) {
                if last_cat.is_some() {
                    children.push(separator_row(&mut commands));
                }
                children.push(section_label(&mut commands, &fonts, category));
                last_cat = Some(category.as_str());
            }
            children.push(shape_row(&mut commands, &fonts, icon, name, id));
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ShapesPopulated);
    }
    queue.apply(world);
}

/// Spawn the clicked shape at the origin (matching the hierarchy "Add Entity"
/// menu) through the undo system, then leave the menu open so several shapes can
/// be added in a row.
fn shape_spawn_click(
    q: Query<(&Interaction, &ShapeSpawn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, shape) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let id = shape.id.clone();
        cmds.push(move |w: &mut World| {
            let Some((shape_id, name, color)) = w
                .get_resource::<ShapeRegistry>()
                .and_then(|r| r.get(&id))
                .map(|e| (e.id.to_string(), e.name.to_string(), e.default_color))
            else {
                warn!("Shape '{id}' not found in registry");
                return;
            };
            execute(
                w,
                UndoContext::Scene,
                Box::new(SpawnShapeCmd {
                    entity: Entity::PLACEHOLDER,
                    shape_id,
                    name,
                    position: Vec3::ZERO,
                    color,
                }),
            );
        });
    }
}

/// Hover/open tinting for the shapes trigger + hover highlight for its rows.
fn update_shape_menu(
    theme: Option<Res<ThemeManager>>,
    mut trigger: Query<
        (&Interaction, &Popup, &mut BackgroundColor),
        (With<ShapeMenuTrigger>, Without<ShapeSpawn>),
    >,
    mut rows: Query<(&Interaction, &mut BackgroundColor), With<ShapeSpawn>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in &mut trigger {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    for (interaction, mut bg) in &mut rows {
        let want = if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
