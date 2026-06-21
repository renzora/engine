//! APK Signature Scheme v2 — sign APKs without requiring Android SDK tools.
//!
//! Generates a debug ECDSA P-256 keypair + self-signed X.509 certificate on first
//! use, stored in the user's renzora config directory. Signs APKs in-place using
//! the APK Signature Scheme v2 (required for Android 7.0+).

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p256::ecdsa::signature::Signer as _;
use p256::ecdsa::SigningKey;
use p256::pkcs8::{DecodePrivateKey as _, EncodePrivateKey as _};
use p256::SecretKey;

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB
const EOCD_SIG: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const APK_SIG_BLOCK_MAGIC: &[u8] = b"APK Sig Block 42";
const V2_BLOCK_ID: u32 = 0x7109871a;
/// ECDSA with SHA-256 (APK Signature Scheme v2 algorithm ID)
const ECDSA_SHA256_ID: u32 = 0x0201;

/// Sign an APK file in-place using APK Signature Scheme v2.
pub fn sign_apk(apk_path: &Path) -> io::Result<()> {
    let (pkcs8_der, cert_der) = load_or_generate_key()?;

    // PKCS8 (de)serialization lives on `SecretKey`; derive the ECDSA signer from it.
    let secret_key = SecretKey::from_pkcs8_der(&pkcs8_der)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Bad signing key: {e}")))?;
    let signing_key = SigningKey::from(&secret_key);

    let apk_data = std::fs::read(apk_path)?;

    // Locate ZIP structures
    let eocd_offset = find_eocd(&apk_data)?;
    let cd_offset = u32::from_le_bytes(
        apk_data[eocd_offset + 16..eocd_offset + 20]
            .try_into()
            .unwrap(),
    ) as usize;

    // Compute content digest over the three APK sections.
    // Per spec, EOCD's CD offset field is treated as pointing to the signing block
    // (which will be inserted at cd_offset), so we use it as-is from the unsigned APK.
    let content_digest = compute_content_digest(
        &apk_data[..cd_offset],
        &apk_data[cd_offset..eocd_offset],
        &apk_data[eocd_offset..],
    );

    // Build the signed-data structure
    let signed_data = build_signed_data(&content_digest, &cert_der);

    // Sign signed_data with ECDSA P-256 SHA-256 (RFC 6979 deterministic).
    // APK v2 (algo 0x0201) wants the ASN.1 DER ECDSA-Sig-Value, which `to_der`
    // produces; the fixed-size form would be rejected.
    let signature: p256::ecdsa::Signature = signing_key
        .try_sign(&signed_data)
        .map_err(|_| io::Error::other("ECDSA signing failed"))?;
    let sig = signature.to_der();

    // Public key as the uncompressed SEC1 point (0x04 || X || Y), the form
    // `build_ec_spki` wraps into a SubjectPublicKeyInfo.
    let verifying_point = signing_key.verifying_key().to_encoded_point(false);
    let spki = build_ec_spki(verifying_point.as_bytes());

    // Assemble the v2 signer block and the full APK Signing Block
    let v2_block = build_v2_block(&signed_data, sig.as_bytes(), &spki);
    let signing_block = build_signing_block(&v2_block);

    // Write the signed APK: [entries][signing block][CD][EOCD']
    let mut out = std::fs::File::create(apk_path)?;
    out.write_all(&apk_data[..cd_offset])?;
    out.write_all(&signing_block)?;
    out.write_all(&apk_data[cd_offset..eocd_offset])?;

    // EOCD with updated central directory offset
    let mut eocd = apk_data[eocd_offset..].to_vec();
    let new_cd_offset = (cd_offset + signing_block.len()) as u32;
    eocd[16..20].copy_from_slice(&new_cd_offset.to_le_bytes());
    out.write_all(&eocd)?;

    bevy::log::info!("APK signed: {}", apk_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Key management
// ---------------------------------------------------------------------------

fn signing_dir() -> io::Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Library/Application Support")
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".config")
            })
    };
    let dir = base.join("renzora").join("signing");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Load an existing debug keypair or generate a new one.
fn load_or_generate_key() -> io::Result<(Vec<u8>, Vec<u8>)> {
    let dir = signing_dir()?;
    let key_path = dir.join("debug.pkcs8");
    let cert_path = dir.join("debug.cert");

    if key_path.exists() && cert_path.exists() {
        let key = std::fs::read(&key_path)?;
        let cert = std::fs::read(&cert_path)?;
        return Ok((key, cert));
    }

    bevy::log::info!("Generating debug signing key...");

    // Generate the P-256 key with RustCrypto, then drive rcgen's certificate
    // builder through its crypto-free `RemoteKeyPair` path so the cert is signed
    // by this very key without rcgen ever pulling `ring`/`aws-lc-rs`.
    let secret_key = SecretKey::random(&mut rand_core::OsRng);
    let pkcs8_der = secret_key
        .to_pkcs8_der()
        .map_err(|e| io::Error::other(format!("PKCS8 encode failed: {e}")))?
        .as_bytes()
        .to_vec();
    let signing_key = SigningKey::from(&secret_key);

    let remote = P256RemoteKey::new(signing_key);
    let key_pair = rcgen::KeyPair::from_remote(Box::new(remote))
        .map_err(|e| io::Error::other(format!("Keygen failed: {e}")))?;

    let mut params = rcgen::CertificateParams::default();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Renzora Debug");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Renzora");
    params
        .distinguished_name
        .push(rcgen::DnType::CountryName, "US");

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| io::Error::other(format!("Cert gen failed: {e}")))?;

    let cert_der = cert.der().to_vec();

    std::fs::write(&key_path, &pkcs8_der)?;
    std::fs::write(&cert_path, &cert_der)?;

    bevy::log::info!("Debug signing key saved to {}", dir.display());
    Ok((pkcs8_der, cert_der))
}

/// Bridges our RustCrypto `p256` signing key into rcgen's bring-your-own-crypto
/// `RemoteKeyPair` trait, so certificate signing is done by this key and rcgen
/// never links `ring`/`aws-lc-rs`.
struct P256RemoteKey {
    key: SigningKey,
    /// Uncompressed SEC1 public point (0x04 || X || Y); rcgen wraps it in SPKI.
    public_key: Vec<u8>,
}

impl P256RemoteKey {
    fn new(key: SigningKey) -> Self {
        let public_key = key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        Self { key, public_key }
    }
}

impl rcgen::RemoteKeyPair for P256RemoteKey {
    fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        // rcgen hands us the raw TBSCertificate; ECDSA-with-SHA256 hashes it
        // internally and the X.509 signature value is the ASN.1 DER Sig-Value.
        let sig: p256::ecdsa::Signature =
            self.key.try_sign(msg).map_err(|_| rcgen::Error::RemoteKeyError)?;
        Ok(sig.to_der().as_bytes().to_vec())
    }

    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm {
        &rcgen::PKCS_ECDSA_P256_SHA256
    }
}

// ---------------------------------------------------------------------------
// ZIP parsing
// ---------------------------------------------------------------------------

fn find_eocd(data: &[u8]) -> io::Result<usize> {
    let min = 22;
    if data.len() < min {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Not a valid ZIP",
        ));
    }
    let start = data.len().saturating_sub(65535 + min);
    for i in (start..=data.len() - min).rev() {
        if data[i..i + 4] == EOCD_SIG {
            return Ok(i);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "ZIP EOCD not found",
    ))
}

// ---------------------------------------------------------------------------
// APK v2 digest computation
// ---------------------------------------------------------------------------

fn compute_content_digest(entries: &[u8], cd: &[u8], eocd: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let mut chunk_digests = Vec::new();
    let mut chunk_count: u32 = 0;

    for section in [entries, cd, eocd] {
        for chunk in section.chunks(CHUNK_SIZE) {
            let mut ctx = Sha256::new();
            ctx.update([0xa5]);
            ctx.update((chunk.len() as u32).to_le_bytes());
            ctx.update(chunk);
            chunk_digests.extend_from_slice(&ctx.finalize());
            chunk_count += 1;
        }
    }

    let mut ctx = Sha256::new();
    ctx.update([0x5a]);
    ctx.update(chunk_count.to_le_bytes());
    ctx.update(&chunk_digests);
    ctx.finalize().to_vec()
}

// ---------------------------------------------------------------------------
// APK v2 signing block construction
// ---------------------------------------------------------------------------

/// Length-prefixed blob (u32 LE length prefix).
fn lp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn build_signed_data(content_digest: &[u8], cert_der: &[u8]) -> Vec<u8> {
    // digests: sequence of (algo_id u32 + length-prefixed digest)
    let mut digest_entry = Vec::new();
    digest_entry.extend_from_slice(&ECDSA_SHA256_ID.to_le_bytes());
    digest_entry.extend_from_slice(&lp(content_digest));
    let digests = lp(&lp(&digest_entry));

    // certificates: sequence of length-prefixed DER certs
    let certs = lp(&lp(cert_der));

    // additional attributes: empty
    let attrs = lp(&[]);

    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&digests);
    signed_data.extend_from_slice(&certs);
    signed_data.extend_from_slice(&attrs);
    signed_data
}

fn build_v2_block(signed_data: &[u8], signature: &[u8], public_key: &[u8]) -> Vec<u8> {
    let lp_signed_data = lp(signed_data);

    // signatures: sequence of (algo_id u32 + length-prefixed sig)
    let mut sig_entry = Vec::new();
    sig_entry.extend_from_slice(&ECDSA_SHA256_ID.to_le_bytes());
    sig_entry.extend_from_slice(&lp(signature));
    let signatures = lp(&lp(&sig_entry));

    let lp_public_key = lp(public_key);

    // signer = signed_data + signatures + public_key
    let mut signer = Vec::new();
    signer.extend_from_slice(&lp_signed_data);
    signer.extend_from_slice(&signatures);
    signer.extend_from_slice(&lp_public_key);

    // signers = sequence of signers
    lp(&lp(&signer))
}

fn build_signing_block(v2_block: &[u8]) -> Vec<u8> {
    // ID-value pair: [pair_size: u64 LE] [id: u32 LE] [value]
    let pair_size = (4 + v2_block.len()) as u64;
    let mut pairs = Vec::new();
    pairs.extend_from_slice(&pair_size.to_le_bytes());
    pairs.extend_from_slice(&V2_BLOCK_ID.to_le_bytes());
    pairs.extend_from_slice(v2_block);

    // block_size covers: pairs + second size field (8) + magic (16)
    let block_size = (pairs.len() + 8 + 16) as u64;

    let mut block = Vec::new();
    block.extend_from_slice(&block_size.to_le_bytes());
    block.extend_from_slice(&pairs);
    block.extend_from_slice(&block_size.to_le_bytes());
    block.extend_from_slice(APK_SIG_BLOCK_MAGIC);
    block
}

// ---------------------------------------------------------------------------
// DER encoding helpers (for SubjectPublicKeyInfo)
// ---------------------------------------------------------------------------

/// Wrap an EC public key (uncompressed point) in a SubjectPublicKeyInfo SEQUENCE.
fn build_ec_spki(ec_public_key: &[u8]) -> Vec<u8> {
    // AlgorithmIdentifier: SEQUENCE { OID ecPublicKey, OID prime256v1 }
    let oid_ec: &[u8] = &[0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
    let oid_p256: &[u8] = &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];

    let mut algo_content = Vec::new();
    algo_content.extend_from_slice(oid_ec);
    algo_content.extend_from_slice(oid_p256);
    let algo_id = der_tag(0x30, &algo_content);

    // BIT STRING with 0 unused bits
    let mut bit_string_content = vec![0x00];
    bit_string_content.extend_from_slice(ec_public_key);
    let bit_string = der_tag(0x03, &bit_string_content);

    // Outer SEQUENCE
    let mut spki = Vec::new();
    spki.extend_from_slice(&algo_id);
    spki.extend_from_slice(&bit_string);
    der_tag(0x30, &spki)
}

fn der_tag(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else if len < 0x100 {
        out.push(0x81);
        out.push(len as u8);
    } else if len < 0x10000 {
        out.push(0x82);
        out.push((len >> 8) as u8);
        out.push(len as u8);
    } else {
        out.push(0x83);
        out.push((len >> 16) as u8);
        out.push((len >> 8) as u8);
        out.push(len as u8);
    }
    out.extend_from_slice(content);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ZIP EOCD location ────────────────────────────────────────────────────

    #[test]
    fn find_eocd_locates_minimal_record() {
        // A bare 22-byte EOCD record with no archive comment.
        let mut data = vec![0u8; 22];
        data[..4].copy_from_slice(&EOCD_SIG);
        assert_eq!(find_eocd(&data).unwrap(), 0);
    }

    #[test]
    fn find_eocd_allows_trailing_comment_and_picks_last_match() {
        // EOCD record at offset 10, followed by comment bytes.
        let mut data = vec![0u8; 40];
        data[10..14].copy_from_slice(&EOCD_SIG);
        assert_eq!(find_eocd(&data).unwrap(), 10);

        // If the signature bytes also appear earlier (e.g. inside entry data),
        // the backwards scan must still return the rearmost record.
        data[2..6].copy_from_slice(&EOCD_SIG);
        assert_eq!(find_eocd(&data).unwrap(), 10);
    }

    #[test]
    fn find_eocd_rejects_undersized_or_missing_record() {
        assert!(find_eocd(&[]).is_err());
        assert!(find_eocd(&[0u8; 21]).is_err());
        assert!(find_eocd(&[0u8; 64]).is_err());
    }

    // ── v2 block primitives ──────────────────────────────────────────────────

    #[test]
    fn lp_prefixes_little_endian_length() {
        assert_eq!(lp(b"abc"), [3, 0, 0, 0, b'a', b'b', b'c']);
        assert_eq!(lp(&[]), [0, 0, 0, 0]);
    }

    #[test]
    fn der_tag_uses_shortest_length_encoding() {
        assert_eq!(der_tag(0x30, &[]), [0x30, 0x00]);
        assert_eq!(der_tag(0x30, &[0xAA; 3])[..2], [0x30, 0x03]);
        assert_eq!(der_tag(0x30, &[0xAA; 3])[2..], [0xAA, 0xAA, 0xAA]);
        // Short form up to 0x7f, then one/two/three length bytes.
        assert_eq!(der_tag(0x04, &[0u8; 0x7f])[..2], [0x04, 0x7f]);
        assert_eq!(der_tag(0x04, &[0u8; 0x80])[..3], [0x04, 0x81, 0x80]);
        assert_eq!(der_tag(0x04, &[0u8; 0xff])[..3], [0x04, 0x81, 0xff]);
        assert_eq!(der_tag(0x04, &[0u8; 0x100])[..4], [0x04, 0x82, 0x01, 0x00]);
        assert_eq!(der_tag(0x04, &[0u8; 0xffff])[..4], [0x04, 0x82, 0xff, 0xff]);
        assert_eq!(
            der_tag(0x04, &[0u8; 0x10000])[..5],
            [0x04, 0x83, 0x01, 0x00, 0x00]
        );
    }

    #[test]
    fn ec_spki_wraps_point_in_der_sequence() {
        // Dummy uncompressed P-256 point: 0x04 || X(32) || Y(32).
        let key = [0x04u8; 65];
        let spki = build_ec_spki(&key);

        assert_eq!(spki.len(), 91);
        // Outer SEQUENCE wrapping AlgorithmIdentifier + BIT STRING.
        assert_eq!(&spki[..2], &[0x30, 89]);
        // AlgorithmIdentifier SEQUENCE { ecPublicKey, prime256v1 }.
        assert_eq!(&spki[2..4], &[0x30, 19]);
        assert_eq!(
            &spki[4..13],
            &[0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01]
        );
        assert_eq!(
            &spki[13..23],
            &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07]
        );
        // BIT STRING with zero unused bits, then the raw point.
        assert_eq!(&spki[23..26], &[0x03, 66, 0x00]);
        assert_eq!(&spki[26..], &key);
    }

    // ── Content digest ───────────────────────────────────────────────────────

    /// Reference v2 digest: 0xa5-prefixed 1 MiB chunk hashes, then a
    /// 0x5a-prefixed hash over the chunk-digest concatenation. Literal
    /// constants on purpose, so production drift gets caught.
    fn spec_digest(sections: &[&[u8]]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let mut chunk_digests = Vec::new();
        let mut count: u32 = 0;
        for section in sections {
            for chunk in section.chunks(1024 * 1024) {
                let mut ctx = Sha256::new();
                ctx.update([0xa5]);
                ctx.update((chunk.len() as u32).to_le_bytes());
                ctx.update(chunk);
                chunk_digests.extend_from_slice(&ctx.finalize());
                count += 1;
            }
        }
        let mut ctx = Sha256::new();
        ctx.update([0x5a]);
        ctx.update(count.to_le_bytes());
        ctx.update(&chunk_digests);
        ctx.finalize().to_vec()
    }

    #[test]
    fn content_digest_follows_chunked_format() {
        let d = compute_content_digest(b"entries", b"central dir", b"eocd");
        assert_eq!(d.len(), 32);
        assert_eq!(d, spec_digest(&[b"entries", b"central dir", b"eocd"]));

        // Empty sections contribute no chunks at all.
        assert_eq!(compute_content_digest(b"", b"x", b""), spec_digest(&[b"x"]));

        // Oversized sections split into 1 MiB chunks.
        let big = vec![7u8; CHUNK_SIZE + 1];
        assert_eq!(
            compute_content_digest(&big, b"cd", b"eocd"),
            spec_digest(&[&big, b"cd", b"eocd"])
        );
    }

    #[test]
    fn content_digest_is_sensitive_to_section_boundaries() {
        // Same byte stream, different section split => different chunk lengths.
        assert_ne!(
            compute_content_digest(b"ab", b"c", b"!"),
            compute_content_digest(b"a", b"bc", b"!")
        );
    }

    // ── Block assembly ───────────────────────────────────────────────────────

    #[test]
    fn signed_data_nests_digest_cert_and_empty_attrs() {
        let digest = [0xABu8; 32];
        let cert = b"CERTDER";
        let sd = build_signed_data(&digest, cert);

        assert_eq!(sd.len(), 67);
        // digests: lp(lp(algo_id + lp(digest)))
        assert_eq!(&sd[0..4], &44u32.to_le_bytes());
        assert_eq!(&sd[4..8], &40u32.to_le_bytes());
        assert_eq!(&sd[8..12], &ECDSA_SHA256_ID.to_le_bytes());
        assert_eq!(&sd[12..16], &32u32.to_le_bytes());
        assert_eq!(&sd[16..48], &digest);
        // certificates: lp(lp(cert_der))
        assert_eq!(&sd[48..52], &11u32.to_le_bytes());
        assert_eq!(&sd[52..56], &7u32.to_le_bytes());
        assert_eq!(&sd[56..63], cert);
        // additional attributes: empty
        assert_eq!(&sd[63..67], &0u32.to_le_bytes());
    }

    #[test]
    fn v2_block_nests_signer_components() {
        let block = build_v2_block(b"SD", b"SIG", b"PK");

        assert_eq!(block.len(), 39);
        // signers sequence > signer
        assert_eq!(&block[0..4], &35u32.to_le_bytes());
        assert_eq!(&block[4..8], &31u32.to_le_bytes());
        // signed data
        assert_eq!(&block[8..12], &2u32.to_le_bytes());
        assert_eq!(&block[12..14], b"SD");
        // signatures sequence > (algo_id, lp(sig))
        assert_eq!(&block[14..18], &15u32.to_le_bytes());
        assert_eq!(&block[18..22], &11u32.to_le_bytes());
        assert_eq!(&block[22..26], &ECDSA_SHA256_ID.to_le_bytes());
        assert_eq!(&block[26..30], &3u32.to_le_bytes());
        assert_eq!(&block[30..33], b"SIG");
        // public key
        assert_eq!(&block[33..37], &2u32.to_le_bytes());
        assert_eq!(&block[37..39], b"PK");
    }

    #[test]
    fn signing_block_frames_pairs_with_size_fields_and_magic() {
        let v2 = b"V2BLOCK!";
        let block = build_signing_block(v2);

        // [size u64][pair_size u64][id u32][value][size u64][magic 16]
        assert_eq!(block.len(), 52);
        assert_eq!(&block[0..8], &44u64.to_le_bytes());
        assert_eq!(&block[8..16], &12u64.to_le_bytes());
        assert_eq!(&block[16..20], &V2_BLOCK_ID.to_le_bytes());
        assert_eq!(&block[20..28], v2);
        assert_eq!(&block[28..36], &44u64.to_le_bytes());
        assert_eq!(&block[36..52], APK_SIG_BLOCK_MAGIC);
        // Both size fields exclude the leading size field itself.
        assert_eq!(block.len() as u64, 44 + 8);
    }
}
