//! Theme switcher state — applies a pending theme selection.

use std::sync::Mutex;

use bevy::prelude::*;

use renzora_editor::SplashState;
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
