//! Ticks notification timers, fading and removing expired ones.

use bevy::prelude::*;

use crate::components::NotificationFeedData;

/// Counts down notification lifetimes and removes expired entries.
///
/// Also trims the list to `max_visible` by dropping the oldest (front) items.
pub fn notification_system(time: Res<Time>, mut feeds: Query<&mut NotificationFeedData>) {
    let dt = time.delta_secs();
    for mut data in &mut feeds {
        data.notifications.retain_mut(|notif| {
            notif.remaining -= dt;
            notif.remaining > 0.0
        });
        // Trim to max visible.
        while data.notifications.len() > data.max_visible {
            data.notifications.remove(0);
        }
    }
}
