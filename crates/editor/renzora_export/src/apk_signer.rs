//! APK Signature Scheme v2 — sign APKs without requiring Android SDK tools.
//!
//! Generates a debug RSA keypair + self-signed X.509 certificate on first use,
//! stored in the user's renzora config directory. Signs APKs in-place using
//! the APK Signature Scheme v2 (required for Android 7.0+).

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use ring::rand::SystemRandom;
use ring::signature::{self, RsaKeyPair};

const CHUNK_SIZE: usize = 1024 * 1024; // 1 MB
const EOCD_SIG: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const APK_SIG_BLOCK_MAGIC: &[u8] = b"APK Sig Block 42";
const V2_BLOCK_ID: u32 = 0x7109871a;
const RSA_PKCS1_SHA256_ID: u32 = 0x0103;

/// Sign an APK file in-place using APK Signature Scheme v2.
pub fn sign_apk(apk_path: &Path) -> io::Result<()> {
    let (pkcs8_der, cert_der) = load_or_generate_key()?;

    let key_pair = RsaKeyPair::from_pkcs8(&pkcs8_der)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Bad signing key: {e}")))?;

    let apk_data = std::fs::read(apk_path)?;

    // Locate ZIP structures
    let eocd_offset = find_eocd(&apk_data)?;
    let cd_offset =
        u32::from_le_bytes(apk_data[eocd_offset + 16..eocd_offset + 20].try_into().unwrap())
            as usize;

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

    // Sign it
    let rng = SystemRandom::new();
    let mut sig_bytes = vec![0u8; key_pair.public().modulus_len()];
    key_pair
        .sign(
            &signature::RSA_PKCS1_SHA256,
            &rng,
            &signed_data,
            &mut sig_bytes,
        )
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "RSA signing failed"))?;

    // Public key in SubjectPublicKeyInfo DER format
    let spki = build_rsa_spki(key_pair.public().as_ref());

    // Assemble the v2 signer block and the full APK Signing Block
    let v2_block = build_v2_block(&signed_data, &sig_bytes, &spki);
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

    let key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_RSA_SHA256)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Keygen failed: {e}")))?;

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
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Cert gen failed: {e}")))?;

    let pkcs8_der = key_pair.serialize_der();
    let cert_der = cert.der().to_vec();

    std::fs::write(&key_path, &pkcs8_der)?;
    std::fs::write(&cert_path, &cert_der)?;

    bevy::log::info!("Debug signing key saved to {}", dir.display());
    Ok((pkcs8_der, cert_der))
}

// ---------------------------------------------------------------------------
// ZIP parsing
// ---------------------------------------------------------------------------

fn find_eocd(data: &[u8]) -> io::Result<usize> {
    let min = 22;
    if data.len() < min {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid ZIP"));
    }
    let start = data.len().saturating_sub(65535 + min);
    for i in (start..=data.len() - min).rev() {
        if data[i..i + 4] == EOCD_SIG {
            return Ok(i);
        }
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP EOCD not found"))
}

// ---------------------------------------------------------------------------
// APK v2 digest computation
// ---------------------------------------------------------------------------

fn compute_content_digest(entries: &[u8], cd: &[u8], eocd: &[u8]) -> Vec<u8> {
    use ring::digest::{Context, SHA256};

    let mut chunk_digests = Vec::new();
    let mut chunk_count: u32 = 0;

    for section in [entries, cd, eocd] {
        for chunk in section.chunks(CHUNK_SIZE) {
            let mut ctx = Context::new(&SHA256);
            ctx.update(&[0xa5]);
            ctx.update(&(chunk.len() as u32).to_le_bytes());
            ctx.update(chunk);
            chunk_digests.extend_from_slice(ctx.finish().as_ref());
            chunk_count += 1;
        }
    }

    let mut ctx = Context::new(&SHA256);
    ctx.update(&[0x5a]);
    ctx.update(&chunk_count.to_le_bytes());
    ctx.update(&chunk_digests);
    ctx.finish().as_ref().to_vec()
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
    digest_entry.extend_from_slice(&RSA_PKCS1_SHA256_ID.to_le_bytes());
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
    sig_entry.extend_from_slice(&RSA_PKCS1_SHA256_ID.to_le_bytes());
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

/// Wrap ring's RSAPublicKey DER in a SubjectPublicKeyInfo SEQUENCE.
fn build_rsa_spki(rsa_public_key: &[u8]) -> Vec<u8> {
    // AlgorithmIdentifier: SEQUENCE { OID rsaEncryption, NULL }
    let oid_rsa: &[u8] = &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
    let null: &[u8] = &[0x05, 0x00];

    let mut algo_content = Vec::new();
    algo_content.extend_from_slice(oid_rsa);
    algo_content.extend_from_slice(null);
    let algo_id = der_tag(0x30, &algo_content);

    // BIT STRING with 0 unused bits
    let mut bit_string_content = vec![0x00];
    bit_string_content.extend_from_slice(rsa_public_key);
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
