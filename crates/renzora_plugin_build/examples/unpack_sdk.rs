//! Unpack a released engine's `sdk.tar.zst` into `<root>/sdk`, for CI jobs that
//! compile plugins against it.
//!
//! The engine's own unpacker (`renzora_plugin_build::unpack`), the one an
//! editor runs on first launch, rather than `tar` and `zstd` on the runner: a
//! Windows runner has no `zstd`, and this is the same code path every user's
//! editor takes, so a SDK it cannot unpack is caught here too.
//!
//! ```text
//! unpack_sdk --archive sdk.tar.zst --root work
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| {
        argv.iter()
            .position(|a| a == name)
            .and_then(|i| argv.get(i + 1))
            .map(PathBuf::from)
    };
    let (Some(archive), Some(root)) = (arg("--archive"), arg("--root")) else {
        eprintln!("usage: unpack_sdk --archive <sdk.tar.zst> --root <dir>");
        return ExitCode::from(2);
    };
    if let Err(e) = std::fs::create_dir_all(&root) {
        eprintln!("could not create {}: {e}", root.display());
        return ExitCode::FAILURE;
    }
    match renzora_plugin_build::unpack::extract(&archive, &root, |_| {}) {
        Ok(sdk) => {
            println!("{}", sdk.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("could not unpack {}: {e}", archive.display());
            ExitCode::FAILURE
        }
    }
}
