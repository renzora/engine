//! Sign one prebuilt plugin library, from facts the signer trusts.
//!
//! For third-party plugins, which CI compiles from their creators' source in
//! jobs that hold no secrets (a crate's build script runs arbitrary code). The
//! library those jobs produce, and any description they write of it, came out
//! of untrusted code, so this signs NONE of what they said. The statement is
//! built from what the signing job knows independently:
//!
//! - `--id` and `--version`: the plugin release the marketplace listed for the
//!   build, so a library cannot relabel itself as another plugin;
//! - `--platform`: the job it came from;
//! - `--stamp`: the official SDK's build stamp;
//! - the SHA-256 and size: measured here, of the bytes being signed.
//!
//! It also checks the library records `--stamp` (the ABI record the loader
//! checks), so it never signs a library an editor would refuse to load.
//!
//! The secret key is read from `PREBUILT_SIGNING_KEY`. Prints the signed
//! statement as JSON, in the shape `check_plugins --emit` writes and the
//! marketplace's upload takes.
//!
//! ```text
//! sign_prebuilt --library out/water.dll --id water --version 1.2.0 \
//!     --platform windows-x64 --stamp 47520d6dacbe8fc5+rustc-1.100.0
//! ```

use std::process::ExitCode;

use renzora_plugin_build::prebuilt::{self, Statement};

fn main() -> ExitCode {
    match run() {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sign_prebuilt: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| -> Result<String, String> {
        let i = argv.iter().position(|a| a == name).ok_or(format!("missing {name}"))?;
        argv.get(i + 1).cloned().ok_or(format!("{name} needs a value"))
    };
    let library = arg("--library")?;
    let id = arg("--id")?;
    let version = arg("--version")?;
    let platform = arg("--platform")?;
    let stamp = arg("--stamp")?;
    let key = std::env::var("PREBUILT_SIGNING_KEY").map_err(|_| "PREBUILT_SIGNING_KEY is not set")?;

    let path = std::path::Path::new(&library);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the library has no file name")?
        .to_string();
    let bytes = std::fs::read(path).map_err(|e| format!("reading {library}: {e}"))?;

    let record = format!("RENZORA_NATIVE_PLUGIN_ABI={stamp}\0");
    if !bytes.windows(record.len()).any(|w| w == record.as_bytes()) {
        return Err(format!("{file_name} does not record the engine build {stamp}"));
    }

    let statement = Statement {
        id,
        version,
        platform,
        engine_stamp: stamp,
        sha256: prebuilt::sha256_hex(&bytes),
        size: bytes.len() as u64,
    };
    let signature = prebuilt::sign(&statement, &key)?;
    let json = serde_json::json!({
        "id": statement.id,
        "version": statement.version,
        "platform": statement.platform,
        "engine_stamp": statement.engine_stamp,
        "sha256": statement.sha256,
        "size": statement.size,
        "file_name": file_name,
        "signature": signature,
    });
    serde_json::to_string_pretty(&json).map_err(|e| e.to_string())
}
