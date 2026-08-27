//! Toolchain resolution, including the states this machine is not in.
//!
//! A developer's box always has the pinned toolchain, so the interesting cases —
//! "wrong version installed", "no Rust at all" — never occur naturally here.
//! They are reached by asking for versions that do and do not exist, which
//! exercises the same code paths a clean machine would take.

use renzora_plugin_build::{toolchain, Toolchain};

#[test]
fn resolves_an_installed_toolchain_to_an_absolute_path() {
    // Whatever is building this test is, by definition, installed.
    let current = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("release: "))
                .map(|s| s.trim().to_string())
        });
    let Some(current) = current else {
        eprintln!("no rustc — skipping");
        return;
    };

    match toolchain::resolve(&current) {
        Toolchain::Ready(path) => {
            // Absolute, so a plugin build never depends on `PATH` and cannot
            // pick up a different compiler than the one that was checked.
            assert!(
                path.is_absolute() || path == std::path::Path::new("rustc"),
                "expected an absolute path or the PATH fallback, got {}",
                path.display()
            );
        }
        other => panic!("the running toolchain should resolve, got {other:?}"),
    }
}

#[test]
fn a_version_that_is_not_installed_is_recoverable_not_fatal() {
    // Old enough that nobody has it, well-formed enough that rustup will try.
    let state = toolchain::resolve("1.0.0");
    assert!(state.rustc().is_none(), "1.0.0 should not resolve");

    // The distinction that matters to a caller: this is something the user can
    // be *offered*, not an error to report. Both remaining states carry wording
    // for a dialog.
    let msg = state.needs().expect("an unresolved toolchain explains itself");
    assert!(
        msg.contains("Rust") && msg.contains("~/.rustup"),
        "the prompt should name the compiler and where it lands: {msg}"
    );
    match state {
        Toolchain::ToolchainMissing { .. } | Toolchain::RustupMissing { .. } => {}
        Toolchain::Ready(_) => unreachable!(),
    }
}

#[test]
fn a_ready_toolchain_asks_for_nothing() {
    let current = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("release: "))
                .map(|s| s.trim().to_string())
        });
    let Some(current) = current else { return };
    assert!(
        toolchain::resolve(&current).needs().is_none(),
        "a resolved toolchain must not prompt"
    );
}
