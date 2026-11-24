//! Which graphics backend this process asks wgpu for, and whose setting that is.
//!
//! wgpu picks the backend once, when `RenderPlugin` is built, and cannot switch
//! afterwards, so the choice is made before `DefaultPlugins` is added and a
//! change applies at the next launch. It is two different settings depending
//! on who is running:
//!
//! - **The editor** (and its companion process) uses the editor user's own
//!   preference, `[editor] renderer_backend` in `~/.renzora/settings.toml`. It
//!   is about their machine and their editor window, not about any game.
//! - **A game** (a shipped build, the Play child, a host or VR session) uses its
//!   project's `[rendering] backend`, read early by [`crate::early_project`].
//!   It used to read the editor preference too, so a game inherited the
//!   developer's editor backend on their machine and fell back to Auto on a
//!   player's, and no exported game could choose one at all.
//!
//! The choice is recorded once per process. The ray-tracing and integrated-GPU
//! probes run later and must look at the same adapter the renderer will, so
//! they read the recorded choice rather than deciding again.

use std::sync::OnceLock;

use bevy::render::settings::Backends;
use renzora_project_api::RendererBackend;

/// The backend this process asked for, and where that came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendChoice {
    pub backend: RendererBackend,
    /// `"editor preference"` or `"project"`, for the startup log line.
    pub source: &'static str,
    /// A backend that was asked for but cannot be created on this platform
    /// (a project set to DX12 running on Linux), and was replaced by `Auto`.
    /// Asking wgpu for it anyway finds no adapter and panics at startup.
    pub unavailable: Option<RendererBackend>,
}

static CHOICE: OnceLock<BackendChoice> = OnceLock::new();

/// Decide the backend for this process, once. Called by each windowed plugin
/// set before it builds `RenderPlugin`; a second call returns the first answer,
/// because the renderer has already been created with it.
pub fn select(is_editor: bool) -> BackendChoice {
    *CHOICE.get_or_init(|| {
        let project = if is_editor {
            None
        } else {
            Some(
                crate::early_project::early_project_config()
                    .map(|c| c.rendering.backend)
                    .unwrap_or_default(),
            )
        };
        choose(
            renzora_project_api::load_renderer_backend,
            project,
            RendererBackend::available(),
        )
    })
}

/// Say which backend was asked for and whose setting it was. Separate from
/// [`select`] because the choice is made before `DefaultPlugins` builds the log
/// subscriber, and a line logged then is dropped without a trace.
pub fn log(choice: BackendChoice) {
    match choice.unavailable {
        None => bevy::log::info!(
            "[runtime] renderer backend: {} (from the {})",
            choice.backend.resolved().label(),
            choice.source
        ),
        Some(wanted) => bevy::log::warn!(
            "[runtime] renderer backend {} (from the {}) is not available on this platform; \
             using {} instead",
            wanted.label(),
            choice.source,
            choice.backend.resolved().label()
        ),
    }
}

/// The recorded choice, or the editor preference when nothing was selected.
/// The fallback is what every path did before games had a setting of their
/// own, so a plugin set that never calls [`select`] behaves as it always has.
pub fn current() -> BackendChoice {
    CHOICE.get().copied().unwrap_or_else(|| {
        choose(
            renzora_project_api::load_renderer_backend,
            None,
            RendererBackend::available(),
        )
    })
}

/// The wgpu backend set for the recorded choice.
pub fn requested_backends() -> Backends {
    backends_for(current().backend)
}

/// Pick between the editor preference and the project's setting. `project` is
/// `Some` exactly when this process is a game, even when the project says
/// `Auto`: a game never falls through to the editor's preference. The editor
/// preference is a closure so a game never reads the editor's settings file.
fn choose(
    editor_preference: impl FnOnce() -> RendererBackend,
    project: Option<RendererBackend>,
    available: &[RendererBackend],
) -> BackendChoice {
    let (wanted, source) = match project {
        Some(backend) => (backend, "project"),
        None => (editor_preference(), "editor preference"),
    };
    if available.contains(&wanted) {
        BackendChoice { backend: wanted, source, unavailable: None }
    } else {
        BackendChoice { backend: RendererBackend::Auto, source, unavailable: Some(wanted) }
    }
}

/// Map a backend setting to the single wgpu backend it requests. Always one:
/// Bevy's default `Backends::all()` loads DX12/DXGI beside Vulkan, and DXGI's
/// occlusion handling clobbered the Vulkan surface when the game window took
/// the foreground, which was the play-mode crash that once pushed the engine
/// toward DX12.
///
/// `Auto` is the per-OS standard: Vulkan on Windows, Linux and the BSDs (the
/// path shared across most platforms, fast to load, no DX12 frame flicker) and
/// Metal on Apple platforms. DX12 stays a choice for Windows machines with weak
/// or missing Vulkan drivers.
pub fn backends_for(backend: RendererBackend) -> Backends {
    match backend {
        RendererBackend::Auto => {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                Backends::METAL
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            {
                Backends::VULKAN
            }
        }
        RendererBackend::Dx12 => Backends::DX12,
        RendererBackend::Vulkan => Backends::VULKAN,
        RendererBackend::Metal => Backends::METAL,
        RendererBackend::Gl => Backends::GL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[RendererBackend] = &[
        RendererBackend::Auto,
        RendererBackend::Dx12,
        RendererBackend::Vulkan,
        RendererBackend::Metal,
        RendererBackend::Gl,
    ];

    #[test]
    fn editor_uses_its_own_preference() {
        let choice = choose(|| RendererBackend::Dx12, None, ALL);
        assert_eq!(choice.backend, RendererBackend::Dx12);
        assert_eq!(choice.source, "editor preference");
    }

    #[test]
    fn game_uses_the_project_and_never_reads_the_editor_preference() {
        let choice = choose(|| panic!("a game read the editor preference"), Some(RendererBackend::Vulkan), ALL);
        assert_eq!(choice.backend, RendererBackend::Vulkan);
        assert_eq!(choice.source, "project");
    }

    #[test]
    fn a_game_on_auto_stays_on_auto() {
        let choice = choose(|| panic!("a game read the editor preference"), Some(RendererBackend::Auto), ALL);
        assert_eq!(choice.backend, RendererBackend::Auto);
    }

    #[test]
    fn a_backend_this_platform_lacks_falls_back_to_auto() {
        let available = &[RendererBackend::Auto, RendererBackend::Vulkan];
        let choice = choose(|| RendererBackend::Auto, Some(RendererBackend::Dx12), available);
        assert_eq!(choice.backend, RendererBackend::Auto);
        assert_eq!(choice.unavailable, Some(RendererBackend::Dx12));
    }

    #[test]
    fn every_setting_requests_exactly_one_backend() {
        for &backend in ALL {
            assert_eq!(backends_for(backend).iter().count(), 1, "{backend:?}");
        }
    }
}
