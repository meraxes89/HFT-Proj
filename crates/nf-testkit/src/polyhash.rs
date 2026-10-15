//! R24 (Run 500+ Track 1) — SPANHASH-64: the CLMUL polynomial-hash lane
//! combine that replaces the 9-multiply FNV-1a-64 serial chain.
//!
//! # The physics this module exists for
//!
//! `affine_tag_eval`/`span_crc32c_8lane` finish every span by combining
//! the 8 CRC32C lane registers + the length into the golden span value.
//! The FNV-1a-64 combine is INHERENTLY SERIAL: each 64-bit integer
//! multiply (IMUL, 3-cycle latency on Zen 5) depends on the previous
//! word's XOR+multiply, a 27-cycle dependency chain per span that
//! out-of-order execution can only hide across span boundaries.
//!
//! # The mathematical re-derivation
//!
//! FNV-1a mixes XOR (GF(2)) with modular multiplication (Z_{2^64}) —
//! algebraically unparallelizable. A Carter-Wegman POLYNOMIAL hash over
//! GF(2^64) is pure GF(2) arithmetic:
//!
//! ```text
//! H(w[0..4), L) = reduce_P( XOR_{j=0..4} clmul128(term_j, K_j) )
//! ```
//!
//! * terms: the 4 packed lane-pair words w[j] = lanes[2j] | lanes[2j+1]<<32
//!   and L = length widened to 32 bits — the EXACT FNV input domain, so
//!   the hash is still a pure function of (lanes, len).
//! * P = x^64 + x^4 + x^3 + x + 1 — IRREDUCIBLE over GF(2) (Rabin test:
//!   x^(2^64) == x mod P and gcd(x^(2^32)-x, P) == 1; proven by
//!   `scripts/r24_polyhash_derive.py`).
//! * K_j = r^(j+1) in GF(2^64) (r = the fold constant 0x9E3779...7C15 —
//!   fixed public keys: the CW *construction* gives the collision
//!   structure, determinism keeps the golden law "identical emissions ->
//!   identical hash" bit-reproducible on every host).
//! * reduce_P = the two-round Barrett-style folding: x = hi*x^64 + lo
//!   with x^64 == p (deg 4) gives x == (hi clmul p) ^ lo — one round
//!   shrinks a 127-bit residue to 67 bits, the second to < 64: the
//!   canonical residue (pinned == polynomial long division by 10k
//!   random residues in the derive script).
//!
//! Because carry-less multiplication is INDEPENDENT across terms, the
//! five clmuls issue as parallel VPCLMULQDQ/PCLMULQDQ uops — the serial
//! chain collapses to ~4 clmul-class uops + a 2-level XOR tree + one
//! folding reduction.
//!
//! # The intrinsics shape on aarch64 (AArch64 NEON + Crypto Extensions)
//!
//! The SoC class this track targets (Helio G99: Cortex-A55 + A76,
//! `/proc/cpuinfo` flags `pmull`/`crc32`/`asimd` — every ARMv8.0 CPU with
//! the crypto extension) exposes the EXACT same 64×64→128 carry-less
//! product x86 gets from PCLMULQDQ: the A64 `PMULL Vd.1Q, Vn.1D, Vm.1D`
//! instruction, surfaced as [`std::arch::aarch64::vmull_p64`] (stable,
//! `neon,aes` feature-gated). Five independent PMULLs (w0..w3, len — the
//! products live in Q registers), an EOR tree in NEON (u128 XOR lowers to
//! EOR v.16b), the first folding round as a sixth PMULL by 0x1B, and the
//! ≤ 4-bit second round in GPR shifts — one-for-one the PCLMUL path's
//! shape. Runtime-gated on `is_aarch64_feature_detected!("aes")` (the
//! ID_AA64ISAR0 AES field = 2 implies both AESE and PMULL.1Q; the Linux
//! kernel tabulates them as the separate `aes`/`pmull` hwcaps but they
//! rise and fall together), bit-identical to the portable oracle (the
//! differential battery `t_r24_*` pins all paths against the derive
//! script's vectors — verified under QEMU aarch64 emulation as well as
//! on silicon).
//!
//! # The exact intrinsics shape (the worker hot path, x86_64)
//!
//! SSE4.2+PCLMUL (universal on target silicon):
//! `_mm_clmulepi64_epi128` x5 (w0..w3, len) + `_mm_xor_si128` tree +
//! one reduction clmul by 0x1B.
//!
//! AVX-512F/BW/VL + VPCLMULQDQ (the R23 vector class):
//! t[2..5] load as ONE 256-bit YMM (`_mm256_loadu_si256`),
//! TWO `_mm256_clmulepi64_epi128` (imm 0x00: w0/w2, imm 0x11: w1/w3)
//! produce all four 128-bit products, `_mm256_xor_si256` folds them,
//! `_mm256_extracti128_si256` + ONE `_mm_ternarylogic_epi64(0x96)`
//! tree-XORs the halves and the length product, and a single
//! `_mm_clmulepi64_si128(hi, 0x1B)` runs the first folding round with
//! a 4-bit GPR tail for the second. All three paths (portable /
//! PCLMUL / VPCLMUL) are bit-identical — the differential battery
//! `t_r24_*` pins them against the derive script's vectors.
//!
//! # Rollback
//!
//! `HFT_SPANHASH_VEC=0` forces the scalar portable path; `=1` forces the
//! vector path when the class is present. Read once per process (law #9:
//! outside every measurement window).

/// The modulus' low word: P = x^64 + x^4 + x^3 + x + 1 (irreducible —
/// see the module docs and the derive script).
pub const SPANHASH_MOD_P_LOW: u64 = 0x0000_0000_0000_001B;

/// The fixed key schedule K0..K4 = r^1..r^5 in GF(2^64) mod P
/// (r = 0x9E3779B97F4A7C15; derived by `scripts/r24_polyhash_derive.py`).
pub const SPANHASH_KEYS: [u64; 5] = [
    0x9E37_79B9_7F4A_7C15,
    0xC949_6682_D48D_B090,
    0x0C29_FAC0_27C1_BE19,
    0x27CA_F68B_98DA_812D,
    0xECC6_D2FF_80E6_1FD6,
];

/// The keys arranged for one 256-bit VPCLMULQDQ operand pair ([K0,K1,K2,K3]).
#[cfg(target_arch = "x86_64")]
const SPANHASH_KEYS_256: [u64; 4] = [
    SPANHASH_KEYS[0],
    SPANHASH_KEYS[1],
    SPANHASH_KEYS[2],
    SPANHASH_KEYS[3],
];

/// Portable carry-less multiply: the GF(2) polynomial product of two
/// 64-bit values (bit-serial; the non-x86_64 and oracle path).
#[inline(always)]
fn clmul64_portable(a: u64, b: u64) -> u128 {
    let mut r: u128 = 0;
    let mut a: u128 = a as u128;
    let mut b = b;
    while b != 0 {
        if b & 1 != 0 {
            r ^= a;
        }
        a <<= 1;
        b >>= 1;
    }
    r
}

/// The two-round folding reduction (the portable reference semantics).
///
/// Round k: x = hi·x^64 + lo, and x^64 == p (mod P) with deg(p) = 4, so
/// x == (hi ⊗ p) ^ lo. From 127 bits: round 1 -> ≤ 67 bits, round 2 ->
/// < 64 bits: the canonical residue. `deg(hi2) ≤ 3` makes hi2 ⊗ p plain
/// GPR shifts: (hi2<<4)^(hi2<<3)^(hi2<<1)^hi2.
#[inline(always)]
fn reduce_mod_p_portable(x: u128) -> u64 {
    let mut x = x;
    for _ in 0..2 {
        if x >> 64 == 0 {
            break;
        }
        let hi = (x >> 64) as u64;
        let lo = x as u64;
        x = clmul64_portable(hi, SPANHASH_MOD_P_LOW) ^ (lo as u128);
    }
    debug_assert!(x < (1 << 64), "two-round folding law");
    x as u64
}

/// The SPANHASH-64 polynomial hash over the 5 terms (4 packed lane-pair
/// words + the length). Pure, portable, deterministic — the oracle every
/// accelerated path must match bit-for-bit.
#[inline(always)]
pub fn spanhash64_portable(words: &[u64; 4], len: u32) -> u64 {
    let terms = [
        words[0],
        words[1],
        words[2],
        words[3],
        len as u64,
    ];
    let mut acc: u128 = 0;
    for i in 0..5 {
        acc ^= clmul64_portable(terms[i], SPANHASH_KEYS[i]);
    }
    reduce_mod_p_portable(acc)
}

// ── x86_64: the PCLMULQDQ (SSE4.2) path — universal on target silicon ────

/// The 5-term combine + folding reduction through XMM PCLMULQDQ.
/// SAFETY contract: caller proves `pclmulqdq` (and `sse4.2`) present.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2,pclmulqdq")]
#[inline]
unsafe fn spanhash64_pclmul(words: &[u64; 4], len: u32) -> u64 {
    use std::arch::x86_64::*;
    // SAFETY: the pointers are aligned per __m128i contract (loadu is
    // defined for any alignment); registers are value-only.
    let a = _mm_loadu_si128(words.as_ptr() as *const __m128i); // [w0, w1]
    let b = _mm_loadu_si128(words.as_ptr().add(2) as *const __m128i); // [w2, w3]
    let k01 = _mm_loadu_si128(SPANHASH_KEYS.as_ptr() as *const __m128i); // [K0, K1]
    let k23 = _mm_loadu_si128(SPANHASH_KEYS.as_ptr().add(2) as *const __m128i); // [K2, K3]
    // imm 0x00: low qwords; imm 0x11: high qwords — four INDEPENDENT
    // 64x64 carry-less products, zero serial dependency.
    let p01 = _mm_clmulepi64_si128(a, k01, 0x00);
    let p23 = _mm_clmulepi64_si128(a, k01, 0x11);
    let p45 = _mm_clmulepi64_si128(b, k23, 0x00);
    let p67 = _mm_clmulepi64_si128(b, k23, 0x11);
    // The length term: L in the low qword of an XMM.
    let l = _mm_cvtsi32_si128(len as i32);
    let k4 = _mm_cvtsi64_si128(SPANHASH_KEYS[4] as i64);
    let pl = _mm_clmulepi64_si128(l, k4, 0x00);
    // Tree-XOR of the five 128-bit products.
    let s = _mm_xor_si128(
        _mm_xor_si128(p01, p23),
        _mm_xor_si128(_mm_xor_si128(p45, p67), pl),
    );
    // Folding round 1: x = hi*x^64 + lo -> (hi clmul p) ^ lo.
    let hi = _mm_unpackhi_epi64(s, s);
    let plow = _mm_cvtsi32_si128(SPANHASH_MOD_P_LOW as i32);
    let t1 = _mm_clmulepi64_si128(hi, plow, 0x00);
    // THE FOLDING LAW: x == (hi clmul p) ^ lo — lo is the LOW qword
    // only; _mm_move_epi64 zero-extends it ([qw0, 0]) so the original hi
    // bits LEAVE the register (they were replaced by hi clmul p).
    let x1 = _mm_xor_si128(t1, _mm_move_epi64(s));
    // Round 2 (the ≤ 4-bit residue): hi2 ⊗ p in the GPR — shifts only.
    let hi2 = _mm_cvtsi128_si64(_mm_unpackhi_epi64(x1, x1)) as u64 & 0xF;
    let lo = _mm_cvtsi128_si64(x1) as u64;
    lo ^ ((hi2 << 4) ^ (hi2 << 3) ^ (hi2 << 1) ^ hi2)
}

// ── x86_64: the VPCLMULQDQ (AVX-512 vector) path — the hot-path shape ────

/// The 4-word combine through ONE 256-bit load + TWO VPCLMULQDQ + a
/// VPTERNLOGQ tree + the folding reduction.
/// SAFETY contract: caller proves `avx512f,avx512bw,avx512vl,vpclmulqdq`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw,avx512vl,vpclmulqdq")]
#[inline]
unsafe fn spanhash64_vec256(words: &[u64; 4], len: u32) -> u64 {
    use std::arch::x86_64::*;
    // The four lane-pair words as ONE 256-bit register (t[2..5] of the
    // sidecar are contiguous — the load is the descriptor's own width).
    let a = _mm256_loadu_si256(words.as_ptr() as *const __m256i);
    let k = _mm256_loadu_si256(SPANHASH_KEYS_256.as_ptr() as *const __m256i);
    // imm 0x00: (w0,K0) and (w2,K2); imm 0x11: (w1,K1) and (w3,K3) —
    // all four products in TWO instructions, fully parallel.
    let pl = _mm256_clmulepi64_epi128(a, k, 0x00);
    let ph = _mm256_clmulepi64_epi128(a, k, 0x11);
    let s = _mm256_xor_si256(pl, ph);
    // Fold the two 128-bit lanes + the length product in ONE ternary op
    // (VPTERNLOGQ 0x96 = a^b^c — the tree-XOR the roadmap prescribes).
    let lo = _mm256_castsi256_si128(s);
    let hi = _mm256_extracti128_si256(s, 1);
    let l = _mm_cvtsi32_si128(len as i32);
    let k4 = _mm_cvtsi64_si128(SPANHASH_KEYS[4] as i64);
    let plen = _mm_clmulepi64_si128(l, k4, 0x00);
    let s128 = _mm_ternarylogic_epi64(lo, hi, plen, 0x96);
    // The folding reduction — identical to the SSE path from here.
    let h = _mm_unpackhi_epi64(s128, s128);
    let plow = _mm_cvtsi32_si128(SPANHASH_MOD_P_LOW as i32);
    let t1 = _mm_clmulepi64_si128(h, plow, 0x00);
    // The folding law's zero-extended low qword (see the SSE path).
    let x1 = _mm_xor_si128(t1, _mm_move_epi64(s128));
    let hi2 = _mm_cvtsi128_si64(_mm_unpackhi_epi64(x1, x1)) as u64 & 0xF;
    let lo2 = _mm_cvtsi128_si64(x1) as u64;
    lo2 ^ ((hi2 << 4) ^ (hi2 << 3) ^ (hi2 << 1) ^ hi2)
}

// ── aarch64: the PMULL (NEON carry-less multiply) path — vmull_p64 ──────

/// The 5-term combine + folding reduction through the A64 crypto
/// extension's polynomial multiply (PMULL, 64-bit operand form).
///
/// `vmull_p64(a, b)` IS the 64×64→128 carry-less product PCLMULQDQ
/// computes — the u128 result rides a Q register, so the five term
/// multiplies and the XOR tree execute as parallel NEON EOR/PMULL ops
/// (Cortex-A76: one crypto-pipe PMULL.1Q each, ~3-cycle latency, dual
/// issue with the vector pipes; the independent terms hide the latency
/// entirely). The reduction is the same two-round folding law as the
/// x86 paths: round 1 is a sixth PMULL by P's low word, round 2 (the
/// ≤ 4-bit residue) runs in GPR shifts.
/// SAFETY contract: caller proves the `aes` (crypto/pmull) feature.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon,aes")]
#[inline]
unsafe fn spanhash64_pmull(words: &[u64; 4], len: u32) -> u64 {
    use std::arch::aarch64::vmull_p64;
    // Five INDEPENDENT 64x64->128 carry-less multiplies — the exact
    // PCLMUL/VPCLMUL term algebra, zero serial dependency. SAFETY: the
    // feature contract is the caller's; registers are value-only.
    let p01 = vmull_p64(words[0], SPANHASH_KEYS[0]);
    let p23 = vmull_p64(words[1], SPANHASH_KEYS[1]);
    let p45 = vmull_p64(words[2], SPANHASH_KEYS[2]);
    let p67 = vmull_p64(words[3], SPANHASH_KEYS[3]);
    let pl = vmull_p64(len as u64, SPANHASH_KEYS[4]);
    // Tree-XOR of the five 128-bit products (u128 XOR lowers to EOR
    // v.16b — the products never leave the NEON register file).
    let s = p01 ^ p23 ^ p45 ^ p67 ^ pl;
    // Folding round 1: x = hi·x^64 + lo -> (hi ⊗ p) ^ lo. The 128-bit
    // product hi ⊗ 0x1B stays in a Q register (deg ≤ 67); the low
    // qword zero-extends exactly like _mm_move_epi64 on x86.
    let hi = (s >> 64) as u64;
    let t1 = vmull_p64(hi, SPANHASH_MOD_P_LOW);
    let x1 = t1 ^ (s as u64 as u128);
    // Round 2 (the ≤ 4-bit residue): hi2 ⊗ p in the GPR — shifts only,
    // the same tail as both x86 paths.
    let hi2 = (x1 >> 64) as u64 & 0xF;
    let lo = x1 as u64;
    lo ^ ((hi2 << 4) ^ (hi2 << 3) ^ (hi2 << 1) ^ hi2)
}

/// The dispatch mode (resolved once — law #9, outside every window).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Portable,
    #[cfg(target_arch = "x86_64")]
    Pclmul,
    #[cfg(target_arch = "x86_64")]
    Vec256,
    #[cfg(target_arch = "aarch64")]
    Pmull,
}

#[inline]
fn resolve_mode() -> Mode {
    #[cfg(target_arch = "x86_64")]
    {
        match std::env::var("HFT_SPANHASH_VEC").as_deref() {
            Ok("0") => return Mode::Portable,
            Ok("1") => {
                if spanhash_vec_class() {
                    return Mode::Vec256;
                }
            }
            _ => {}
        }
        if spanhash_vec_class() {
            Mode::Vec256
        } else if std::arch::is_x86_feature_detected!("pclmulqdq") {
            Mode::Pclmul
        } else {
            Mode::Portable
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        match std::env::var("HFT_SPANHASH_VEC").as_deref() {
            Ok("0") => return Mode::Portable,
            Ok("1") => {
                if spanhash_pmull_class() {
                    return Mode::Pmull;
                }
            }
            _ => {}
        }
        if spanhash_pmull_class() {
            Mode::Pmull
        } else {
            Mode::Portable
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        Mode::Portable
    }
}

/// The aarch64 PMULL class gate: the crypto extension's polynomial
/// multiply (AES field value 2 — AESE *and* PMULL.1Q; the kernel's
/// `pmull` hwcap rides the same ID field). Plain ARMv8 without crypto
/// falls back to the portable bit-serial path.
#[cfg(target_arch = "aarch64")]
#[inline]
fn spanhash_pmull_class() -> bool {
    std::arch::is_aarch64_feature_detected!("aes")
}

/// The AVX-512 vector class gate (mirrors `crcfold::fold512_available`'s
/// F/BW/VPCLMULQDQ class, plus VL for the VPTERNLOGQ lane form).
#[cfg(target_arch = "x86_64")]
fn spanhash_vec_class() -> bool {
    std::arch::is_x86_feature_detected!("avx512f")
        && std::arch::is_x86_feature_detected!("avx512bw")
        && std::arch::is_x86_feature_detected!("avx512vl")
        && std::arch::is_x86_feature_detected!("vpclmulqdq")
}

/// SPANHASH-64: the golden span-value combine over the 4 packed lane-pair
/// words + the span length. Bit-identical on every path and every host —
/// the differential battery pins portable/PCLMUL/VPCLMUL/PMULL against
/// the derive script's published vectors.
#[inline(always)]
pub fn spanhash64(words: &[u64; 4], len: u32) -> u64 {
    static M: std::sync::OnceLock<Mode> = std::sync::OnceLock::new();
    match *M.get_or_init(resolve_mode) {
        Mode::Portable => spanhash64_portable(words, len),
        #[cfg(target_arch = "x86_64")]
        Mode::Pclmul => {
            // SAFETY: resolve_mode proved the pclmulqdq feature.
            unsafe { spanhash64_pclmul(words, len) }
        }
        #[cfg(target_arch = "x86_64")]
        Mode::Vec256 => {
            // SAFETY: resolve_mode proved the AVX-512 vector class.
            unsafe { spanhash64_vec256(words, len) }
        }
        #[cfg(target_arch = "aarch64")]
        Mode::Pmull => {
            // SAFETY: resolve_mode proved the crypto/pmull class.
            unsafe { spanhash64_pmull(words, len) }
        }
    }
}

/// R24: the lane-register form — pack the 8 u32 lanes into 4 words and
/// hash with the length (the drop-in replacement for the FNV-1a-64 lane
/// combine; `span_lane_combine`'s body).
#[inline(always)]
pub fn spanhash64_lanes(lanes: &[u32; 8], len: usize) -> u64 {
    let words = [
        lanes[0] as u64 | ((lanes[1] as u64) << 32),
        lanes[2] as u64 | ((lanes[3] as u64) << 32),
        lanes[4] as u64 | ((lanes[5] as u64) << 32),
        lanes[6] as u64 | ((lanes[7] as u64) << 32),
    ];
    spanhash64(&words, len as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derive script's published vectors — pins the constants and the
    /// portable semantics (any drift in table, packing, or reduction
    /// fails here first).
    #[test]
    fn t_r24_vectors_portable() {
        const TV: [(([u64; 4], u32), u64); 10] = [
            (([0, 0, 0, 0], 0), 0x0000000000000000),
            (([1, 0, 0, 0], 1), 0x72F1AB46FFAC63C3),
            (([0xFFFFFFFFFFFFFFFF, 0, 0, 0], 0), 0x1A619CE5AFDFB722),
            (([0, 0, 0, 0], 0xFFFFFFFF), 0xDBE044E0F5524FBC),
            (
                (
                    [0xDEADBEEFCAFEBABE, 0x0123456789ABCDEF, 0xFEDCBA9876543210, 0x13579BDF2468ACE0],
                    1364,
                ),
                0x5B57584402D1A224,
            ),
            (
                ([0x1111111111111111, 0x2222222222222222, 0x3333333333333333, 0x4444444444444444], 8),
                0x51BECF7578A3C009,
            ),
            (
                (
                    [0x00000000AABBCCDD, 0xEEFF001122334455, 0x66778899AABBCCDD, 0xEEFF001122334455],
                    0,
                ),
                0xE16DA92A2E40B80E,
            ),
            (
                (
                    [0xC661538E409D625D, 0xAF3E5B9AE51BB0C9, 0x7372E2F94E87704A, 0xAA546698F31E40F7],
                    2544668819,
                ),
                0xCF66E030BC3D66E9,
            ),
            (
                (
                    [0xD3120787BD55389C, 0x4A145929FDA2AA4C, 0x88B4BA129DD7581F, 0x2C31F876ADCF4BAB],
                    1590619330,
                ),
                0x60655E7372721F34,
            ),
            (
                (
                    [0xEC4788E3113B1F88, 0x9BAC1004AB648519, 0x467FF0A1587281B0, 0xDFA3FA583D0A2C77],
                    3228527105,
                ),
                0xF645EED78E24330A,
            ),
        ];
        for ((w, l), want) in TV {
            assert_eq!(spanhash64_portable(&w, l), want, "portable vector drift");
            assert_eq!(spanhash64(&w, l), want, "dispatched vector drift");
        }
    }

    /// Path parity: the dispatched (accelerated) path must equal the
    /// portable oracle on a pseudo-random corpus (LCG — deterministic,
    /// allocation-free). Covers every compiled dispatch class: on x86_64
    /// the VPCLMUL/PCLMUL arms, on aarch64 the PMULL arm (pinned under
    /// QEMU aarch64 emulation AND on the Helio G99 silicon).
    #[test]
    fn t_r24_path_parity() {
        let mut s = 0x243F_6A88_85A3_08D3u64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..20_000 {
            let w = [next(), next(), next(), next()];
            let l = (next() >> 32) as u32;
            let oracle = spanhash64_portable(&w, l);
            assert_eq!(spanhash64(&w, l), oracle, "path parity break");
        }
    }

    /// The reduction law: fold(long-division) for random 128-bit residues —
    /// reimplements the derive script's cross-check in Rust.
    #[test]
    fn t_r24_reduction_vs_longdiv() {
        let mut s = 0xB7E1_5162_8AED_2A6Au64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..2_000 {
            let x = ((next() as u128) << 64) | (next() as u128);
            let r = reduce_mod_p_portable(x);
            // long division mod P in GF(2)
            let mut y = x;
            let p = (1u128 << 64) | SPANHASH_MOD_P_LOW as u128;
            while y != 0 && y >= (1u128 << 64) {
                let shift = (128 - y.leading_zeros()) - 65;
                y ^= p << shift;
            }
            assert_eq!(r, y as u64, "folding != long division");
        }
    }

    /// The lane-packing law: spanhash64_lanes == spanhash64 over the packed
    /// words (the crcfold/sink combine uses both forms).
    #[test]
    fn t_r24_lane_packing() {
        let lanes = [
            0x11223344u32,
            0x55667788,
            0x99AABBCC,
            0xDDEEFF00,
            0x01FE3254,
            0x6587A9CB,
            0xEDC1F079,
            0x86421357,
        ];
        let packed = [
            lanes[0] as u64 | ((lanes[1] as u64) << 32),
            lanes[2] as u64 | ((lanes[3] as u64) << 32),
            lanes[4] as u64 | ((lanes[5] as u64) << 32),
            lanes[6] as u64 | ((lanes[7] as u64) << 32),
        ];
        for len in [0usize, 1, 19, 1364, 65535, 1 << 20] {
            assert_eq!(
                spanhash64_lanes(&lanes, len),
                spanhash64(&packed, len as u32)
            );
        }
    }
}
