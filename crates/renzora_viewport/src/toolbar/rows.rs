//! The row builders every dropdown in this toolbar is assembled from, plus the
//! two macros that bind a row straight to a `ViewportSettings` field.
//!
//! The macros spell their paths out in full (`crate::toolbar::rows::…`,
//! `renzora::core::viewport_types::ViewportSettings`) rather than relying on
//! what happens to be imported: they expand in four different modules, and a
//! macro that compiles only where its author happened to be standing is a trap
//! for whoever adds the fifth dropdown.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::core::viewport_types::{ProjectionMode, ViewportSettings};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{border, rgb, text_muted, text_primary, value_text};
use renzora_ember::widgets::{drag_value, toggle_switch, DragRange};
use renzora_theme::ThemeManager;

use super::camera::{HeaderClick, ProjOption, SnapBtnKind};
use super::display::DisplayOption;
use super::{col, BTN_H};

/// Builds a [`check_row`] bound to `ViewportSettings.<field-path>`.
macro_rules! toggle_row {
    ($c:expr, $f:expr, $label:expr, $($field:tt)+) => {
        crate::toolbar::rows::check_row(
            $c,
            $f,
            $label,
            |w: &renzora_ember::reactive::Rx| {
                w.get_resource::<renzora::core::viewport_types::ViewportSettings>()
                    .map(|s| s.$($field)+)
                    .unwrap_or(false)
            },
            |w: &mut bevy::prelude::World, v: bool| {
                if let Some(mut s) =
                    w.get_resource_mut::<renzora::core::viewport_types::ViewportSettings>()
                {
                    s.$($field)+ = v;
                }
            },
        )
    };
}
pub(super) use toggle_row;

/// Builds a label + boxed [`drag_value`] row bound to `ViewportSettings.<path>`.
macro_rules! drag_row {
    ($c:expr, $f:expr, $label:expr, $min:expr, $max:expr, $step:expr, $($field:tt)+) => {
        crate::toolbar::rows::drag_row_build(
            $c, $f, $label, $min, $max, $step,
            |w: &renzora_ember::reactive::Rx| {
                w.get_resource::<renzora::core::viewport_types::ViewportSettings>()
                    .map(|s| s.$($field)+)
                    .unwrap_or($min)
            },
            |w: &mut bevy::prelude::World, v: f32| {
                if let Some(mut s) =
                    w.get_resource_mut::<renzora::core::viewport_types::ViewportSettings>()
                {
                    s.$($field)+ = v;
                }
            },
        )
    };
}
pub(super) use drag_row;

pub(super) fn section_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Name::new("vp-section-label"),
        ))
        .id()
}

pub(super) fn separator_row(commands: &mut Commands) -> Entity {
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
pub(super) fn option_row(
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
pub(super) fn check_row(
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

/// A label + click-to-fire row (view angles, reset).
pub(super) fn click_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    click: HeaderClick,
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
            click,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-click-row"),
        ))
        .id();
    commands.entity(row).add_child(txt);
    row
}

/// A projection-mode row (highlights when current).
pub(super) fn proj_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    mode: ProjectionMode,
    label: &str,
) -> Entity {
    let row = click_row(commands, fonts, label, HeaderClick::Projection(mode));
    commands.entity(row).insert(ProjOption(mode));
    row
}

/// A toggle button (Objects / Floor) that fills accent when its snap is on.
pub(super) fn snap_button(
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
            BackgroundColor(rgb(renzora_ember::theme::hover_bg())),
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
pub(super) fn drag_row_build(
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

/// Hover highlight for plain click rows (view angles, reset) — projection rows
/// and snap buttons are handled by [`update_panel_buttons`].
pub(super) fn update_click_rows(
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

pub(super) fn update_panel_buttons(
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
