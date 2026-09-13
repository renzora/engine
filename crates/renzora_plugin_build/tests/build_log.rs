//! A failed build leaves something behind to read.
//!
//! The unit tests in `mod log` cover the format and the rotation. This covers
//! the wiring, which is the part that actually breaks: that `compile_with`
//! reaches the log at all, for a failure that never gets as far as running the
//! compiler. Its own test binary because it sets `RENZORA_BUILD_LOG`, and
//! environment variables are process-wide.

use std::path::PathBuf;

use renzora_plugin_build::Sdk;

/// A manifest pinning a toolchain that cannot exist, so the build fails in
/// `compile_with` before rustc is ever spawned. No staged SDK needed, and no
/// compiler — the point is the logging, not the compiling.
const MANIFEST: &str = r#"{
    "triple": "x86_64-unknown-none",
    "rustc": "0.0.0-not-a-real-toolchain",
    "lib_ext": "dll",
    "extern": { "bevy": "bevy.rlib", "renzora": "renzora.dll" },
    "link_search": { "dependency": [], "native": [] }
}"#;

#[test]
fn a_failed_build_is_written_to_the_log() {
    let tmp = std::env::temp_dir().join(format!("renzora-build-log-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    let sdk_root = tmp.join("sdk");
    std::fs::create_dir_all(&sdk_root).unwrap();
    std::fs::write(sdk_root.join("manifest.json"), MANIFEST).unwrap();

    let plugin = tmp.join("plugins").join("log-me");
    std::fs::create_dir_all(plugin.join("src")).unwrap();
    std::fs::write(plugin.join("src").join("lib.rs"), "// never compiled\n").unwrap();

    let log: PathBuf = tmp.join("build.log");
    std::env::set_var("RENZORA_BUILD_LOG", &log);

    let sdk = Sdk::load(&sdk_root).expect("a manifest is all `load` needs");
    let out = plugin.join("build").join("log_me.dll");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let err = sdk.compile(&plugin, &out).expect_err("no such toolchain exists");

    let text = std::fs::read_to_string(&log).expect("the failure was logged");
    assert!(text.contains("log_me"), "names the plugin:\n{text}");
    assert!(text.contains("toolchain"), "says what kind of failure:\n{text}");
    assert!(
        text.contains("0.0.0-not-a-real-toolchain"),
        "carries the error's own text:\n{text}"
    );
    assert!(
        text.contains(&err.to_string().lines().next().unwrap().to_string()),
        "the log and the returned error agree:\n{text}"
    );

    // A second failure appends rather than replacing — a log that only ever held
    // the last entry would lose the one that explains the current state.
    sdk.compile(&plugin, &out).expect_err("still no such toolchain");
    let again = std::fs::read_to_string(&log).unwrap();
    assert!(again.len() > text.len(), "the second failure was appended");

    // And it can be switched off, which is what makes it safe to run builds in
    // a test or a CI lane without writing to somebody's home directory.
    std::env::set_var("RENZORA_BUILD_LOG", "off");
    assert!(renzora_plugin_build::log::path().is_none());

    std::env::remove_var("RENZORA_BUILD_LOG");
    let _ = std::fs::remove_dir_all(&tmp);
}
