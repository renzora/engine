//! Stateless randomness: every random number is a hash of where it is used.
//!
//! A particle's randoms are a function of (emitter seed, particle spawn index,
//! salt, stream) and nothing else. There is no generator to advance, so the
//! order particles are processed in, how steps are batched, and whether the
//! effect was played or scrubbed to a time all give the same numbers. That is
//! what makes `simulate_to` a faithful replay, and it is the only scheme a GPU
//! backend can reproduce bit for bit, since a compute shader has no shared
//! generator either.
//!
//! The mixer is the PCG hash from Jarzynski and Olano, "Hash Functions for GPU
//! Rendering" (JCGT 2020): all 32-bit integer ops, cheap in WGSL, and good
//! enough that chaining it over each input avoids visible correlation between
//! neighbouring particles. A GPU backend must use exactly this chain.

/// One round of the PCG hash.
#[inline]
pub fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

/// The hash a particle's randoms start from.
#[inline]
pub fn particle_hash(seed: u64, id: u32, salt_bits: u32, stream: u32) -> u32 {
    let mut h = pcg(stream);
    h = pcg(h ^ salt_bits);
    h = pcg(h ^ id);
    h = pcg(h ^ (seed as u32));
    pcg(h ^ ((seed >> 32) as u32))
}

/// The top 24 bits as a float in 0..1 (never 1.0): exactly representable, so
/// the CPU and a GPU agree on it.
#[inline]
pub fn unit_float(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Four independent uniforms in 0..1 for one (seed, id, salt, stream).
#[inline]
pub fn random4(seed: u64, id: u32, salt_bits: u32, stream: u32) -> [f32; 4] {
    let h = particle_hash(seed, id, salt_bits, stream);
    [
        unit_float(pcg(h)),
        unit_float(pcg(h ^ 0x68E3_1DA4)),
        unit_float(pcg(h ^ 0xB529_7A4D)),
        unit_float(pcg(h ^ 0x1B56_C4E9)),
    ]
}

/// Salt as the interpreter hashes it: the float's bits, with -0.0 folded into
/// 0.0 so a value that prints the same hashes the same.
#[inline]
pub fn salt_bits(salt: f32) -> u32 {
    if salt == 0.0 {
        0
    } else {
        salt.to_bits()
    }
}
