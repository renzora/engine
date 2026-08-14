//! Theme switcher state — applies a pending theme selection.

use std::sync::Mutex;

use bevy::prelude::*;

use renzora::SplashState;
use renzora_theme::ThemeManager;

// ============================================================================
// Deferred-apply channel
// ============================================================================

/// Carries a pending theme selection into a mutable-world system that applies
/// it. (The native shell's theme switcher writes `next`; the old egui status
/// item that also drove this has been removed.)
#[derive(Resource, Default)]
struct ThemeStatusPending {
    next: Mutex<Option<String>>,
}

fn apply_pending_theme(pending: Res<ThemeStatusPending>, mut tm: ResMut<ThemeManager>) {
    if let Ok(mut slot) = pending.next.lock() {
        if let Some(name) = slot.take() {
            if name != tm.active_theme_name {
                tm.load_theme(&name);
            }
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct ThemeStatusPlugin;

impl Plugin for ThemeStatusPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ThemeStatusPlugin");

        app.init_resource::<ThemeStatusPending>();
        app.add_systems(
            Update,
            apply_pending_theme.run_if(in_state(SplashState::Editor)),
        );
    }
}

renzora::add!(ThemeStatusPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A world with the two resources the system takes, and a theme already
    /// active so "switching" and "re-selecting the same one" are distinguishable.
    fn world_with(active: &str, pending: Option<&str>) -> World {
        let mut world = World::new();
        let mut tm = ThemeManager::default();
        tm.load_theme(active);
        world.insert_resource(tm);
        world.insert_resource(ThemeStatusPending {
            next: Mutex::new(pending.map(|s| s.to_string())),
        });
        world
    }

    #[test]
    fn a_pending_selection_is_applied() {
        let mut world = world_with("Dark", Some("Light"));
        world.run_system_once(apply_pending_theme).unwrap();
        assert_eq!(world.resource::<ThemeManager>().active_theme_name, "Light");
    }

    /// `take()` is what makes this a channel rather than a latch. If the slot
    /// were only read, every later frame would re-load the theme — and reloading
    /// discards unsaved edits in the theme editor.
    #[test]
    fn the_pending_slot_is_drained_after_one_apply() {
        let mut world = world_with("Dark", Some("Light"));
        world.run_system_once(apply_pending_theme).unwrap();
        assert!(
            world
                .resource::<ThemeStatusPending>()
                .next
                .lock()
                .unwrap()
                .is_none(),
            "the selection should have been taken, not left in place"
        );
    }

    /// Re-selecting the already-active theme must not reload it. `load_theme`
    /// clears `has_unsaved_changes`, so a needless reload silently throws away
    /// the user's in-progress theme edits — which is exactly what the name
    /// comparison guards.
    #[test]
    fn re_selecting_the_active_theme_does_not_reload_it() {
        let mut world = world_with("Dark", Some("Dark"));
        world.resource_mut::<ThemeManager>().has_unsaved_changes = true;

        world.run_system_once(apply_pending_theme).unwrap();

        let tm = world.resource::<ThemeManager>();
        assert_eq!(tm.active_theme_name, "Dark");
        assert!(
            tm.has_unsaved_changes,
            "the theme was reloaded and unsaved changes were discarded"
        );
    }

    #[test]
    fn an_empty_slot_changes_nothing() {
        let mut world = world_with("Dark", None);
        world.resource_mut::<ThemeManager>().has_unsaved_changes = true;
        world.run_system_once(apply_pending_theme).unwrap();

        let tm = world.resource::<ThemeManager>();
        assert_eq!(tm.active_theme_name, "Dark");
        assert!(tm.has_unsaved_changes);
    }

    #[test]
    fn the_plugin_registers_the_pending_resource() {
        let mut app = App::new();
        app.insert_resource(ThemeManager::default());
        app.add_plugins(ThemeStatusPlugin);
        assert!(app.world().get_resource::<ThemeStatusPending>().is_some());
    }
}
