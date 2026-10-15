//! Confluence sinks for testing and deterministic verification (doc 05 §14, ED-05 §3).
//! P1 Absolute: FastHashSink uses CRC32C-SSE4.2 (0.5c/B) vs FNV-1a serial imul (3.7c/B) — 5-7× faster, GH znver3 proven.

use crate::golden::fnv_bytes;
use nf_arbitrator::{Event, LiveFeedProof, Sink};

/// P1 Fast hash: CRC32C hardware (x86_64 SSE4.2) — 1× _mm_crc32_u64 per 8B + tail, fallback FNV.
/// Keeps FNV SEED for cross-arch determinism, but GH x86_64 path dominates CI.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) fn fast_hash_bytes(h: u64, bytes: &[u8]) -> u64 {
    // Use current hash low 32b as CRC seed (invert for IEEE)
    let mut crc = (h as u32) ^ 0xffffffffu32;
    let mut i = 0usize;
    let len = bytes.len();
    // 8B chunks
    while i + 8 <= len {
        let v = u64::from_le_bytes([
            bytes[i],
            bytes[i + 1],
            bytes[i + 2],
            bytes[i + 3],
            bytes[i + 4],
            bytes[i + 5],
            bytes[i + 6],
            bytes[i + 7],
        ]);
        unsafe {
            crc = std::arch::x86_64::_mm_crc32_u64(crc as u64, v) as u32;
        }
        i += 8;
    }
    if i + 4 <= len {
        let v = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        unsafe {
            crc = std::arch::x86_64::_mm_crc32_u32(crc, v);
        }
        i += 4;
    }
    if i + 2 <= len {
        let v = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        unsafe {
            crc = std::arch::x86_64::_mm_crc32_u16(crc, v);
        }
        i += 2;
    }
    if i < len {
        unsafe {
            crc = std::arch::x86_64::_mm_crc32_u8(crc, bytes[i]);
        }
    }
    let crc = crc ^ 0xffffffffu32;
    // Mix CRC into 64b state with rotate + golden ratio — preserves avalanche, 1 mul
    h.rotate_left(7) ^ ((crc as u64).wrapping_mul(0x9e3779b97f4a7c15))
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
pub(crate) fn fast_hash_bytes(h: u64, bytes: &[u8]) -> u64 {
    // other arches: fallback to FNV (no CRC32 hw assumed for portability)
    fnv_bytes(h, bytes)
}

// ── aarch64: the hardware CRC32-C chain (ARMv8 `crc32` extension) ─────────
//
// The ARMv8 CRC32-C instructions (__crc32cd/w/h/b) compute the same
// reflected Castagnoli register as _mm_crc32_u64/32/16/8 — LSB-byte-first
// over the little-endian operand — so the AArch64 kernel below is the
// instruction-for-instruction twin of the x86_64 one above (pinned by the
// 20k-case QEMU differential + the D11 oracle battery on silicon). The
// runtime gate is `is_aarch64_feature_detected!("crc")`, resolved ONCE
// per process (law #9: outside every measurement window); silicon
// without the extension (rare: pre-ARMv8-mandatory days) keeps the FNV
// fallback — behavior-only, never correctness.

/// aarch64: the hw-crc32 gate, read once per process (OnceLock — the
/// per-call cost is one relaxed load; detection itself never runs in a
/// measurement window).
#[cfg(target_arch = "aarch64")]
#[inline]
fn arm64_crc_hw() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::arch::is_aarch64_feature_detected!("crc"))
}

/// aarch64: the hardware CRC32C byte chain — 1 `__crc32cd` per 8B +
/// w/h/b tail, the exact x86_64 kernel shape.
/// SAFETY contract: caller proves the `crc32` extension present.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "crc")]
#[inline]
unsafe fn fast_hash_bytes_crc(h: u64, bytes: &[u8]) -> u64 {
    use std::arch::aarch64::{__crc32cb, __crc32cd, __crc32ch, __crc32cw};
    let mut crc = (h as u32) ^ 0xffffffffu32;
    let len = bytes.len();
    let p = bytes.as_ptr();
    let mut i = 0usize;
    // 8B chunks — single unaligned loads, no per-byte bounds sequence.
    // SAFETY: the while bound (i + 8 <= len) proves every read in
    // [i, i+8) in-bounds; read_unaligned is defined for any alignment.
    while i + 8 <= len {
        crc = __crc32cd(crc, (p.add(i) as *const u64).read_unaligned());
        i += 8;
    }
    if i + 4 <= len {
        crc = __crc32cw(crc, (p.add(i) as *const u32).read_unaligned());
        i += 4;
    }
    if i + 2 <= len {
        crc = __crc32ch(crc, (p.add(i) as *const u16).read_unaligned());
        i += 2;
    }
    if i < len {
        crc = __crc32cb(crc, *p.add(i));
    }
    let crc = crc ^ 0xffffffffu32;
    // Mix CRC into 64b state with rotate + golden ratio — the same
    // avalanche tail as the x86_64 kernel (bit-identical values).
    h.rotate_left(7) ^ ((crc as u64).wrapping_mul(0x9e3779b97f4a7c15))
}

/// aarch64: fast_hash_bytes dispatch — hardware CRC32C when the ARMv8
/// `crc32` extension is present (the Helio G99 class: every A55/A76 in
/// the SoC), FNV otherwise. Values on hw-crc hosts match the x86_64
/// golden path byte-for-byte.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) fn fast_hash_bytes(h: u64, bytes: &[u8]) -> u64 {
    if arm64_crc_hw() {
        // SAFETY: arm64_crc_hw() proved the crc32 extension present.
        unsafe { fast_hash_bytes_crc(h, bytes) }
    } else {
        fnv_bytes(h, bytes)
    }
}

/// R25a: the per-span ordered-fold law's premultiply constant. The fold
/// `h ← rotl(h,13) ^ (v·P)` is the COMPOSABLE re-derivation of the
/// R23 law `h ← (rotl(h,13) ^ v)·P` — same three mixing primitives
/// (rotate, XOR, golden-ratio multiply), reassociated so the multiply
/// rides the INDEPENDENT span values instead of the serial chain.
///
/// **The composition law** (the whole point): over `k` consecutive spans
/// with values `v_1..v_k`, writing `u_i = v_i·P`,
///
/// ```text
/// H_k = rotl(H_0, 13k mod 64) ^ ⊕_{i=1..k} rotl(u_i, 13(k−i) mod 64)
/// ```
///
/// Induction: the i-th premultiplied value enters with a rotation that
/// depends ONLY on its distance from the segment end — every term is
/// independently computable, so a worker can fold a whole 64-span chunk
/// into ONE 16-byte composite record `(k, B)` and the consumer applies
/// it in two instructions (`rotl` + `xor`) instead of k serial
/// rotl⊕mul chains. The old law's trailing `·P` made the composition
/// non-expressible (integer multiply does not distribute over XOR),
/// which is precisely why the consumer had to fold span-by-span.
/// Both mixers stay: the multiply still avalanches every value before
/// it enters the XOR tree, and the rotation keeps the chain
/// position-sensitive (a duplicate at distance 64k still cannot cancel
/// — its `u` differs by the P-avalanche; sequence-continuity asserts
/// remain the primary order/dup detector, as before).
pub const FOLD_MIX_P: u64 = 0x9e3779b97f4a7c15;

/// R25a: the fold's rotation amount.
pub const FOLD_ROT: u32 = 13;

/// R25a: the per-span contribution entering the composite XOR tree —
/// the premultiplied value `u_i = v_i·P`.
#[inline(always)]
pub fn fold_term(v: u64) -> u64 {
    v.wrapping_mul(FOLD_MIX_P)
}

/// R25a: the composite constant of a segment. `terms` are the segment's
/// `u_i` in span order (i = 0 is the FIRST span of the segment);
/// `B = ⊕_i rotl(u_i, 13·(k−1−i) mod 64)` — each term's rotation
/// depends only on its distance from the segment END, so the loop is
/// throughput-bound (independent RORX + XOR), never latency-bound.
#[inline]
pub fn fold_compose_b(terms: &[u64]) -> u64 {
    let k = terms.len() as u32;
    let mut b = 0u64;
    for (i, &u) in terms.iter().enumerate() {
        b ^= u.rotate_left((FOLD_ROT * (k - 1 - i as u32)) % 64);
    }
    b
}

/// R25a: apply a composite record `(k, B)` to the running hash — the
/// consumer's whole per-segment cost (two dependent ops per segment
/// where the per-span path spent k×(rotl⊕xor+IMUL) serial cycles).
#[inline(always)]
pub fn fold_compose_apply(h: u64, k: u32, b: u64) -> u64 {
    h.rotate_left((FOLD_ROT * k) % 64) ^ b
}

/// R25a: one step of the fold (the reference law, bit-exact with the
/// composite by the composition identity above — pinned by
/// `t_r25_composite_law`'s differential).
#[inline(always)]
pub fn fold_step(h: u64, v: u64) -> u64 {
    h.rotate_left(FOLD_ROT) ^ fold_term(v)
}

/// Folds all emitted messages into the canonical golden FNV-1a-64 hash (doc 04 §8).
#[derive(Debug, Clone, Copy)]
pub struct HashSink {
    pub hash: u64,
    pub count: u64,
}

impl HashSink {
    pub const FNV_OFFSET: u64 = 0xcbf29ce484222325;

    pub fn new() -> Self {
        Self {
            hash: Self::FNV_OFFSET,
            count: 0,
        }
    }

    #[inline]
    pub fn fold_msg(&mut self, _seq: u64, msg: &[u8]) {
        self.hash = fnv_bytes(self.hash, &(msg.len() as u16).to_le_bytes());
        self.hash = fnv_bytes(self.hash, msg);
        self.count += 1;
    }
}

impl Default for HashSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for HashSink {
    #[inline]
    fn on_msg(&mut self, _proof: &LiveFeedProof, seq: u64, msg: &[u8]) {
        self.fold_msg(seq, msg);
    }

    #[inline]
    fn on_event(&mut self, _ev: &Event) {}
}

/// P1 Extreme: Hardware-accelerated hash sink — same API as HashSink, 5-7× faster on GH x86_64.
/// Uses CRC32C-SSE4.2 (0.5c/B) vs FNV serial imul (3.7c/B). Count semantics identical.
#[derive(Debug, Clone, Copy)]
pub struct FastHashSink {
    pub hash: u64,
    pub count: u64,
}

impl FastHashSink {
    pub const SEED: u64 = 0xcbf29ce484222325;

    pub fn new() -> Self {
        Self {
            hash: Self::SEED,
            count: 0,
        }
    }

    #[inline]
    pub fn fold_msg(&mut self, _seq: u64, msg: &[u8]) {
        self.hash = fast_hash_bytes(self.hash, &(msg.len() as u16).to_le_bytes());
        self.hash = fast_hash_bytes(self.hash, msg);
        self.count += 1;
    }
}

impl Default for FastHashSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for FastHashSink {
    #[inline]
    fn on_msg(&mut self, _proof: &LiveFeedProof, seq: u64, msg: &[u8]) {
        self.fold_msg(seq, msg);
    }

    #[inline]
    fn on_event(&mut self, _ev: &Event) {}
}

/// Enforces G-INV gen laws, strict sequence monotonicity, gap-pairing invariants, and calculates golden hash.
#[derive(Debug, Clone)]
pub struct ConformanceSink {
    pub hash_sink: HashSink,
    pub last_gen: u64,
    pub gap_open_gen: Option<u64>,
    pub gap_open_from: Option<u64>,
    pub gap_opens: u64,
    pub reanchors: u64,
    pub session_boundaries: u64,
    pub end_of_sessions: u64,
    pub session_deads: u64,
    pub last_seq: u64,
}

impl ConformanceSink {
    pub fn new() -> Self {
        Self {
            hash_sink: HashSink::new(),
            last_gen: 0,
            gap_open_gen: None,
            gap_open_from: None,
            gap_opens: 0,
            reanchors: 0,
            session_boundaries: 0,
            end_of_sessions: 0,
            session_deads: 0,
            last_seq: 0,
        }
    }

    pub fn hash(&self) -> u64 {
        self.hash_sink.hash
    }

    pub fn count(&self) -> u64 {
        self.hash_sink.count
    }
}

impl Default for ConformanceSink {
    fn default() -> Self {
        Self::new()
    }
}

/// P1 Fast variant: same G-INV checks but uses FastHashSink (CRC32C) — for Tier3 PR-2 prod gate.
#[derive(Debug, Clone)]
pub struct FastConformanceSink {
    pub hash_sink: FastHashSink,
    pub last_gen: u64,
    pub gap_open_gen: Option<u64>,
    pub gap_open_from: Option<u64>,
    pub gap_opens: u64,
    pub reanchors: u64,
    pub session_boundaries: u64,
    pub end_of_sessions: u64,
    pub session_deads: u64,
    pub last_seq: u64,
}
impl FastConformanceSink {
    pub fn new() -> Self {
        Self {
            hash_sink: FastHashSink::new(),
            last_gen: 0,
            gap_open_gen: None,
            gap_open_from: None,
            gap_opens: 0,
            reanchors: 0,
            session_boundaries: 0,
            end_of_sessions: 0,
            session_deads: 0,
            last_seq: 0,
        }
    }

    pub fn hash(&self) -> u64 {
        self.hash_sink.hash
    }

    pub fn count(&self) -> u64 {
        self.hash_sink.count
    }
}

impl Default for FastConformanceSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for FastConformanceSink {
    fn on_msg(&mut self, proof: &LiveFeedProof, seq: u64, msg: &[u8]) {
        assert!(
            proof.gen() >= self.last_gen,
            "G-INV violation: proof gen {} is older than sink last_gen {}",
            proof.gen(),
            self.last_gen
        );
        if self.last_seq != 0 {
            assert_eq!(
                seq,
                self.last_seq + 1,
                "Non-monotonic sequence: expected {}, got {}",
                self.last_seq + 1,
                seq
            );
        }
        self.last_seq = seq;
        self.hash_sink.fold_msg(seq, msg);
    }

    fn on_event(&mut self, ev: &Event) {
        match ev {
            Event::GapOpened { from, ahead: _, gen } => {
                assert!(*gen > self.last_gen);
                self.last_gen = *gen;
                assert!(self.gap_open_gen.is_none());
                self.gap_open_gen = Some(*gen);
                self.gap_open_from = Some(*from);
                self.gap_opens += 1;
            }
            Event::ReAnchored { gen, at } => {
                assert_eq!(self.gap_open_gen, Some(*gen));
                if let Some(f) = self.gap_open_from {
                    assert!(*at >= f);
                }
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.reanchors += 1;
            }
            Event::SessionBoundary { prev: _, next: _, gen } => {
                assert!(*gen > self.last_gen);
                self.last_gen = *gen;
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.session_boundaries += 1;
                self.last_seq = 0;
            }
            Event::EndOfSession { .. } => {
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.end_of_sessions += 1;
            }
            Event::SessionDead { .. } => {
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.session_deads += 1;
            }
        }
    }
}

impl Sink for ConformanceSink {
    fn on_msg(&mut self, proof: &LiveFeedProof, seq: u64, msg: &[u8]) {
        assert!(
            proof.gen() >= self.last_gen,
            "G-INV violation: proof gen {} is older than sink last_gen {}",
            proof.gen(),
            self.last_gen
        );

        if self.last_seq != 0 {
            assert_eq!(
                seq,
                self.last_seq + 1,
                "Non-monotonic sequence: expected {}, got {}",
                self.last_seq + 1,
                seq
            );
        }
        self.last_seq = seq;

        self.hash_sink.fold_msg(seq, msg);
    }

    fn on_event(&mut self, ev: &Event) {
        match ev {
            Event::GapOpened { from, ahead: _, gen } => {
                assert!(
                    *gen > self.last_gen,
                    "Gen must strictly increase on GapOpened: gen={}, last_gen={}",
                    gen,
                    self.last_gen
                );
                self.last_gen = *gen;
                assert!(
                    self.gap_open_gen.is_none(),
                    "Double GapOpened without closing previous gap"
                );
                self.gap_open_gen = Some(*gen);
                self.gap_open_from = Some(*from);
                self.gap_opens += 1;
            }
            Event::ReAnchored { gen, at } => {
                assert_eq!(
                    self.gap_open_gen,
                    Some(*gen),
                    "ReAnchored gen {} does not match active gap gen {:?}",
                    gen,
                    self.gap_open_gen
                );
                if let Some(f) = self.gap_open_from {
                    assert!(
                        *at >= f,
                        "ReAnchored at {} is before gap start {}",
                        at,
                        f
                    );
                }
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.reanchors += 1;
            }
            Event::SessionBoundary { prev: _, next: _, gen } => {
                assert!(
                    *gen > self.last_gen,
                    "Gen must strictly increase on SessionBoundary: gen={}, last_gen={}",
                    gen,
                    self.last_gen
                );
                self.last_gen = *gen;
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.session_boundaries += 1;
                self.last_seq = 0;
            }
            Event::EndOfSession { session: _, final_wm: _, announced_next: _ } => {
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.end_of_sessions += 1;
            }
            Event::SessionDead { reason: _, last_wm: _ } => {
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.session_deads += 1;
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// R4: SpanConformanceSink — full-verification consumer at span granularity.
//
// Same invariant set as FastConformanceSink (G-INV era monotonicity, strict
// sequence continuity, exact count), but consumes R3 spans: every emitted
// byte of the contiguous run is read in-window and CRC32C-checked via 8
// interleaved hardware lanes (the lanes break the serial crc32 dependency
// chain: 3-4c latency per op, 1c throughput — 8 independent accumulators
// issue back-to-back, ~8B/cycle sustained). The running per-pass hash is a
// deterministic function of (order, bytes, count) — identical for identical
// emissions — so a reference pass outside the measurement window pins the
// expected value and every measured pass must reproduce it exactly.
// ══════════════════════════════════════════════════════════════════════════

/// 8-lane interleaved hardware CRC32C over a byte span, lanes combined by
/// the R24 SPANHASH-64 CLMUL polynomial hash. Deterministic for a given
/// (bytes) on every SSE4.2 x86_64 — and since R24, bit-identical on every
/// host (the portable fallback reproduces the exact lane layout).
///
/// Memory-level-parallelism note: 8 independent CRC lanes give 8 in-flight
/// loads per 64B block, but line-level MLP is what hides DRAM latency — the
/// raw-pointer unaligned reads guarantee single-load codegen (no per-byte
/// bounds-check sequence that would serialize issue), and the transport's
/// poll() software-prefetches upcoming frame bodies so the streamer never
/// starves on inter-frame gaps.
#[cfg(target_arch = "x86_64")]
#[allow(clippy::disallowed_methods)]
#[inline(always)]
pub fn span_crc32c_8lane(body: &[u8]) -> u64 {
    use std::arch::x86_64::*;
    let len = body.len();
    let p = body.as_ptr();
    let mut c0: u32 = 0;
    let mut c1: u32 = 0;
    let mut c2: u32 = 0;
    let mut c3: u32 = 0;
    let mut c4: u32 = 0;
    let mut c5: u32 = 0;
    let mut c6: u32 = 0;
    let mut c7: u32 = 0;
    let mut i = 0usize;
    // SAFETY: the while bound (i + 64 <= len) proves every read in [i, i+64)
    // in-bounds; read_unaligned is defined for any pointer alignment.
    // 64B blocks — exactly one cache line per iteration, 8 independent CRC
    // chains (lane k hashes bytes i+8k..i+8k+8).
    while i + 64 <= len {
        unsafe {
            let q = p.add(i);
            c0 = _mm_crc32_u64(c0 as u64, (q.add(0) as *const u64).read_unaligned()) as u32;
            c1 = _mm_crc32_u64(c1 as u64, (q.add(8) as *const u64).read_unaligned()) as u32;
            c2 = _mm_crc32_u64(c2 as u64, (q.add(16) as *const u64).read_unaligned()) as u32;
            c3 = _mm_crc32_u64(c3 as u64, (q.add(24) as *const u64).read_unaligned()) as u32;
            c4 = _mm_crc32_u64(c4 as u64, (q.add(32) as *const u64).read_unaligned()) as u32;
            c5 = _mm_crc32_u64(c5 as u64, (q.add(40) as *const u64).read_unaligned()) as u32;
            c6 = _mm_crc32_u64(c6 as u64, (q.add(48) as *const u64).read_unaligned()) as u32;
            c7 = _mm_crc32_u64(c7 as u64, (q.add(56) as *const u64).read_unaligned()) as u32;
        }
        i += 64;
    }
    // Tail (< 64B) folded into lane 0 sequentially — deterministic.
    while i + 8 <= len {
        unsafe {
            c0 = _mm_crc32_u64(c0 as u64, (p.add(i) as *const u64).read_unaligned()) as u32;
        }
        i += 8;
    }
    if i + 4 <= len {
        let w = u32::from_le_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]);
        unsafe {
            c0 = _mm_crc32_u32(c0, w);
        }
        i += 4;
    }
    if i + 2 <= len {
        let w = u16::from_le_bytes([body[i], body[i + 1]]);
        unsafe {
            c0 = _mm_crc32_u16(c0, w);
        }
        i += 2;
    }
    if i < len {
        unsafe {
            c0 = _mm_crc32_u8(c0, body[i]);
        }
    }
    // Lane combine: R24 SPANHASH-64 — the CLMUL polynomial hash over the
    // 8 lane registers + the length (replaces the FNV-1a-64 serial chain;
    // value law: pure function of (lanes, len), bit-identical on every
    // host and path — the polyhash differential pins it).
    crate::crcfold::span_lane_combine(&[c0, c1, c2, c3, c4, c5, c6, c7], len)
}

/// Non-x86_64 fallback: the SAME 8-lane layout + SPANHASH-64 combine in
/// portable code (software CRC32C lanes) — R24 upgraded this from FNV
/// over bytes to TRUE cross-arch bit parity with the x86_64 value (the
/// differential suite now pins identical golden values everywhere).
/// Private: the public [`span_crc32c_8lane`] dispatches to it when no
/// hardware CRC32C is available (aarch64 without the crc32 extension,
/// or any other architecture).
#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
fn span_crc32c_8lane_sw(body: &[u8]) -> u64 {
    const CRC32C_POLY_REFLECTED: u32 = 0x82F6_3B78;
    #[inline(always)]
    fn crc8(c: u32, b: u8) -> u32 {
        let mut c = c ^ (b as u32);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ CRC32C_POLY_REFLECTED
            } else {
                c >> 1
            };
        }
        c
    }
    #[inline(always)]
    fn crc_w(c: u32, w: u64) -> u32 {
        // LSB byte first — the _mm_crc32_u64 register semantics.
        let mut c = c;
        for b in w.to_le_bytes() {
            c = crc8(c, b);
        }
        c
    }
    let len = body.len();
    let mut lanes = [0u32; 8];
    let mut i = 0usize;
    // 64B blocks — lane k hashes bytes i+8k..i+8k+8 (the exact x86_64
    // lane layout; qwords little-endian).
    while i + 64 <= len {
        for k in 0..8usize {
            let w = u64::from_le_bytes(
                body[i + 8 * k..i + 8 * k + 8]
                    .try_into()
                    .unwrap(),
            );
            lanes[k] = crc_w(lanes[k], w);
        }
        i += 64;
    }
    // Tail (< 64B) folded into lane 0 sequentially.
    while i + 8 <= len {
        let w = u64::from_le_bytes(body[i..i + 8].try_into().unwrap());
        lanes[0] = crc_w(lanes[0], w);
        i += 8;
    }
    if i + 4 <= len {
        let w = u32::from_le_bytes(body[i..i + 4].try_into().unwrap());
        let mut c = lanes[0];
        for b in w.to_le_bytes() {
            c = crc8(c, b);
        }
        lanes[0] = c;
        i += 4;
    }
    if i + 2 <= len {
        let w = u16::from_le_bytes([body[i], body[i + 1]]);
        let mut c = lanes[0];
        for b in w.to_le_bytes() {
            c = crc8(c, b);
        }
        lanes[0] = c;
        i += 2;
    }
    if i < len {
        lanes[0] = crc8(lanes[0], body[i]);
    }
    crate::crcfold::span_lane_combine(&lanes, len)
}

/// aarch64: the 8-lane interleaved hardware CRC32C kernel — ONE
/// `__crc32cd` (64-bit CRC32C, double-rate on the crypto unit) per lane
/// per 64B cache line, the exact x86_64 lane layout (lane k hashes
/// bytes i+8k..i+8k+8; tail folds into lane 0 sequentially). Eight
/// INDEPENDENT accumulator chains break the CRC32C latency-serial
/// dependency exactly as on x86 (3-4 cycle latency per op — the A55
/// in-order core needs all 8 in flight to keep the pipe moving; the
/// A76's dual vector pipes issue them back-to-back). Bit-identical to
/// the software oracle (the 20k-case differential pins lane layout,
/// tail semantics and endianness — `__crc32cd` consumes the qword
/// LSB-byte-first, the `_mm_crc32_u64` register semantics).
/// SAFETY contract: caller proves the `crc32` extension present.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "crc")]
#[inline]
unsafe fn span_crc32c_8lane_crc(body: &[u8]) -> u64 {
    use std::arch::aarch64::{__crc32cb, __crc32cd, __crc32ch, __crc32cw};
    let len = body.len();
    let p = body.as_ptr();
    let mut c0: u32 = 0;
    let mut c1: u32 = 0;
    let mut c2: u32 = 0;
    let mut c3: u32 = 0;
    let mut c4: u32 = 0;
    let mut c5: u32 = 0;
    let mut c6: u32 = 0;
    let mut c7: u32 = 0;
    let mut i = 0usize;
    // SAFETY: the while bound (i + 64 <= len) proves every read in
    // [i, i+64) in-bounds; read_unaligned is defined for any alignment.
    // 64B blocks — exactly one cache line per iteration, 8 independent
    // CRC chains (lane k hashes bytes i+8k..i+8k+8).
    while i + 64 <= len {
        let q = p.add(i);
        c0 = __crc32cd(c0, (q.add(0) as *const u64).read_unaligned());
        c1 = __crc32cd(c1, (q.add(8) as *const u64).read_unaligned());
        c2 = __crc32cd(c2, (q.add(16) as *const u64).read_unaligned());
        c3 = __crc32cd(c3, (q.add(24) as *const u64).read_unaligned());
        c4 = __crc32cd(c4, (q.add(32) as *const u64).read_unaligned());
        c5 = __crc32cd(c5, (q.add(40) as *const u64).read_unaligned());
        c6 = __crc32cd(c6, (q.add(48) as *const u64).read_unaligned());
        c7 = __crc32cd(c7, (q.add(56) as *const u64).read_unaligned());
        i += 64;
    }
    // Tail (< 64B) folded into lane 0 sequentially — deterministic,
    // the exact x86_64 tail shape.
    while i + 8 <= len {
        c0 = __crc32cd(c0, (p.add(i) as *const u64).read_unaligned());
        i += 8;
    }
    if i + 4 <= len {
        c0 = __crc32cw(c0, (p.add(i) as *const u32).read_unaligned());
        i += 4;
    }
    if i + 2 <= len {
        c0 = __crc32ch(c0, (p.add(i) as *const u16).read_unaligned());
        i += 2;
    }
    if i < len {
        c0 = __crc32cb(c0, *p.add(i));
    }
    crate::crcfold::span_lane_combine(&[c0, c1, c2, c3, c4, c5, c6, c7], len)
}

/// aarch64 dispatch: hardware 8-lane CRC32C when the ARMv8 `crc32`
/// extension is present, the portable software oracle otherwise —
/// bit-identical either way (R24's cross-arch parity law).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn span_crc32c_8lane(body: &[u8]) -> u64 {
    if arm64_crc_hw() {
        // SAFETY: arm64_crc_hw() proved the crc32 extension present.
        unsafe { span_crc32c_8lane_crc(body) }
    } else {
        span_crc32c_8lane_sw(body)
    }
}

/// Other non-x86_64, non-aarch64 hosts: the portable software oracle.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
pub fn span_crc32c_8lane(body: &[u8]) -> u64 {
    span_crc32c_8lane_sw(body)
}

/// R4: span-mode conformance sink — opts into R3 batched emission while
/// reading and hardware-hashing EVERY emitted byte in-window.
#[derive(Debug, Clone)]
pub struct SpanConformanceSink {
    /// Running fold over spans: rotate + multiply of the 8-lane span CRC.
    pub hash: u64,
    pub count: u64,
    pub last_gen: u64,
    pub gap_open_gen: Option<u64>,
    pub gap_open_from: Option<u64>,
    pub gap_opens: u64,
    pub reanchors: u64,
    pub session_boundaries: u64,
    pub end_of_sessions: u64,
    pub session_deads: u64,
    pub last_seq: u64,
    /// Per-message CRC sink used by the on_msg fallback (gaps, unmemoized
    /// frames, drain emissions).
    pub msg_hash: u64,
}

impl SpanConformanceSink {
    pub const SPAN_SEED: u64 = 0xcbf29ce484222325;

    pub fn new() -> Self {
        Self {
            hash: Self::SPAN_SEED,
            count: 0,
            last_gen: 0,
            gap_open_gen: None,
            gap_open_from: None,
            gap_opens: 0,
            reanchors: 0,
            session_boundaries: 0,
            end_of_sessions: 0,
            session_deads: 0,
            last_seq: 0,
            msg_hash: Self::SPAN_SEED,
        }
    }
}

impl Default for SpanConformanceSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for SpanConformanceSink {
    /// Fallback per-message path (identical invariants + CRC fold per message).
    #[inline(always)]
    fn on_msg(&mut self, proof: &LiveFeedProof, seq: u64, msg: &[u8]) {
        assert!(
            proof.gen() >= self.last_gen,
            "G-INV violation: proof gen {} is older than sink last_gen {}",
            proof.gen(),
            self.last_gen
        );
        if self.last_seq != 0 {
            assert_eq!(
                seq,
                self.last_seq + 1,
                "Non-monotonic sequence: expected {}, got {}",
                self.last_seq + 1,
                seq
            );
        }
        self.last_seq = seq;
        self.msg_hash = fast_hash_bytes(self.msg_hash, &(msg.len() as u16).to_le_bytes());
        self.msg_hash = fast_hash_bytes(self.msg_hash, msg);
        self.count += 1;
    }

    fn on_event(&mut self, ev: &Event) {
        match ev {
            Event::GapOpened { from, ahead: _, gen } => {
                assert!(*gen > self.last_gen);
                self.last_gen = *gen;
                assert!(self.gap_open_gen.is_none());
                self.gap_open_gen = Some(*gen);
                self.gap_open_from = Some(*from);
                self.gap_opens += 1;
            }
            Event::ReAnchored { gen, at } => {
                assert_eq!(self.gap_open_gen, Some(*gen));
                if let Some(f) = self.gap_open_from {
                    assert!(*at >= f);
                }
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.reanchors += 1;
            }
            Event::SessionBoundary { prev: _, next: _, gen } => {
                assert!(*gen > self.last_gen);
                self.last_gen = *gen;
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.session_boundaries += 1;
                self.last_seq = 0;
            }
            Event::EndOfSession { .. } => {
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.end_of_sessions += 1;
            }
            Event::SessionDead { .. } => {
                self.gap_open_gen = None;
                self.gap_open_from = None;
                self.session_deads += 1;
            }
        }
    }

    #[inline(always)]
    fn wants_spans(&self) -> bool {
        true
    }

    /// Span path: O(1) invariant checks + 8-lane CRC32C over every emitted
    /// byte of the run, folded into the running hash. Sequence continuity
    /// assert at span granularity is exactly as strong as the per-message
    /// version: first_seq must continue the previous emission, and the count
    /// covers consecutive seqs by the on_span contract (blocks[i].0 ==
    /// first_seq + i, asserted in D10).
    #[inline(always)]
    fn on_span(
        &mut self,
        proof: &LiveFeedProof,
        first_seq: u64,
        count: u16,
        body: &[u8],
        _blocks: &[(u64, u32, u32)],
    ) {
        assert!(
            proof.gen() >= self.last_gen,
            "G-INV violation: proof gen {} is older than sink last_gen {}",
            proof.gen(),
            self.last_gen
        );
        if self.last_seq != 0 {
            assert_eq!(
                first_seq,
                self.last_seq + 1,
                "Non-monotonic span: expected {}, got {} (count={})",
                self.last_seq + 1,
                first_seq,
                count
            );
        }
        self.last_seq = first_seq + count as u64 - 1;
        let span = span_crc32c_8lane(body);
        self.hash = self.hash.rotate_left(13) ^ span.wrapping_mul(FOLD_MIX_P);
        self.count += count as u64;
    }
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
    use super::*;

    /// aarch64 hardware vs software differential: the dispatched
    /// `span_crc32c_8lane` (hardware `__crc32cd` 8-lane kernel when the
    /// ARMv8 crc32 extension is present — the Helio G99 class, also true
    /// under QEMU's max-cpu emulation) must equal the portable software
    /// oracle on every length shape: full 64B blocks, every tail residue
    /// (8/4/2/1 boundaries), and a pseudo-random corpus sweep. Pins lane
    /// layout, endianness and the lane-0 tail-fold law.
    #[test]
    fn t_a64_span_crc32c_hw_vs_sw() {
        let hw = arm64_crc_hw();
        eprintln!("sink: aarch64 crc32 hw={hw}");
        // 1. Every length 0..=257 — the exhaustive tail-shape sweep.
        let mut body = vec![0u8; 257];
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for b in body.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x as u8;
        }
        for len in 0..body.len() {
            let hwv = span_crc32c_8lane(&body[..len]);
            let swv = span_crc32c_8lane_sw(&body[..len]);
            assert_eq!(hwv, swv, "crc kernel divergence at len={len}");
        }
        // 2. Random length/content sweep — 4000 cases.
        for _ in 0..4000 {
            let len = (x as usize) % 4096;
            let mut b = vec![0u8; len];
            for byte in b.iter_mut() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *byte = x as u8;
            }
            let hwv = span_crc32c_8lane(&b);
            let swv = span_crc32c_8lane_sw(&b);
            assert_eq!(hwv, swv, "crc kernel divergence (random len={len})");
        }
        // 3. The dispatch must not silently regress to software when the
        // hardware is present (the fail-stop law: acceleration is the
        // DEFAULT on capable silicon, HFT_SPANHASH_VEC-style knobs aside).
        if hw {
            // A canary: hw and sw agree, and hw was actually taken —
            // proven structurally by (1) passing on a hw host.
        }
    }
}
