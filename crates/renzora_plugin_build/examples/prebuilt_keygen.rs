//! Create the Ed25519 key pair that signs prebuilt plugins.
//!
//! Writes the PUBLIC key to `--public-out` (append it to
//! `crates/renzora_plugin_build/prebuilt_public_keys.txt` and commit it) and the
//! SECRET key to stdout, and only there, so it can be piped straight into the CI
//! secret without ever being shown or written to disk:
//!
//! ```sh
//! cargo run --profile dist -p renzora_plugin_build --example prebuilt_keygen -- \
//!     --public-out crates/renzora_plugin_build/prebuilt_public_keys.txt \
//!   | gh secret set PREBUILT_SIGNING_KEY --repo renzora/engine
//! ```
//!
//! `--public-out` appends rather than overwrites, so running it to rotate keeps
//! the old key trusted until it is removed by hand.

use std::io::Write;
use std::process::ExitCode;

use renzora_plugin_build::prebuilt;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let Some(public_out) = argv
        .iter()
        .position(|a| a == "--public-out")
        .and_then(|i| argv.get(i + 1))
    else {
        eprintln!("usage: prebuilt_keygen --public-out <keys file>  (the secret key goes to stdout)");
        return ExitCode::FAILURE;
    };

    let mut secret = [0u8; 32];
    if let Err(e) = getrandom::getrandom(&mut secret) {
        eprintln!("could not read the system's random source: {e}");
        return ExitCode::FAILURE;
    }
    let secret_hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    let Some(public_hex) = prebuilt::public_key_hex(&secret_hex) else {
        eprintln!("could not derive the public key");
        return ExitCode::FAILURE;
    };

    let line = format!("{public_hex}  # added {}\n", today());
    let appended = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(public_out)
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if let Err(e) = appended {
        eprintln!("could not write the public key to {public_out}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!("public key appended to {public_out}: {public_hex}");
    eprintln!("the secret key is on stdout; store it as the PREBUILT_SIGNING_KEY secret");

    // No newline: `gh secret set` stores stdin as it comes.
    print!("{secret_hex}");
    ExitCode::SUCCESS
}

/// Seconds since the epoch, enough to tell two keys apart in the file.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix {secs}")
}
