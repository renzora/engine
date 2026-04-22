//! Subprocess workers: spawn Docker commands and stream their output.
//!
//! Each entry point spawns a std::thread and returns the receiver end of an
//! mpsc channel carrying [`WorkerMsg`]. The drain system (in `lib.rs`) reads
//! messages and updates the authoritative [`DockerBuilderState`].

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::state::{Stage, WorkerMsg};

/// Compute the persistent-container name the Makefile would produce for a CWD.
pub fn container_name(cwd: &Path) -> String {
    let path_str = cwd.to_string_lossy();
    let digest = md5::compute(path_str.as_bytes());
    let hex = format!("{:x}", digest);
    format!("renzora-{}", &hex[..8])
}

/// Parse a "=== Building ... ===" header into (platform, feature).
fn parse_target_header(line: &str) -> Option<(String, String)> {
    let stripped = line
        .strip_prefix("=== Building ")
        .and_then(|s| s.strip_suffix(" ==="))?;

    // "<platform> (<feature>)"
    if let (Some(open), Some(close)) = (stripped.find(" ("), stripped.rfind(')')) {
        if close > open + 2 {
            return Some((
                stripped[..open].to_string(),
                stripped[open + 2..close].to_string(),
            ));
        }
    }

    // Special single-target headers emitted by build-all.sh.
    match stripped {
        "WASM Runtime" => Some(("web-wasm32".into(), "runtime".into())),
        "Android ARM64 Runtime" => Some(("android-arm64".into(), "runtime".into())),
        "Android x86_64 Runtime" => Some(("android-x86".into(), "runtime".into())),
        "iOS ARM64 Runtime" => Some(("ios-arm64".into(), "runtime".into())),
        _ => None,
    }
}

fn forward_stream<R: BufRead>(
    reader: R,
    tx: mpsc::Sender<WorkerMsg>,
    stop_flag: Arc<AtomicBool>,
) {
    for line in reader.lines().map_while(Result::ok) {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        let _ = tx.send(WorkerMsg::Log(line));
    }
}

/// Run `docker build` on the engine-builder image.
pub fn spawn_image_build(
    repo_root: PathBuf,
    stop_flag: Arc<AtomicBool>,
) -> mpsc::Receiver<WorkerMsg> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(WorkerMsg::Stage(Stage::BuildingImage));
        let mut child = match Command::new("docker")
            .current_dir(&repo_root)
            .args([
                "build",
                "-f",
                "docker/engine-builder/Dockerfile",
                "-t",
                "renzora/engine",
                ".",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(WorkerMsg::Log(format!("failed to spawn docker: {}", e)));
                let _ = tx.send(WorkerMsg::Stage(Stage::Failed(e.to_string())));
                let _ = tx.send(WorkerMsg::Finished);
                return;
            }
        };

        drain_child(&mut child, &tx, &stop_flag);
        finalize_child(child, &tx, &stop_flag);
    });
    rx
}

/// Run the full build-all.sh flow inside the persistent container.
pub fn spawn_full_build(
    repo_root: PathBuf,
    stop_flag: Arc<AtomicBool>,
) -> mpsc::Receiver<WorkerMsg> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let name = container_name(&repo_root);

        // Ensure container exists; create if not.
        let _ = tx.send(WorkerMsg::Stage(Stage::StartingContainer));
        let exists = Command::new("docker")
            .args([
                "ps",
                "-a",
                "--filter",
                &format!("name=^{}$", name),
                "--format",
                "{{.Names}}",
            ])
            .output()
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|l| l.trim() == name)
            })
            .unwrap_or(false);

        if !exists {
            let _ = tx.send(WorkerMsg::Log(format!(
                "Creating persistent container '{}'…",
                name
            )));
            let mount = format!("{}:/app/src", repo_root.to_string_lossy());
            let status = Command::new("docker")
                .args([
                    "create",
                    "--name",
                    &name,
                    "-v",
                    &mount,
                    "-w",
                    "/app/src",
                    "renzora/engine",
                    "sleep",
                    "infinity",
                ])
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => {
                    let _ = tx.send(WorkerMsg::Stage(Stage::Failed(format!(
                        "docker create exited {}",
                        s
                    ))));
                    let _ = tx.send(WorkerMsg::Finished);
                    return;
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::Stage(Stage::Failed(e.to_string())));
                    let _ = tx.send(WorkerMsg::Finished);
                    return;
                }
            }
        }

        let _ = Command::new("docker").args(["start", &name]).status();

        let _ = tx.send(WorkerMsg::Stage(Stage::Building));
        let mut child = match Command::new("docker")
            .args([
                "exec",
                &name,
                "/app/src/scripts/build-all.sh",
                "/app/src/dist",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(WorkerMsg::Stage(Stage::Failed(e.to_string())));
                let _ = tx.send(WorkerMsg::Finished);
                return;
            }
        };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let tx_err = tx.clone();
        let stop_err = stop_flag.clone();
        thread::spawn(move || {
            forward_stream(BufReader::new(stderr), tx_err, stop_err);
        });

        let reader = BufReader::new(stdout);
        let mut current: Option<(String, String)> = None;
        let mut build_ok = true;

        for line in reader.lines().map_while(Result::ok) {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            if let Some((platform, feature)) = parse_target_header(&line) {
                if let Some((p, f)) = current.take() {
                    let _ = tx.send(WorkerMsg::TargetDone(p, f));
                }
                let _ = tx.send(WorkerMsg::TargetStart(platform.clone(), feature.clone()));
                current = Some((platform, feature));
            } else if line.starts_with("=== Build complete") {
                if let Some((p, f)) = current.take() {
                    let _ = tx.send(WorkerMsg::TargetDone(p, f));
                }
            } else if line.trim_start().starts_with("Compiling ") {
                if let Some((p, f)) = current.as_ref() {
                    let _ = tx.send(WorkerMsg::TargetCompileTick(p.clone(), f.clone()));
                }
            } else if line.starts_with("error: ") || line.starts_with("error[") {
                build_ok = false;
            }

            let _ = tx.send(WorkerMsg::Log(line));
        }

        match child.wait() {
            Ok(status) if status.success() && build_ok => {
                if let Some((p, f)) = current.take() {
                    let _ = tx.send(WorkerMsg::TargetDone(p, f));
                }
                let _ = tx.send(WorkerMsg::Stage(Stage::Done));
            }
            Ok(status) => {
                if let Some((p, f)) = current.take() {
                    let _ = tx.send(WorkerMsg::TargetFailed(
                        p,
                        f,
                        format!("build-all.sh exited {}", status),
                    ));
                }
                let _ = tx.send(WorkerMsg::Stage(Stage::Failed(format!(
                    "build-all.sh exited {}",
                    status
                ))));
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Stage(Stage::Failed(e.to_string())));
            }
        }
        let _ = tx.send(WorkerMsg::Finished);
    });
    rx
}

/// Run `rm -rf target` inside the persistent container.
pub fn spawn_clean(repo_root: PathBuf) -> mpsc::Receiver<WorkerMsg> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let name = container_name(&repo_root);
        let _ = tx.send(WorkerMsg::Stage(Stage::Cleaning));
        let _ = tx.send(WorkerMsg::Log("Cleaning build cache…".into()));
        let res = Command::new("docker")
            .args([
                "exec",
                &name,
                "bash",
                "-c",
                "du -sh target 2>/dev/null; rm -rf target && echo 'Done.'",
            ])
            .output();
        match res {
            Ok(o) => {
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    let _ = tx.send(WorkerMsg::Log(line.to_string()));
                }
                for line in String::from_utf8_lossy(&o.stderr).lines() {
                    let _ = tx.send(WorkerMsg::Log(line.to_string()));
                }
                let _ = tx.send(WorkerMsg::Stage(Stage::Done));
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Stage(Stage::Failed(e.to_string())));
            }
        }
        let _ = tx.send(WorkerMsg::Finished);
    });
    rx
}

fn drain_child(child: &mut Child, tx: &mpsc::Sender<WorkerMsg>, stop_flag: &Arc<AtomicBool>) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(err) = stderr {
        let tx2 = tx.clone();
        let sf = stop_flag.clone();
        thread::spawn(move || forward_stream(BufReader::new(err), tx2, sf));
    }
    if let Some(out) = stdout {
        forward_stream(BufReader::new(out), tx.clone(), stop_flag.clone());
    }
}

fn finalize_child(mut child: Child, tx: &mpsc::Sender<WorkerMsg>, stop_flag: &Arc<AtomicBool>) {
    if stop_flag.load(Ordering::Relaxed) {
        let _ = child.kill();
    }
    match child.wait() {
        Ok(s) if s.success() => {
            let _ = tx.send(WorkerMsg::Stage(Stage::Done));
        }
        Ok(s) => {
            let _ = tx.send(WorkerMsg::Stage(Stage::Failed(format!("exit {}", s))));
        }
        Err(e) => {
            let _ = tx.send(WorkerMsg::Stage(Stage::Failed(e.to_string())));
        }
    }
    let _ = tx.send(WorkerMsg::Finished);
}
