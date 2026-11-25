//! Compile a directory of Rust scripts against a staged SDK and report on it.
//!
//! The engine ships its example scripts as source in `assets/scripts/`, and a
//! script is compiled on the machine that opens the project, so an API change
//! that breaks one is invisible until someone attaches it and reads the
//! Console. This answers "does every shipped script still compile" without an
//! editor running.
//!
//! Each script goes through [`build_to_path`], the function the editor itself
//! calls on save, so a pass here is a pass there. A build that compiles but has
//! no `renzora::script!` line is reported as a failure too: the editor would
//! load it and say "exports no entry point".
//!
//! Like `check_plugins` (see there for why this is an example and not an xtask
//! subcommand), it compiles and does not run anything.
//!
//! # Usage
//!
//! ```text
//! cargo run -p renzora_plugin_build --profile dist --example check_scripts -- \
//!     --sdk dist/windows-x64/sdk \
//!     --scripts assets/scripts \
//!     [--only <text>] [--jobs <n>] [--logs <dir>] [--strict]
//! ```
//!
//! `--only` keeps the scripts whose file name contains the text, or any of
//! several comma-separated texts. `--logs`
//! writes each failure's whole compiler output to `<dir>/<script>.txt`; the
//! screen shows the first error and where it points. Builds are cached under
//! `RENZORA_SCRIPT_CHECK_DIR` when it is set (a second run then skips every
//! unchanged script, exactly as the editor does), and in a throwaway folder
//! otherwise.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use renzora_plugin_build::script::build_to_path;
use renzora_plugin_build::Sdk;

/// The symbol `renzora::script!` exports and the dispatcher looks up.
const ENTRY: &[u8] = b"renzora_script_update";

struct Outcome {
    name: String,
    ok: bool,
    detail: String,
    log: String,
    secs: f32,
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: check_scripts --sdk <dir> --scripts <dir> [--only <text>] [--jobs <n>] [--logs <dir>] [--strict]";
    let (Some(sdk_dir), Some(scripts_dir)) = (flag(&argv, "--sdk"), flag(&argv, "--scripts")) else {
        eprintln!("{usage}");
        return ExitCode::from(2);
    };
    let strict = argv.iter().any(|a| a == "--strict");
    let only = flag(&argv, "--only");
    let logs = flag(&argv, "--logs").map(PathBuf::from);
    let jobs = flag(&argv, "--jobs")
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4))
        .max(1);

    let sdk = match Sdk::load(&sdk_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not load the SDK at {sdk_dir}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut scripts: Vec<PathBuf> = match std::fs::read_dir(&scripts_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
            .filter(|p| match &only {
                Some(list) => p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| list.split(',').map(str::trim).any(|t| !t.is_empty() && n.contains(t))),
                None => true,
            })
            .collect(),
        Err(e) => {
            eprintln!("could not read {scripts_dir}: {e}");
            return ExitCode::FAILURE;
        }
    };
    scripts.sort();
    if scripts.is_empty() {
        eprintln!("no .rs scripts under {scripts_dir}");
        return ExitCode::FAILURE;
    }

    // `build_to_path` writes into `<project>/.renzora/scripts/<stem>/`, so the
    // "project" here is only a place to put builds. Never the scripts folder
    // itself: that would litter the repository with build output.
    let kept = std::env::var_os("RENZORA_SCRIPT_CHECK_DIR").map(PathBuf::from);
    let project = kept
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join(format!("renzora-script-check-{}", std::process::id())));
    let _ = std::fs::create_dir_all(&project);
    if let Some(dir) = &logs {
        let _ = std::fs::create_dir_all(dir);
    }

    println!("checking {} scripts against {} with {jobs} jobs\n", scripts.len(), sdk.manifest().triple);

    // Each script is its own rustc run in its own build folder, so they share
    // nothing and run side by side.
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(scripts.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(src) = scripts.get(i) else { break };
                let r = check_one(&sdk, &project, src);
                println!(
                    "  {} {:<40} {:>6.1}s  {}",
                    if r.ok { "ok  " } else { "FAIL" },
                    r.name,
                    r.secs,
                    r.detail
                );
                results.lock().unwrap_or_else(|p| p.into_inner()).push(r);
            });
        }
    });
    let mut results = results.into_inner().unwrap_or_else(|p| p.into_inner());
    results.sort_by(|a, b| a.name.cmp(&b.name));

    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&project);
    }

    let failed: Vec<&Outcome> = results.iter().filter(|r| !r.ok).collect();
    println!("\n{} of {} scripts built", results.len() - failed.len(), results.len());
    for r in &failed {
        println!("  FAIL {}: {}", r.name, r.detail);
        if let Some(dir) = &logs {
            let path = dir.join(format!("{}.txt", r.name.trim_end_matches(".rs")));
            if let Err(e) = std::fs::write(&path, &r.log) {
                eprintln!("could not write {}: {e}", path.display());
            }
        }
    }

    if strict && !failed.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn check_one(sdk: &Sdk, project: &Path, src: &Path) -> Outcome {
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
    let started = Instant::now();
    let (ok, detail, log) = match build_to_path(sdk, project, src) {
        Ok(lib) if !exports(&lib, ENTRY) => (
            false,
            "built but exports no entry point (missing `renzora::script!`?)".to_string(),
            String::new(),
        ),
        Ok(_) => (true, String::new(), String::new()),
        Err(e) => (false, summarize(&e), e),
    };
    Outcome { name, ok, detail, log, secs: started.elapsed().as_secs_f32() }
}

/// The first error and the place it points at, which is what a person needs to
/// find the line; the whole diagnostic goes to `--logs`.
fn summarize(log: &str) -> String {
    let mut lines = log.lines().map(str::trim);
    let Some(first) = lines.by_ref().find(|l| l.starts_with("error")) else {
        return log.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    };
    match lines.find(|l| l.starts_with("-->")) {
        Some(at) => {
            let file = at.trim_start_matches("-->").trim();
            let short = file.rsplit(['/', '\\']).next().unwrap_or(file);
            format!("{first} ({short})")
        }
        None => first.to_string(),
    }
}

fn exports(lib: &Path, symbol: &[u8]) -> bool {
    let Ok(bytes) = std::fs::read(lib) else { return false };
    bytes.windows(symbol.len()).any(|w| w == symbol)
}

fn flag(argv: &[String], name: &str) -> Option<String> {
    let i = argv.iter().position(|a| a == name)?;
    argv.get(i + 1).cloned()
}
