//! Child processes the editor starts: tying them to the editor's lifetime and
//! piping their output into the Console.
//!
//! Shared by everything the editor launches as a copy of itself (Play's game,
//! the companion), so each one dies with the editor and is heard from the same
//! way. It lived in `renzora_viewport` while Play was the only child.

use crate::core::console_log::{console_log, LogLevel};

/// Put `child` in the editor's job object, so it ends when the editor does.
///
/// A normal quit kills children on its own; a crashed editor, or one ended from
/// Task Manager, never gets the chance. A child left behind holds the executable
/// open, which fails the next build at the staging step with "being used by
/// another process" and nothing on screen to close. Best effort: a child that
/// could not be added still runs, and a normal quit still ends it.
///
/// `what` names the child in the warning, e.g. `"the game"`.
#[cfg(target_os = "windows")]
pub fn tie_to_editor(child: &std::process::Child, what: &str) {
    job::tie(child, what);
}

/// Elsewhere there is no job object; a normal quit is what ends children, and
/// the companion also exits when its socket to the editor closes.
#[cfg(not(target_os = "windows"))]
pub fn tie_to_editor(_child: &std::process::Child, _what: &str) {}

/// The job object: closing a job ends its processes when the job's last handle
/// closes, and the only handle is this process's. Windows closes it when the
/// editor ends, for any reason, and takes every child with it.
#[cfg(target_os = "windows")]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

    #[repr(C)]
    #[derive(Default)]
    struct BasicLimits {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        counts: [u64; 6],
    }

    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimits {
        basic: BasicLimits,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const core::ffi::c_void, name: *const u16) -> isize;
        fn SetInformationJobObject(
            job: isize,
            class: i32,
            info: *const core::ffi::c_void,
            len: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: isize, process: isize) -> i32;
    }

    /// The one job every child joins, created on first use and never closed:
    /// closing it is what would end the children, and that is the process
    /// exit's job.
    fn editor_job() -> Option<isize> {
        static JOB: OnceLock<Option<isize>> = OnceLock::new();
        *JOB.get_or_init(|| {
            // SAFETY: null attributes and name are documented as valid, and the
            // limits struct is a correctly laid out JOBOBJECT_EXTENDED_LIMIT_INFORMATION
            // that outlives the call.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job == 0 {
                    return None;
                }
                let mut limits = ExtendedLimits::default();
                limits.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    (&limits as *const ExtendedLimits).cast(),
                    std::mem::size_of::<ExtendedLimits>() as u32,
                );
                (ok != 0).then_some(job)
            }
        })
    }

    pub(super) fn tie(child: &std::process::Child, what: &str) {
        let Some(job) = editor_job() else {
            bevy::log::warn!(
                "could not create a job object; a crashed editor may leave {what} running"
            );
            return;
        };
        // SAFETY: the handle is the live child's own, borrowed for the call.
        let ok = unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as isize) };
        if ok == 0 {
            bevy::log::warn!(
                "could not tie {what} to the editor; a crashed editor may leave it running"
            );
        }
    }

    #[cfg(test)]
    mod tests {
        /// The struct is passed by pointer and size, so a wrong layout is a
        /// silent misread by the kernel rather than a compile error. 144 is
        /// the 64-bit size; nothing ships for 32-bit Windows.
        #[cfg(target_pointer_width = "64")]
        #[test]
        fn the_limits_struct_matches_the_win32_layout() {
            assert_eq!(std::mem::size_of::<super::ExtendedLimits>(), 144);
        }
    }
}

/// Read `stream` line by line on its own thread, pushing each line into the
/// editor Console under `category`. Detached: the read ends when the child
/// closes the pipe, so the thread retires itself with the process it was
/// reading.
///
/// A piped child *must* be drained: a pipe nobody reads fills up and then
/// blocks the child on its next log line. On Windows the child is a
/// GUI-subsystem binary launched from another GUI process, so without this
/// everything it logged went nowhere.
pub fn forward_to_console(stream: impl std::io::Read + Send + 'static, category: &'static str) {
    use std::io::{BufRead, BufReader};

    std::thread::spawn(move || {
        // Lossy rather than strict UTF-8: a child that dies mid-line, or logs a
        // path the OS handed it in some other encoding, must not silence the
        // rest of the stream.
        for line in BufReader::new(stream).split(b'\n') {
            let Ok(bytes) = line else { break };
            let line = String::from_utf8_lossy(&bytes);
            let line = strip_ansi(line.trim_end_matches('\r'));
            if line.trim().is_empty() {
                continue;
            }
            let (level, message) = classify_line(&line);
            console_log(level, category, message);
        }
    });
}

/// Drop SGR colour escapes. `tracing_subscriber` colours its output whether or
/// not the sink is a terminal, and a pipe is never one, so without this every
/// forwarded line arrives wrapped in `\x1b[…m` noise.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // `ESC [ … <final byte in @..~>`. Anything else after ESC is a short
        // escape we simply drop along with its one following character.
        if chars.next() != Some('[') {
            continue;
        }
        for c in chars.by_ref() {
            if ('@'..='~').contains(&c) {
                break;
            }
        }
    }
    out
}

/// Split a forwarded log line into a console level and the text to show.
///
/// The child's lines look like `<timestamp>  INFO  <target>: <message>`. Both
/// the timestamp and the level are dropped: the console renders its own level
/// colour and the timestamp is noise next to the editor's own lines.
fn classify_line(line: &str) -> (LogLevel, String) {
    let mut rest = line.trim_start();
    // An RFC3339 timestamp is the first token and always ends in `Z`.
    if let Some((first, tail)) = rest.split_once(char::is_whitespace) {
        if first.ends_with('Z') && first.contains(':') {
            rest = tail.trim_start();
        }
    }

    for (token, level) in [
        ("ERROR", LogLevel::Error),
        ("WARN", LogLevel::Warning),
        ("INFO", LogLevel::Info),
        ("DEBUG", LogLevel::Info),
        ("TRACE", LogLevel::Info),
    ] {
        if let Some(tail) = rest.strip_prefix(token) {
            if tail.starts_with(char::is_whitespace) {
                return (level, tail.trim_start().to_string());
            }
        }
    }

    // No level token: wgpu/naga chatter, or a panic. A panic is the one thing
    // here nobody can afford to have filed as routine information.
    let level = if line.contains("panicked at") || line.starts_with("thread '") {
        LogLevel::Error
    } else {
        LogLevel::Info
    };
    (level, rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_escapes_are_stripped() {
        let line = "\u{1b}[2m2026-08-10T13:24:25Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m ready";
        assert_eq!(strip_ansi(line), "2026-08-10T13:24:25Z  INFO ready");
    }

    #[test]
    fn the_timestamp_and_level_come_off_the_front() {
        let (level, message) =
            classify_line("2026-08-10T13:24:25.159700Z  WARN renzora_audio: no ears");
        assert_eq!(level, LogLevel::Warning);
        assert_eq!(message, "renzora_audio: no ears");
    }

    /// A line the child's own logger didn't write (wgpu chatter, a panic) still
    /// has to arrive, and a panic must not arrive as routine info.
    #[test]
    fn an_unprefixed_line_survives_and_a_panic_is_an_error() {
        let (level, message) = classify_line("wgpu: picked adapter");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(message, "wgpu: picked adapter");

        let (level, _) = classify_line("thread 'main' panicked at src/main.rs:12:5:");
        assert_eq!(level, LogLevel::Error);
    }

    /// `INFORMATIONAL` is not the `INFO` token, and a word boundary is the only
    /// thing that tells them apart.
    #[test]
    fn a_level_token_needs_a_word_boundary() {
        let (_, message) = classify_line("INFOrmational text");
        assert_eq!(message, "INFOrmational text");
    }
}
