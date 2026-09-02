//! What the viewport is looking at and how: the View and Mode dropdowns, the
//! per-viewport view-angle menu, the World/Local gizmo-space toggle, and the two
//! systems that hide 3D-only or 2D-only widgets.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::core::viewport_types::{ViewportMode, ViewportSettings, ViewportView};
use renzora_editor_framework::GizmoSpace;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{rgb, tab_active, text_muted, text_primary};
use renzora_ember::widgets::{
    dropdown_compact, popup_anchor, popup_panel, EmberDropdownOption, Popup,
};

use super::camera::HeaderClick;
use super::display::DisplayTrigger;
use super::rows::click_row;
use super::{loc_opt, ThreeDOnly, TwoDOnly, BTN_H, BTN_W};

/// Per-viewport view-angle presets: (label, yaw, pitch). "Perspective" is the
/// default free 3/4 angle; the rest are the orthographic-style snaps.
pub(super) const VIEW_ANGLE_OPTIONS: &[(&str, f32, f32)] = {
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
pub(super) struct ModeDropdown;

/// A viewport's view-angle menu trigger: which slot it drives, and the `Text`
/// entity showing the current pick. Picking a row writes that label and closes
/// the menu — ember's `popup_dismiss` deliberately leaves a popup open when the
/// click lands inside it (the Display/Snap/Camera panels want that, since you
/// flip several switches in a row), but a one-shot action list should close.
#[derive(Component)]
pub(super) struct ViewAngleTrigger {
    pub(super) slot: usize,
    pub(super) label: Entity,
}

/// The shared **View** combobox (3D / 2D / UI), bound to
/// `ViewportSettings::viewport_view`.
pub(super) fn view_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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
pub(super) fn mode_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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
pub(super) fn update_mode_options(
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
pub(super) fn view_angle_menu(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
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

// ── World / Local gizmo-space toggle ─────────────────────────────────────────

/// Tags a space-toggle button with the viewport slot it controls (each viewport
/// has its own World/Local toggle, acting independently — see
/// `renzora::core::viewport_types::ViewportGizmoSpace`).
#[derive(Component, Clone, Copy)]
pub(super) struct SpaceToggleSlot(usize);

/// Points a space-toggle button at its child glyph `Text` entity.
#[derive(Component)]
pub(super) struct SpaceToggleGlyphRef(Entity);

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
pub(super) fn space_toggle(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
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
pub(super) fn space_toggle_click(
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
pub(super) fn update_space_toggle(
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

// ── 2D / 3D gating ───────────────────────────────────────────────────────────

pub(super) fn update_three_d_only(
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

/// Keep the interaction mode legal for the active view: switching views
/// while in a view-specific mode (Sculpt is 3D-only, Erase is 2D-only)
/// falls back to Select, matching what the Mode dropdown offers. Covers
/// every entry path — the dropdown, Tab shortcuts, and panels that set the
/// mode directly.
pub(super) fn sanitize_mode_for_view(settings: Option<ResMut<ViewportSettings>>) {
    let Some(mut s) = settings else { return };
    if !ViewportMode::for_view(s.viewport_view).contains(&s.viewport_mode) {
        s.viewport_mode = ViewportMode::Scene;
    }
}

/// Sibling of [`update_three_d_only`]: shows `TwoDOnly` widgets only in 2D view.
pub(super) fn update_two_d_only(
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
