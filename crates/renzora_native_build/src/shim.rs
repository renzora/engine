//! The `RUSTC_WRAPPER` a plugin's third-party dependency build runs under.
//!
//! This file is two things at once. `deps.rs` embeds it with `include_str!` and
//! compiles it with the pinned `rustc` into a small executable, which cargo then
//! runs in place of every `rustc` call while it builds a plugin's dependencies.
//! It is also a module of this crate under `cfg(test)`, so the decisions below are
//! unit-tested without spawning anything. Hence `std` only and no `crate::` paths:
//! the executable is compiled from this one file and has nothing else.
//!
//! # What it is for
//!
//! A plugin may depend on a crate that is itself built on Bevy: avian, an
//! ecosystem crate, anything that names `bevy_ecs` or `glam`. Built by cargo the
//! ordinary way, that crate would get a Bevy (and a `glam`) of its own, which
//! are different types from the engine's, and the plugin would either fail to
//! compile on a type mismatch or load and read the engine's `World` through the
//! wrong layouts.
//!
//! The engine's shared images already contain those crates, and the SDK holds
//! their metadata. So when cargo asks for one that the SDK has, at the same
//! version and with the features it needs, this does not compile it at all. It
//! records where the SDK's copy is, and every later compile that names the crate
//! with `--extern` is pointed at that copy instead. Everything else compiles
//! normally, against the engine's Bevy, and its code is linked through the
//! shared images exactly as a plugin's own code is.
//!
//! # The table
//!
//! `shared.tsv` in the SDK, written by `xtask`'s staging: one crate per line,
//! `package <TAB> version <TAB> crate <TAB> proc_macro <TAB> features <TAB> path`,
//! features comma-separated and the path relative to the SDK root. Only crates
//! inside a shared image are listed: a crate that the engine linked statically
//! into an executable has metadata in the SDK but no code any plugin can reach,
//! and substituting it would fail the final link with `E0461`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Environment the wrapper reads, set by `deps.rs` on the cargo it spawns.
pub const ENV_TABLE: &str = "RENZORA_SHIM_TABLE";
/// The directory the substitutions are recorded in, one file per skipped unit.
pub const ENV_MAP: &str = "RENZORA_SHIM_MAP";
/// The SDK's `-L dependency=` directories, joined like `PATH`.
pub const ENV_SEARCH: &str = "RENZORA_SHIM_SEARCH";

/// One crate the SDK provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedCrate {
    pub package: String,
    pub version: String,
    pub crate_name: String,
    pub proc_macro: bool,
    pub features: BTreeSet<String>,
    /// Absolute, once the table has been read against the SDK root.
    pub path: PathBuf,
}

/// Parse `shared.tsv`. Lines that do not have all six fields are ignored rather
/// than refused: a malformed line can only mean a crate is compiled instead of
/// substituted, which is the behaviour before this file existed.
pub fn parse_table(text: &str, sdk_root: &Path) -> Vec<SharedCrate> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            continue;
        }
        out.push(SharedCrate {
            package: f[0].to_string(),
            version: f[1].to_string(),
            crate_name: f[2].to_string(),
            proc_macro: f[3] == "1",
            features: f[4].split(',').filter(|s| !s.is_empty()).map(str::to_string).collect(),
            path: sdk_root.join(f[5]),
        });
    }
    out
}

/// Render one table line. The inverse of [`parse_table`], used by `xtask`.
pub fn table_line(c: &SharedCrate, relative_path: &str) -> String {
    let features: Vec<&str> = c.features.iter().map(String::as_str).collect();
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        c.package,
        c.version,
        c.crate_name,
        if c.proc_macro { "1" } else { "0" },
        features.join(","),
        relative_path
    )
}

/// What to do with one `rustc` invocation.
#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    /// Run the real compiler with these arguments.
    Run(Vec<String>),
    /// Do not compile: record `key -> path` and write an empty dep-info file.
    Skip { key: String, crate_name: String, path: PathBuf, dep_info: Option<PathBuf> },
    /// The SDK has the crate, but not with every feature asked for.
    Refuse(String),
}

/// The value following `flag`, whether written `--flag value` or `--flag=value`.
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let eq = format!("{flag}=");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == flag {
            return it.next().map(String::as_str);
        }
        if let Some(v) = a.strip_prefix(&eq) {
            return Some(v);
        }
    }
    None
}

/// `-C extra-filename=-<hash>`, cargo's per-unit suffix, or empty.
fn extra_filename(args: &[String]) -> String {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let value = if a == "-C" { it.next().map(String::as_str) } else { a.strip_prefix("-C") };
        if let Some(v) = value.and_then(|v| v.strip_prefix("extra-filename=")) {
            return v.to_string();
        }
    }
    String::new()
}

/// Every `--cfg feature="x"` on the command line.
fn features(args: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let value = if a == "--cfg" { it.next().map(String::as_str) } else { a.strip_prefix("--cfg=") };
        if let Some(v) = value.and_then(|v| v.strip_prefix("feature=")) {
            out.insert(v.trim_matches('"').to_string());
        }
    }
    out
}

/// The key a unit is recorded under: its output file's stem, which is the crate
/// name plus cargo's `extra-filename`. A later `--extern name=<path>` is matched
/// by the stem of `<path>`, with the `lib` prefix rlibs carry taken off.
pub fn unit_key(crate_name: &str, extra: &str) -> String {
    format!("{crate_name}{extra}")
}

/// Candidate keys for an `--extern` path, most specific first.
pub fn keys_for_extern(path: &str) -> Vec<String> {
    let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    let mut keys = vec![stem.clone()];
    if let Some(bare) = stem.strip_prefix("lib") {
        keys.push(bare.to_string());
    }
    keys
}

/// Decide what to do with one compiler invocation.
///
/// `package` is cargo's `CARGO_PKG_NAME`/`CARGO_PKG_VERSION` for the unit, absent
/// when cargo is only asking the compiler about itself (`-vV`, `--print`).
/// `recorded` looks up a key a previous skip wrote.
pub fn plan(
    args: &[String],
    package: Option<(&str, &str)>,
    table: &[SharedCrate],
    recorded: &dyn Fn(&str) -> Option<PathBuf>,
    search: &[PathBuf],
) -> Plan {
    let Some((pkg, version)) = package else {
        return Plan::Run(args.to_vec());
    };
    // Cargo's query of the compiler (`--print=cfg` on a dummy `___` crate read
    // from stdin) is not a unit. It can still arrive with a package in the
    // environment, inherited from whatever started cargo, so it is recognised by
    // its own arguments rather than by the environment.
    if args.iter().any(|a| a.starts_with("--print")) {
        return Plan::Run(args.to_vec());
    }
    let crate_name = flag_value(args, "--crate-name").unwrap_or_default().to_string();
    let shared = table.iter().find(|c| c.package == pkg && c.version == version);

    if let Some(shared) = shared {
        // Only units built FOR the plugin are substituted. A unit cargo builds for
        // the compiler itself (a proc macro's own dependencies, a build script's)
        // is linked statically into that host program and needs real code, which
        // the SDK's metadata does not carry: `E0463`/"required to be available in
        // rlib format". `deps.rs` passes an explicit `--target` so cargo marks
        // the difference: only plugin-side units carry the flag. A proc macro is
        // the exception, because the SDK keeps those whole.
        let for_plugin = shared.proc_macro || flag_value(args, "--target").is_some();
        if crate_name == shared.crate_name && for_plugin {
            let wanted = features(args);
            let missing: Vec<&String> = wanted.difference(&shared.features).collect();
            if !missing.is_empty() {
                let list: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
                return Plan::Refuse(format!(
                    "this plugin needs `{pkg} {version}` with the feature(s) {}, and the engine's \
                     build of it does not have them. The engine shares its copy of `{pkg}` with \
                     every plugin, so a plugin cannot turn a feature on that the engine left off. \
                     Ask for a plugin version that does without them, or enable them in the \
                     engine's own dependencies.",
                    list.join(", ")
                ));
            }
            let extra = extra_filename(args);
            let dep_info = flag_value(args, "--out-dir").map(|dir| {
                Path::new(dir).join(format!("{crate_name}{extra}.d"))
            });
            return Plan::Skip {
                key: unit_key(&crate_name, &extra),
                crate_name,
                path: shared.path.clone(),
                dep_info,
            };
        }
    }

    // A crate that is compiled: point every `--extern` that names a skipped unit
    // at the SDK's copy, and let rustc find the SDK's other crates (everything
    // those copies depend on) by their exact hashes in its directories.
    let mut out = Vec::with_capacity(args.len() + search.len() * 2);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--extern" {
            out.push(a.clone());
            if let Some(v) = it.next() {
                out.push(rewrite_extern(v, recorded));
            }
        } else if let Some(v) = a.strip_prefix("--extern=") {
            out.push(format!("--extern={}", rewrite_extern(v, recorded)));
        } else {
            out.push(a.clone());
        }
    }
    for dir in search {
        out.push("-L".to_string());
        out.push(format!("dependency={}", dir.display()));
    }
    Plan::Run(out)
}

/// `[opts:]name=path`, with `path` swapped for the SDK's copy when one was recorded.
fn rewrite_extern(value: &str, recorded: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let Some((name, path)) = value.split_once('=') else {
        return value.to_string();
    };
    for key in keys_for_extern(path) {
        if let Some(sdk) = recorded(&key) {
            return format!("{name}={}", sdk.display());
        }
    }
    value.to_string()
}

/// The recorded SDK path for `key`, if a skip wrote one.
pub fn read_record(map: &Path, key: &str) -> Option<PathBuf> {
    std::fs::read_to_string(map.join(key)).ok().map(|s| PathBuf::from(s.trim()))
}

/// The second key a skip is recorded under: the bare crate name.
///
/// Cargo reports a unit's files only when they exist, so a skipped one arrives
/// in its JSON with no filenames, and there is no hash to look it up by. For the
/// plugin's DIRECT dependencies the crate name is enough: those are built once,
/// for the plugin.
pub fn crate_key(crate_name: &str) -> String {
    format!("crate-{crate_name}")
}

/// The executable's entry point. Never returns.
#[allow(dead_code)]
pub fn run() -> ! {
    let mut argv = std::env::args().skip(1);
    let Some(rustc) = argv.next() else {
        eprintln!("renzora rustc shim: no compiler given");
        std::process::exit(2);
    };
    let args: Vec<String> = argv.collect();

    let table_path = std::env::var_os(ENV_TABLE).map(PathBuf::from);
    let sdk_root = table_path.as_deref().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
    let table = table_path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_table(&t, &sdk_root))
        .unwrap_or_default();
    let map = std::env::var_os(ENV_MAP).map(PathBuf::from).unwrap_or_default();
    // `split_paths` of an empty variable yields one empty path, and rustc
    // refuses an empty `-L`.
    let search: Vec<PathBuf> = std::env::var_os(ENV_SEARCH)
        .map(|s| std::env::split_paths(&s).filter(|p| !p.as_os_str().is_empty()).collect())
        .unwrap_or_default();
    let name = std::env::var("CARGO_PKG_NAME").ok();
    let version = std::env::var("CARGO_PKG_VERSION").ok();
    let package = name.as_deref().zip(version.as_deref());

    let recorded = |key: &str| read_record(&map, key);
    match plan(&args, package, &table, &recorded, &search) {
        Plan::Run(args) => {
            let status = std::process::Command::new(&rustc).args(&args).status();
            match status {
                Ok(s) => std::process::exit(s.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("renzora rustc shim: could not run {rustc}: {e}");
                    std::process::exit(1);
                }
            }
        }
        Plan::Skip { key, crate_name, path, dep_info } => {
            // Recorded before exiting, because cargo starts the units that depend
            // on this one as soon as it does, and they look the key up.
            let _ = std::fs::create_dir_all(&map);
            for record in [key, crate_key(&crate_name)] {
                if let Err(e) = std::fs::write(map.join(&record), path.display().to_string()) {
                    eprintln!("renzora rustc shim: could not record {record}: {e}");
                    std::process::exit(1);
                }
            }
            // Cargo reads the compiler's dep-info file to fingerprint the unit,
            // and fails the build if there is none. An empty one lists no sources.
            if let Some(d) = dep_info {
                let _ = std::fs::write(d, "");
            }
            std::process::exit(0);
        }
        Plan::Refuse(message) => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn glam(features: &[&str]) -> SharedCrate {
        SharedCrate {
            package: "glam".into(),
            version: "0.33.12".into(),
            crate_name: "glam".into(),
            proc_macro: false,
            features: features.iter().map(|s| s.to_string()).collect(),
            path: PathBuf::from("/sdk/deps/libglam-aaaa.rmeta"),
        }
    }

    fn none(_: &str) -> Option<PathBuf> {
        None
    }

    /// The line `xtask` writes is the line this reads back, path resolved
    /// against the SDK root.
    #[test]
    fn a_table_line_round_trips() {
        let c = glam(&["std", "approx"]);
        let line = table_line(&c, "deps/libglam-aaaa.rmeta");
        let back = parse_table(&format!("# header\n{line}\n"), Path::new("/sdk"));
        assert_eq!(back, vec![c]);
    }

    #[test]
    fn a_short_line_is_ignored() {
        assert!(parse_table("glam\t0.33.12\tglam\n", Path::new("/sdk")).is_empty());
    }

    /// A plugin-side compile of a crate the SDK has, with features it has: not
    /// compiled, recorded under its output stem.
    #[test]
    fn a_shared_crate_built_for_the_plugin_is_skipped() {
        let a = args(&[
            "--crate-name", "glam", "--edition=2021", "src/lib.rs", "--crate-type", "lib",
            "--cfg", "feature=\"std\"", "-C", "extra-filename=-1234", "--out-dir", "/t/deps",
            "--target", "x86_64-pc-windows-msvc",
        ]);
        let plan = plan(&a, Some(("glam", "0.33.12")), &[glam(&["std", "approx"])], &none, &[]);
        assert_eq!(
            plan,
            Plan::Skip {
                key: "glam-1234".into(),
                crate_name: "glam".into(),
                path: PathBuf::from("/sdk/deps/libglam-aaaa.rmeta"),
                dep_info: Some(Path::new("/t/deps").join("glam-1234.d")),
            }
        );
    }

    /// The same crate built for the compiler (no `--target`) needs real code.
    #[test]
    fn a_shared_crate_built_for_the_host_is_compiled() {
        let a = args(&["--crate-name", "glam", "src/lib.rs", "-C", "extra-filename=-1234"]);
        let plan = plan(&a, Some(("glam", "0.33.12")), &[glam(&["std"])], &none, &[]);
        assert!(matches!(plan, Plan::Run(_)));
    }

    /// Proc macros are kept whole in the SDK, so they are skipped either way.
    #[test]
    fn a_shared_proc_macro_is_skipped_without_a_target() {
        let mut derive = glam(&[]);
        derive.package = "num-derive".into();
        derive.crate_name = "num_derive".into();
        derive.proc_macro = true;
        let a = args(&["--crate-name", "num_derive", "--crate-type", "proc-macro", "-Cextra-filename=-9"]);
        let plan = plan(&a, Some(("num-derive", "0.33.12")), &[derive], &none, &[]);
        assert!(matches!(plan, Plan::Skip { ref key, .. } if key == "num_derive-9"));
    }

    /// Another version of the crate is a different crate, and is compiled.
    #[test]
    fn a_different_version_is_compiled() {
        let a = args(&["--crate-name", "glam", "--target", "t"]);
        let plan = plan(&a, Some(("glam", "0.31.1")), &[glam(&["std"])], &none, &[]);
        assert!(matches!(plan, Plan::Run(_)));
    }

    /// A feature the engine's copy lacks cannot be turned on from a plugin.
    #[test]
    fn a_missing_feature_is_refused_by_name() {
        let a = args(&["--crate-name", "glam", "--cfg", "feature=\"mint\"", "--target", "t"]);
        let Plan::Refuse(message) = plan(&a, Some(("glam", "0.33.12")), &[glam(&["std"])], &none, &[]) else {
            panic!("expected a refusal");
        };
        assert!(message.contains("mint"), "{message}");
    }

    /// A compiled crate that names a skipped one is pointed at the SDK's copy,
    /// and can find the SDK's other crates.
    #[test]
    fn an_extern_on_a_skipped_unit_is_rewritten() {
        let recorded = |key: &str| (key == "glam-1234").then(|| PathBuf::from("/sdk/deps/libglam-aaaa.rmeta"));
        let a = args(&[
            "--crate-name", "parry3d", "--extern", "glam=/t/deps/libglam-1234.rmeta",
            "--extern", "nalgebra=/t/deps/libnalgebra-5.rmeta",
        ]);
        let Plan::Run(out) = plan(&a, Some(("parry3d", "0.31.0")), &[], &recorded, &[PathBuf::from("/sdk/deps")])
        else {
            panic!("expected a run");
        };
        assert!(out.contains(&format!("glam={}", Path::new("/sdk/deps/libglam-aaaa.rmeta").display())));
        assert!(out.contains(&"nalgebra=/t/deps/libnalgebra-5.rmeta".to_string()));
        assert!(out.ends_with(&args(&["-L", &format!("dependency={}", Path::new("/sdk/deps").display())])));
    }

    /// A proc macro's output has no `lib` prefix on Windows and one elsewhere;
    /// both name the same unit.
    #[test]
    fn extern_keys_cover_both_naming_schemes() {
        assert_eq!(keys_for_extern("/t/libglam-1234.rmeta"), vec!["libglam-1234", "glam-1234"]);
        assert_eq!(keys_for_extern("/t/num_derive-9.dll"), vec!["num_derive-9"]);
        // A crate whose own name starts with `lib`.
        assert!(keys_for_extern("/t/liblibm-3.rlib").contains(&"libm-3".to_string()));
    }

    /// Cargo asking the compiler about itself is passed straight through.
    #[test]
    fn a_query_without_a_package_runs_unchanged() {
        let a = args(&["-vV"]);
        assert_eq!(plan(&a, None, &[glam(&[])], &none, &[PathBuf::from("/sdk")]), Plan::Run(a));
    }

    /// The same query with a package inherited into the environment: still not a
    /// unit, and must not grow `-L` arguments.
    #[test]
    fn a_print_query_with_an_inherited_package_runs_unchanged() {
        let a = args(&["-", "--crate-name", "___", "--print=cfg", "--crate-type", "bin"]);
        let plan = plan(&a, Some(("glam", "0.33.12")), &[glam(&[])], &none, &[PathBuf::from("/sdk")]);
        assert_eq!(plan, Plan::Run(a));
    }
}
