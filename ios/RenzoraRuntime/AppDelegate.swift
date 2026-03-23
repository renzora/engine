import UIKit

/// C function exported by the Rust static library.
@_silgen_name("renzora_main")
func renzora_main()

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Bevy/winit creates its own UIWindow via the Metal backend.
        // We just need to kick off the Rust entry point.
        renzora_main()
        return true
    }
}
