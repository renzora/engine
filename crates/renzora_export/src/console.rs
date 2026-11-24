//! Running the lean build's cargo in a terminal window of its own.
//!
//! The dialog can only show cargo's output as lines; a terminal shows it as
//! cargo means it to look, with colours and the progress bar it redraws in
//! place, and keeps a failed build's errors on screen. Cargo has to see a real
//! terminal for any of that, so its output is not piped: the window owns it,
//! and only the exit code comes back.
//!
//! # Getting the exit code back
//!
//! On Windows the console's own process is `cmd.exe`, a child of ours, so
//! waiting on it is waiting on the build. Elsewhere the terminal is started
//! through a launcher (`open`, `gnome-terminal`, ...) that returns at once, and
//! the build is not our child at all. There cargo runs from a small script that
//! records its process id and then its exit code in files beside it, and this
//! side watches those files. The script's process leads its own process group
//! (a terminal starts its command in a session of its own), so Cancel ends
//! cargo and every rustc under it with one signal to the group.

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

/// How a console build ended.
pub(crate) enum Finished {
    /// Cargo exited with this code.
    Code(i32),
    /// Cancel was pressed; the build has been stopped.
    Cancelled,
    /// No terminal could be opened here (no display, or none installed). Says
    /// why, so the caller can tell the user before building in the dialog.
    /// Windows always has its console, so only the other platforms build it.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    NoTerminal(String),
}

/// Run `cmd`, a fully set up cargo command, in a terminal window and wait for
/// it, stopping it if `cancel` is set. `scratch` is a directory this may write
/// its script and status files into.
pub(crate) fn run(cmd: &Command, scratch: &Path, cancel: &AtomicBool) -> Result<Finished, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = scratch;
        windows::run(cmd, cancel)
    }
    #[cfg(unix)]
    {
        unix::run(cmd, scratch, cancel)
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    {
        let _ = (cmd, scratch, cancel);
        Ok(Finished::NoTerminal("This platform has no build console.".into()))
    }
}

/// How often the build is checked on while it runs.
const POLL: Duration = Duration::from_millis(150);

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

    /// Wrapped in `cmd /c "<cargo> || pause"`: a console closes when its
    /// process exits, which would take a failed build's errors with it, so the
    /// window waits for a key on failure and closes by itself on success. The
    /// exit code survives the wrapper.
    pub(super) fn run(cmd: &Command, cancel: &AtomicBool) -> Result<Finished, String> {
        // Quoted for `cmd` when it has a space; nothing a cargo line carries
        // (paths, `--profile`, a comma-separated feature list) needs more.
        let quote = |s: &std::ffi::OsStr| {
            let s = s.to_string_lossy();
            if s.contains(' ') { format!("\"{s}\"") } else { s.into_owned() }
        };
        let mut line = quote(cmd.get_program());
        for arg in cmd.get_args() {
            line.push(' ');
            line.push_str(&quote(arg));
        }
        let mut wrapper = Command::new("cmd.exe");
        wrapper.raw_arg(format!(
            "/c \"{line} || (echo. & echo The build failed. Press any key to close this window. & pause >nul & exit /b 1)\""
        ));
        if let Some(dir) = cmd.get_current_dir() {
            wrapper.current_dir(dir);
        }
        for (key, value) in cmd.get_envs() {
            match value {
                Some(value) => wrapper.env(key, value),
                None => wrapper.env_remove(key),
            };
        }
        wrapper.creation_flags(CREATE_NEW_CONSOLE);
        let mut child = wrapper
            .spawn()
            .map_err(|e| format!("Failed to open the build console: {e}"))?;
        loop {
            if let Some(status) = child.try_wait().map_err(|e| format!("Failed waiting for cargo: {e}"))? {
                return Ok(Finished::Code(status.code().unwrap_or(-1)));
            }
            if cancel.load(Ordering::Relaxed) {
                // The child is `cmd.exe`; killing it alone would leave cargo
                // compiling in the window. End the whole tree.
                let _ = Command::new("taskkill")
                    .args(["/T", "/F", "/PID", &child.id().to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Finished::Cancelled);
            }
            std::thread::sleep(POLL);
        }
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    /// How long the terminal gets to start the script before the console is
    /// given up on and the build runs in the dialog instead.
    const START_TIMEOUT: Duration = Duration::from_secs(30);

    pub(super) fn run(cmd: &Command, scratch: &Path, cancel: &AtomicBool) -> Result<Finished, String> {
        let dir = scratch.join("build-console");
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let pid_file = dir.join("pid");
        let status_file = dir.join("status");
        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&status_file);

        // `.command` is what macOS Terminal runs when handed a file; Linux
        // terminals do not care about the name.
        let script = dir.join(if cfg!(target_os = "macos") { "build.command" } else { "build.sh" });
        std::fs::write(&script, script_text(cmd, &pid_file, &status_file))
            .map_err(|e| format!("write {}: {e}", script.display()))?;
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
        }

        if let Err(why) = launch(&script) {
            return Ok(Finished::NoTerminal(why));
        }

        let started = Instant::now();
        let mut pid: Option<u32> = None;
        loop {
            if let Some(code) = read_status(&status_file) {
                return Ok(Finished::Code(code));
            }
            if pid.is_none() {
                pid = std::fs::read_to_string(&pid_file).ok().and_then(|s| s.trim().parse().ok());
            }
            if cancel.load(Ordering::Relaxed) {
                if let Some(pid) = pid {
                    // The script leads its process group: signal the group, so
                    // cargo and its rustc children go too.
                    let _ = Command::new("kill").args(["-TERM", "--", &format!("-{pid}")]).status();
                    let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
                }
                return Ok(Finished::Cancelled);
            }
            match pid {
                // Gone without writing a status: the window was closed.
                Some(pid) if !alive(pid) => {
                    // The status may have landed between the two checks.
                    if let Some(code) = read_status(&status_file) {
                        return Ok(Finished::Code(code));
                    }
                    return Err("The build console was closed before the build finished.".into());
                }
                Some(_) => {}
                None if started.elapsed() > START_TIMEOUT => {
                    return Ok(Finished::NoTerminal("The terminal did not start the build.".into()));
                }
                None => {}
            }
            std::thread::sleep(POLL);
        }
    }

    /// The exit code, once the script has written it. Written to a temporary
    /// name and renamed, so a half-written file is never read.
    fn read_status(path: &Path) -> Option<i32> {
        std::fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    fn alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// `'value'` for `sh`, with any `'` inside closed, escaped and reopened.
    fn sh_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }

    /// The script the terminal runs.
    ///
    /// The environment is written into it rather than inherited: a terminal
    /// started through `open` or a terminal server never sees ours, and
    /// `CARGO_ENCODED_RUSTFLAGS` is what makes the build static.
    pub(super) fn script_text(cmd: &Command, pid_file: &Path, status_file: &Path) -> String {
        let mut s = String::from("#!/bin/sh\n");
        s.push_str(&format!("echo $$ > {}\n", sh_quote(&pid_file.to_string_lossy())));
        if let Some(dir) = cmd.get_current_dir() {
            s.push_str(&format!("cd {} || exit 1\n", sh_quote(&dir.to_string_lossy())));
        }
        for (key, value) in cmd.get_envs() {
            let key = key.to_string_lossy();
            match value {
                Some(value) => s.push_str(&format!("export {key}={}\n", sh_quote(&value.to_string_lossy()))),
                None => s.push_str(&format!("unset {key}\n")),
            }
        }
        let mut line = sh_quote(&cmd.get_program().to_string_lossy());
        for arg in cmd.get_args() {
            line.push(' ');
            line.push_str(&sh_quote(&arg.to_string_lossy()));
        }
        s.push_str(&line);
        s.push('\n');
        let status = sh_quote(&status_file.to_string_lossy());
        let tmp = sh_quote(&format!("{}.tmp", status_file.to_string_lossy()));
        s.push_str("code=$?\n");
        s.push_str(&format!("echo $code > {tmp} && mv {tmp} {status}\n"));
        s.push_str(
            "if [ $code -ne 0 ]; then\n  echo\n  echo 'The build failed. Press Enter to close this window.'\n  read _\nfi\nexit $code\n",
        );
        s
    }

    /// Open a terminal window running `script`.
    fn launch(script: &Path) -> Result<(), String> {
        let spawn = |program: &str, args: &[&str]| {
            Command::new(program)
                .args(args)
                .arg(script)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|_| ())
        };

        if cfg!(target_os = "macos") {
            return spawn("open", &["-a", "Terminal"]).map_err(|e| format!("Could not open Terminal: {e}."));
        }

        // A terminal needs a desktop to open on. Without one (a headless
        // machine, an SSH session) there is nothing to show the console in.
        let has_display = std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
        if !has_display {
            return Err("There is no desktop to open a build console on.".into());
        }
        // The user's choice first, then the distribution's default, then the
        // common terminals, each with the flag that makes it run a command.
        let mut candidates: Vec<(String, Vec<&str>)> = Vec::new();
        if let Ok(own) = std::env::var("TERMINAL") {
            if !own.trim().is_empty() {
                candidates.push((own, vec!["-e"]));
            }
        }
        for (program, args) in [
            ("x-terminal-emulator", vec!["-e"]),
            ("gnome-terminal", vec!["--"]),
            ("konsole", vec!["-e"]),
            ("xfce4-terminal", vec!["-x"]),
            ("kitty", vec![]),
            ("alacritty", vec!["-e"]),
            ("wezterm", vec!["start", "--"]),
            ("foot", vec![]),
            ("xterm", vec!["-e"]),
        ] {
            candidates.push((program.to_string(), args));
        }
        for (program, args) in &candidates {
            if on_path(program).is_some() && spawn(program, args).is_ok() {
                return Ok(());
            }
        }
        Err("No terminal emulator was found.".into())
    }

    fn on_path(program: &str) -> Option<PathBuf> {
        if program.contains('/') {
            let p = PathBuf::from(program);
            return p.is_file().then_some(p);
        }
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|dir| dir.join(program))
            .find(|p| p.is_file())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The script carries the environment, quotes what needs quoting, and
        /// records the exit code where the exporter looks for it.
        #[test]
        fn the_script_carries_the_build_and_reports_its_exit_code() {
            let mut cmd = Command::new("/opt/rust/bin/cargo");
            cmd.current_dir("/tmp/it's here")
                .env("CARGO_ENCODED_RUSTFLAGS", "")
                .args(["build", "--features", "runtime,static_scripts"]);
            let text = script_text(&cmd, Path::new("/s/pid"), Path::new("/s/status"));
            assert!(text.contains("cd '/tmp/it'\\''s here' || exit 1"));
            assert!(text.contains("export CARGO_ENCODED_RUSTFLAGS=''"));
            assert!(text.contains("'/opt/rust/bin/cargo' 'build' '--features' 'runtime,static_scripts'"));
            assert!(text.contains("echo $$ > '/s/pid'"));
            assert!(text.contains("mv '/s/status.tmp' '/s/status'"));
        }
    }
}
