//! Anti-aliasing: one [`AntiAliasing`] component on a camera, choosing one of
//! MSAA, FXAA, SMAA, TAA or DLSS, plus sharpening ([`CasSettings`]) on top.
//!
//! It used to be a component per method, which let a camera run several at
//! once: TAA under FXAA blurs twice, and TAA beside DLSS fights over one
//! jitter. One component with one method makes those combinations impossible
//! to set up. Scenes saved with the old components are converted as they load
//! (see [`legacy`]).
//!
//! Not every method can run on every camera. MSAA cannot share a camera with
//! deferred shading, SSAO, the sky or OIT; DLSS needs the SDK in the build and
//! an RTX GPU. [`compat`] holds those rules once, for both the systems that
//! apply a method and the inspector that warns about one.

pub mod compat;
pub mod legacy;
mod settings;
mod sharpening;
mod systems;

use bevy::prelude::*;

pub use legacy::{FxaaSettings, SmaaSettings, TaaSettings};
pub use settings::{AntiAliasing, AntiAliasingMethod};
pub use sharpening::CasSettings;

#[derive(Default)]
pub struct AntiAliasingPlugin;

impl Plugin for AntiAliasingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AntiAliasingPlugin");
        app.register_type::<AntiAliasing>();
        app.register_type::<CasSettings>();
        app.register_type::<FxaaSettings>();
        app.register_type::<SmaaSettings>();
        app.register_type::<TaaSettings>();
        app.add_systems(Update, (legacy::migrate_legacy, sharpening::sync_cas));
        app.add_systems(
            PostUpdate,
            (systems::reconcile_anti_aliasing, systems::release_temporal_jitter).chain(),
        );
    }
}

renzora::add!(AntiAliasingPlugin);
