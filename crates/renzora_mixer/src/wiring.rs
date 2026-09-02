//! Mixer plugin wiring — registers the audio component inspectors.
//!
//! The mixer panel itself is bevy_ui (ember) native; see [`crate::native_strips`].

use super::inspectors;

use bevy::prelude::*;

pub fn build(app: &mut App) {
    app.init_resource::<renzora::InspectorRegistry>();
    inspectors::register_audio_inspectors(
        &mut app
            .world_mut()
            .resource_mut::<renzora::InspectorRegistry>(),
    );
    inspectors::register_audio_native(app);
}
