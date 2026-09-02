//! The inline snap pills and the camera-speed widget — the two controls that
//! live in the bar itself rather than inside a dropdown.

use bevy::prelude::*;

use renzora::core::viewport_types::{SnapSettings, ViewportSettings};
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{hover_bg, rgb, text_primary, value_text};
use renzora_ember::widgets::{drag_value_flat, DragRange};
use renzora_theme::ThemeManager;

use super::{col, HeaderBg, WidgetBg, BTN_H};

/// Which snap the icon toggle in a snap-pair enables/disables.
#[derive(Component, Clone, Copy)]
pub(super) enum SnapToggle {
    Translate,
    Rotate,
    Scale,
}

/// The snap-pair pill, tagged with which snap it represents so its *whole*
/// background fills accent when that snap is enabled.
#[derive(Component, Clone, Copy)]
pub(super) struct SnapPillOf(SnapToggle);

/// Keep the bar + widget pills matched to the active theme so the toolbar reads
/// identically wherever it is mounted.
pub(super) fn update_header_chrome(
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

pub(super) fn snap_val(w: &Rx, f: impl Fn(&SnapSettings) -> f32) -> f32 {
    w.get_resource::<ViewportSettings>()
        .map(|s| f(&s.snap))
        .unwrap_or(0.0)
}

pub(super) fn set_snap(w: &mut World, f: impl Fn(&mut SnapSettings) -> &mut f32, v: f32) {
    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
        *f(&mut s.snap) = v;
    }
}

/// An icon toggle (enable/disable) + a scrubbable snap amount, in a pill.
#[allow(clippy::too_many_arguments)]
pub(super) fn snap_pair(
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
    // The pill chrome is shared with the UI editor's toolbar — see
    // `renzora_ember::widgets::toolbar`. Only the wiring is viewport-specific:
    // which setting the toggle flips, and what the number is bound to.
    let pill = renzora_ember::widgets::toolbar_pill(commands, fonts, icon, min, max, step);
    commands.entity(pill.toggle).insert(which);
    commands.entity(pill.root).insert(SnapPillOf(which));
    // Whole-number steps: the model quantizes to 1, so the readout never shows
    // decimals and every scrub/wheel/typed value lands on an integer.
    commands
        .entity(pill.value)
        .insert(renzora_ember::widgets::DragSnap(1.0));
    bind_2way(commands, pill.value, get, move |w, v: &f32| set(w, *v));
    pill.root
}

/// A camera icon + scrubbable move-speed (3D fly-cam).
pub(super) fn cam_speed_widget(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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

pub(super) fn snap_toggle_click(
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

pub(super) fn update_snap_toggles(
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
