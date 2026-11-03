//! The per-method components [`AntiAliasing`] replaced, kept so scenes saved
//! with them still load, and turned into an [`AntiAliasing`] as they arrive.
//!
//! They stay registered under their old type paths because a scene names its
//! components by type path: a scene holding `renzora_antialiasing::TaaSettings`
//! with no such type registered fails to load the entity, it does not skip the
//! one component. Nothing reads them once migrated, and nothing inserts them;
//! the inspector no longer offers them.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::settings::{AntiAliasing, AntiAliasingMethod};

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FxaaSettings {
    /// 0=Low, 1=Medium, 2=High, 3=Ultra, 4=Extreme
    pub edge_threshold: u32,
    pub edge_threshold_min: u32,
    pub enabled: bool,
}

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SmaaSettings {
    /// 0=Low, 1=Medium, 2=High, 3=Ultra
    pub preset: u32,
    pub enabled: bool,
}

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaaSettings {
    pub reset: bool,
    pub enabled: bool,
}

/// Fold an entity's old components into one [`AntiAliasing`].
///
/// They could all be on at once before, so one has to win. TAA first, as the
/// one the default camera carried and the one the rest of the rendering is
/// tuned for; then SMAA over FXAA, as the sharper of the two. When none was on,
/// the method is still taken from what was there (in the same order) and the
/// component lands disabled, so the choice survives for whoever turns it on.
///
/// An entity that already has an [`AntiAliasing`] keeps it untouched, and a 2D
/// camera gets none: the old components are only dropped. Old 2D cameras carry
/// a `TaaSettings` the camera seeder gave them before it learned to skip 2D,
/// and TAA on a 2D camera would pull the 3D prepasses onto it.
#[allow(clippy::type_complexity)]
pub(crate) fn migrate_legacy(
    mut commands: Commands,
    old: Query<
        (
            Entity,
            Option<&TaaSettings>,
            Option<&SmaaSettings>,
            Option<&FxaaSettings>,
            Has<AntiAliasing>,
            Has<Camera2d>,
        ),
        Or<(With<TaaSettings>, With<SmaaSettings>, With<FxaaSettings>)>,
    >,
) {
    for (entity, taa, smaa, fxaa, has_new, is_2d) in &old {
        let mut ec = commands.entity(entity);
        ec.try_remove::<(TaaSettings, SmaaSettings, FxaaSettings)>();
        if has_new || is_2d {
            continue;
        }
        ec.try_insert(merge(taa, smaa, fxaa));
    }
}

fn merge(
    taa: Option<&TaaSettings>,
    smaa: Option<&SmaaSettings>,
    fxaa: Option<&FxaaSettings>,
) -> AntiAliasing {
    let mut aa = AntiAliasing::default();
    if let Some(s) = smaa {
        aa.smaa_quality = s.preset.min(3);
    }
    if let Some(f) = fxaa {
        aa.fxaa_quality = f.edge_threshold.min(4);
    }
    let on = |e: Option<bool>| e == Some(true);
    let (method, enabled) = if on(taa.map(|t| t.enabled)) {
        (AntiAliasingMethod::Taa, true)
    } else if on(smaa.map(|s| s.enabled)) {
        (AntiAliasingMethod::Smaa, true)
    } else if on(fxaa.map(|f| f.enabled)) {
        (AntiAliasingMethod::Fxaa, true)
    } else if taa.is_some() {
        (AntiAliasingMethod::Taa, false)
    } else if smaa.is_some() {
        (AntiAliasingMethod::Smaa, false)
    } else {
        (AntiAliasingMethod::Fxaa, false)
    };
    aa.method = method;
    aa.enabled = enabled;
    aa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taa_wins_over_the_post_process_methods() {
        let aa = merge(
            Some(&TaaSettings { enabled: true, ..default() }),
            Some(&SmaaSettings { enabled: true, preset: 3 }),
            None,
        );
        assert_eq!((aa.method, aa.enabled), (AntiAliasingMethod::Taa, true));
        // The losing method's setting is kept, for switching to it later.
        assert_eq!(aa.smaa_quality, 3);
    }

    #[test]
    fn the_enabled_one_wins_over_a_disabled_taa() {
        let aa = merge(
            Some(&TaaSettings { enabled: false, ..default() }),
            None,
            Some(&FxaaSettings { enabled: true, edge_threshold: 4, edge_threshold_min: 2 }),
        );
        assert_eq!((aa.method, aa.enabled, aa.fxaa_quality), (AntiAliasingMethod::Fxaa, true, 4));
    }

    #[test]
    fn nothing_on_keeps_the_choice_but_lands_disabled() {
        let aa = merge(None, Some(&SmaaSettings { enabled: false, preset: 1 }), None);
        assert_eq!((aa.method, aa.enabled), (AntiAliasingMethod::Smaa, false));
    }
}
