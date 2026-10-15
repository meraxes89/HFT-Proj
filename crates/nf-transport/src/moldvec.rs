//! R30: the VECTORIZED MoldUDP64 header decode — nf-transport's SIMD twin
//! of `nf_protocol::moldudp64::parse_header`.
//!
//! WHY HERE: `nf-protocol` is `#![forbid(unsafe_code)]` — the pure protocol
//! law (audit-grade, zero unsafe in the grammar layer) — so the vector
//! decode cannot live beside the scalar law it mirrors. nf-transport is
//! the workspace's ingest-SIMD layer (the AVX-512 array commit lives in
//! `rxdesc.rs`); the twin lives here as a SAFE API (the unsafe is confined
//! to this module) for every transport/recovery-side consumer that decodes
//! 20-byte headers in a loop. The sustained arm's per-frame validation
//! (the sequencer's SoA ladder) is already vectorized; this module serves
//! the eager-validation shapes (`parse`-equivalent consumers on the
//! transport side) — one 256-bit load + two byte-shuffles replace the
//! scalar per-byte assemble (8 loads + shifts + ORs for the seq alone).
//!
//! BIT-IDENTICAL to the scalar law on every input: the differential cell
//! (`t_moldvec_header_parity`) pins 10k random headers plus the
//! lane-split byte patterns.
//!
//! Grammar law: docs/02-moldudp64.md. Verify claims there before relying.

use nf_protocol::moldudp64::{Header, SessionId, HEADER_LEN};

/// R30: the AVX2 gate, read once per process (the r25c OnceLock pattern —
/// `is_x86_feature_detected!` caches internally, but the OnceLock keeps
/// the branch data local to this module's call sites).
#[cfg(target_arch = "x86_64")]
#[inline]
fn avx2_enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::arch::is_x86_feature_detected!("avx2"))
}

/// R30: the vectorized 20-byte header decode for WIDE frames
/// (`buf.len() >= 32` — every multi-block data frame; heartbeat/EOS and
/// stubby frames are exactly 20 bytes and keep the scalar law in
/// `nf_protocol::moldudp64::parse_header`). Returns `None` when the frame
/// is too short or the silicon lacks AVX2 — callers fall back to the
/// scalar reference (the twin law: identical results either way).
///
/// The decode: ONE `_mm256_loadu_si256` over the header's first 32
/// bytes; `_mm_shuffle_epi8` gathers the seq's eight big-endian bytes
/// (split across the two 128-bit lanes, with the low six in lane 0 and
/// the high two in lane 1) into one register's low qword, and the
/// count's two bytes into another's low word. Two shuffles, one OR,
/// two register extracts.
///
#[cfg(target_arch = "x86_64")]
pub fn parse_header_wide(buf: &[u8]) -> Option<Header> {
    if buf.len() < 32 || !avx2_enabled() {
        return None;
    }
    Some(unsafe { parse_header_vec(buf) })
}

/// The non-x86_64 twin: never armed (the scalar law serves every frame).
#[cfg(not(target_arch = "x86_64"))]
pub fn parse_header_wide(_buf: &[u8]) -> Option<Header> {
    None
}

/// The raw-fields form (seq, count) — the transport-side fast extract for
/// callers that hold the session in template words already (the scan's
/// sess_lo/sess_hi pattern). Same gate contract as `parse_header_wide`.
#[cfg(target_arch = "x86_64")]
pub fn decode_seq_count_wide(buf: &[u8]) -> Option<(u64, u16)> {
    if buf.len() < 32 || !avx2_enabled() {
        return None;
    }
    let h = unsafe { parse_header_vec(buf) };
    Some((h.seq, h.count))
}

/// The non-x86_64 twin: never armed.
#[cfg(not(target_arch = "x86_64"))]
pub fn decode_seq_count_wide(_buf: &[u8]) -> Option<(u64, u16)> {
    None
}

/// R30: the vector decode body. SAFETY: callers prove `buf.len() >= 32`
/// (32 bytes readable at `buf.as_ptr()`) and AVX2 detected at the gate.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn parse_header_vec(buf: &[u8]) -> Header {
    use std::arch::x86_64::*;
    // SAFETY: len >= 32 proven by the caller; the load is unaligned-safe.
    let v = _mm256_loadu_si256(buf.as_ptr() as *const __m256i);
    // Lane 0 carries frame bytes 0..15 (session 0..10 + seq bytes
    // 10..15); lane 1 carries bytes 16..31 (seq bytes 16..17 + count
    // 18..19 + payload head).
    // SAFETY: AVX2 detected; casts/extracts are register moves.
    let lo = _mm256_castsi256_si128(v);
    let hi = _mm256_extracti128_si256(v, 1);
    // seq = BE(buf[10..18]): the LE qword's bytes are
    // [b17, b16, b15, b14, b13, b12, b11, b10] — b10..b15 from lane 0
    // (dest bytes 2..7), b16..b17 from lane 1 (dest bytes 0..1). The
    // OR merges the disjoint halves.
    // SAFETY: AVX2 detected; both operands initialized.
    let a = _mm_shuffle_epi8(
        lo,
        _mm_setr_epi8(-1, -1, 15, 14, 13, 12, 11, 10, -1, -1, -1, -1, -1, -1, -1, -1),
    );
    let b = _mm_shuffle_epi8(
        hi,
        _mm_setr_epi8(1, 0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
    );
    // SAFETY: pure register op.
    let seqv = _mm_or_si128(a, b);
    // SAFETY: the low qword was fully defined by the two shuffles.
    let seq = _mm_cvtsi128_si64(seqv) as u64;
    // count = BE(buf[18..19]): the LE word's bytes are [b19, b18] —
    // both from lane 1 (dest bytes 0..1).
    // SAFETY: AVX2 detected; operand initialized.
    let c = _mm_shuffle_epi8(
        hi,
        _mm_setr_epi8(3, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
    );
    // SAFETY: the low word was fully defined by the shuffle.
    let count = (_mm_cvtsi128_si64(c) as u64) as u16;
    let mut session: SessionId = [0u8; 10];
    session.copy_from_slice(&buf[0..10]);
    Header { session, seq, count }
}

/// The scalar reference used by the differential cell (the exact
/// `nf_protocol::moldudp64::parse_header` law, restated for local
/// comparison — the protocol crate's own fn is the normative source).
#[allow(dead_code)]
pub(crate) fn parse_header_scalar_ref(buf: &[u8]) -> Option<Header> {
    if buf.len() < HEADER_LEN {
        return None;
    }
    nf_protocol::moldudp64::parse_header(buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R30: the vectorized header decode's differential law — 10k random
    /// wide frames (len >= 32, the vector gate's domain) through
    /// `parse_header_wide` (the AVX2 shuffle extract when the silicon has
    /// it) must equal the scalar law bit-for-bit; the short-frame shapes
    /// (20, 31 — HB/EOS and stubby data) must fall back to `None` (the
    /// caller serves them the scalar path). Recognizable fields (seq
    /// across the lane split at bytes 10..18, count at 18..20) pin the
    /// shuffle masks' exact placement on every trial.
    #[test]
    fn t_moldvec_header_parity() {
        // A tiny xorshift — deterministic, allocation-free.
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut vec_hits = 0u32;
        for trial in 0..10_000u32 {
            let len = 32 + (next() % 64) as usize;
            let mut buf = vec![0u8; len];
            for b in buf.iter_mut() {
                *b = (next() >> 24) as u8;
            }
            let seq = next();
            buf[10..18].copy_from_slice(&seq.to_be_bytes());
            let count = (next() % 0x1_0000) as u16;
            buf[18..20].copy_from_slice(&count.to_be_bytes());
            let want = parse_header_scalar_ref(&buf).expect("scalar reference decodes");
            match parse_header_wide(&buf) {
                Some(got) => {
                    vec_hits += 1;
                    assert_eq!(got, want, "R30 vec/scalar divergence at trial {trial}");
                    assert_eq!(got.seq, seq, "R30 vec seq at trial {trial}");
                    assert_eq!(got.count, count, "R30 vec count at trial {trial}");
                    let (s2, c2) =
                        decode_seq_count_wide(&buf).expect("raw-fields twin decodes");
                    assert_eq!((s2, c2), (seq, count), "R30 raw twin at trial {trial}");
                }
                None => {
                    // The gate's fallback domain (no AVX2 here) — the
                    // scalar law still serves the frame.
                }
            }
        }
        // On AVX2 silicon the vector path must have engaged (the gate is
        // process-wide; either every wide frame took it or none did).
        #[cfg(target_arch = "x86_64")]
        {
            let armed = avx2_enabled();
            assert_eq!(
                vec_hits > 0,
                armed,
                "R30 gate engagement mismatch (armed={armed}, hits={vec_hits})"
            );
        }
        // The short-frame fallback law: 20 (HB/EOS width) and 31.
        for len in [20usize, 31] {
            let buf = vec![0xA5u8; len];
            assert!(
                parse_header_wide(&buf).is_none(),
                "R30 short frame must fall back (len={len})"
            );
            assert!(
                parse_header_scalar_ref(&buf).is_some(),
                "R30 scalar serves short frames (len={len})"
            );
        }
    }
}
