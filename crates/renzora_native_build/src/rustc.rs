//! The `rustc` command line that compiles a native plugin.
//!
//! One definition, used by both callers. `renzora_plugin_build` runs it for a
//! plugin a user installed into a downloaded editor; `xtask` runs it for the
//! repo's own plugins during staging. They must agree exactly — a flag present
//! in one and not the other produces a plugin that builds for a developer and
//! not for a user, or the reverse, and neither side would be obviously wrong to
//! read.
//!
//! # What this returns, and what it deliberately does not
//!
//! Arguments only. The two callers differ in ways that have nothing to do with
//! the command line: the editor resolves an absolute `rustc` through a pinned
//! rustup toolchain, while xtask uses whatever `rustc` is on `PATH`. Handing
//! back a `Vec<String>` lets each build its own `Command` and keeps the shared
//! part to the thing that actually has to match.
//!
//! Both callers must still set [`ENV_VARS`] — see its note, because the failure
//! when they do not is a bare "proc macro panicked".

use std::path::{Path, PathBuf};

use crate::deps;

/// Everything the command line depends on, resolved to absolute paths.
///
/// The two callers reach these from different places (a serde-parsed manifest in
/// one, a hand-parsed one in the other), so this takes the resolved values
/// rather than a manifest type neither of them would naturally hold.
pub struct Target<'a> {
    /// e.g. `x86_64-pc-windows-msvc`. Only the linker choice reads it.
    pub triple: &'a str,
    /// The plugin's crate name, hyphens already turned into underscores.
    pub crate_name: &'a str,
    /// `--extern bevy=` — the facade **rlib**, which declares
    /// `extern crate bevy_dylib` and routes Bevy's code to the shared image.
    pub extern_bevy: &'a Path,
    /// `--extern renzora=` — the `renzora_dylib` shared library, aliased.
    pub extern_renzora: &'a Path,
    /// `--extern renzora_ember=` — the shared UI image. Absent only for an SDK
    /// staged before panels were reachable from a plugin.
    pub extern_ember: Option<&'a Path>,
    /// `-L dependency=` — crate metadata and linkable dylibs.
    pub dependency: &'a [PathBuf],
    /// `-L native=` — native import libraries from build scripts.
    pub native: &'a [PathBuf],
    /// The plugin's source directory, holding `Cargo.toml` and `src/`.
    pub plugin_dir: &'a Path,
    /// Where the plugin's own artefacts go; the third-party dependency build
    /// gets a `deps/` subdirectory of it.
    pub build_dir: &'a Path,
    /// The crate root, normally `<plugin_dir>/src/lib.rs`.
    pub src: &'a Path,
    /// The library to write.
    pub out: &'a Path,
}

/// Environment both callers must set before running the returned arguments.
///
/// Bevy's derives resolve their own crate paths through `BevyManifest`, which
/// reads `$CARGO_MANIFEST_DIR/Cargo.toml` to decide whether to emit `bevy::…` or
/// `bevy_ecs::…`. Running `rustc` directly means no cargo set it, and the
/// failure is a bare `error: proc macro panicked` naming the macro rather than
/// the missing variable — reached by anything using `#[derive(Component)]` or
/// `bsn!`, which is most plugins.
///
/// Returns `(CARGO_MANIFEST_DIR, CARGO_PKG_NAME)`.
pub fn env_vars(t: &Target) -> [(&'static str, String); 2] {
    [
        ("CARGO_MANIFEST_DIR", t.plugin_dir.display().to_string()),
        ("CARGO_PKG_NAME", t.crate_name.to_string()),
    ]
}

/// Assemble the full argument list.
///
/// Compiles the plugin's third-party dependencies as a side effect when it has
/// any (see [`crate::deps`]); returns their error unchanged when it cannot.
pub fn args(t: &Target) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = Vec::new();
    // A macro rather than a closure: a closure capturing `a` holds the mutable
    // borrow for the whole function, which blocks the `a.push(format!(…))` calls
    // that have to be interleaved with these.
    macro_rules! push {
        ($($s:expr),+ $(,)?) => { $( a.push($s.to_string()); )+ };
    }

    // Without this rustc names the crate after the FILE, so every plugin is
    // called `lib` and every log line it emits is tagged `INFO lib:`,
    // indistinguishable from every other plugin's.
    push!("--crate-name", t.crate_name);
    push!("--edition", "2021");
    push!("--crate-type", "dylib");
    // The plugin must IMPORT Bevy and the contract crate, not embed them.
    // Without this it links its own copies and stops sharing the `World` the
    // whole design exists to share.
    push!("-C", "prefer-dynamic");
    // A bare `rustc` defaults to `opt-level=0`, so without this every plugin and
    // script built here runs UNOPTIMISED — which for a script called once per
    // frame per entity is the expensive half; the size is only the visible one.
    //
    // 2 rather than 3: measured on a small script, 224 KB -> 109 KB with no
    // change in build time, where 3 gained nothing (110 KB) and `s`/`z` were
    // worse (122 KB). It also matches the engine's own `[profile.dist]`.
    push!("-C", "opt-level=2");

    // rust-lld, matching `.cargo/config.toml`. That file configures *cargo*, so
    // a bare rustc silently falls back to MSVC `link.exe`, which fails this link
    // on the exported-symbol count.
    if t.triple.contains("windows-msvc") {
        push!("-C", "linker=rust-lld");
    }

    push!("--extern", format!("bevy={}", t.extern_bevy.display()));
    push!("--extern", format!("renzora={}", t.extern_renzora.display()));
    // Passed unconditionally when the SDK has it — an unused `--extern` costs a
    // plugin nothing (it is not even linked), and requiring plugins to opt in
    // would mean an author's first panel fails to resolve `use renzora_ember::…`
    // with no hint that a switch exists.
    if let Some(ember) = t.extern_ember {
        push!("--extern", format!("renzora_ember={}", ember.display()));
    }

    // Third-party crates the plugin declared, compiled by cargo from a manifest
    // that mentions no Bevy. Empty — and cargo never runs — unless the plugin
    // asked for something.
    let extra = deps::build(t.plugin_dir, t.build_dir)?;
    for (name, rlib) in &extra.externs {
        push!("--extern", format!("{name}={}", rlib.display()));
    }
    if let Some(search) = &extra.search {
        push!("-L", format!("dependency={}", search.display()));
    }

    for d in t.dependency {
        push!("-L", format!("dependency={}", d.display()));
    }
    for n in t.native {
        push!("-L", format!("native={}", n.display()));
    }
    push!("-o", t.out.display(), t.src.display());
    Ok(a)
}

/// Delete the compile by-products that sit beside a built plugin and are not
/// needed to load it.
///
/// `-o foo.dll` does not produce one file. On Windows MSVC the linker also emits
/// an **import library** (`foo.dll.lib`) and, if anything asked for symbols, a
/// **PDB** (`foo.pdb`) that is routinely larger than the plugin itself —
/// measured on the vignette plugin, 492 KB of `.dll` arrived with 744 KB of
/// `.pdb` and 16 KB of `.lib`, so nearly two thirds of what shipped could not be
/// loaded by anything. macOS has the same shape with a `.dSYM` bundle.
///
/// None of it reaches the loader. An import library exists so another crate can
/// *link against* this one at build time, and nothing links against a plugin —
/// the host opens it by symbol name at runtime. So these are build residue that
/// happened to be written into the install directory.
///
/// Done by deletion rather than by passing `-C strip`: the import library is the
/// linker's doing and no `rustc` flag suppresses it, so a flag would solve half
/// the problem and leave the other half looking deliberate. Deleting also stays
/// correct if a future toolchain starts emitting something new — the rule here
/// is "keep the loadable image", not "suppress today's known extras".
///
/// Best-effort throughout. A plugin that built is a plugin that works, and
/// failing a build because a stray file could not be unlinked (a debugger
/// holding the PDB open is the ordinary case) would trade a working install for
/// a tidy directory.
///
/// **Keeping symbols for a crash.** A plugin that segfaults now has no PDB to
/// symbolicate, which is the cost of this. Comment out the call while
/// diagnosing one, rather than shipping the file to everyone who installs the
/// plugin.
/// # What is deliberately NOT removed
///
/// The crate-metadata section is not deleted either — see [`hollow_metadata`],
/// which empties it in place for reasons that took a broken editor to establish.
pub fn prune_byproducts(out: &Path) {
    hollow_metadata(out);
    let Some(dir) = out.parent() else {
        return;
    };
    let Some(file) = out.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    // Every candidate is derived from the artifact's own name, so this can only
    // ever touch siblings the compiler just wrote for THIS plugin — never
    // `stamp.txt`, never another plugin, never the loadable image itself.
    let stem = out.file_stem().and_then(|s| s.to_str()).unwrap_or(file);
    for name in [
        // Windows: `vignette.dll.lib` / `vignette.dll.exp` (linker names them
        // after the full output) and `vignette.pdb` (after the stem).
        format!("{file}.lib"),
        format!("{file}.exp"),
        format!("{stem}.lib"),
        format!("{stem}.exp"),
        format!("{stem}.pdb"),
    ] {
        let path = dir.join(&name);
        if path != out && path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
    }
    // macOS ships debug info as a *directory* beside the dylib.
    let dsym = dir.join(format!("{file}.dSYM"));
    if dsym.is_dir() {
        let _ = std::fs::remove_dir_all(&dsym);
    }
}

/// Rewrite the absolute install names macOS bakes into a freshly linked plugin,
/// so it binds to the shared images the host already has mapped.
///
/// # Why a plugin that links the right dylib still gets the wrong one
///
/// Every other platform identifies a shared library by NAME. An ELF `DT_NEEDED`
/// records a SONAME and `ld.so` reuses whatever is loaded under it; a PE import
/// table records a bare filename and Windows reuses the loaded module. So a
/// plugin linked against the SDK's copy of `bevy_dylib` and a host that loaded
/// its own copy from the install directory converge on one image by accident of
/// how the loader keys its table.
///
/// Mach-O records the dependency's **install name**, which for a cargo-built
/// dylib is its absolute path in `target/`, and dyld keys images by the path a
/// load command resolves to. The SDK stages those dylibs as hardlinks, so they
/// carry the build tree's absolute id — and a plugin linked against them asks
/// for `/…/target/dist/deps/libbevy_dylib-<hash>.dylib` while the host asks for
/// `@rpath/libbevy_dylib-<hash>.dylib` beside the executable. Two paths, two
/// images, two of every process-global static in 118 MB of Bevy.
///
/// That is not a subtle divergence. The task pools, the asset server's handles,
/// the type registry and the contract crate's translation table and Console
/// buffers all exist twice, and the plugin gets the half nobody initialised. It
/// surfaces as `IoTaskPool has not been initialized` the first time a plugin
/// loads an asset in `Plugin::finish` — a panic that accuses Bevy of an
/// ordering bug the host does not have.
///
/// # The rewrite
///
/// Point every non-system dependency at `@rpath/<basename>` and let it resolve
/// against the rpaths already in the process — the executable carries
/// `@loader_path`, which is the directory holding the shared images, so the
/// plugin lands on the identical file the host opened. No rpath is added to the
/// plugin itself on purpose: dyld searches the whole loading chain including the
/// main executable, and a `@loader_path/../..` of our own would encode how deep
/// the artifact happens to sit, which differs between a plugin and a Rust
/// script.
///
/// An absolute path outside `/usr/lib` and `/System` is worth rewriting whether
/// or not it is one of ours: it names a directory on the machine that did the
/// build, so it is already broken for anyone else and `@rpath` cannot be worse.
///
/// Best-effort, like [`prune_byproducts`] — a plugin that built is a plugin that
/// works on the machine that built it, and the tools here ship with Xcode.
#[cfg(target_os = "macos")]
pub fn fixup_install_names(out: &Path) {
    // Fully qualified rather than imported: a `use` at the top of the file would
    // be unused on every other platform, and CI's clippy gate runs on Linux.
    use std::process::Command;

    let Ok(listing) = Command::new("otool").arg("-L").arg(out).output() else {
        return;
    };
    let listing = String::from_utf8_lossy(&listing.stdout);
    let mut changed = false;
    // `skip(1)` drops the header naming the file. The entry that follows it is
    // the library's OWN id, which `-change` cannot touch anyway (that is `-id`)
    // — it is skipped so the loop does not spend a subprocess discovering that
    // and then re-sign over a rewrite that never happened.
    for line in listing.lines().skip(1) {
        let dep = line.trim().split_whitespace().next().unwrap_or_default();
        if !dep.starts_with('/')
            || dep.starts_with("/usr/lib")
            || dep.starts_with("/System")
            || Path::new(dep) == out
        {
            continue;
        }
        let Some(base) = dep.rsplit('/').next() else {
            continue;
        };
        let ok = Command::new("install_name_tool")
            .args(["-change", dep, &format!("@rpath/{base}")])
            .arg(out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        changed |= ok;
    }
    // `install_name_tool` invalidates the ad-hoc signature the linker left, and
    // arm64 macOS refuses to map an image whose signature does not match — so
    // skipping this would trade a wrong-image bug for a plugin that does not
    // load at all. Only when something was actually rewritten.
    if changed {
        let _ = Command::new("codesign").args(["-s", "-", "-f"]).arg(out).status();
    }
}

/// Nothing to do: ELF and PE both resolve a dependency by name, so a plugin and
/// its host share one image without help. See the macOS variant for what that
/// costs when a platform keys images by path instead.
#[cfg(not(target_os = "macos"))]
pub fn fixup_install_names(_out: &Path) {}

/// Empty the `.rustc` crate-metadata section out of a built PE plugin.
///
/// `--crate-type dylib` embeds the crate's full metadata so another Rust crate
/// can `--extern` this library. Nothing does that to a plugin — the host opens it
/// by symbol name at runtime — and it is the **largest thing in the file**: on
/// the vignette plugin, 203 KB of metadata against 183 KB of actual code, 41% of
/// a 492 KB library, which is more than every other saving here combined.
///
/// # Why it is emptied rather than removed
///
/// The obvious move, `llvm-objcopy --remove-section=.rustc`, produces a library
/// that **does not load**: `LoadLibraryExW` fails with error 193,
/// `ERROR_BAD_EXE_FORMAT`. objcopy drops the section's bytes and closes the gap
/// in the *file*, but leaves every remaining section's `VirtualAddress` where it
/// was, so the mapped layout acquires a hole:
///
/// ```text
/// .pdata   ends at VA 0x4A000
/// .reloc  starts at VA 0x7D000     <- 0x33000 mapping nothing
/// ```
///
/// Windows requires the section table to tile the image contiguously and rejects
/// that. Confirmed by patching the three fields that close it — `.reloc`'s
/// `VirtualAddress`, the base-relocation directory RVA, and `SizeOfImage` — after
/// which the identical 294,912 bytes load fine.
///
/// This function takes the other route, which is both simpler and general:
/// **keep the section header, drop only its file bytes.** `SizeOfRawData` and
/// `PointerToRawData` go to zero, which is exactly how an uninitialised (`.bss`)
/// section is expressed in PE, so the loader maps that address range as zero-fill.
/// Nothing moves in virtual address space, so `SizeOfImage`, every data directory
/// and every other section's VA stay untouched — and it does not care where
/// `.rustc` sits or how many sections follow it, where relocating `.reloc` only
/// worked because it happened to be the single section after it.
///
/// It also leaves the image *safer* than deletion did: the `rust_metadata_*`
/// export still points at a mapped, zero-filled page rather than outside the
/// image entirely.
///
/// # Verify any change here by loading the result
///
/// Every static check passes on the broken variant — all exports present, both
/// loader symbols resolvable, imports intact, `SizeOfImage` and all sixteen data
/// directories byte-identical to a working build, section table self-consistent.
/// `llvm-readobj` cannot see the fault. Only `LoadLibraryExW` can.
///
/// Best-effort and non-PE-safe: anything unrecognised leaves the file untouched.
fn hollow_metadata(out: &Path) {
    let Ok(mut b) = std::fs::read(out) else {
        return;
    };
    let rd32 = |b: &[u8], o: usize| -> Option<u32> {
        Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
    };
    let rd16 = |b: &[u8], o: usize| -> Option<u16> {
        Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
    };
    // Not a PE at all (an ELF/Mach-O host, or a fixture) — nothing to do.
    if b.first_chunk::<2>() != Some(b"MZ") {
        return;
    }
    let Some(pe) = rd32(&b, 0x3C).map(|v| v as usize) else { return };
    if b.get(pe..pe + 4) != Some(b"PE\0\0") {
        return;
    }
    let opt = pe + 24;
    let (Some(n_sec), Some(opt_size)) = (rd16(&b, pe + 6), rd16(&b, pe + 20)) else {
        return;
    };
    // A signed image records its certificate by FILE OFFSET, so shrinking the
    // file would invalidate it. Plugins are not signed; refuse rather than
    // silently corrupt one that is.
    if rd32(&b, opt + 112 + 4 * 8).is_none_or(|rva| rva != 0) {
        return;
    }
    let sec = opt + opt_size as usize;
    let hdr = |i: usize| sec + i * 40;
    let Some(target) = (0..n_sec as usize).find(|&i| {
        b.get(hdr(i)..hdr(i) + 8).is_some_and(|n| n.starts_with(b".rustc\0"))
    }) else {
        return;
    };
    let (Some(size), Some(ptr)) = (rd32(&b, hdr(target) + 16), rd32(&b, hdr(target) + 20)) else {
        return;
    };
    let (size, ptr) = (size as usize, ptr as usize);
    if size == 0 || ptr == 0 || ptr + size > b.len() {
        return;
    }
    // Every section whose data sits after the hole slides down by exactly the
    // amount removed. `PointerToRawData == 0` means "no file data", so those are
    // left alone rather than wrapped around.
    for i in 0..n_sec as usize {
        let Some(p) = rd32(&b, hdr(i) + 20) else { return };
        if p as usize > ptr {
            b[hdr(i) + 20..hdr(i) + 24].copy_from_slice(&(p - size as u32).to_le_bytes());
        }
    }
    // The section keeps its VirtualAddress and VirtualSize; only its presence in
    // the file goes away.
    b[hdr(target) + 16..hdr(target) + 20].copy_from_slice(&0u32.to_le_bytes());
    b[hdr(target) + 20..hdr(target) + 24].copy_from_slice(&0u32.to_le_bytes());
    b.drain(ptr..ptr + size);

    // Written through a temp file and renamed, so a crash mid-write cannot leave
    // a truncated library where a working one was.
    let tmp = out.with_extension("hollow.tmp");
    if std::fs::write(&tmp, &b).is_ok() {
        if std::fs::rename(&tmp, out).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

