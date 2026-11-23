//! Fetching a plugin's prebuilt binary for this engine build, checking it, and
//! putting it where a source build would have left its library.
//!
//! Two callers, so it lives here, below both: the marketplace's installer, when
//! a plugin is installed, and [`crate::prebuild`], when the engine has moved to
//! a new build and an installed plugin's binary no longer matches it (every
//! nightly is a new build, so without this every prebuilt plugin would fall
//! back to compiling from source the first launch after each update).
//!
//! Trust is `renzora_plugin_build::prebuilt`'s: a statement signed by a key
//! compiled into the engine, bytes matching it, and the library recording this
//! engine build. Anything short of that is an `Err` with a reason, and the
//! caller compiles the source instead.

use std::path::Path;

use renzora_plugin_build::prebuilt::{self, Statement};
use serde::Deserialize;

/// A prebuilt that was installed.
pub struct Fetched {
    pub version: String,
    /// The engine release or nightly it was built for, for messages.
    pub engine_tag: String,
}

/// The marketplace's description of one binary. The statement fields are what
/// CI signed.
#[derive(Deserialize)]
struct PrebuiltInfo {
    marketplace_id: String,
    version: String,
    platform: String,
    engine_stamp: String,
    #[serde(default)]
    engine_tag: String,
    sha256: String,
    size: u64,
    signature: String,
    download_url: String,
}

/// The marketplace's address: `RENZORA_API_BASE` for a local or staging server,
/// as the marketplace client honours, otherwise renzora.com.
pub fn api_base() -> String {
    std::env::var("RENZORA_API_BASE")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://renzora.com".to_string())
}

/// Fetch, check and install the prebuilt of marketplace asset `asset_id` for
/// engine build `stamp` into `plugin_dir`, whose source is already installed.
pub fn fetch_and_install(
    token: Option<&str>,
    asset_id: &str,
    plugin_dir: &Path,
    stamp: &str,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<Fetched, String> {
    if !prebuilt::prebuilts_supported() {
        return Err("this engine trusts no prebuilt signing key yet".into());
    }
    let (marketplace_id, version) = source_identity(plugin_dir)?;

    let base = api_base();
    let url = format!(
        "{base}/api/prebuilt-plugins/{asset_id}?engine={}&platform={}",
        url_encode(stamp),
        url_encode(prebuilt::platform_id())
    );
    let response = renzora_net_api::net::Request::get(&url)
        .maybe_bearer(token)
        .send()
        .map_err(|e| format!("the marketplace could not be reached ({e})"))?;
    if response.status == 404 {
        return Err("no prebuilt exists for this engine build and platform".into());
    }
    let info: PrebuiltInfo = response.json().map_err(|e| format!("the marketplace's answer was unreadable ({e})"))?;

    // A valid signature on ANOTHER plugin's binary is still a valid signature,
    // so the binary must name the plugin and version whose source is installed.
    if info.marketplace_id != marketplace_id {
        return Err(format!("the prebuilt is for '{}', not '{marketplace_id}'", info.marketplace_id));
    }
    if info.version != version {
        return Err(format!("the prebuilt is version {}, the source is {version}", info.version));
    }

    let mut stream = renzora_net_api::net::Request::get(&format!("{base}{}", info.download_url))
        .maybe_bearer(token)
        .max_bytes(MAX_PREBUILT)
        .send_stream()
        .map_err(|e| format!("the download failed ({e})"))?;
    let mut bytes = Vec::new();
    for chunk in &mut stream {
        bytes.extend_from_slice(&chunk.data);
        on_bytes(bytes.len() as u64);
    }
    if let Some(e) = stream.error() {
        return Err(format!("the download failed ({e})"));
    }

    let statement = Statement {
        id: info.marketplace_id,
        version: info.version.clone(),
        platform: info.platform,
        engine_stamp: info.engine_stamp,
        sha256: info.sha256,
        size: info.size,
    };
    prebuilt::verify(&statement, &info.signature, stamp, &bytes).map_err(|refusal| refusal.to_string())?;
    // The loader's own check, before the bytes go anywhere it looks.
    match crate::image_abi(&bytes) {
        crate::ImageAbi::Stamped(have) if have == stamp => {}
        _ => return Err("the library does not record this engine build".into()),
    }

    write_library(plugin_dir, &bytes, stamp)?;
    Ok(Fetched { version: info.version, engine_tag: info.engine_tag })
}

/// Largest prebuilt accepted, well above any plugin so far (the largest is a
/// few MB) and well below anything that would be a problem to hold in memory.
const MAX_PREBUILT: u32 = 256 * 1024 * 1024;

/// The marketplace listing a plugin was installed from, read off the sidecar
/// the marketplace writes beside it (`plugin.toml`, `asset_id = "…"`). `None`
/// for a plugin that did not come from the marketplace, which has no prebuilt.
///
/// A line scan rather than a TOML parser this crate does not otherwise need:
/// the sidecar is written by `toml::to_string_pretty` from a struct, so the key
/// is always a plain `asset_id = "…"` line.
pub fn sidecar_asset_id(plugin_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(plugin_dir.join("plugin.toml")).ok()?;
    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "asset_id")
            .then(|| value.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty())
    })
}

/// The marketplace id and version the installed source declares, from its
/// `Cargo.toml` (`[package] version` and `[package.metadata.renzora]
/// marketplace_id`), by the same plain line scan.
fn source_identity(plugin_dir: &Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(plugin_dir.join("Cargo.toml"))
        .map_err(|e| format!("the source has no readable Cargo.toml ({e})"))?;
    let version = toml_value(&text, "package", "version").ok_or("its Cargo.toml has no version")?;
    let id = toml_value(&text, "package.metadata.renzora", "marketplace_id")
        .ok_or("it has no marketplace_id, so no prebuilt is published for it")?;
    Ok((id, version))
}

fn toml_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut current = String::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = name.trim().to_string();
            continue;
        }
        if current != section {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim() == key {
            return Some(v.trim().trim_matches('"').to_string()).filter(|v| !v.is_empty());
        }
    }
    None
}

/// Put the library where a source build would have left it, with the stamp
/// that tells the loader it is current.
///
/// Named for the directory, which is what the loader derives the library's name
/// from: a plugin installed under another name (two listings shipping the same
/// crate) lives in a differently named directory. Written to a temporary name
/// and renamed, so an interrupted write never leaves a truncated library.
fn write_library(plugin_dir: &Path, bytes: &[u8], stamp: &str) -> Result<(), String> {
    let dir_name = plugin_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the plugin directory has no name")?;
    let build = plugin_dir.join("build");
    std::fs::create_dir_all(&build).map_err(|e| format!("creating {} ({e})", build.display()))?;
    let library = build.join(format!("{}.{}", dir_name.replace('-', "_"), std::env::consts::DLL_EXTENSION));
    let partial = build.join(".prebuilt-incoming");
    std::fs::write(&partial, bytes).map_err(|e| format!("writing the library ({e})"))?;
    // A library the running editor has mapped cannot be replaced on Windows;
    // the rename fails, the old one stays, and the caller builds from source on
    // the next start as it would have.
    std::fs::rename(&partial, &library).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("placing the library ({e})")
    })?;
    std::fs::write(build.join("stamp.txt"), stamp).map_err(|e| format!("writing the stamp ({e})"))?;
    // A failure recorded by an earlier source build no longer describes it.
    let _ = std::fs::remove_file(build.join("failed.txt"));
    Ok(())
}

/// Percent-encode a query value. Stamps carry `+`, which a query string would
/// otherwise read as a space.
fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("renzora-prebuilt-fetch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_the_source_identity_and_the_sidecar() {
        let d = dir("identity");
        std::fs::write(
            d.join("Cargo.toml"),
            "[package]\nname = \"water\"\nversion = \"1.2.0\"\n\n[package.metadata.renzora]\nmarketplace_id = \"water\"\n",
        )
        .unwrap();
        std::fs::write(d.join("plugin.toml"), "asset_id = \"3e59-uuid\"\nname = \"Water\"\n").unwrap();
        assert_eq!(source_identity(&d).unwrap(), ("water".to_string(), "1.2.0".to_string()));
        assert_eq!(sidecar_asset_id(&d).as_deref(), Some("3e59-uuid"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn writes_the_library_under_the_directory_name_with_its_stamp() {
        let d = dir("my-plugin");
        std::fs::create_dir_all(d.join("build")).unwrap();
        std::fs::write(d.join("build").join("failed.txt"), "old").unwrap();
        write_library(&d, b"bytes", "abc+rustc-1.100.0").unwrap();
        let name = d.file_name().unwrap().to_str().unwrap().replace('-', "_");
        let lib = d.join("build").join(format!("{name}.{}", std::env::consts::DLL_EXTENSION));
        assert_eq!(std::fs::read(lib).unwrap(), b"bytes");
        assert_eq!(std::fs::read_to_string(d.join("build").join("stamp.txt")).unwrap(), "abc+rustc-1.100.0");
        assert!(!d.join("build").join("failed.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_stamp_survives_the_query_string() {
        assert_eq!(url_encode("abc+rustc-1.100.0"), "abc%2Brustc-1.100.0");
    }
}
