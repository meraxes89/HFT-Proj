//! R24 (Run 500+ Track 4) — the homomorphic LOB state monoid: O(1)
//! order-book window resolution without per-message state mutation.
//!
//! # The physics this module exists for
//!
//! Verifying ITCH messages at multi-billion msg/s is only half the
//! pipeline: the moment the consumer APPLIES messages to a Limit Order
//! Book, the sequential dependency `S_i = f(S_{i-1}, m_i)` (AddOrder ->
//! OrderExecuted -> OrderCancel) collapses the parallel fabric — every
//! standard LOB mutates shared memory state per message, through a
//! pointer-chased hash table of order references.
//!
//! # The mathematical re-derivation
//!
//! Represent the aggregate book state at a tracked price level (or the
//! top-of-book aggregate) as a vector over Z^3:
//!
//! ```text
//! S = (V, PV, N)   volume, price*volume, order count   (VWAP = PV / V)
//! ```
//!
//! Every ITCH order event is a PURE TRANSLATION of S:
//!
//! ```text
//! Add(px, qty)    ->  S + (+qty, +qty*px, +1)
//! Exec(px, qty)   ->  S + (-qty, -qty*px, -1)
//! Cancel(px, qty) ->  S + (-qty, -qty*px, -1)
//! ```
//!
//! The translation group of Z^3 is COMMUTATIVE and ASSOCIATIVE — the
//! composite transition of a 64-msg window is the plain SUM of the
//! window's deltas, computable as a TREE REDUCTION of INDEPENDENT adds
//! (VPADDQ-class, 8 lanes per instruction) and applied ONCE at the
//! window's end:
//!
//! ```text
//! S_window_end = S_window_start + D_window        (O(1) apply)
//! D_window     = sum(deltas)                      (tree, no serial state)
//! ```
//!
//! Two workers fold disjoint halves of a burst and compose by addition —
//! the exact shape the R23 affine CRC law gave the verifier, now for
//! book state. Per-message book mutation (and the order-ref hash walk)
//! disappears from the measured window; the price resolution for
//! Exec/Cancel rides the PRODUCER's order-reference sidecar (built in
//! the same untimed pass as the affine ledger), never in the fold.
//!
//! # The generalization (the affine matrix monoid)
//!
//! Translations are the degenerate case of AFFINE MAPS x -> Ax + b
//! (Z^4 homogeneous coordinates, bottom row [0 0 0 1]). The full
//! [`Affine4`] monoid composes by matrix multiplication and covers
//! non-translation transitions (percentage fees, stock splits, the
//! top-K level migrations) at the cost of 4x4 integer products — the
//! scalar generalization, kept for the classes the translation law
//! cannot express. The tropical (max-plus) variant — best-bid tracking
//! under the (max, +) semiring — is the [`best_px`] scan.
//!
//! # Determinism law
//!
//! All folds are u64 WRAPPING arithmetic: addition is associative and
//! commutative even under wrap, so every reduction order (scalar loop,
//! tree, VPADDQ) is bit-identical — the golden-value law the rest of
//! the engine obeys. The differential batteries pin this.

use std::arch::is_x86_feature_detected;

/// One order-book event (the DECODED form — ITCH bytes are decoded by
/// the producer's untimed pass; exec/cancel prices resolve through the
/// order-reference sidecar there, keeping the fold free of pointer
/// chases).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobKind {
    Add,
    Exec,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobEvent {
    pub kind: LobKind,
    /// Price in ticks (u32 — the ITCH 32-bit fixed-point field).
    pub px: u32,
    /// Quantity in shares.
    pub qty: u32,
}

/// The aggregate book state vector (V, PV, N).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobState {
    /// Total resting volume (shares) at the tracked level.
    pub volume: u64,
    /// Sum of price*qty (the VWAP numerator, in tick*shares).
    pub pv: u64,
    /// Live order count.
    pub count: u64,
}

impl LobState {
    /// Apply ONE event (the sequential reference law, S += delta).
    #[inline(always)]
    pub fn apply(&mut self, e: LobEvent) {
        let (dv, dpv, dn) = event_delta(e);
        self.volume = self.volume.wrapping_add(dv);
        self.pv = self.pv.wrapping_add(dpv);
        self.count = self.count.wrapping_add(dn);
    }

    /// VWAP in ticks (None on zero volume).
    #[inline]
    pub fn vwap_ticks(&self) -> Option<u64> {
        if self.volume == 0 {
            None
        } else {
            Some(self.pv / self.volume)
        }
    }
}

/// The translation delta of one event: (dV, dPV, dN).
#[inline(always)]
pub fn event_delta(e: LobEvent) -> (u64, u64, u64) {
    let q = e.qty as u64;
    let pqp = (e.px as u64).wrapping_mul(q);
    match e.kind {
        LobKind::Add => (q, pqp, 1),
        LobKind::Exec | LobKind::Cancel => (q.wrapping_neg(), pqp.wrapping_neg(), u64::MAX),
    }
}

/// The window composite: the SUM of an event batch's deltas — the
/// homomorphism's image (fold the batch, apply ONCE). Pure u64 wrapping
/// adds: every reduction order is bit-identical.
pub fn batch_compose(events: &[LobEvent]) -> (u64, u64, u64) {
    batch_compose_impl(events)
}

/// The AVX-512 VPADDQ batch fold (8 events per instruction, 3 arrays),
/// with the portable scalar loop as the everywhere-else path —
/// bit-identical (wrapping u64 addition is order-free).
fn batch_compose_impl(events: &[LobEvent]) -> (u64, u64, u64) {
    #[cfg(target_arch = "x86_64")]
    {
        if events.len() >= 32 && is_x86_feature_detected!("avx512f") {
            // SAFETY: the AVX-512F class was just detected.
            return unsafe { batch_compose_vec(events) };
        }
    }
    let (mut dv, mut dpv, mut dn) = (0u64, 0u64, 0u64);
    for &e in events {
        let (a, b, c) = event_delta(e);
        dv = dv.wrapping_add(a);
        dpv = dpv.wrapping_add(b);
        dn = dn.wrapping_add(c);
    }
    (dv, dpv, dn)
}

/// The VPADDQ fold: 8-event blocks, three 512-bit lane accumulators
/// (V, PV, N), horizontal-reduced once at the end.
/// SAFETY: caller proves AVX-512F.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn batch_compose_vec(events: &[LobEvent]) -> (u64, u64, u64) {
    use std::arch::x86_64::*;
    let mut acc_v = _mm512_setzero_si512();
    let mut acc_pv = _mm512_setzero_si512();
    let mut acc_n = _mm512_setzero_si512();
    let mut i = 0usize;
    while i + 8 <= events.len() {
        let mut v = [0u64; 8];
        let mut pv = [0u64; 8];
        let mut n = [0u64; 8];
        for j in 0..8 {
            let (a, b, c) = event_delta(events[i + j]);
            v[j] = a;
            pv[j] = b;
            n[j] = c;
        }
        // SAFETY: the arrays are 64-byte aligned stack storage; loads and
        // stores are value-only register ops.
        acc_v = _mm512_add_epi64(acc_v, _mm512_loadu_si512(v.as_ptr() as *const __m512i));
        acc_pv = _mm512_add_epi64(acc_pv, _mm512_loadu_si512(pv.as_ptr() as *const __m512i));
        acc_n = _mm512_add_epi64(acc_n, _mm512_loadu_si512(n.as_ptr() as *const __m512i));
        i += 8;
    }
    let mut dv = hsum512(acc_v);
    let mut dpv = hsum512(acc_pv);
    let mut dn = hsum512(acc_n);
    for &e in &events[i..] {
        let (a, b, c) = event_delta(e);
        dv = dv.wrapping_add(a);
        dpv = dpv.wrapping_add(b);
        dn = dn.wrapping_add(c);
    }
    (dv, dpv, dn)
}

/// Horizontal sum of a ZMM's 8 u64 lanes (pairwise VPADDQ tree).
/// SAFETY: caller proves AVX-512F.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn hsum512(x: std::arch::x86_64::__m512i) -> u64 {
    use std::arch::x86_64::*;
    // Stack unload + scalar tree (8 u64 lanes — the store-forwarded
    // round trip is 64 bytes, amortized once per batch).
    let mut buf = [0u64; 8];
    _mm512_storeu_si512(buf.as_mut_ptr() as *mut __m512i, x);
    let mut a = [0u64; 4];
    for j in 0..4 {
        a[j] = buf[2 * j].wrapping_add(buf[2 * j + 1]);
    }
    a[0]
        .wrapping_add(a[1])
        .wrapping_add(a[2])
        .wrapping_add(a[3])
}

/// The windowed prefix scan: emits the state at EVERY window boundary
/// (blocks of `block` events) with O(1) per window — the pre-collapsed
/// 64-msg span operators. Returns the states AFTER each block (the
/// input's final state is the last element).
pub fn window_states(state0: LobState, events: &[LobEvent], block: usize) -> Vec<LobState> {
    assert!(block > 0, "window block must be positive");
    let mut out = Vec::new();
    let mut s = state0;
    let mut i = 0usize;
    while i < events.len() {
        let end = (i + block).min(events.len());
        let (dv, dpv, dn) = batch_compose(&events[i..end]);
        s.volume = s.volume.wrapping_add(dv);
        s.pv = s.pv.wrapping_add(dpv);
        s.count = s.count.wrapping_add(dn);
        out.push(s);
        i = end;
    }
    out
}

/// The tropical (max-plus) scan: the best price seen among Add events —
/// the (max, +) semiring's prefix fold (best bid tracking). Pure u64
/// max is order-free: every reduction is bit-identical.
pub fn best_px(events: &[LobEvent]) -> Option<u32> {
    events
        .iter()
        .filter(|e| e.kind == LobKind::Add)
        .map(|e| e.px)
        .max()
}

/// The affine matrix monoid — the GENERAL transition class: 4x4 integer
/// matrices over homogeneous Z^4 coordinates, bottom row fixed at
/// [0 0 0 1] (affine maps x -> Ax + b). Composes by matrix product;
/// translations (the LobEvent class) are the b-only specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Affine4 {
    /// Row-major 4x4; row 3 is implicitly [0, 0, 0, 1] (enforced at
    /// compose).
    pub m: [[u64; 4]; 3],
    /// The affine column (b).
    pub b: [u64; 4],
}

impl Affine4 {
    /// The identity map.
    pub const IDENTITY: Self = Self {
        m: [
            [1, 0, 0, 0],
            [0, 1, 0, 0],
            [0, 0, 1, 0],
        ],
        b: [0, 0, 0, 1],
    };

    /// A pure translation by (dv, dpv, dn, dw) — the LobEvent class.
    #[inline]
    pub fn translation(d: [u64; 4]) -> Self {
        Self {
            m: Self::IDENTITY.m,
            b: d,
        }
    }

    /// Compose: `self` AFTER `other` (self.other(x) — apply other first).
    /// Wrapping arithmetic throughout (the monoid is over Z_2^64 —
    /// order-free, bit-deterministic).
    pub fn compose(&self, other: &Self) -> Self {
        let mut m = [[0u64; 4]; 3];
        let mut b = [0u64; 4];
        for r in 0..3 {
            for c in 0..4 {
                let mut acc = 0u64;
                for k in 0..4 {
                    let a = self.m[r][k];
                    let o = if k < 3 { other.m[k][c] } else { [0, 0, 0, 1][c] };
                    acc = acc.wrapping_add(a.wrapping_mul(o));
                }
                m[r][c] = acc;
            }
            let mut acc = self.b[r];
            for k in 0..3 {
                acc = acc.wrapping_add(self.m[r][k].wrapping_mul(other.b[k]));
            }
            b[r] = acc;
        }
        b[3] = 1;
        Self { m, b }
    }

    /// Apply to a homogeneous state vector.
    #[inline]
    pub fn apply(&self, x: [u64; 4]) -> [u64; 4] {
        let mut out = [0u64; 4];
        for r in 0..3 {
            let mut acc = self.b[r];
            for c in 0..4 {
                acc = acc.wrapping_add(self.m[r][c].wrapping_mul(x[c]));
            }
            out[r] = acc;
        }
        out[3] = 1;
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// R34 (Run 700) — the O(1) affine ingestion fold: the 𝒜₅.₀ collapse
// ═══════════════════════════════════════════════════════════════════════════
//
// docs/solution.md (the math team's blueprint) proves the ITCH 5.0 fold
// algebra does not merely compose fast — it COLLAPSES. The state chain
// `S_k = A_k S_{k-1} + B_k` over ℤ³ with `A ∈ {I₃, D₀}` (Theorem 1: the
// K-matrix chain is ONE OR bit — D₀ is idempotent) factors through the
// commutative monoid `𝒜₅.₀ = (ℤ², +) × (Tg, ⊔_seq)` (Theorem 2). Batch
// composition is 2 wrapping u64 adds + 1 tagged max, independent of K:
// a 512-message chunk is a 32-byte signature, and a 500,000-message pass
// is 977 chunk signatures combined by 976 ⊕ operations — any partition,
// any merge order (Theorem 4), bit-identical to the sequential oracle
// (Theorem 8) under the discipline pinned below.
//
// THE PORT CONTRACT (docs/solution.md §4, §6.6, §7.4 — every clause is a
// cell):
//   * K1 volumes (db, da): u64 WRAPPING sums. `(ℤ/2⁶⁴, +)` is an abelian
//     group — every reduction order (scalar loop, tree, VPADDQ) is
//     bit-identical, and the oracle accumulates identically (H4).
//   * K2 price: LAST-WRITER-WINS. The comparison key is `seq`, FIRST and
//     UNSIGNED (`vpcmpuq` semantics — T4: a key at 2⁶³⁺ must dominate any
//     low key); the price is payload, NEVER key (§4.1: a price-first
//     compare silently replaces recency with price-priority on every
//     up-print-then-down-print pattern). Payload transfers atomically
//     from the single ⊔-winner (Thm 6′(iii)) — never field-wise merges.
//   * The sentinel lives in seq (0), never in px (T3): `s* == 0 ⇔ no
//     setter in the batch`; px is dead payload when seq == 0. Wire
//     positions are 1-BASED (position 0 collides with ⊥ by construction).
//   * Keys are injective 1-based positions (T2/T5): lane-count-varied
//     repartitionings fold identically; the vector path carries a
//     `popcnt == 1`-style debug guard so a future key regression fails
//     loudly in CI instead of silently in production.
// The reference kernel in §6.6 double-counts the sub-8 tail's db/da
// (vsum over all n + a full-token tail combine); this port folds the
// tail exactly once — the differential cells below pin the fix.

/// The R34 chunk size (solution.md §3.5: 500,000 = 976·512 + 288 ⇒ 977
/// chunks; the tail is legal at ANY length — Theorem 4).
pub const SIG_CHUNK: usize = 512;

/// One element of 𝒜₅.₀ — the 32-byte chunk/pass signature
/// `(Δb, Δa, p*, s*)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sig {
    /// K1: the batch's net bid-volume delta (wrapping u64 = ℤ/2⁶⁴).
    pub db: u64,
    /// K1: the batch's net ask-volume delta.
    pub da: u64,
    /// K2 payload: the seq-maximal setter's price — read ONLY when
    /// `seq > 0` (dead payload otherwise, never a sentinel).
    pub px: u64,
    /// K2 key: the 1-based wire position of the last setter; 0 = ⊥
    /// (T3). Compared UNSIGNED and FIRST.
    pub seq: u64,
}

impl Sig {
    /// The monoid identity `(0, 0, ⊥)`.
    pub const IDENTITY: Sig = Sig { db: 0, da: 0, px: 0, seq: 0 };
}

/// ⊕ — Theorem 2(iii): 2 wrapping adds + 1 tagged max. Branch-free
/// shape; an equal nonzero key pair (a T2/T5 injectivity violation
/// upstream) keeps the LEFT operand deterministically — unreachable
/// under 1-based emission.
#[inline(always)]
pub fn sig_combine(x: Sig, y: Sig) -> Sig {
    let use_y = y.seq > x.seq;
    Sig {
        db: x.db.wrapping_add(y.db),
        da: x.da.wrapping_add(y.da),
        px: if use_y { y.px } else { x.px },
        seq: if use_y { y.seq } else { x.seq },
    }
}

/// The `(b, a, p) ∈ ℤ³` book state — the R34 coordinate set
/// (K1 volume × 2, K2 last-writer price).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lob3 {
    /// Displayed bid volume (shares).
    pub b: u64,
    /// Displayed ask volume (shares).
    pub a: u64,
    /// Last trade price in integer ticks (the K2 coordinate).
    pub p: u64,
}

impl Lob3 {
    /// Ψ — Theorem 2's application map: the A_net volume rows are unit
    /// (Cor. 1.1a) so plain wrapping adds; the price row is D₀ iff
    /// `seq > 0` (the T3 invariant: ε and the tag can never disagree),
    /// i.e. an assignment from the winner's payload.
    #[inline(always)]
    pub fn apply(&mut self, g: Sig) {
        self.b = self.b.wrapping_add(g.db);
        self.a = self.a.wrapping_add(g.da);
        if g.seq != 0 {
            self.p = g.px;
        }
    }
}

/// The resolved-token fold input (SoA, solution.md §6.2): 512 tokens =
/// 16 KB = one third of L1D. `seq[i] == 0` marks a non-setter (⊥);
/// `px[i]` is then dead payload (T3).
#[derive(Clone)]
pub struct SigTokens {
    pub seq: [u64; SIG_CHUNK],
    pub px: [u64; SIG_CHUNK],
    pub db: [u64; SIG_CHUNK],
    pub da: [u64; SIG_CHUNK],
}

impl Default for SigTokens {
    fn default() -> Self {
        Self {
            seq: [0; SIG_CHUNK],
            px: [0; SIG_CHUNK],
            db: [0; SIG_CHUNK],
            da: [0; SIG_CHUNK],
        }
    }
}

impl SigTokens {
    /// Emit one resolved token at slot `i`: (seq, px, db, da).
    #[inline(always)]
    pub fn emit(&mut self, i: usize, seq: u64, px: u64, db: u64, da: u64) {
        self.seq[i] = seq;
        self.px[i] = px;
        self.db[i] = db;
        self.da[i] = da;
    }

    /// Zero the buffer (all-⊥, zero deltas).
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// The portable twin — the sequential oracle for the fold factor. A
/// different evaluation order from the vector path; an identical result
/// (Corollary 2.1 — the algebra is order-free).
pub fn sig_fold_scalar(seq: &[u64], px: &[u64], db: &[u64], da: &[u64]) -> Sig {
    debug_assert_eq!(seq.len(), px.len());
    debug_assert_eq!(seq.len(), db.len());
    debug_assert_eq!(seq.len(), da.len());
    let mut acc = Sig::IDENTITY;
    for i in 0..seq.len() {
        acc = sig_combine(
            acc,
            Sig { db: db[i], da: da[i], px: px[i], seq: seq[i] },
        );
    }
    acc
}

/// The dispatching chunk fold: AVX-512 when the class is present and the
/// row is wide enough to amortize the horizontal finish; the portable
/// twin otherwise. Both paths are the same commutative monoid — their
/// equality on all inputs is Corollary 2.1, and the differential cells
/// assert it per fuzz trial.
pub fn sig_fold(seq: &[u64], px: &[u64], db: &[u64], da: &[u64]) -> Sig {
    #[cfg(target_arch = "x86_64")]
    {
        if seq.len() >= 16 && is_x86_feature_detected!("avx512f") {
            // SAFETY: the AVX-512F class was just detected at runtime.
            return unsafe { sig_fold_avx512(seq, px, db, da) };
        }
    }
    sig_fold_scalar(seq, px, db, da)
}

/// The AVX-512 chunk fold (solution.md §6.6): K1 by VPADDQ lane-sums,
/// K2 by pairwise tagged max — `vpcmpuq` key compare (T4: unsigned),
/// `_mm512_mask_mov` payload blend (Thm 6′(iii): atomic winner
/// transfer), scalar tail folded exactly ONCE (the reference kernel's
/// tail double-count is the port's documented fix).
/// SAFETY: caller proves AVX-512F.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn sig_fold_avx512(seq: &[u64], px: &[u64], db: &[u64], da: &[u64]) -> Sig {
    use std::arch::x86_64::*;
    let n = seq.len();
    let nv = n & !7usize;
    // K1 over the vector span [0, nv); the tail folds as full tokens below.
    let mut acc = Sig {
        db: vsum512_tree(&db[..nv]),
        da: vsum512_tree(&da[..nv]),
        px: 0,
        seq: 0,
    };
    if nv >= 8 {
        let mut s = _mm512_loadu_si512(seq.as_ptr() as *const __m512i);
        let mut p = _mm512_loadu_si512(px.as_ptr() as *const __m512i);
        let mut off = 8usize;
        while off < nv {
            // SAFETY: off + 8 <= nv <= slice len (loop invariant).
            let s2 = _mm512_loadu_si512(seq.as_ptr().add(off) as *const __m512i);
            let p2 = _mm512_loadu_si512(px.as_ptr().add(off) as *const __m512i);
            // Lanes where operand 2's key strictly wins (unsigned, T4);
            // key: unsigned max; payload: atomic winner blend (Thm 6′(iii)).
            let w = _mm512_cmpgt_epu64_mask(s2, s);
            s = _mm512_max_epu64(s, s2);
            p = _mm512_mask_mov_epi64(p, w, p2);
            off += 8;
        }
        let mut ks = [0u64; 8];
        let mut ps = [0u64; 8];
        // SAFETY: 64-byte stack arrays; register-value stores.
        _mm512_storeu_si512(ks.as_mut_ptr() as *mut __m512i, s);
        _mm512_storeu_si512(ps.as_mut_ptr() as *mut __m512i, p);
        // The T5 debug guard: an exact key tie inside the register means
        // a non-injective key source — fail loudly, never silently. (An
        // all-⊥ register is legal: 8 sentinel keys — T3.)
        let maxk = ks.iter().copied().max().unwrap_or(0);
        if maxk > 0 {
            debug_assert_eq!(
                ks.iter().filter(|&&k| k == maxk).count(),
                1,
                "T5: key collision (non-injective seq source)"
            );
        }
        // Horizontal finish: 8 tagged-maxes via ⊕ (K2-only tokens).
        for i in 0..8 {
            acc = sig_combine(acc, Sig { db: 0, da: 0, px: ps[i], seq: ks[i] });
        }
    }
    // The scalar tail: full tokens, folded exactly once.
    for i in nv..n {
        acc = sig_combine(
            acc,
            Sig { db: db[i], da: da[i], px: px[i], seq: seq[i] },
        );
    }
    acc
}

/// K1 lane-sum over 8-lane VPADDQ blocks (the caller pins the span);
/// horizontal finish via the existing pairwise [`hsum512`].
/// SAFETY: caller proves AVX-512F.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn vsum512_tree(v: &[u64]) -> u64 {
    use std::arch::x86_64::*;
    let mut acc = _mm512_setzero_si512();
    let mut i = 0usize;
    while i + 8 <= v.len() {
        // SAFETY: i + 8 <= len.
        acc = _mm512_add_epi64(acc, _mm512_loadu_si512(v.as_ptr().add(i) as *const __m512i));
        i += 8;
    }
    let mut s = hsum512(acc);
    for &x in &v[i..] {
        s = s.wrapping_add(x);
    }
    s
}

/// Per-chunk signatures at `chunk` granularity — Theorem 4: for any
/// partition of the stream, the ⊕-combination of the parts equals the
/// whole-stream fold (this is the memoized-replay store: 32 B per
/// chunk, 31 KB per 500k messages, L1-resident).
pub fn sig_chunk_signatures(
    seq: &[u64],
    px: &[u64],
    db: &[u64],
    da: &[u64],
    chunk: usize,
) -> Vec<Sig> {
    assert!(chunk > 0, "chunk size must be positive");
    let n = seq.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < n {
        let end = (i + chunk).min(n);
        out.push(sig_fold(&seq[i..end], &px[i..end], &db[i..end], &da[i..end]));
        i = end;
    }
    out
}

/// Ψ-commit at chunk boundaries: the state after each chunk — the
/// DEFINED points the conformance cells compare (Theorem 8; mid-chunk
/// states are nobody's business — solution.md §7.2). The same binary
/// serves cell mode (chunk = 1) and performance mode (chunk = 512):
/// identical boundary states by construction.
pub fn sig_pass_states(
    s0: Lob3,
    seq: &[u64],
    px: &[u64],
    db: &[u64],
    da: &[u64],
    chunk: usize,
) -> Vec<Lob3> {
    let mut s = s0;
    sig_chunk_signatures(seq, px, db, da, chunk)
        .into_iter()
        .map(|g| {
            s.apply(g);
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic xorshift — the test PRNG (allocation-light).
    struct Xs(u64);
    impl Xs {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// THE HOMOMORPHISM LAW: sequential per-event application ==
    /// batch-compose + one apply — for every batch split (the monoid's
    /// whole reason to exist; wrapped u64 sums make every order
    /// bit-identical).
    #[test]
    fn t_r24_lob_homomorphism() {
        let mut rng = Xs(0x5132_0000_0000_0B10);
        for n in [0usize, 1, 7, 8, 9, 31, 32, 63, 64, 65, 127, 128, 1000, 4096] {
            let events: Vec<LobEvent> = (0..n)
                .map(|_| {
                    let kind = match rng.next() % 3 {
                        0 => LobKind::Add,
                        1 => LobKind::Exec,
                        _ => LobKind::Cancel,
                    };
                    LobEvent {
                        kind,
                        px: (rng.next() >> 40) as u32,
                        qty: (rng.next() >> 48) as u32,
                    }
                })
                .collect();
            // Sequential reference.
            let mut seq = LobState::default();
            for &e in &events {
                seq.apply(e);
            }
            // Batch composite, applied once (several split points).
            for split in [n, n / 2 + 1, n / 3 + 1, 8, 64] {
                let split = split.min(n);
                let (dv, dpv, dn) = batch_compose(&events[..split]);
                let mut b = LobState::default();
                b.volume = b.volume.wrapping_add(dv);
                b.pv = b.pv.wrapping_add(dpv);
                b.count = b.count.wrapping_add(dn);
                let (dv2, dpv2, dn2) = batch_compose(&events[split..]);
                b.volume = b.volume.wrapping_add(dv2);
                b.pv = b.pv.wrapping_add(dpv2);
                b.count = b.count.wrapping_add(dn2);
                assert_eq!(b, seq, "homomorphism break at n={n} split={split}");
            }
        }
    }

    /// The VWAP law: PV/V equals the sequential price*qty weighted mean
    /// on a controlled small corpus (no wrapping).
    #[test]
    fn t_r24_lob_vwap() {
        let events = [
            LobEvent { kind: LobKind::Add, px: 100, qty: 10 },
            LobEvent { kind: LobKind::Add, px: 104, qty: 30 },
            LobEvent { kind: LobKind::Exec, px: 100, qty: 10 },
        ];
        let mut s = LobState::default();
        for &e in &events {
            s.apply(e);
        }
        assert_eq!(s.volume, 30);
        assert_eq!(s.pv, 100 * 10 + 104 * 30 - 100 * 10);
        assert_eq!(s.count, 1);
        assert_eq!(s.vwap_ticks(), Some(104));
    }

    /// The window scan: every boundary state equals the sequential
    /// state at that boundary (the O(1)-per-window law).
    #[test]
    fn t_r24_lob_window_states() {
        let mut rng = Xs(0xC1E1_C0DE);
        let events: Vec<LobEvent> = (0..500)
            .map(|_| LobEvent {
                kind: match rng.next() % 3 {
                    0 => LobKind::Add,
                    1 => LobKind::Exec,
                    _ => LobKind::Cancel,
                },
                px: (rng.next() >> 44) as u32,
                qty: (rng.next() >> 52) as u32,
            })
            .collect();
        for block in [1usize, 7, 64, 128, 500] {
            let windows = window_states(LobState::default(), &events, block);
            // Boundary k covers events[..block*(k+1)].
            for (k, &w) in windows.iter().enumerate() {
                let mut seq = LobState::default();
                for &e in &events[..(block * (k + 1)).min(events.len())] {
                    seq.apply(e);
                }
                assert_eq!(w, seq, "window state drift at block={block} k={k}");
            }
        }
    }

    /// The affine matrix monoid: composition law (associativity against
    /// sequential application) + the translation specialization.
    #[test]
    fn t_r24_lob_affine_monoid() {
        let mut rng = Xs(0xA11F_E_E01);
        // Random affine maps.
        let rand_affine = |rng: &mut Xs| -> Affine4 {
            let mut m = [[0u64; 4]; 3];
            for r in 0..3 {
                m[r][r] = 1; // near-identity to stay in wrapping range
                m[r][(r + 1) % 3] = (rng.next() >> 48) as u64;
            }
            let b = [
                (rng.next() >> 48) as u64,
                (rng.next() >> 48) as u64,
                (rng.next() >> 48) as u64,
                1,
            ];
            Affine4 { m, b }
        };
        let x0: [u64; 4] = [7, 11, 13, 1];
        for _ in 0..200 {
            let a = rand_affine(&mut rng);
            let b = rand_affine(&mut rng);
            let c = rand_affine(&mut rng);
            // (a.compose(b)).compose(c) == a.compose(b.compose(c)) —
            // associativity, the scan's license.
            let left = a.compose(&b).compose(&c);
            let right = a.compose(&b.compose(&c));
            assert_eq!(left.apply(x0), right.apply(x0), "affine associativity");
            // Sequential application == composite application.
            let seq = a.apply(b.apply(c.apply(x0)));
            assert_eq!(left.apply(x0), seq, "affine composition value");
        }
        // The translation specialization == LobState's law.
        let t1 = Affine4::translation([10, 1000, 1, 0]);
        let t2 = Affine4::translation([5, 550, 1, 0]);
        let comp = t1.compose(&t2);
        assert_eq!(comp.b, [15, 1550, 2, 1]);
        assert_eq!(comp.m, Affine4::IDENTITY.m);
        let x = comp.apply([0, 0, 0, 1]);
        assert_eq!(x, [15, 1550, 2, 1]);
    }

    /// The tropical scan: best price among adds.
    #[test]
    fn t_r24_lob_best_px() {
        let events = [
            LobEvent { kind: LobKind::Add, px: 100, qty: 1 },
            LobEvent { kind: LobKind::Exec, px: 999, qty: 1 }, // not an add
            LobEvent { kind: LobKind::Add, px: 104, qty: 1 },
            LobEvent { kind: LobKind::Cancel, px: 990, qty: 1 }, // not an add
        ];
        assert_eq!(best_px(&events), Some(104));
        assert_eq!(best_px(&[]), None);
    }

    // ═══ R34: the 𝒜₅.₀ collapse cells (docs/solution.md §4.5, §7.5) ═══
    //
    // Every cell pins one clause of the port contract; together they
    // demo Theorems 2, 4, 6, 6', 8 on the adversarial tie taxonomy and
    // the dual-path (AVX-512 == scalar) equality — Corollary 2.1.

    /// A Vec-backed token stream (the pass store: SoA, arbitrary length).
    struct TokStream {
        seq: Vec<u64>,
        px: Vec<u64>,
        db: Vec<u64>,
        da: Vec<u64>,
    }

    impl TokStream {
        fn new(n: usize) -> Self {
            Self {
                seq: vec![0; n],
                px: vec![0; n],
                db: vec![0; n],
                da: vec![0; n],
            }
        }

        fn emit(&mut self, i: usize, seq: u64, px: u64, db: u64, da: u64) {
            self.seq[i] = seq;
            self.px[i] = px;
            self.db[i] = db;
            self.da[i] = da;
        }

        /// Deterministic random fill: full-range wrapping K1 deltas,
        /// ~1/4 sparse setters with INJECTIVE 1-based keys, garbage dead
        /// payload on non-setters (T3 discipline: px never a sentinel).
        fn fill_rand(&mut self, rng: &mut Xs, setter_p: u64) {
            for i in 0..self.seq.len() {
                let is_setter = rng.next() % setter_p == 0;
                let (s, p) = if is_setter {
                    (i as u64 + 1, rng.next() >> 32)
                } else {
                    (0, rng.next()) // dead payload: never read
                };
                let (db, da) = (rng.next(), rng.next());
                self.emit(i, s, p, db, da);
            }
        }

        /// The sequential oracle: apply every token in stream order.
        fn oracle(&self, s0: Lob3) -> Lob3 {
            let mut s = s0;
            for i in 0..self.seq.len() {
                s.apply(Sig {
                    db: self.db[i],
                    da: self.da[i],
                    px: self.px[i],
                    seq: self.seq[i],
                });
            }
            s
        }
    }

    /// §4.1 — the up-print-then-down-print stream: recency wins, NOT
    /// max-price. A price-first comparator returns 101.00 here.
    #[test]
    fn t_r34_recency_last_write_wins() {
        let mut t = TokStream::new(2);
        t.emit(0, 1, 10100, 0, 0); // print 101.00, s=1
        t.emit(1, 2, 9950, 0, 0); // print 99.50, s=2
        let c = sig_fold(&t.seq, &t.px, &t.db, &t.da);
        assert_eq!(c.seq, 2);
        assert_eq!(c.px, 9950, "recency must win over max-price");
        let mut s = Lob3::default();
        s.apply(c);
        assert_eq!(s.p, 9950);
        // And the sequential oracle agrees.
        assert_eq!(t.oracle(Lob3::default()).p, 9950);
    }

    /// T1 — equal prices, distinct seqs: price output invariant; the
    /// whole payload must come from the seq-winner.
    #[test]
    fn t_r34_equal_price_distinct_seq() {
        let mut t = TokStream::new(3);
        t.emit(0, 1, 10100, 5, 0);
        t.emit(1, 2, 9950, 0, 0);
        t.emit(2, 7, 9950, 0, 3); // equal price, later key
        let c = sig_fold(&t.seq, &t.px, &t.db, &t.da);
        assert_eq!((c.seq, c.px), (7, 9950));
        assert_eq!((c.db, c.da), (5, 3));
    }

    /// T4 — the unsigned key domain: a key above 2⁶³ (and at the u64
    /// ceiling) must dominate any low key; a signed compare flips it.
    #[test]
    fn t_r34_unsigned_key_domain() {
        for big in [(1u64 << 63) + 5, u64::MAX - 2] {
            let mut t = TokStream::new(2);
            t.emit(0, big, 11111, 0, 0);
            t.emit(1, 42, 99999, 0, 0);
            let c = sig_fold(&t.seq, &t.px, &t.db, &t.da);
            assert_eq!(c.px, 11111, "key {big:#x} must dominate unsigned");
            assert_eq!(c.seq, big);
            // The boundary also holds through the forced vector path.
            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("avx512f") {
                let v = unsafe { sig_fold_avx512(&t.seq, &t.px, &t.db, &t.da) };
                assert_eq!(v, c);
            }
        }
    }

    /// T3 — the all-⊥ chunk: I₃ semantics; the price coordinate is
    /// untouched (dead px payloads are never read); the invariant
    /// lemma: s* == 0 ⇔ no setter.
    #[test]
    fn t_r34_sentinel_all_bottom() {
        let mut rng = Xs(0xB077_0A0C_0000_0001u64);
        let mut t = TokStream::new(SIG_CHUNK);
        for i in 0..SIG_CHUNK {
            t.emit(i, 0, 0xDEAD_BEEF, rng.next(), rng.next());
        }
        let c = sig_fold(&t.seq, &t.px, &t.db, &t.da);
        assert_eq!(c.seq, 0, "all-⊥ chunk must fold to the sentinel");
        assert_eq!(c.db, t.db.iter().fold(0u64, |a, &x| a.wrapping_add(x)));
        let mut s = Lob3 { b: 5, a: 7, p: 4242 };
        s.apply(c);
        assert_eq!(s.p, 4242, "I₃: the price coordinate is untouched");
        assert_eq!(s.b, 5u64.wrapping_add(c.db));
    }

    /// §7.5 row 5 — the zero-price print: `P(0)` is a REAL assignment
    /// (the D₀ path), never mistaken for "no setter".
    #[test]
    fn t_r34_zero_price_print() {
        let mut t = TokStream::new(3);
        t.emit(0, 1, 10100, 0, 0);
        t.emit(1, 2, 0, 0, 0); // print 0.00
        t.emit(2, 3, 9900, 0, 0);
        // Truncate: fold only the first two (the zero print is LAST).
        let c = sig_fold(&t.seq[..2], &t.px[..2], &t.db[..2], &t.da[..2]);
        assert_eq!(c.seq, 2);
        assert_eq!(c.px, 0);
        let mut s = Lob3::default();
        s.apply(c);
        assert_eq!(s.p, 0, "px == 0 must assign, not skip");
    }

    /// The monoid laws on ⊕ (Theorem 2(iii) / Appendix A.2): identity,
    /// commutativity, associativity, ⊔-idempotence on distinct keys,
    /// and the ⊥ unit law.
    #[test]
    fn t_r34_combine_laws() {
        let mut rng = Xs(0xA5A5_5A5A_DEAD_0042);
        let rtok = |rng: &mut Xs| Sig {
            db: rng.next(),
            da: rng.next(),
            px: rng.next() >> 32,
            seq: rng.next() >> 32, // distinct keys, all > 0
        };
        for _ in 0..500 {
            let a = rtok(&mut rng);
            let b = rtok(&mut rng);
            let c = rtok(&mut rng);
            // Identity.
            assert_eq!(sig_combine(a, Sig::IDENTITY), a);
            assert_eq!(sig_combine(Sig::IDENTITY, a), a);
            // Commutativity (distinct keys — Lemma 2's domain).
            if a.seq != b.seq {
                assert_eq!(sig_combine(a, b), sig_combine(b, a));
            }
            // Associativity.
            assert_eq!(
                sig_combine(sig_combine(a, b), c),
                sig_combine(a, sig_combine(b, c))
            );
        }
        // ⊔-idempotence: a tag combined with itself is itself (the
        // K2 half; the K1 half wraps — idempotence does NOT hold for
        // addition, by design).
        let g = Sig { db: 0, da: 0, px: 4242, seq: 17 };
        assert_eq!(sig_combine(g, g), g);
    }

    /// Theorem 4 / §7.5 row 7 — partition invariance: the same stream
    /// folded at chunk sizes {1, 16, 64, 256, 512} yields identical
    /// whole-stream signatures, identical final states, and identical
    /// boundary states at shared indices; AND the chunk signatures
    /// combine identically under forward, reverse, and pairwise-tree
    /// merge orders.
    #[test]
    fn t_r34_partition_invariance() {
        let mut rng = Xs(0x84E7_1710_0DD0_0001u64);
        let mut t = TokStream::new(5000);
        t.fill_rand(&mut rng, 4);
        let s0 = Lob3 { b: 1000, a: 1000, p: 3 };
        let whole = sig_fold(&t.seq, &t.px, &t.db, &t.da);
        let oracle_final = t.oracle(s0);
        // The oracle's state at EVERY prefix length (O(n), once) — the
        // boundary comparisons below are then index lookups.
        let mut oracle_states = Vec::with_capacity(t.seq.len() + 1);
        let mut cur = s0;
        oracle_states.push(cur);
        for i in 0..t.seq.len() {
            cur.apply(Sig {
                db: t.db[i],
                da: t.da[i],
                px: t.px[i],
                seq: t.seq[i],
            });
            oracle_states.push(cur);
        }
        for chunk in [1usize, 16, 64, 256, 512] {
            let sigs = sig_chunk_signatures(&t.seq, &t.px, &t.db, &t.da, chunk);
            // Forward merge.
            let fwd = sigs.iter().copied().fold(Sig::IDENTITY, sig_combine);
            assert_eq!(fwd, whole, "forward merge at chunk={chunk}");
            // Reverse merge.
            let rev = sigs.iter().rev().copied().fold(Sig::IDENTITY, sig_combine);
            assert_eq!(rev, whole, "reverse merge at chunk={chunk}");
            // Pairwise-tree merge.
            let mut levels = sigs.clone();
            while levels.len() > 1 {
                let mut next = Vec::with_capacity((levels.len() + 1) / 2);
                let mut i = 0;
                while i + 1 < levels.len() {
                    next.push(sig_combine(levels[i], levels[i + 1]));
                    i += 2;
                }
                if i < levels.len() {
                    next.push(levels[i]);
                }
                levels = next;
            }
            assert_eq!(levels[0], whole, "tree merge at chunk={chunk}");
            // Ψ-commit: the final boundary state equals the oracle.
            let states = sig_pass_states(s0, &t.seq, &t.px, &t.db, &t.da, chunk);
            assert_eq!(*states.last().unwrap(), oracle_final, "final state at chunk={chunk}");
            // Boundary points agree with the oracle (chunk=1 pins EVERY
            // point; larger chunks pin their own).
            for (k, &w) in states.iter().enumerate() {
                let end = (chunk * (k + 1)).min(t.seq.len());
                assert_eq!(w, oracle_states[end], "boundary k={k} at chunk={chunk}");
            }
        }
    }

    /// Theorem 8 / Corollary 2.1 — the differential fuzz: sequential
    /// oracle == fold + Ψ, and the scalar path == the dispatched path,
    /// for random streams (sizes crossing every tail regime) — 2000
    /// trials, the C self-test's protocol.
    #[test]
    fn t_r34_oracle_bit_exact_fuzz() {
        let mut rng = Xs(0x9E37_79B9_7F4A_7C15);
        let mut t = TokStream::new(SIG_CHUNK);
        for trial in 0..2000 {
            let n = 1 + (rng.next() % SIG_CHUNK as u64) as usize;
            t.fill_rand(&mut rng, 4);
            let toks = TokStream {
                seq: t.seq[..n].to_vec(),
                px: t.px[..n].to_vec(),
                db: t.db[..n].to_vec(),
                da: t.da[..n].to_vec(),
            };
            let scalar = sig_fold_scalar(&toks.seq, &toks.px, &toks.db, &toks.da);
            let dispatched = sig_fold(&toks.seq, &toks.px, &toks.db, &toks.da);
            assert_eq!(scalar, dispatched, "dual-path break at trial={trial} n={n}");
            // Fold + Ψ == sequential oracle.
            let mut folded = Lob3 { b: 100_000, a: 100_000, p: 0 };
            folded.apply(dispatched);
            assert_eq!(folded, toks.oracle(Lob3 { b: 100_000, a: 100_000, p: 0 }),
                "oracle divergence at trial={trial} n={n}");
        }
        // n = 0: the identity.
        assert_eq!(sig_fold(&[], &[], &[], &[]), Sig::IDENTITY);
    }

    /// Theorem 8 at commit points — a 500,000-message pass: 977 chunks
    /// (976 full + the 288 tail), every 512-boundary state equal to the
    /// sequential state after that message (the defined-points law,
    /// §7.2), and the 976-⊕ replay of the stored signatures identical
    /// under every merge order (the regime-3 arithmetic).
    #[test]
    fn t_r34_pass_boundaries_and_replay() {
        let mut rng = Xs(0xFEED_C0DE_5EED_0011);
        const N: usize = 500_000;
        let mut t = TokStream::new(N);
        t.fill_rand(&mut rng, 4);
        let s0 = Lob3 { b: 1 << 20, a: 1 << 20, p: 77 };
        // The chunk arithmetic: 977 = 976·512 + 288.
        let sigs = sig_chunk_signatures(&t.seq, &t.px, &t.db, &t.da, SIG_CHUNK);
        assert_eq!(sigs.len(), (N + SIG_CHUNK - 1) / SIG_CHUNK);
        assert_eq!(sigs.len(), 977);
        // Every boundary state == the sequential state at that message.
        let states = sig_pass_states(s0, &t.seq, &t.px, &t.db, &t.da, SIG_CHUNK);
        let mut oracle = s0;
        let mut next_boundary = SIG_CHUNK;
        let mut boundary_idx = 0;
        for i in 0..N {
            oracle.apply(Sig {
                db: t.db[i],
                da: t.da[i],
                px: t.px[i],
                seq: t.seq[i],
            });
            if i + 1 == next_boundary || i + 1 == N {
                assert_eq!(
                    states[boundary_idx], oracle,
                    "boundary {boundary_idx} (msg {i}) diverged"
                );
                boundary_idx += 1;
                next_boundary += SIG_CHUNK;
            }
        }
        assert_eq!(boundary_idx, states.len());
        // The 976-⊕ replay: stored signatures, three merge orders.
        let fwd = sigs.iter().copied().fold(Sig::IDENTITY, sig_combine);
        let rev = sigs.iter().rev().copied().fold(Sig::IDENTITY, sig_combine);
        assert_eq!(fwd, rev);
        let whole = sig_fold(&t.seq, &t.px, &t.db, &t.da);
        assert_eq!(fwd, whole, "the 976-⊕ replay equals the whole-stream fold");
        let mut replayed = s0;
        replayed.apply(fwd);
        assert_eq!(replayed, oracle, "the replay's Ψ equals the sequential oracle");
    }
}
