//! Tooltips — one global, cursor-following bubble driven by [`HoverTooltip`].
//!
//! WHY a global layer instead of per-widget bubble children: bevy_ui clips
//! absolutely-positioned children by every scrolling/clipping ancestor, so a
//! bubble spawned inside a panel silently vanishes the moment it pokes past
//! the panel bounds (`GlobalZIndex` fixes paint order, not clipping — this is
//! exactly why toolbar/inspector tooltips "didn't display"). The shared bubble
//! is a parentless root node, so nothing can clip it, and `Pickable::IGNORE`
//! keeps it from stealing hover from the widget under it.

use bevy::prelude::*;

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

/// Attach to any entity that has `Interaction`: hovering it shows `0` in the
/// shared tooltip bubble after a short delay.
#[derive(Component, Clone)]
pub struct HoverTooltip(pub String);

impl HoverTooltip {
    pub fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

/// Legacy wrapper API: wraps `target` in a hoverable node carrying a
/// [`HoverTooltip`]. Prefer inserting `HoverTooltip` directly on widgets that
/// already track `Interaction`.
pub fn tooltip(
    commands: &mut Commands,
    _font: &bevy::text::FontSource,
    label: &str,
    target: Entity,
) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            HoverTooltip::new(label),
            Name::new("tooltip"),
        ))
        .id();
    commands.entity(wrap).add_child(target);
    wrap
}

#[derive(Component)]
pub(crate) struct HoverTipRoot;

#[derive(Component)]
pub(crate) struct HoverTipText;

/// Seconds of steady hover before the bubble appears.
const SHOW_DELAY: f32 = 0.35;
/// Cursor → bubble offset (logical px).
const OFFSET: Vec2 = Vec2::new(14.0, 20.0);

pub(crate) fn hover_tooltip_system(
    mut commands: Commands,
    time: Res<Time>,
    windows: Query<(Entity, &Window)>,
    dock_windows: Option<Res<crate::dock::DockWindows>>,
    fonts: Option<Res<EmberFonts>>,
    tips: Query<(Entity, &Interaction, &HoverTooltip)>,
    mut root_q: Query<(Entity, &mut Node, &ComputedNode), With<HoverTipRoot>>,
    mut text_q: Query<&mut Text, With<HoverTipText>>,
    mut state: Local<Option<(Entity, f32)>>,
    mut last_cam: Local<Option<Option<Entity>>>,
) {
    let hide = |root_q: &mut Query<(Entity, &mut Node, &ComputedNode), With<HoverTipRoot>>| {
        if let Ok((_, mut node, _)) = root_q.single_mut() {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
    };

    let hovered = tips
        .iter()
        .find(|(_, i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed));
    // The window the cursor is in — hover only fires there, so the tooltip's
    // coordinates and rendering target both follow it (a widget in a floating
    // dock window shows its tooltip in that window, not the primary).
    let cursor_win = windows
        .iter()
        .find_map(|(e, w)| w.cursor_position().map(|c| (e, w, c)));
    let (Some((widget, _, tip)), Some((win_entity, win, cursor))) = (hovered, cursor_win) else {
        *state = None;
        hide(&mut root_q);
        return;
    };

    // Restart the delay whenever the hovered widget changes.
    let now = time.elapsed_secs();
    let started = match *state {
        Some((e, t)) if e == widget => t,
        _ => {
            *state = Some((widget, now));
            now
        }
    };
    if now - started < SHOW_DELAY {
        hide(&mut root_q);
        return;
    }

    // Lazily spawn the shared bubble as a ROOT node (no parent → no ancestor
    // can clip it away).
    let Ok((root_entity, mut node, cn)) = root_q.single_mut() else {
        let Some(fonts) = fonts else { return };
        let root = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(cursor.x + OFFSET.x),
                    top: Val::Px(cursor.y + OFFSET.y),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    // Hidden until next frame — its size isn't measured yet,
                    // so clamping to the window edges can't work this frame.
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(rgb(window_bg())),
                BorderColor::all(rgb(border())),
                GlobalZIndex(10_000),
                Pickable::IGNORE,
                HoverTipRoot,
                Name::new("hover-tooltip"),
            ))
            .id();
        let txt = commands
            .spawn((
                Text::new(tip.0.clone()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::no_wrap(),
                Pickable::IGNORE,
                HoverTipText,
            ))
            .id();
        commands.entity(root).add_child(txt);
        return;
    };

    if let Ok(mut text) = text_q.single_mut() {
        if text.0 != tip.0 {
            text.0.clone_from(&tip.0);
        }
    }

    // Render the bubble on the cursor window's camera: `UiTargetCamera` toward
    // a floating dock window's camera, or none (→ the primary default UI
    // camera). Guarded on change so it doesn't churn the entity every frame.
    let float_cam = dock_windows
        .as_ref()
        .and_then(|ws| ws.0.iter().find(|s| s.window == win_entity))
        .map(|s| s.camera);
    if *last_cam != Some(float_cam) {
        *last_cam = Some(float_cam);
        match float_cam {
            Some(cam) => {
                commands.entity(root_entity).insert(bevy::ui::UiTargetCamera(cam));
            }
            None => {
                commands.entity(root_entity).remove::<bevy::ui::UiTargetCamera>();
            }
        }
    }

    // Place beside the cursor, flipping/clamping at the window edges.
    // ComputedNode is physical px; Node offsets are logical.
    let size = cn.size() * cn.inverse_scale_factor();
    let mut pos = cursor + OFFSET;
    if pos.x + size.x > win.width() {
        pos.x = (cursor.x - size.x - 8.0).max(0.0);
    }
    if pos.y + size.y > win.height() {
        pos.y = (cursor.y - size.y - 8.0).max(0.0);
    }
    node.left = Val::Px(pos.x);
    node.top = Val::Px(pos.y);
    if node.display != Display::Flex {
        node.display = Display::Flex;
    }
}
