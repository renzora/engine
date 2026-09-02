//! Native (bevy_ui) modal-scale HUD — recreates the egui overlay that was
//! dropped in the bevy_ui migration.
//!
//! When a Blender-style modal **scale** (S) is in progress, this draws a
//! screen-space reference circle at the pivot, a line from the pivot out to the
//! cursor (the cursor itself is hidden during the drag), a centre dot, and the
//! live scale factor. When the cursor sits on the circle the factor is exactly
//! 1; dragging inward shrinks, outward grows — the same ratio `apply_scale`
//! uses. All values come from [`ModalTransformHud`], which the gizmo crate fills
//! every frame (`sync_modal_hud`); this crate owns the rendering because it has
//! the editor fonts.
//!
//! The overlay is rebuilt each frame (it's only alive for the brief gesture) as
//! flat, root-level absolute nodes in window coordinates — the same approach the
//! box-selection rectangle uses — so positions map straight from the HUD's
//! screen-space pivot/cursor.

use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::UiTransform;

use renzora::core::ModalTransformHud;
use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};

/// Marker on every node spawned for the modal-scale HUD, so the whole overlay
/// can be cleared in one query each frame.
#[derive(Component)]
struct ModalHudPart;

/// Smallest reference-circle radius (px). The circle tracks the start distance,
/// but when the gesture began with the cursor on (or very near) the pivot — e.g.
/// the cursor was warped onto a pivot that wasn't under it — fall back to this
/// so there's still something to read against.
const MIN_RADIUS: f32 = 36.0;
const DOT: f32 = 7.0;

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        render_modal_scale_hud.run_if(in_state(SplashState::Editor)),
    );
}

fn render_modal_scale_hud(
    mut commands: Commands,
    hud: Option<Res<ModalTransformHud>>,
    fonts: Option<Res<EmberFonts>>,
    existing: Query<Entity, With<ModalHudPart>>,
) {
    // Clear the previous frame's overlay; rebuilt below while the gesture lasts.
    for e in &existing {
        commands.entity(e).despawn();
    }

    let Some(hud) = hud else { return };
    if !hud.active || !hud.is_scale {
        return;
    }
    let Some(p) = hud.pivot else { return };

    let pivot = Vec2::new(p[0], p[1]);
    let cursor = Vec2::new(hud.cursor[0], hud.cursor[1]);
    let to_cursor = cursor - pivot;
    let dist = to_cursor.length().max(0.001);
    let angle = to_cursor.y.atan2(to_cursor.x);
    let radius = hud.ref_radius.max(MIN_RADIUS);

    // Axis-constraint colour (white when unconstrained); alpha tuned per element.
    let col = |alpha: f32| {
        let [r, g, b, _] = hud.axis_color;
        Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, alpha)
    };

    // Reference circle (when the cursor is on it, factor == 1).
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(pivot.x - radius),
            top: Val::Px(pivot.y - radius),
            width: Val::Px(radius * 2.0),
            height: Val::Px(radius * 2.0),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(radius)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(col(0.5)),
        GlobalZIndex(9000),
        bevy::ui::FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        ModalHudPart,
        Name::new("modal-scale-hud-circle"),
    ));

    // Line from the pivot to the cursor. The node is centred on the segment
    // midpoint and rotated about its own centre, so it spans pivot -> cursor.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(pivot.x + to_cursor.x * 0.5 - dist * 0.5),
            top: Val::Px(pivot.y + to_cursor.y * 0.5 - 1.0),
            width: Val::Px(dist),
            height: Val::Px(2.0),
            border_radius: BorderRadius::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(col(0.95)),
        UiTransform::from_rotation(Rot2::radians(angle)),
        GlobalZIndex(9001),
        bevy::ui::FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        ModalHudPart,
        Name::new("modal-scale-hud-line"),
    ));

    // Centre dot on the pivot.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(pivot.x - DOT * 0.5),
            top: Val::Px(pivot.y - DOT * 0.5),
            width: Val::Px(DOT),
            height: Val::Px(DOT),
            border_radius: BorderRadius::all(Val::Px(DOT * 0.5)),
            ..default()
        },
        BackgroundColor(col(1.0)),
        GlobalZIndex(9002),
        bevy::ui::FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        ModalHudPart,
        Name::new("modal-scale-hud-dot"),
    ));

    // Readout near the cursor: the typed value if the user is entering one,
    // otherwise the live distance-ratio factor.
    if let Some(fonts) = fonts {
        let label = if hud.numeric_display.is_empty() {
            format!("{:.3}", hud.scale_factor)
        } else {
            hud.numeric_display.clone()
        };
        commands.spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 14.0),
                top: Val::Px(cursor.y - 6.0),
                ..default()
            },
            GlobalZIndex(9003),
            bevy::ui::FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
            ModalHudPart,
            Name::new("modal-scale-hud-text"),
        ));
    }
}
