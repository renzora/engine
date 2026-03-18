use bevy::prelude::*;

/// Entry point called from the Swift/Objective-C bridge.
///
/// The iOS app host calls this from `AppDelegate` after UIKit is ready.
/// Bevy's winit backend handles the Metal surface via the existing UIWindow.
#[unsafe(no_mangle)]
pub extern "C" fn renzora_main() {
    let mut app = renzora::build_runtime_app();
    app.run();
}
