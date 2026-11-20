//! Prebuilt plugins: binaries CI compiled against one official engine build,
//! and what an editor checks before it trusts one.
//!
//! A native plugin normally ships as source and is compiled on the machine that
//! installs it, against that engine's staged SDK (see the crate doc). That needs
//! the 1.9 GB SDK unpacked and a Rust toolchain. For an official nightly or
//! release the result is the same on every machine, because every copy of that
//! build has the same engine images, so CI compiles each plugin in
//! `renzora/plugins` once per platform and the marketplace serves the result.
//!
//! # Which engine a binary is for
//!
//! The engine's build stamp ([`crate::Sdk::stamp`]): a content hash of the
//! images a plugin links. Two copies of the same nightly share it; a local
//! `cargo renzora` build has its own, which no prebuilt carries, so a source
//! build finds nothing to download and installs from source without being told
//! to. No "is this an official build" flag exists or is needed.
//!
//! # What makes a binary trustworthy
//!
//! A binary is a native library the editor maps into its own process, so it is
//! accepted only if all of these hold:
//!
//! 1. A [`Statement`] names it: plugin id, version, platform, engine stamp,
//!    SHA-256 and size.
//! 2. That statement is signed with Ed25519 by a key in [`trusted_keys`], which
//!    ships inside the engine. CI holds the private key as a secret; the
//!    marketplace never has it, so a compromised marketplace can serve old or
//!    missing binaries but cannot make an editor accept a binary CI did not
//!    build.
//! 3. The bytes downloaded hash to the statement's SHA-256 and have its size.
//!
//! The caller then checks the engine stamp the plugin carries inside itself (the
//! ABI record `renzora::plugin!` writes) against its own, before the library is
//! ever mapped. That is the loader's own check, run again at install time so a
//! mismatch is reported then rather than at the next launch.
//!
//! The statement is plain text, one `key: value` line each, so what is signed is
//! exactly what a person reading it sees. No field can contain a newline (ids,
//! versions and stamps are validated), so the lines cannot be forged by an
//! embedded one.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// The first line of every statement. A future format changes the number, and
/// an editor that does not know it refuses rather than misreads.
pub const STATEMENT_HEADER: &str = "renzora-prebuilt-plugin/1";

/// The public keys an editor accepts prebuilt plugins from, as committed in
/// `prebuilt_public_keys.txt` beside this crate's manifest: one hex key per
/// line, `#` comments allowed. More than one, so a key can be rotated without
/// orphaning every binary signed with the old one.
///
/// Empty means no key has been set up yet, and then every prebuilt is refused
/// and plugins install from source, which is what happened before prebuilts
/// existed.
const PUBLIC_KEYS: &str = include_str!("../prebuilt_public_keys.txt");

/// What one prebuilt binary is, as signed by CI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    /// The plugin's marketplace id, as in its manifest's `marketplace_id`.
    pub id: String,
    /// The plugin's version, from its `Cargo.toml`.
    pub version: String,
    /// [`platform_id`] of the machine it was built on and runs on.
    pub platform: String,
    /// [`crate::Sdk::stamp`] of the engine it was compiled against.
    pub engine_stamp: String,
    /// Lowercase hex SHA-256 of the library.
    pub sha256: String,
    /// The library's length in bytes.
    pub size: u64,
}

impl Statement {
    /// Describe `library` for plugin `id` at `version`, built here against
    /// `engine_stamp`.
    pub fn for_library(id: &str, version: &str, engine_stamp: &str, library: &[u8]) -> Self {
        Self {
            id: id.to_string(),
            version: version.to_string(),
            platform: platform_id().to_string(),
            engine_stamp: engine_stamp.to_string(),
            sha256: sha256_hex(library),
            size: library.len() as u64,
        }
    }

    /// The exact text that is signed.
    pub fn text(&self) -> String {
        format!(
            "{STATEMENT_HEADER}\nid: {}\nversion: {}\nplatform: {}\nengine: {}\nsha256: {}\nsize: {}\n",
            self.id, self.version, self.platform, self.engine_stamp, self.sha256, self.size
        )
    }

    /// Whether every field is one line of printable text, so [`text`](Self::text)
    /// means one thing. Checked before signing and before verifying.
    pub fn well_formed(&self) -> bool {
        let line = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_graphic() || c == ' ');
        line(&self.id)
            && line(&self.version)
            && line(&self.platform)
            && line(&self.engine_stamp)
            && self.sha256.len() == 64
            && self.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }
}

/// Why a prebuilt was refused. Each is a reason to install from source instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The engine has no trusted key, so it can accept no prebuilt at all.
    NoTrustedKey,
    /// The statement has a field that is not one plain line.
    Malformed,
    /// The signature is not valid hex of the right length.
    BadSignatureEncoding,
    /// No trusted key signed this statement.
    NotSigned,
    /// The statement is for another platform.
    WrongPlatform { have: String },
    /// The statement is for another engine build.
    WrongEngine { have: String },
    /// The bytes are not the size the statement says.
    WrongSize { expected: u64, got: u64 },
    /// The bytes do not hash to the statement's SHA-256.
    WrongHash,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTrustedKey => write!(f, "this engine trusts no prebuilt signing key"),
            Self::Malformed => write!(f, "its description is malformed"),
            Self::BadSignatureEncoding => write!(f, "its signature is not readable"),
            Self::NotSigned => write!(f, "it is not signed by a key this engine trusts"),
            Self::WrongPlatform { have } => write!(f, "it was built for {have}, not {}", platform_id()),
            Self::WrongEngine { have } => write!(f, "it was built for another engine build ({have})"),
            Self::WrongSize { expected, got } => {
                write!(f, "it is {got} bytes where {expected} were signed for")
            }
            Self::WrongHash => write!(f, "its contents do not match the checksum that was signed"),
        }
    }
}

/// Check a downloaded prebuilt: that a trusted key signed `statement`, that it
/// is for this platform and for `engine_stamp`, and that `library` is the bytes
/// it describes. Returns the first reason to refuse it.
pub fn verify(
    statement: &Statement,
    signature_hex: &str,
    engine_stamp: &str,
    library: &[u8],
) -> Result<(), Refusal> {
    verify_with(&trusted_keys(), statement, signature_hex, engine_stamp, library)
}

/// [`verify`] against an explicit key set, so it can be tested without the
/// committed keys.
pub fn verify_with(
    keys: &[VerifyingKey],
    statement: &Statement,
    signature_hex: &str,
    engine_stamp: &str,
    library: &[u8],
) -> Result<(), Refusal> {
    if keys.is_empty() {
        return Err(Refusal::NoTrustedKey);
    }
    if !statement.well_formed() {
        return Err(Refusal::Malformed);
    }
    // The signature first: until it checks out, nothing else in the statement
    // is worth comparing against, and a forged statement could say anything.
    let signature = decode_hex::<64>(signature_hex)
        .map(|bytes| Signature::from_bytes(&bytes))
        .ok_or(Refusal::BadSignatureEncoding)?;
    let text = statement.text();
    if !keys.iter().any(|key| key.verify(text.as_bytes(), &signature).is_ok()) {
        return Err(Refusal::NotSigned);
    }
    if statement.platform != platform_id() {
        return Err(Refusal::WrongPlatform { have: statement.platform.clone() });
    }
    if statement.engine_stamp != engine_stamp {
        return Err(Refusal::WrongEngine { have: statement.engine_stamp.clone() });
    }
    if statement.size != library.len() as u64 {
        return Err(Refusal::WrongSize { expected: statement.size, got: library.len() as u64 });
    }
    if sha256_hex(library) != statement.sha256 {
        return Err(Refusal::WrongHash);
    }
    Ok(())
}

/// Sign `statement` with a hex-encoded 32-byte Ed25519 secret key. For CI only;
/// an editor has no secret key and never signs anything.
pub fn sign(statement: &Statement, secret_hex: &str) -> Result<String, String> {
    if !statement.well_formed() {
        return Err("the statement has a field that is not one plain line".into());
    }
    let secret = decode_hex::<32>(secret_hex.trim()).ok_or("the signing key is not 64 hex digits")?;
    let key = SigningKey::from_bytes(&secret);
    Ok(encode_hex(&key.sign(statement.text().as_bytes()).to_bytes()))
}

/// The public half of a hex secret key, as the line to add to
/// `prebuilt_public_keys.txt`.
pub fn public_key_hex(secret_hex: &str) -> Option<String> {
    let secret = decode_hex::<32>(secret_hex.trim())?;
    Some(encode_hex(SigningKey::from_bytes(&secret).verifying_key().as_bytes()))
}

/// The keys in `prebuilt_public_keys.txt`. A line that is not a valid key is
/// skipped rather than trusted, so a typo makes prebuilts unavailable rather
/// than unchecked.
pub fn trusted_keys() -> Vec<VerifyingKey> {
    parse_keys(PUBLIC_KEYS)
}

fn parse_keys(text: &str) -> Vec<VerifyingKey> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .filter_map(decode_hex::<32>)
        .filter_map(|bytes| VerifyingKey::from_bytes(&bytes).ok())
        .collect()
}

/// Whether this engine can accept prebuilt plugins at all: it trusts at least
/// one key. Without one there is no point asking the marketplace for any.
pub fn prebuilts_supported() -> bool {
    !trusted_keys().is_empty()
}

/// This machine's platform, named as the engine's `dist/` directories are and
/// as the marketplace files prebuilts under.
pub fn platform_id() -> &'static str {
    let arm = cfg!(target_arch = "aarch64");
    if cfg!(target_os = "windows") {
        if arm { "windows-arm64" } else { "windows-x64" }
    } else if cfg!(target_os = "macos") {
        if arm { "macos-arm64" } else { "macos-x64" }
    } else if arm {
        "linux-arm64"
    } else {
        "linux-x64"
    }
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    encode_hex(&Sha256::digest(bytes))
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0xf) as usize] as char);
    }
    out
}

fn decode_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.as_bytes();
    if text.len() != N * 2 {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = [0u8; N];
    for (i, pair) in text.chunks(2).enumerate() {
        out[i] = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";

    fn key() -> VerifyingKey {
        VerifyingKey::from_bytes(&decode_hex::<32>(&public_key_hex(SECRET).unwrap()).unwrap()).unwrap()
    }

    fn signed(library: &[u8]) -> (Statement, String) {
        let statement = Statement::for_library("water", "1.2.0", "abc123+rustc-1.100.0", library);
        let signature = sign(&statement, SECRET).unwrap();
        (statement, signature)
    }

    #[test]
    fn a_signed_matching_library_is_accepted() {
        let library = b"pretend this is a dll";
        let (statement, signature) = signed(library);
        assert_eq!(verify_with(&[key()], &statement, &signature, "abc123+rustc-1.100.0", library), Ok(()));
    }

    #[test]
    fn changed_bytes_are_refused() {
        let (statement, signature) = signed(b"pretend this is a dll");
        let tampered = b"pretend this is a dlL";
        assert_eq!(
            verify_with(&[key()], &statement, &signature, "abc123+rustc-1.100.0", tampered),
            Err(Refusal::WrongHash)
        );
    }

    /// Editing the statement to match tampered bytes breaks the signature,
    /// which is what makes the checksum worth anything.
    #[test]
    fn a_statement_rewritten_for_other_bytes_is_refused() {
        let (mut statement, signature) = signed(b"pretend this is a dll");
        let other = b"something else entirely";
        statement.sha256 = sha256_hex(other);
        statement.size = other.len() as u64;
        assert_eq!(
            verify_with(&[key()], &statement, &signature, "abc123+rustc-1.100.0", other),
            Err(Refusal::NotSigned)
        );
    }

    #[test]
    fn another_engine_build_is_refused() {
        let library = b"pretend this is a dll";
        let (statement, signature) = signed(library);
        assert!(matches!(
            verify_with(&[key()], &statement, &signature, "def456+rustc-1.100.0", library),
            Err(Refusal::WrongEngine { .. })
        ));
    }

    #[test]
    fn an_untrusted_key_is_refused() {
        let library = b"pretend this is a dll";
        let (statement, signature) = signed(library);
        let stranger = SigningKey::from_bytes(&[7u8; 32]).verifying_key();
        assert_eq!(
            verify_with(&[stranger], &statement, &signature, "abc123+rustc-1.100.0", library),
            Err(Refusal::NotSigned)
        );
        assert_eq!(
            verify_with(&[], &statement, &signature, "abc123+rustc-1.100.0", library),
            Err(Refusal::NoTrustedKey)
        );
    }

    #[test]
    fn a_field_with_a_newline_cannot_be_signed() {
        let mut statement = Statement::for_library("water", "1.2.0", "abc", b"x");
        statement.version = "1.2.0\nsha256: 00".into();
        assert!(sign(&statement, SECRET).is_err());
    }

    #[test]
    fn keys_file_skips_comments_and_bad_lines() {
        let good = public_key_hex(SECRET).unwrap();
        let text = format!("# the CI key\n{good}\nnot-a-key\n\n");
        assert_eq!(parse_keys(&text).len(), 1);
        assert!(parse_keys("# nothing yet\n").is_empty());
    }
}
