//! Startup check for the Linux system this process landed on, so an unsupported
//! machine gets a sentence that says what is missing instead of a panic deep in
//! winit or wgpu.
//!
//! Two layers check the same machine, because each can only see part of it:
//!
//! - The AppImage's `AppRun` (`renzora_export/src/apprun_preflight.sh`) runs
//!   **before** the dynamic loader touches the binary. Only it can report a
//!   glibc that is too old, a musl distro, NixOS without `nix-ld`, or a linked
//!   library that is not installed: in every one of those cases the loader
//!   refuses the executable before `main` and nothing in Rust ever runs.
//! - This module runs at the top of `main`, so it also covers a binary started
//!   without the AppImage. What it checks is what the loader cannot see: the
//!   libraries winit and wgpu `dlopen` later (Wayland, X11, xkbcommon, Vulkan,
//!   EGL) and whether there is a display server to open a window on. Without
//!   them the process starts, gets as far as creating the window or the GPU
//!   instance, and dies with a message that names neither the library nor the
//!   package.
//!
//! The probes go through the same `dlopen` the real loaders use, with the same
//! sonames, so they honour `LD_LIBRARY_PATH` and NixOS's `nix-ld` exactly the way
//! winit and wgpu will. A probed library is kept open, never closed: the real
//! loader opens it again moments later, and unloading a driver stack in between
//! buys nothing and has hung inside `dlclose` with some vendors' Vulkan layers.
//!
//! `RENZORA_SKIP_PREFLIGHT=1` turns both layers off, for a setup the checks
//! misjudge (an unusual library path, a custom compositor).

use std::path::Path;

use renzora_project_api::RendererBackend;

/// What to tell the user, or `None` when the machine looks fine.
struct Problem {
    summary: &'static str,
    detail: String,
}

/// Check the machine and, if it cannot run this process, explain why and exit.
/// `windowed` is false for a dedicated server, which needs neither a display
/// nor a GPU.
pub fn check_or_exit(is_editor: bool, windowed: bool) {
    if std::env::var_os("RENZORA_SKIP_PREFLIGHT").is_some_and(|v| v == "1") || !windowed {
        return;
    }
    if let Some(problem) = find_problem(is_editor) {
        report(&problem);
        std::process::exit(1);
    }
}

fn find_problem(is_editor: bool) -> Option<Problem> {
    let nixos = is_nixos();
    let hint = |packages: &str| -> String {
        if nixos {
            format!(
                "On NixOS, add these to programs.nix-ld.libraries (or run the \
                 AppImage with appimage-run): {packages}"
            )
        } else {
            format!("Install them with your package manager: {packages}")
        }
    };

    // A window first: without one nothing else matters, and it is the cheaper
    // check to read.
    let wayland_session = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11_session = std::env::var_os("DISPLAY").is_some();
    if !wayland_session && !x11_session {
        return Some(Problem {
            summary: "No display server was found.",
            detail: "Neither WAYLAND_DISPLAY nor DISPLAY is set, so there is no Wayland \
                     or X11 session to open a window on. Start it from a desktop session. \
                     To run a game headless, launch it with --server."
                .into(),
        });
    }

    // winit tries Wayland first and falls back to X11, so one working path is
    // enough. xkbcommon is needed by both: keyboard input goes through it on
    // Wayland, and through its X11 half on X11.
    let wayland_ok = wayland_session && can_open("libwayland-client.so.0");
    let x11_ok = x11_session && can_open("libX11.so.6") && can_open("libXcursor.so.1");
    if !wayland_ok && !x11_ok {
        let mut missing = Vec::new();
        if wayland_session {
            missing.push("libwayland-client.so.0 (wayland / libwayland-client0)");
        }
        if x11_session {
            missing.push("libX11.so.6 and libXcursor.so.1 (libx11 / libxcursor)");
        }
        return Some(Problem {
            summary: "The windowing libraries are missing.",
            detail: format!(
                "A session was found but the library to talk to it was not: {}. {}",
                missing.join(", "),
                hint("wayland, libX11, libXcursor")
            ),
        });
    }
    let xkb = if wayland_ok { "libxkbcommon.so.0" } else { "libxkbcommon-x11.so.0" };
    if !can_open(xkb) {
        return Some(Problem {
            summary: "The keyboard library is missing.",
            detail: format!("{xkb} could not be loaded. {}", hint("libxkbcommon")),
        });
    }

    // The renderer asks for exactly one backend (see `renderer_backend`), so
    // check the one it will ask for.
    let backend = crate::renderer_backend::select(is_editor).backend.resolved();
    if backend == RendererBackend::Gl {
        if !can_open("libEGL.so.1") {
            return Some(Problem {
                summary: "OpenGL (EGL) is not available.",
                detail: format!(
                    "The OpenGL renderer is selected but libEGL.so.1 could not be loaded. {}",
                    hint("mesa (libegl1)")
                ),
            });
        }
    } else if !can_open("libvulkan.so.1") {
        return Some(Problem {
            summary: "Vulkan is not installed.",
            detail: format!(
                "libvulkan.so.1 could not be loaded. Renzora renders with Vulkan, which \
                 needs the Vulkan loader and a driver for your GPU (Mesa for AMD and \
                 Intel, the proprietary driver for NVIDIA). {}",
                hint("vulkan-loader (libvulkan1) and mesa-vulkan-drivers")
            ),
        });
    }
    None
}

/// `dlopen` a soname the way winit and wgpu will, and keep it open on success.
fn can_open(soname: &str) -> bool {
    // SAFETY: these are system libraries (windowing, keyboard, GPU loaders) that
    // the process is about to load anyway; their initialisers are the ones that
    // would run moments later.
    match unsafe { libloading::Library::new(soname) } {
        Ok(lib) => {
            std::mem::forget(lib);
            true
        }
        Err(_) => false,
    }
}

fn is_nixos() -> bool {
    Path::new("/etc/NIXOS").exists()
        || std::fs::read_to_string("/etc/os-release")
            .is_ok_and(|s| s.lines().any(|l| l.trim() == "ID=nixos"))
}

/// Print to stderr, and put up a dialog when there is a desktop to put it on:
/// launched from a file manager or an app menu, stderr goes nowhere anyone
/// reads. Shells out to whichever dialog tool the desktop ships rather than
/// linking a toolkit, because the failures this reports are exactly the ones
/// where a toolkit might not load either.
fn report(problem: &Problem) {
    let title = "Renzora cannot start on this system";
    let text = format!("{}\n\n{}", problem.summary, problem.detail);
    eprintln!("{title}\n\n{text}");

    let has_display =
        std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if !has_display {
        return;
    }
    let attempts: [(&str, Vec<String>); 4] = [
        ("zenity", vec!["--error".into(), format!("--title={title}"), "--no-markup".into(), format!("--text={text}")]),
        ("kdialog", vec!["--title".into(), title.into(), "--error".into(), text.clone()]),
        ("notify-send", vec!["--urgency=critical".into(), title.into(), text.clone()]),
        ("xmessage", vec!["-center".into(), format!("{title}\n\n{text}")]),
    ];
    for (tool, args) in attempts {
        if std::process::Command::new(tool).args(&args).status().is_ok() {
            return;
        }
    }
}
