//! Updates compass marker positions based on heading.

use bevy::prelude::*;

use crate::components::{CompassData, UiWidgetPart};

/// Repositions compass marker children based on current heading.
///
/// Markers that fall outside the visible FOV are hidden.
/// Each marker child has role `"marker_N"` where N is the index into
/// `CompassData::markers`.
pub fn compass_system(
    compasses: Query<(&CompassData, &Children), Changed<CompassData>>,
    mut nodes: Query<&mut Node>,
    parts: Query<&UiWidgetPart>,
) {
    for (data, children) in &compasses {
        let half_fov = data.fov * 0.5;
        for child in children.iter() {
            let Ok(part) = parts.get(child) else { continue };
            if !part.role.starts_with("marker_") {
                continue;
            }
            let idx: usize = match part.role.strip_prefix("marker_").and_then(|s| s.parse().ok())
            {
                Some(i) if i < data.markers.len() => i,
                _ => continue,
            };
            let marker = &data.markers[idx];

            // Calculate angular offset from heading, normalized to -180..180.
            let mut delta = marker.angle - data.heading;
            while delta > 180.0 {
                delta -= 360.0;
            }
            while delta < -180.0 {
                delta += 360.0;
            }

            let Ok(mut node) = nodes.get_mut(child) else {
                continue;
            };
            if delta.abs() > half_fov {
                // Outside visible FOV — hide.
                node.display = Display::None;
            } else {
                node.display = Display::Flex;
                // Position as percentage along the strip.
                let pct = (delta / data.fov + 0.5) * 100.0;
                node.left = Val::Percent(pct);
            }
        }
    }
}
