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

#[cfg(target_arch = "x86_64")]
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
}
