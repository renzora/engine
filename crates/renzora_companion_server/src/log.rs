//! Log lines, in the shape the editor's Console reads.
//!
//! The editor pipes this process's output into its Console under `Companion`
//! and files each line by its first word (`INFO`, `WARN`, `ERROR`; see
//! `renzora::child_process::forward_to_console`). The engine gets that shape
//! from Bevy's logger; this program has no logger, so it writes the shape
//! itself.
//!
//! Written with `writeln!` and the error dropped, never `println!`: when the
//! editor goes, the pipe closes before this process notices the link closing,
//! and `println!` panics on a closed pipe.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set in the window process, whose stdout carries its commands to the
/// companion; its stderr is the companion's, which the editor also reads.
static TO_STDERR: AtomicBool = AtomicBool::new(false);

pub(crate) fn to_stderr() {
    TO_STDERR.store(true, Ordering::Relaxed);
}

fn line(level: &str, message: &str) {
    let _ = if TO_STDERR.load(Ordering::Relaxed) {
        writeln!(std::io::stderr(), "{level} [companion] {message}")
    } else {
        writeln!(std::io::stdout(), "{level} [companion] {message}")
    };
}

pub(crate) fn info(message: impl AsRef<str>) {
    line("INFO", message.as_ref());
}

pub(crate) fn warn(message: impl AsRef<str>) {
    line("WARN", message.as_ref());
}

pub(crate) fn error(message: impl AsRef<str>) {
    line("ERROR", message.as_ref());
}
