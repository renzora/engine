//! The editor's one piece of Graphics Quality: a hint pointing an
//! integrated-GPU user at the `Low` tier.
//!
//! The tier itself is not applied here. It is a game setting, so the runtime
//! applies it in every session, the editor viewport included
//! (`renzora_engine::graphics_quality`); this crate used to carry a second,
//! editor-only copy of that enforcement, which made the editor the only thing
//! applying the tier while editing.

use bevy::prelude::*;

use renzora_project_api::{CurrentProject, SplashState};
use renzora_viewport_api::GraphicsQuality;

/// Suggest the `Low` tier once per session when running on an integrated GPU.
///
/// The tier defaults to `Medium`, and the users who most need `Low` are exactly
/// the ones least likely to go looking for Settings > Project > Rendering:
/// the editor just feels slow and they have no reason to suspect a setting.
///
/// Deliberately a *hint*, not an action: nothing changes the user's tier for
/// them. A silently-applied override would be indistinguishable from the engine
/// misbehaving, and someone on integrated graphics may well have picked their
/// tier on purpose.
///
/// Needs no "already asked" flag on disk because the condition is
/// **self-clearing**: it only fires while the tier is not already `Low`, so
/// acting on it silences it permanently. Ignoring it costs one toast per launch,
/// which is the right pressure for a hint the user hasn't acted on.
fn suggest_low_tier_on_integrated_gpu(
    integrated: Option<Res<renzora_render_api::GpuIsIntegrated>>,
    project: Option<Res<CurrentProject>>,
    toasts: Option<ResMut<renzora_ui::Toasts>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let (Some(integrated), Some(project), Some(mut toasts)) = (integrated, project, toasts) else {
        return;
    };
    if !integrated.yes {
        *done = true;
        return;
    }
    if project.config.rendering.graphics_quality == GraphicsQuality::Low {
        *done = true;
        return;
    }
    toasts.info(renzora_lang_api::t("settings.hint.integrated_gpu"));
    *done = true;
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        PostUpdate,
        suggest_low_tier_on_integrated_gpu.run_if(in_state(SplashState::Editor)),
    );
}
