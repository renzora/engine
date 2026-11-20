//! The distance beside the orbit pivot's ring.
//!
//! The camera draws the ring (`renzora_camera::pivot_marker`) and publishes
//! where it is and how strongly it shows in [`PivotMarkerState`]; this labels
//! it with how far away it is. That number is the point of the marker: orbit,
//! pan and zoom all scale with it, so "1.1 km" says at a glance why a zoom
//! lurched.
//!
//! Spawned at the root rather than inside a viewport panel: the pivot belongs
//! to whichever viewport the editor camera is in, and a root node can follow it
//! there.

use bevy::prelude::*;

use renzora::core::viewport_types::{PivotMarkerState, ViewportState};
use renzora::core::EditorCamera;
use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};

/// Logical pixels from the pivot to the label's top-left corner: clear of the
/// ring, to its lower right.
const OFFSET: Vec2 = Vec2::new(12.0, 6.0);

#[derive(Component)]
struct PivotLabel;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, update_pivot_label.run_if(in_state(SplashState::Editor)));
}

fn update_pivot_label(
    mut commands: Commands,
    marker: Option<Res<PivotMarkerState>>,
    viewport: Option<Res<ViewportState>>,
    fonts: Option<Res<EmberFonts>>,
    camera: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut label: Query<(&mut Node, &mut Text, &mut TextColor, &mut BackgroundColor), With<PivotLabel>>,
) {
    let Some(marker) = marker else { return };
    // Where the pivot is on screen, in window logical pixels.
    let spot = viewport.as_ref().filter(|_| marker.opacity > 0.0).and_then(|vp| {
        let (cam, at) = camera.single().ok()?;
        let in_target = cam.world_to_viewport(at, marker.position).ok()?;
        let size = vp.current_size.as_vec2();
        let local = in_target * vp.screen_size / size.max(Vec2::ONE);
        let inside = local.cmpge(Vec2::ZERO).all() && local.cmple(vp.screen_size).all();
        inside.then_some(vp.screen_position + local)
    });

    let Ok((mut node, mut text, mut color, mut background)) = label.single_mut() else {
        if let (Some(_), Some(fonts)) = (spot, fonts) {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    display: Display::None,
                    ..default()
                },
                Text::new(""),
                ui_font(&fonts.ui, 11.0),
                TextColor(Color::WHITE),
                BackgroundColor(Color::BLACK.with_alpha(0.5)),
                GlobalZIndex(8500),
                bevy::picking::Pickable::IGNORE,
                PivotLabel,
                Name::new("vp-pivot-label"),
            ));
        }
        return;
    };

    let Some(spot) = spot else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    };
    node.display = Display::Flex;
    node.left = Val::Px(spot.x + OFFSET.x);
    node.top = Val::Px(spot.y + OFFSET.y);
    let next = format_distance(marker.distance);
    if text.0 != next {
        text.0 = next;
    }
    // The ring's colour: amber while pivot lock holds it in place.
    let tint = if marker.locked { Color::srgb(1.0, 0.72, 0.2) } else { Color::WHITE };
    color.0 = tint.with_alpha(marker.opacity);
    background.0 = Color::BLACK.with_alpha(0.5 * marker.opacity);
}

/// Metres to a short readout: a decimal close up, whole metres further out,
/// kilometres beyond that.
fn format_distance(metres: f32) -> String {
    if metres < 10.0 {
        format!("{metres:.1} m")
    } else if metres < 1000.0 {
        format!("{:.0} m", metres.round())
    } else {
        format!("{:.1} km", metres / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_read_at_a_sensible_precision() {
        assert_eq!(format_distance(4.5), "4.5 m");
        assert_eq!(format_distance(128.4), "128 m");
        assert_eq!(format_distance(1149.0), "1.1 km");
    }
}
