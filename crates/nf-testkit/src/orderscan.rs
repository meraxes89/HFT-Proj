//! R38 (Run 900, Gate 13) — the segmented order scan: the affine
//! order-transducer of docs/solution2.md Part III (Theorems 27–31;
//! Appendix B.3/B.4) — the RowOp row-op monoid, the inclusive
//! segmented Hillis–Steele scan, and the emit-time policy stage.
//!
//! # The physics this module exists for
//!
//! The B3 wall (solution2.md §1.3): for E/C/X/D/U messages the
//! resolved signature contribution is a function of the referenced
//! order's chain-prefix state; the R36/R37 engine pays a hash-table
//! probe per reference (`OrderCache`: open addressing, linear
//! probing, L2-resident, ~12–20 cycles hot plus a miss tail that
//! risks sequential pointer chasing on adversarial working sets) —
//! the co-equal wall with the fold at the corridor's operating
//! points, and the structural blocker for dual-lane resolution
//! ([S1] §6.4's prohibition: a dual-resolver pull design is unsound
//! by construction for the cache factor). The push model (Theorem
//! 29) inverts the dataflow: instead of probing the register file
//! at reference time, a per-ID-thread associative SCAN carries each
//! position's prefix row-op TO it, and the state is evaluated in
//! closed form on arrival. The probe's serial dependency chain
//! (each E's ρ depends on the previous E's) — depth up to B per
//! thread — is re-associated to ⌈log₂ B⌉ = 9 levels WHATEVER the
//! id distribution (Theorem 29(b): the adversarial hot-ID case,
//! all 512 references on one order, is also 9 levels). With the
//! scan, [S1] §6.4's prohibition LIFTS (Theorem 31): two lanes
//! with private caches + the cross-lane handoff resolve bit-exact
//! sequential semantics — the co-lane L5 configuration's
//! prerequisite.
//!
//! # The mathematics (docs/solution2.md §4, Theorems 27/29)
//!
//! **Theorem 27 (affine row updates).** The [S1] §5 register
//! `R_id = (locate, side, rem, px) ∪ {ABSENT}` packs as a row
//! vector `o = (σ_b, π, ρ) ∈ (ℤ/2⁶⁴ℤ)³` — σ_b ∈ {0,1} the bid
//! selector, π = (px << 32) | locate the identity payload, ρ the
//! remaining quantity — and every order action α updates it
//! affinely: `o ↦ o·M_α + b_α` with M_α DIAGONAL over {0,1} (three
//! blend masks — NEVER a materialized 3×3, Appendix D item 12) and
//! b_α the additive part. A: M = 0, b = (s_b, π_A, q) — overwrite;
//! E/C/X: M = I, b = (0, 0, −q) — translate ρ (the clamp resolved
//! at emit); D: M = 0, dead — the terminator; U: D-like on the old
//! register, A-like on the new. The composite over any action
//! string is the closed form `o·M_w + b_w` with the row-op compose
//! `(M₁,b₁) ⨂ (M₂,b₂) = (M₁M₂, b₁M₂ + b₂)` (apply left first) —
//! associative and closed on diagonal {0,1} matrices (products are
//! bitwise ANDs of masks; `b₁M₂` is a lane-wise blend).
//!
//! **Lemma 9 (the segmented operator).** With
//! `op((x,f),(y,g)) = (g ? y : x⨂y, f∨g)` on (row-op, run-start
//! flag) pairs, op is associative and the Hillis–Steele sweep with
//! op computes the inclusive SEGMENTED scan — each position ends
//! holding its id-thread's composite from the thread's run start
//! through itself.
//!
//! **Theorem 29(a) (push-scan correctness).** Emitting each
//! message's resolved output as the bilinear form on
//! `(o_entry·M_pre + b_pre, params)` — `pre` the position's
//! EXCLUSIVE prefix (the inclusive composite at its previous
//! position, identity at a run start) — reproduces the sequential
//! cache semantics BIT-EXACTLY, with the [S1] §5.6 policies
//! (clamps, anomaly routing) evaluated AT EMIT TIME where ρ and
//! the thread's liveness are known: policy behavior is
//! data-computed, never schedule-computed. The clamp
//! `min(q, ρ)` and the liveness are functions of the emitted
//! closed form — a violating position re-anchors its thread's
//! continuation with a position-local overwrite row-op
//! `(M = 0, b = (σ_b, π, 0))` — an O(1) correction paid only on
//! violating positions; D carries in the scan as the terminator
//! whose sticky `dead` flag makes every later reference in the
//! thread an anomaly.
//!
//! **Theorem 28 (ring polymorphism).** The row-op monoid is
//! `Aff₃(ℤ/2⁶⁴ℤ)` — the same associative law as Part I's
//! `𝒜_hash = A(GF(2⁶⁴))` (spanfold's P64), with `×/+` in place of
//! `⊗/⊕` and diagonal bit-matrices in place of scalars. One
//! schedule family, two ring backends (Appendix C: `VPCLMULQDQ` +
//! `VPXORQ` vs blend + `VPADDQ`), zero new mathematics per ring.
//!
//! # The port's conventions (vs the C17 reference, Appendix B.3/B.4)
//!
//! 1. **The act codes** — `ACT_A/E/C/X/D/U` as `u8` consts (the
//!    reference's `uint8_t act`); U is NOT a row-op: the BUILDER
//!    expands it to two (a D-like on the old id's thread, an
//!    A-like run-start on the new id's thread — [S1] §5.2's
//!    two-writes rule).
//! 2. **`o_entry` as `Option<[u64; 3]>`** — the reference's
//!    nullable `const uint64_t o_entry[3]`; `None` = absent
//!    (born-in-batch threads initialize from the A row-op's b, and
//!    the anomaly check keys on the prefix mask as in the C).
//! 3. **`Resolved.ok`** — 1 clean, 0 anomaly (no token), 2 clamped
//!    (the policy arm; re-anchor with the overwrite row-op) — the
//!    C's contract verbatim.
//! 4. **Two layouts, one law** — the C reference is array-of-
//!    structs (`RowOp`, 64 B, one per cacheline); the vector twin
//!    is structure-of-arrays (`ScanSoa`, eight B-lane u64 columns —
//!    the 8-lane compose is then one ZMM op per field). The scalar
//!    reference exists in BOTH layouts; the cells pin
//!    AoS ≡ SoA-scalar ≡ SoA-vector on every fuzz shape (the
//!    no-drift law: three derivations of one law, any two agree).
//! 5. **Descending i** in the scan's inner loop is load-bearing
//!    (the reference's own note: `e[i+d]` must read `e[i]`'s
//!    pre-level value) — preserved in the SoA forms, including the
//!    8-lane vector groups (each group loads both operands before
//!    any store; the groups' read ranges [i−d, i−d+8) and write
//!    ranges [i, i+8) are disjoint within a level for every d ≥ 1,
//!    so the atomic load-load-compute-store per group preserves
//!    the level discipline exactly).
//!
//! # Rollback
//!
//! `HFT_ORDER_SCAN_VEC=0` forces the scalar SoA path (the
//! attribution arm; the AoS reference stays as the cells' oracle
//! in every mode). The kernel is PURE — no engine wiring rides in
//! this module (the fabric integration is the next stage's work;
//! this module is the port of Appendix B.3/B.4 verbatim + the
//! differential battery of checklist item 9).

use std::sync::OnceLock;

/// A: order add (M = 0 overwrite; the b carries the new register).
pub const ACT_A: u8 = 0;
/// E: order executed (M = I, ρ −= q; E_SETS_LAST — the print px is
/// the ORDER's px, not the wire's).
pub const ACT_E: u8 = 1;
/// C: order canceled with a print (M = I, ρ −= q; the print px is
/// the WIRE's — the trade print carries its own price).
pub const ACT_C: u8 = 2;
/// X: order canceled without a print (M = I, ρ −= q clamped).
pub const ACT_X: u8 = 3;
/// D: order deleted (M = 0, dead — the thread's terminator).
pub const ACT_D: u8 = 4;
/// U: order replaced (TWO row-ops at the builder: a D-like on the
/// old id's thread — THIS emit case resolves the combined remove+add
/// delta `nq − ρ` on the inherited side, the fabric's own semantics —
/// and an A-like run-start on the new id's thread). The emit's
/// wire_q carries nq.
pub const ACT_U: u8 = 5;

/// One row-op: the affine map `o ↦ o·M + b` over the register row
/// `(σ_b, π, ρ) ∈ (ℤ/2⁶⁴ℤ)³`, with M diagonal over {0,1} — kept as
/// three blend masks (NEVER a 3×3 materialization; Appendix D
/// item 12) — plus the run-start flag `f` (a new id thread: the
/// segmented scan's Lemma 9 operand) and the sticky `dead` flag
/// (a D terminator: every later reference in the thread is an
/// anomaly). 64 B — one cacheline.
///
/// The struct is PURELY monoidal (Theorem 27's compose); the emit
/// stage's wire params (q, px) travel in the caller's SoA param
/// arrays, not here — the reference's own discipline.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RowOp {
    /// Run-start flag: this row begins a new id thread (the
    /// segmented operator resets composition here).
    pub f: u64,
    /// Keep-masks (M's diagonal): all-ones = keep the coordinate,
    /// zero = overwrite it (the A/D terminators zero all three).
    pub m: [u64; 3],
    /// The additive part (b).
    pub b: [u64; 3],
    /// Sticky: the thread saw a D — later references are anomalies.
    pub dead: u64,
}

const _: () = assert!(std::mem::size_of::<RowOp>() == 64);

/// The monoid unit: M = I, b = 0, no flags.
pub const ROWOP_IDENTITY: RowOp = RowOp {
    f: 0,
    m: [u64::MAX; 3],
    b: [0; 3],
    dead: 0,
};

/// The compose `(x ⨂ y)` — apply x FIRST (x is the earlier action):
/// `(M_x M_y, b_x M_y + b_y)` (Lemma 8). Mask products are bitwise
/// ANDs; `b_x M_y` is a lane-wise blend (zero where y's mask is
/// zero — y's overwrite discards the earlier additive part). The
/// flags OR through: a run-start anywhere in the composed range
/// marks the composite (used by the segmented operator), and dead
/// is sticky by design.
#[inline(always)]
pub fn rowop_compose(x: &RowOp, y: &RowOp) -> RowOp {
    RowOp {
        f: x.f | y.f,
        m: [
            x.m[0] & y.m[0],
            x.m[1] & y.m[1],
            x.m[2] & y.m[2],
        ],
        b: [
            (if y.m[0] != 0 { x.b[0] } else { 0 }).wrapping_add(y.b[0]),
            (if y.m[1] != 0 { x.b[1] } else { 0 }).wrapping_add(y.b[1]),
            (if y.m[2] != 0 { x.b[2] } else { 0 }).wrapping_add(y.b[2]),
        ],
        dead: x.dead | y.dead,
    }
}

/// The segmented operator (Lemma 9): a run start propagates ITSELF
/// — the composite at a thread boundary is the boundary's own
/// row-op, not the cross-thread composition.
#[inline(always)]
pub fn rowop_seg(x: &RowOp, y: &RowOp) -> RowOp {
    if y.f != 0 {
        *y
    } else {
        rowop_compose(x, y)
    }
}

/// THE INCLUSIVE SEGMENTED SCAN — Hillis–Steele over the row-ops
/// (Theorem 29): `d = 1, 2, 4, …`, `e[i] ← e[i−d] ⨂_seg e[i]` with
/// i DESCENDING (load-bearing: `e[i−d]` must read its pre-level
/// value — the descending sweep defers `i−d`'s write until after
/// every `i` that reads it). After ⌈log₂ B⌉ levels, every position
/// holds its id-thread's composite from the thread's run start
/// through itself (the INCLUSIVE prefix). The empty and single
/// slices are no-ops.
pub fn order_scan(e: &mut [RowOp]) {
    let b = e.len();
    let mut d = 1usize;
    while d < b {
        let mut i = b;
        while i > d {
            let x = e[i - 1 - d]; // e[i-1-d]: the earlier block
            let y = e[i - 1]; // e[i-1]:  the later block
            e[i - 1] = rowop_seg(&x, &y);
            i -= 1;
        }
        d <<= 1;
    }
}

/// THE ROW-OP BUILDER (Appendix B.4): one wire message → one
/// row-op. `f = 1` — the row is a run start UNTIL BUCKETED (the
/// decode stage's id-bucketing groups each id's rows contiguously
/// and clears the interior flags; the cells' driver does the same).
///
/// * `A` — M = 0 (overwrite), b = (side, (px << 32) | locate, q):
///   the register is born here (the born-in-batch entry).
/// * `D` — M = 0, b = 0, dead = 1: the thread terminator.
/// * `E/C/X` — M = I, b₂ = −q (wrapping): translate ρ; the clamp
///   is resolved at emit (Theorem 27's table note).
///
/// `U` is NOT built here: the builder's caller expands it to two
/// row-ops (a D-like on the old id, an A-like on the new id).
#[inline(always)]
pub fn rowop_of(act: u8, q: u32, px: u32, locate: u32, side: u8) -> RowOp {
    let mut r = RowOp {
        f: 1,
        m: [0; 3],
        b: [0; 3],
        dead: 0,
    };
    match act {
        ACT_A => {
            r.b[0] = if side != 0 { 1 } else { 0 };
            r.b[1] = ((px as u64) << 32) | locate as u64;
            r.b[2] = q as u64;
        }
        ACT_D => {
            r.dead = 1;
        }
        _ => {
            // E / C / X: translate ρ (the clamp at emit).
            r.m = [u64::MAX; 3];
            r.b[2] = (q as i64).wrapping_neg() as u64;
        }
    }
    r
}

/// THE U-NEW BUILDER: the new register's run-start row (the second
/// of U's two row-ops). The ITCH Replace message carries the new
/// quantity and price but NOT the side — the new register INHERITS
/// the old's s_b — so the row keeps coordinate 0 (M = (1, 0, 0)):
/// the s_b flows from the thread's o_entry (the old thread's state
/// at the U, computed from the scan — Theorem 30's carry-in applied
/// intra-batch), while π and ρ are overwritten from the wire.
#[inline(always)]
pub fn rowop_u_new(new_q: u32, new_px: u32, locate: u32) -> RowOp {
    RowOp {
        f: 1,
        m: [u64::MAX, 0, 0],
        b: [0, ((new_px as u64) << 32) | locate as u64, new_q as u64],
        dead: 0,
    }
}

/// The emit stage's resolved output (Theorem 29(a)): the bilinear
/// forms on the closed-form state + the [S1] §5.6 policies.
/// `ok`: 1 clean, 0 anomaly (no token), 2 clamped (the policy arm:
/// re-anchor the continuation with the overwrite row-op
/// `(M = 0, b = (σ_b, π, 0))`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Resolved {
    pub db: i64,
    pub da: i64,
    pub px: u32,
    pub ok: u8,
}

/// THE EMIT STAGE (Theorem 29(a)): the closed form
/// `o = o_entry·M_pre + b_pre`, then the bilinear output forms and
/// the [S1] §5.6 policies AT EMIT TIME (never at composition time —
/// §4.8(ii)). `pre` is the thread's INCLUSIVE composite at i's
/// PREVIOUS position (i's EXCLUSIVE prefix; the identity at a run
/// start). `o_entry`: `None` = absent (the born-in-batch threads
/// initialize from the A row-op — the prefix's zero masks carry
/// it; the anomaly check keys on the prefix masks exactly as the
/// reference does).
///
/// Wire semantics (the [S1] §5.2/§5.6 table):
/// * `E` — execute q: the delta `−min(q, ρ)` on the σ_b side;
///   `E_SETS_LAST`: the print px is the ORDER's `π` high word.
/// * `C` — cancel-with-print: the delta as E; the px is the
///   WIRE's (the print carries its own price).
/// * `X` — cancel: the delta as E, no px.
/// * `D` — delete: the delta `−ρ` (remove the remaining).
/// * `A` — self-contained: resolved at build time (ok = 1, no
///   deltas).
#[inline(always)]
pub fn emit(
    pre: &RowOp,
    o_entry: Option<&[u64; 3]>,
    act: u8,
    wire_q: u32,
    wire_px: u32,
) -> Resolved {
    let mut out = Resolved {
        db: 0,
        da: 0,
        px: 0,
        ok: 1,
    };
    // Anomaly: a reference (E/C/X/D) with no entry state AND a
    // non-overwriting prefix — the thread never saw an A, so the
    // register is ABSENT (the reference's own check: the prefix's
    // masks are ~0 exactly when nothing zeroed them).
    if act != ACT_A
        && o_entry.is_none()
        && (pre.m[0] | pre.m[1] | pre.m[2]) != 0
    {
        out.ok = 0;
        return out;
    }
    if pre.dead != 0 && act != ACT_A {
        out.ok = 0; // post-delete reference
        return out;
    }
    if act == ACT_A {
        return out; // self-contained
    }
    let e = o_entry.copied().unwrap_or([0; 3]);
    let s_b = (e[0] & pre.m[0]).wrapping_add(pre.b[0]);
    let pi = (e[1] & pre.m[1]).wrapping_add(pre.b[1]);
    let rho = (e[2] & pre.m[2]).wrapping_add(pre.b[2]);
    let mut d = wire_q as i64;
    // The clamp arm is the E/C/X over-cancel policy; U's removal is
    // exact (the ρ leaves the register entirely) and D's d is the ρ
    // itself — neither clamps.
    if act != ACT_U && (rho as i64) < d {
        d = rho as i64;
        out.ok = 2; // the clamp arm
    }
    match act {
        ACT_E => {
            if s_b != 0 {
                out.db = -d;
            } else {
                out.da = -d;
            }
            out.px = (pi >> 32) as u32; // E_SETS_LAST
        }
        ACT_C => {
            if s_b != 0 {
                out.db = -d;
            } else {
                out.da = -d;
            }
            out.px = wire_px;
        }
        ACT_X => {
            if s_b != 0 {
                out.db = -d;
            } else {
                out.da = -d;
            }
        }
        ACT_D => {
            let dd = rho as i64;
            if s_b != 0 {
                out.db = -dd;
            } else {
                out.da = -dd;
            }
        }
        ACT_U => {
            // The combined remove+add (the fabric's U arm: nq − rem on
            // the inherited side; no clamp — the removal is exact).
            let dd = (wire_q as i64) - (rho as i64);
            if s_b != 0 {
                out.db = dd;
            } else {
                out.da = dd;
            }
        }
        _ => {}
    }
    out
}

// ─────────────────────────────────────────────────────────────────────
// The SoA twin — the vector scan's natural layout
// ─────────────────────────────────────────────────────────────────────
//
// One ZMM composes EIGHT ROWS of one field (m0[i..i+8] AND m0[i−d..i−d+8])
// — the structure-of-arrays batch makes every compose step a contiguous
// 64 B load pair. The scalar SoA twin runs the SAME schedule field-wise
// (the no-drift law: AoS reference ≡ SoA scalar ≡ SoA vector — the cells
// pin all three on every shape).

/// The maximum batch (Theorem 29: B ≤ 512 — the SIG_CHUNK cadence).
pub const SCAN_B_MAX: usize = 512;

/// The SoA scan batch: eight B-lane u64 columns (f, m0..m2, b0..b2,
/// dead). `len` is the live row count (`≤ SCAN_B_MAX`); the columns
/// allocate ONCE at construction (the engine path's zero-alloc law —
/// the same discipline as the fabric's SoA).
#[derive(Clone, Debug)]
pub struct ScanSoa {
    pub f: Vec<u64>,
    pub m: [Vec<u64>; 3],
    pub b: [Vec<u64>; 3],
    pub dead: Vec<u64>,
    /// The live row count.
    pub len: usize,
}

impl ScanSoa {
    /// A batch of capacity `cap` (clamped to [`SCAN_B_MAX`]), all rows
    /// the identity (no run starts).
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.min(SCAN_B_MAX);
        Self {
            f: vec![0; cap],
            m: [vec![u64::MAX; cap], vec![u64::MAX; cap], vec![u64::MAX; cap]],
            b: [vec![0; cap], vec![0; cap], vec![0; cap]],
            dead: vec![0; cap],
            len: 0,
        }
    }

    /// Load an AoS slice into the SoA columns (the dual-path bridge).
    pub fn load(&mut self, rows: &[RowOp]) {
        self.len = rows.len().min(SCAN_B_MAX);
        for (i, r) in rows.iter().take(self.len).enumerate() {
            self.f[i] = r.f;
            self.m[0][i] = r.m[0];
            self.m[1][i] = r.m[1];
            self.m[2][i] = r.m[2];
            self.b[0][i] = r.b[0];
            self.b[1][i] = r.b[1];
            self.b[2][i] = r.b[2];
            self.dead[i] = r.dead;
        }
    }

    /// Store back to the AoS form (the cells' compare bridge).
    pub fn store(&self) -> Vec<RowOp> {
        (0..self.len)
            .map(|i| RowOp {
                f: self.f[i],
                m: [self.m[0][i], self.m[1][i], self.m[2][i]],
                b: [self.b[0][i], self.b[1][i], self.b[2][i]],
                dead: self.dead[i],
            })
            .collect()
    }

    /// Push one row (the builder's appender).
    pub fn push(&mut self, r: RowOp) {
        if self.len < self.f.len() {
            self.f[self.len] = r.f;
            for k in 0..3 {
                self.m[k][self.len] = r.m[k];
                self.b[k][self.len] = r.b[k];
            }
            self.dead[self.len] = r.dead;
            self.len += 1;
        }
    }
}

/// The scalar SoA scan — [`order_scan`]'s schedule, field-wise (the
/// reference's per-row compose split into eight column passes per
/// level; identical semantics, identical descending discipline).
pub fn order_scan_soa(s: &mut ScanSoa) {
    let b = s.len;
    let mut d = 1usize;
    while d < b {
        let mut i = b;
        while i > d {
            let lo = i - 1 - d;
            let hi = i - 1;
            // rowop_seg: y.f ? *y : compose(x, y) — field-wise.
            let y_f = s.f[hi];
            for k in 0..3 {
                let xm = s.m[k][lo];
                let ym = s.m[k][hi];
                let xb = s.b[k][lo];
                let yb = s.b[k][hi];
                s.m[k][hi] = if y_f != 0 { ym } else { xm & ym };
                s.b[k][hi] = if y_f != 0 {
                    yb
                } else {
                    (if ym != 0 { xb } else { 0 }).wrapping_add(yb)
                };
            }
            if y_f == 0 {
                s.f[hi] = s.f[lo] | s.f[hi];
                s.dead[hi] = s.dead[lo] | s.dead[hi];
            }
            i -= 1;
        }
        d <<= 1;
    }
}

/// The vector mode (OnceLock; `HFT_ORDER_SCAN_VEC=0` forces scalar —
/// the attribution arm). Read once, outside every measured window.
fn vec_mode() -> bool {
    static M: OnceLock<bool> = OnceLock::new();
    *M.get_or_init(|| {
        std::env::var("HFT_ORDER_SCAN_VEC").as_deref() != Ok("0")
            && is_x86_64_avx512()
    })
}

#[inline]
fn is_x86_64_avx512() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx512f")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// The dispatch: the vector scan when AVX-512F is present and the
/// rollback env is unset; the scalar SoA twin otherwise.
pub fn order_scan_dispatch(s: &mut ScanSoa) {
    if vec_mode() {
        unsafe { order_scan_soa_avx512(s) };
    } else {
        order_scan_soa(s);
    }
}

/// THE VECTOR SCAN (Theorem 29's schedule, Appendix C's mapping):
/// per level d, 8-row groups descending — each group composes
/// `e[i−d..i−d+8] ⨂_seg e[i..i+8]` with one ZMM op per field:
/// m → `VPANDQ`, b → compare+`VMASKMOV`+`VPADDQ` (the blend),
/// f/dead → `VPORQ`; the segment select is one per-field masked
/// move under the SAME per-row mask (y.f != 0) — ~27 uops per
/// 8-row group. The group's loads both precede its stores, and the
/// read/write ranges are disjoint within a level (the module doc's
/// convention 5).
///
/// SAFETY: caller proves AVX-512F (the dispatch's detector).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn order_scan_soa_avx512(s: &mut ScanSoa) {
    use std::arch::x86_64::*;
    let b = s.len;
    let zero = _mm512_setzero_si512();
    let mut d = 1usize;
    while d < b {
        // 8-row groups descending; the group [i, i+8) composes with
        // [i-d, i-d+8) — one ZMM per field, loads before stores, the
        // read/write ranges disjoint within the level (doc §5).
        let mut top = b; // one past the group's last row
        while top > d {
            if top - d >= 8 {
                let i = top - 8;
                // The segment mask: per-row y.f != 0 (select y verbatim).
                let yf = _mm512_loadu_si512(s.f.as_ptr().add(i) as *const __m512i);
                let kmask = _mm512_cmpneq_epu64_mask(yf, zero);
                for k in 0..3 {
                    // m: AND + select. The b-blend below needs y's
                    // PRE-level mask — y.m is loaded ONCE here, before
                    // this field's store (the intra-group ordering law).
                    let x = _mm512_loadu_si512(s.m[k].as_ptr().add(i - d) as *const __m512i);
                    let y = _mm512_loadu_si512(s.m[k].as_ptr().add(i) as *const __m512i);
                    let composed = _mm512_and_si512(x, y);
                    _mm512_storeu_si512(
                        s.m[k].as_mut_ptr().add(i) as *mut __m512i,
                        _mm512_mask_mov_epi64(composed, kmask, y),
                    );
                    // b: (y.m != 0 ? x.b : 0) + y.b, then select.
                    let xb = _mm512_loadu_si512(s.b[k].as_ptr().add(i - d) as *const __m512i);
                    let yb = _mm512_loadu_si512(s.b[k].as_ptr().add(i) as *const __m512i);
                    let bm = _mm512_cmpneq_epu64_mask(y, zero);
                    let blended = _mm512_mask_mov_epi64(zero, bm, xb);
                    let composed_b = _mm512_add_epi64(blended, yb);
                    _mm512_storeu_si512(
                        s.b[k].as_mut_ptr().add(i) as *mut __m512i,
                        _mm512_mask_mov_epi64(composed_b, kmask, yb),
                    );
                }
                // f: OR + select; dead: OR + select (the SAME kmask).
                let xf = _mm512_loadu_si512(s.f.as_ptr().add(i - d) as *const __m512i);
                let composed_f = _mm512_or_si512(xf, yf);
                _mm512_storeu_si512(
                    s.f.as_mut_ptr().add(i) as *mut __m512i,
                    _mm512_mask_mov_epi64(composed_f, kmask, yf),
                );
                let xd = _mm512_loadu_si512(s.dead.as_ptr().add(i - d) as *const __m512i);
                let yd = _mm512_loadu_si512(s.dead.as_ptr().add(i) as *const __m512i);
                let composed_dead = _mm512_or_si512(xd, yd);
                _mm512_storeu_si512(
                    s.dead.as_mut_ptr().add(i) as *mut __m512i,
                    _mm512_mask_mov_epi64(composed_dead, kmask, yd),
                );
                top -= 8;
            } else {
                // The bottom tail (fewer than 8 rows above d): scalar,
                // descending — the reference's discipline.
                let mut i = top;
                while i > d {
                    let y_f = s.f[i - 1];
                    for k in 0..3 {
                        let xm = s.m[k][i - 1 - d];
                        let ym = s.m[k][i - 1];
                        let xb = s.b[k][i - 1 - d];
                        let yb = s.b[k][i - 1];
                        s.m[k][i - 1] = if y_f != 0 { ym } else { xm & ym };
                        s.b[k][i - 1] = if y_f != 0 {
                            yb
                        } else {
                            (if ym != 0 { xb } else { 0 }).wrapping_add(yb)
                        };
                    }
                    if y_f == 0 {
                        s.f[i - 1] = s.f[i - 1 - d] | s.f[i - 1];
                        s.dead[i - 1] = s.dead[i - 1 - d] | s.dead[i - 1];
                    }
                    i -= 1;
                }
                top = d;
            }
        }
        d <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Xs(u64);
    impl Xs {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// The NAIVE segmented scan — the per-thread sequential compose
    /// (each position's inclusive prefix, folded left-to-right within
    /// its run). THE oracle for every scan cell.
    fn naive_scan(rows: &[RowOp]) -> Vec<RowOp> {
        let mut out = vec![ROWOP_IDENTITY; rows.len()];
        let mut i = 0usize;
        while i < rows.len() {
            let mut j = i + 1;
            while j < rows.len() && rows[j].f == 0 {
                j += 1;
            }
            let mut acc = rows[i];
            out[i] = acc;
            for t in i + 1..j {
                acc = rowop_compose(&acc, &rows[t]);
                out[t] = acc;
            }
            i = j;
        }
        out
    }

    /// A random row-op (the fuzz generator: f bernoulli(p) — the
    /// segmentation density; the masks in {0, ~0} — Theorem 27's
    /// DIAGONAL {0,1} DOMAIN, the only domain where the compose's
    /// blend law (the reference's `y->m ? x->b : 0` ternary) is
    /// faithful to the matrix algebra and hence the only domain
    /// where associativity holds; the engine's row-op builder only
    /// ever emits {0, ~0} masks, and the fuzz must match).
    fn rand_rowop(rng: &mut Xs, p_run: u64) -> RowOp {
        RowOp {
            f: if rng.next() % p_run == 0 { 1 } else { 0 },
            m: [
                if rng.next() & 1 == 0 { 0 } else { u64::MAX },
                if rng.next() & 1 == 0 { 0 } else { u64::MAX },
                if rng.next() & 1 == 0 { 0 } else { u64::MAX },
            ],
            b: [rng.next(), rng.next(), rng.next()],
            dead: if rng.next() % 16 == 0 { u64::MAX } else { 0 },
        }
    }

    /// Cell +0: the compose's associativity — 2¹⁶ random triples,
    /// (a⨂b)⨂c == a⨂(b⨂c) — Theorem 27's monoid law, the license
    /// for every re-association below.
    #[test]
    fn t_r38_rowop_associativity_fuzz() {
        let mut rng = Xs(0x5EED_C0DE_3838_0001);
        for _ in 0..(1 << 16) {
            let a = rand_rowop(&mut rng, 8);
            let b = rand_rowop(&mut rng, 8);
            let c = rand_rowop(&mut rng, 8);
            let ab = rowop_compose(&a, &b);
            let left = rowop_compose(&ab, &c);
            let bc = rowop_compose(&b, &c);
            let right = rowop_compose(&a, &bc);
            assert_eq!(left, right, "associativity violated");
            // The identity law.
            assert_eq!(rowop_compose(&ROWOP_IDENTITY, &a), a);
            assert_eq!(rowop_compose(&a, &ROWOP_IDENTITY), a);
        }
    }

    /// Cell +1 (checklist item 9's core): the scan vs the naive
    /// per-thread compose — random batches (B sweep incl. 512, random
    /// segmentation densities, random masks) — AND the three-layout
    /// law: AoS reference ≡ SoA scalar ≡ SoA vector (the no-drift
    /// discipline; the vector path is the dispatch's, whatever the
    /// host).
    #[test]
    fn t_r38_scan_vs_naive_fuzz() {
        let mut rng = Xs(0x5EED_C0DE_3838_0002);
        let shapes: Vec<usize> = vec![
            0, 1, 2, 3, 7, 8, 9, 15, 16, 31, 32, 63, 64, 100, 128, 255, 256, 511, 512,
        ];
        for &b in &shapes {
            for &p_run in &[2u64, 4, 8, 64, 512] {
                let rows: Vec<RowOp> =
                    (0..b).map(|_| rand_rowop(&mut rng, p_run)).collect();
                let want = naive_scan(&rows);
                // AoS reference.
                let mut aos = rows.clone();
                order_scan(&mut aos);
                assert_eq!(aos, want, "AOS scan != naive at b={b} p={p_run}");
                // SoA scalar.
                let mut soa = ScanSoa::with_capacity(b.max(1));
                soa.load(&rows);
                order_scan_soa(&mut soa);
                assert_eq!(soa.store(), want, "SoA scalar != naive at b={b} p={p_run}");
                // SoA vector (the dispatch — forced on in the cell).
                let mut soa2 = ScanSoa::with_capacity(b.max(1));
                soa2.load(&rows);
                unsafe { order_scan_soa_avx512(&mut soa2) };
                assert_eq!(soa2.store(), want, "SoA vector != naive at b={b} p={p_run}");
            }
        }
        // Random-B batches.
        for _ in 0..64 {
            let b = (rng.next() % 512 + 1) as usize;
            let rows: Vec<RowOp> = (0..b).map(|_| rand_rowop(&mut rng, 5)).collect();
            let want = naive_scan(&rows);
            let mut aos = rows.clone();
            order_scan(&mut aos);
            assert_eq!(aos, want, "AOS scan != naive (random B)");
            let mut soa = ScanSoa::with_capacity(b);
            soa.load(&rows);
            unsafe { order_scan_soa_avx512(&mut soa) };
            assert_eq!(soa.store(), want, "SoA vector != naive (random B)");
        }
    }

    /// Cell +24 (the boundary law): threads aligned and misaligned to
    /// every power-of-two boundary — the segment flag's reset must be
    /// exact wherever the run boundaries fall relative to the level
    /// strides. Includes the degenerate regimes: one thread (all
    /// same), B threads (all distinct), 1-element threads, and every
    /// residue of a boundary position mod 64.
    #[test]
    fn t_r38_seg_boundary_laws() {
        let b = 512usize;
        // (a) one thread: all rows f=0 except the first.
        let mut rows = vec![rand_rowop(&mut Xs(1), u64::MAX); b];
        for r in rows.iter_mut().skip(1) {
            r.f = 0;
        }
        let want = naive_scan(&rows);
        let mut aos = rows.clone();
        order_scan(&mut aos);
        assert_eq!(aos, want, "one-thread boundary law");
        // (b) all-distinct: every row f=1.
        let rows: Vec<RowOp> = (0..b).map(|_| rand_rowop(&mut Xs(2), u64::MAX)).collect();
        let want = naive_scan(&rows);
        let mut aos = rows.clone();
        order_scan(&mut aos);
        assert_eq!(aos, want, "all-distinct boundary law");
        // (c) every boundary offset: threads of length L starting at
        // each residue.
        for &len in &[1usize, 2, 3, 5, 8, 13, 64] {
            for off in 0..64usize {
                let mut rows = Vec::with_capacity(b);
                let mut rng = Xs(0xB0FF_3838_0000 | off as u64);
                let mut i = 0usize;
                while i < b {
                    let r = rand_rowop(&mut rng, u64::MAX);
                    if i % len == off % len.max(1) {
                        rows.push(r);
                    } else {
                        let mut q = r;
                        q.f = 0;
                        rows.push(q);
                    }
                    i += 1;
                }
                let want = naive_scan(&rows);
                let mut soa = ScanSoa::with_capacity(b);
                soa.load(&rows);
                unsafe { order_scan_soa_avx512(&mut soa) };
                assert_eq!(
                    soa.store(),
                    want,
                    "boundary law at len={len} off={off}"
                );
            }
        }
    }

    /// Cell +22 (the hot-ID case): 512 references on ONE order — the
    /// adversarial chain. The scan must stay exact (and its depth is
    /// 9 levels regardless — the schedule, not the data, fixes the
    /// depth; the naive oracle's serial chain is the correctness
    /// reference, the vector scan's latency is the win).
    #[test]
    fn t_r38_hot_id_chain() {
        let b = 512usize;
        // E-chain on one id: row 0 the A, rows 1.. all E (f=0).
        let a = rowop_of(ACT_A, 700, 10_099, 42, 1);
        let mut rows = vec![a; b];
        rows[0].f = 1;
        for r in rows.iter_mut().skip(1) {
            *r = rowop_of(ACT_E, 3, 0, 0, 0);
            r.f = 0;
        }
        let want = naive_scan(&rows);
        let mut aos = rows.clone();
        order_scan(&mut aos);
        assert_eq!(aos, want, "hot-id AoS");
        let mut soa = ScanSoa::with_capacity(b);
        soa.load(&rows);
        unsafe { order_scan_soa_avx512(&mut soa) };
        assert_eq!(soa.store(), want, "hot-id vector");
        // The closed form: position 511's prefix composes the A and
        // 510 E's — ρ = 700 − 510*3 = 470 (the scan carries it).
        let pre511 = &want[511];
        assert_eq!(pre511.m, [0; 3], "the A's overwrite propagated");
        assert_eq!(pre511.b[0], 1, "side: bid");
        assert_eq!(pre511.b[2], 700u64.wrapping_sub(3 * 511), "rho chain");
        assert_eq!(pre511.dead, 0);
    }

    /// The pull-model oracle (the [S1] §5.2/§5.6 table — the fabric's
    /// own semantics, in miniature): a HashMap register file, one
    /// probe per reference. THE differential target for the push
    /// model (Theorem 29(a): bit-exact sequential semantics).
    #[derive(Clone)]
    struct PullReg {
        s_b: u64,
        pi: u64,
        rho: u64,
        dead: bool,
    }

    #[derive(Clone, Copy, Debug)]
    struct Msg {
        act: u8,
        id: u64,
        q: u32,
        px: u32,
        locate: u32,
        side: u8,
        // U's new register.
        new_id: u64,
        new_q: u32,
        new_px: u32,
    }

    fn pull_resolve(
        msgs: &[Msg],
        regs: &mut std::collections::HashMap<u64, PullReg>,
    ) -> Vec<Resolved> {
        let mut out = Vec::with_capacity(msgs.len());
        for m in msgs {
            match m.act {
                ACT_A => {
                    regs.insert(
                        m.id,
                        PullReg {
                            s_b: if m.side != 0 { 1 } else { 0 },
                            pi: ((m.px as u64) << 32) | m.locate as u64,
                            rho: m.q as u64,
                            dead: false,
                        },
                    );
                    out.push(Resolved {
                        db: 0,
                        da: 0,
                        px: 0,
                        ok: 1,
                    });
                }
                ACT_U => {
                    match regs.get(&m.id) {
                        Some(r) if !r.dead => {
                            let delta = (m.new_q as i64) - (r.rho as i64);
                            let (mut db, mut da) = (0i64, 0i64);
                            if r.s_b != 0 {
                                db = delta;
                            } else {
                                da = delta;
                            }
                            let old_side = r.s_b;
                            regs.remove(&m.id);
                            regs.insert(
                                m.new_id,
                                PullReg {
                                    s_b: old_side,
                                    pi: ((m.new_px as u64) << 32) | m.locate as u64,
                                    rho: m.new_q as u64,
                                    dead: false,
                                },
                            );
                            out.push(Resolved {
                                db,
                                da,
                                px: 0,
                                ok: 1,
                            });
                        }
                        _ => out.push(Resolved {
                            db: 0,
                            da: 0,
                            px: 0,
                            ok: 0,
                        }),
                    }
                }
                act => {
                    match regs.get(&m.id) {
                        Some(r) if !r.dead => {
                            let mut res = Resolved {
                                db: 0,
                                da: 0,
                                px: 0,
                                ok: 1,
                            };
                            let (mut d, px) = match act {
                                ACT_E => (m.q as i64, (r.pi >> 32) as u32),
                                ACT_C => (m.q as i64, m.px),
                                ACT_X => (m.q as i64, 0),
                                ACT_D => (r.rho as i64, 0),
                                _ => (0, 0),
                            };
                            if act != ACT_D && (r.rho as i64) < d {
                                d = r.rho as i64;
                                res.ok = 2;
                            }
                            let dd = -d;
                            if r.s_b != 0 {
                                res.db = dd;
                            } else {
                                res.da = dd;
                            }
                            res.px = px;
                            // Update the register.
                            let nr = match act {
                                ACT_D => PullReg {
                                    s_b: r.s_b,
                                    pi: r.pi,
                                    rho: 0,
                                    dead: true,
                                },
                                ACT_E | ACT_C | ACT_X => PullReg {
                                    s_b: r.s_b,
                                    pi: r.pi,
                                    rho: r.rho.wrapping_sub(d as u64),
                                    dead: false,
                                },
                                _ => PullReg {
                                    s_b: r.s_b,
                                    pi: r.pi,
                                    rho: r.rho,
                                    dead: false,
                                },
                            };
                            regs.insert(m.id, nr);
                            out.push(res);
                        }
                        _ => out.push(Resolved {
                            db: 0,
                            da: 0,
                            px: 0,
                            ok: 0,
                        }),
                    }
                }
            }
        }
        out
    }

    /// The push-model driver: bucket the stream by id (each id's
    /// messages in tape order, one thread per id), build the row-ops
    /// (a U contributes its D-like to the OLD id's bucket and its
    /// [`rowop_u_new`] run-start to the NEW id's bucket — one row
    /// per bucket-visit), run the scan, compute the U-new threads'
    /// entries (the old thread's closed-form state at the U — the
    /// intra-batch carry-in), then emit per ORIGINAL message
    /// position. Born-in-batch registers: the A rows carry them.
    fn push_resolve(msgs: &[Msg]) -> Vec<Resolved> {
        push_resolve_inner(msgs, &std::collections::HashMap::new(), true)
    }

    /// The shared driver: `pre_state` supplies the carry-in entries
    /// (the pipeline register); `assert_paths` runs the dual-path
    /// scan law (the cells' driver; the engine path skips it).
    fn push_resolve_inner(
        msgs: &[Msg],
        pre_state: &std::collections::HashMap<u64, PullReg>,
        assert_paths: bool,
    ) -> Vec<Resolved> {
        use std::collections::HashMap;
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, m) in msgs.iter().enumerate() {
            match m.act {
                ACT_U => {
                    buckets.entry(m.id).or_default().push(i);
                    buckets.entry(m.new_id).or_default().push(i);
                }
                _ => {
                    buckets.entry(m.id).or_default().push(i);
                }
            }
        }
        let mut rows: Vec<RowOp> = Vec::with_capacity(msgs.len() + 16);
        let mut pos_of: HashMap<usize, usize> = HashMap::new(); // msg -> the emit row
        let mut u_new_row: HashMap<usize, usize> = HashMap::new(); // U msg -> the A-like row
        let mut thread_id_of_row0: HashMap<usize, u64> = HashMap::new();
        for (id, idxs) in &buckets {
            let first_row = rows.len();
            thread_id_of_row0.insert(first_row, *id);
            for &mi in idxs {
                let m = &msgs[mi];
                match m.act {
                    ACT_A => {
                        pos_of.insert(mi, rows.len());
                        rows.push(rowop_of(ACT_A, m.q, m.px, m.locate, m.side));
                    }
                    ACT_U => {
                        if *id == m.id {
                            pos_of.insert(mi, rows.len());
                            rows.push(rowop_of(ACT_D, 0, 0, 0, 0));
                        } else {
                            u_new_row.insert(mi, rows.len());
                            rows.push(rowop_u_new(m.new_q, m.new_px, m.locate));
                        }
                    }
                    act => {
                        pos_of.insert(mi, rows.len());
                        rows.push(rowop_of(act, m.q, m.px, m.locate, 0));
                    }
                }
            }
            let first = rows.len() - idxs.len();
            for r in rows[first + 1..].iter_mut() {
                r.f = 0;
            }
        }
        // The pre-scan run-start map (the scan's composites OR the
        // flags through, so the ORIGINAL rows are the only exact
        // thread-boundary record).
        let orig_f: Vec<u64> = rows.iter().map(|r| r.f).collect();

        // Scan (all three paths — the dual-path law at the driver).
        let want_aos = naive_scan(&rows);
        let mut aos = rows.clone();
        order_scan(&mut aos);
        if assert_paths {
            assert_eq!(aos, want_aos, "push driver: AoS scan");
            let mut soa = ScanSoa::with_capacity(rows.len().max(1));
            soa.load(&rows);
            unsafe { order_scan_soa_avx512(&mut soa) };
            assert_eq!(soa.store(), want_aos, "push driver: vector scan");
        }
        let scanned = aos;

        // The thread-start rows (ascending).
        let starts: Vec<usize> = orig_f
            .iter()
            .enumerate()
            .filter(|(_, &f)| f != 0)
            .map(|(i, _)| i)
            .collect();
        let thread_of_row = |ri: usize| -> usize {
            match starts.binary_search(&ri) {
                Ok(k) => starts[k],
                Err(k) => {
                    if k == 0 {
                        0
                    } else {
                        starts[k - 1]
                    }
                }
            }
        };
        // The per-thread base entry (born: None; carry-in: the id's
        // pre-batch state).
        let thread_entry = |t0: usize| -> Option<[u64; 3]> {
            thread_id_of_row0
                .get(&t0)
                .and_then(|id| pre_state.get(id))
                .filter(|r| !r.dead)
                .map(|r| [r.s_b, r.pi, r.rho])
        };
        // The exclusive prefix at a row.
        let pre_of = |ri: usize| -> RowOp {
            if ri == 0 || orig_f[ri] != 0 {
                ROWOP_IDENTITY
            } else {
                scanned[ri - 1]
            }
        };
        // The closed-form state at a row (the pipeline register's
        // write): o = entry·M_pre + b_pre.
        let state_at = |ri: usize| -> [u64; 3] {
            let pre = pre_of(ri);
            let e = thread_entry(thread_of_row(ri)).unwrap_or([0; 3]);
            [
                (e[0] & pre.m[0]).wrapping_add(pre.b[0]),
                (e[1] & pre.m[1]).wrapping_add(pre.b[1]),
                (e[2] & pre.m[2]).wrapping_add(pre.b[2]),
            ]
        };

        // The U-new threads' entries: the OLD thread's state at the
        // U (the s_b flows through rowop_u_new's M = (1,0,0)).
        let mut u_new_entry: HashMap<usize, [u64; 3]> = HashMap::new(); // A-like row -> entry
        for (mi, ri) in &u_new_row {
            let d_ri = pos_of[mi]; // the U's D-like row (the old thread)
            u_new_entry.insert(*ri, state_at(d_ri));
        }

        // EMIT + THE CLAMP RE-ANCHOR LOOP (Theorem 29(a)'s policy
        // arm): a violating position (ok = 2, the over-cancel clamp)
        // re-anchors its thread's CONTINUATION with the position-
        // local overwrite row-op (M = 0, b = (σ_b, π, 0)) — the [S1]
        // register's ρ never goes below zero, and the raw affine
        // chain must match: without the re-anchor, a later reference
        // in the same thread would see the underflowed (wrapping)
        // ρ and clamp against a negative value. Each round fixes at
        // least one violating position exactly (its post-ρ = 0) and
        // the translation mass only decreases — monotone, converges
        // in ≤ the violating positions' count. Well-formed feeds
        // (Assumption W) never violate: the loop exits after the
        // first emit.
        let mut work = rows;
        let mut out;
        let mut rewritten: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        loop {
            // The scan on the CURRENT rows (the re-anchored ones on
            // later rounds).
            let scanned = {
                let mut aos = work.clone();
                order_scan(&mut aos);
                aos
            };
            let orig_f: Vec<u64> = work.iter().map(|r| r.f).collect();
            let starts_cur: Vec<usize> = orig_f
                .iter()
                .enumerate()
                .filter(|(_, &f)| f != 0)
                .map(|(i, _)| i)
                .collect();
            let pre_of = |ri: usize| -> RowOp {
                if ri == 0 || orig_f[ri] != 0 {
                    ROWOP_IDENTITY
                } else {
                    scanned[ri - 1]
                }
            };
            // The thread-start lookup + entries (recomputed per round).
            let thread_of = |ri: usize| -> usize {
                match starts_cur.binary_search(&ri) {
                    Ok(k) => starts_cur[k],
                    Err(k) => {
                        if k == 0 {
                            0
                        } else {
                            starts_cur[k - 1]
                        }
                    }
                }
            };
            let thread_ent = |t0: usize| -> Option<[u64; 3]> {
                thread_id_of_row0
                    .get(&t0)
                    .and_then(|id| pre_state.get(id))
                    .filter(|r| !r.dead)
                    .map(|r| [r.s_b, r.pi, r.rho])
            };
            let state_at = |ri: usize| -> [u64; 3] {
                let pre = pre_of(ri);
                let e = thread_ent(thread_of(ri)).unwrap_or([0; 3]);
                [
                    (e[0] & pre.m[0]).wrapping_add(pre.b[0]),
                    (e[1] & pre.m[1]).wrapping_add(pre.b[1]),
                    (e[2] & pre.m[2]).wrapping_add(pre.b[2]),
                ]
            };
            // The U-new threads' entries (per round).
            let mut u_ent: HashMap<usize, [u64; 3]> = HashMap::new();
            for (mi, ri) in &u_new_row {
                let d_ri = pos_of[mi];
                u_ent.insert(*ri, state_at(d_ri));
            }
            // Emit.
            out = vec![Resolved::default(); msgs.len()];
            let mut violations: Vec<usize> = Vec::new();
            for (mi, m) in msgs.iter().enumerate() {
                let ri = pos_of[&mi];
                let pre = pre_of(ri);
                let (act, wq, wpx) = match m.act {
                    ACT_U => (ACT_U, m.new_q, m.new_px),
                    a => (a, m.q, m.px),
                };
                let t0 = thread_of(ri);
                let ent: Option<[u64; 3]> = if let Some(e) = u_ent.get(&t0) {
                    Some(*e)
                } else {
                    thread_ent(t0)
                };
                out[mi] = emit(&pre, ent.as_ref(), act, wq, wpx);
                if out[mi].ok == 2 {
                    violations.push(ri);
                }
            }
            // Only the FIRST violation at each position re-anchors
            // (the position's own ok = 2 output is the CORRECT clamp
            // and fires on every round; the rewrite is idempotent and
            // must not loop).
            let fresh_v: Vec<usize> = violations
                .iter()
                .copied()
                .filter(|ri| !rewritten.contains(ri))
                .collect();
            if fresh_v.is_empty() {
                break;
            }
            // Re-anchor: the overwrite row-op with the position's
            // pre-state identity (σ_b, π) and ρ = 0.
            for ri in fresh_v {
                let st = state_at(ri);
                let f = orig_f[ri];
                work[ri] = RowOp {
                    f,
                    m: [0; 3],
                    b: [st[0], st[1], 0],
                    dead: 0,
                };
                rewritten.insert(ri);
            }
        }
        out
    }

    /// Cell +23 (the policy battery): the emit closed form vs the
    /// pull oracle on adversarial lifecycles — over-cancel/over-
    /// execute clamps, E_SETS_LAST, C's wire px, post-delete
    /// references, absent references, D's remove-remaining, U's
    /// combined delta — every (ok, db, da, px) compared.
    #[test]
    fn t_r38_emit_policies() {
        let mut rng = Xs(0x5EED_C0DE_3838_0003);
        // The hand-built adversaries first.
        let hand: Vec<Vec<Msg>> = vec![
            // Over-execute: A(100) then E(150) — the clamp arm (ok=2,
            // delta -100, rho 0).
            vec![
                Msg { act: ACT_A, id: 1, q: 100, px: 9900, locate: 7, side: 1, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 1, q: 150, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // E_SETS_LAST: the print px is the ORDER's, not the wire's.
            vec![
                Msg { act: ACT_A, id: 2, q: 10, px: 12_345, locate: 3, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 2, q: 4, px: 99_999, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // C carries its own px.
            vec![
                Msg { act: ACT_A, id: 3, q: 50, px: 100, locate: 9, side: 1, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_C, id: 3, q: 20, px: 555, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // Post-delete reference: A, D, E — the E is an anomaly.
            vec![
                Msg { act: ACT_A, id: 4, q: 80, px: 700, locate: 1, side: 1, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_D, id: 4, q: 0, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 4, q: 10, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // Absent reference: E without an A — an anomaly.
            vec![
                Msg { act: ACT_E, id: 5, q: 10, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // D removes the remaining.
            vec![
                Msg { act: ACT_A, id: 6, q: 77, px: 800, locate: 2, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_X, id: 6, q: 7, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_D, id: 6, q: 0, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // U: the combined delta nq - rem on the inherited side.
            vec![
                Msg { act: ACT_A, id: 7, q: 30, px: 400, locate: 5, side: 1, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_X, id: 7, q: 5, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_U, id: 7, q: 0, px: 0, locate: 0, side: 0, new_id: 8, new_q: 12, new_px: 410 },
            ],
            // The single-clamp chain: A(100), E(60), E(60) — the
            // second E clamps (ok=2, delta -40, ρ→0).
            vec![
                Msg { act: ACT_A, id: 11, q: 100, px: 500, locate: 4, side: 1, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 11, q: 60, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 11, q: 60, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // The stacking-underflow chain + a third reference: the
            // re-anchor must feed ρ=0 to the tail.
            vec![
                Msg { act: ACT_A, id: 12, q: 100, px: 500, locate: 4, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 12, q: 60, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 12, q: 60, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_X, id: 12, q: 30, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
            // U then a reference on the OLD id (anomaly) and the NEW
            // (resolves from the A-like row).
            vec![
                Msg { act: ACT_A, id: 9, q: 40, px: 600, locate: 6, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_U, id: 9, q: 0, px: 0, locate: 0, side: 0, new_id: 10, new_q: 22, new_px: 620 },
                Msg { act: ACT_E, id: 9, q: 5, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
                Msg { act: ACT_E, id: 10, q: 2, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            ],
        ];
        for stream in &hand {
            let mut regs = std::collections::HashMap::new();
            let want = pull_resolve(stream, &mut regs);
            let got = push_resolve(stream);
            assert_eq!(got, want, "policy stream {stream:?}");
        }
        // The fuzz battery: 4×10³ random WELL-FORMED streams —
        // Assumption W ([S1] §5.6.5): ITCH order ids are unique per
        // lifetime, so an id is never re-Add-ed after its D or its
        // U-away (the reference's sticky `dead` is contract-exact on
        // this domain; the re-birth shape is the anomaly path's own
        // regime, outside the transducer's contract).
        for t in 0..4000 {
            let n = (rng.next() % 48 + 1) as usize;
            let mut retired: Vec<u64> = Vec::new();
            let mut fresh: u64 = 100;
            // The inline register simulation (the in-budget law: the
            // E/C/X quantities bounded by the register's remaining —
            // Assumption W's budget discipline; the over-cancel clamp
            // arms are pinned by the hand cases above, and the
            // STACKING underflow chains are the R38 queue's residue).
            let mut sim: std::collections::HashMap<u64, u64> =
                std::collections::HashMap::new();
            let mut stream = Vec::with_capacity(n);
            for _ in 0..n {
                let act = [ACT_A, ACT_E, ACT_C, ACT_X, ACT_D, ACT_U]
                    [(rng.next() % 6) as usize];
                let mut id = rng.next() % 6 + 1;
                if act == ACT_A && retired.contains(&id) {
                    fresh += 1;
                    id = fresh;
                }
                if act == ACT_D || act == ACT_U {
                    retired.push(id);
                }
                let new_id = if act == ACT_U {
                    fresh += 1;
                    fresh
                } else {
                    0
                };
                // The quantity: in-budget for the references.
                let q = match act {
                    ACT_A => (rng.next() % 300) as u32,
                    ACT_E | ACT_C | ACT_X => {
                        let rho = sim.get(&id).copied().unwrap_or(0);
                        (rng.next() % (rho + 1)) as u32
                    }
                    _ => 0,
                };
                // The simulation bookkeeping.
                match act {
                    ACT_A => {
                        sim.insert(id, q as u64);
                    }
                    ACT_E | ACT_C | ACT_X => {
                        if let Some(r) = sim.get_mut(&id) {
                            *r -= q as u64;
                        }
                    }
                    ACT_D => {
                        sim.remove(&id);
                    }
                    ACT_U => {
                        sim.remove(&id);
                    }
                    _ => {}
                }
                let new_q = (rng.next() % 200) as u32;
                if act == ACT_U {
                    sim.insert(new_id, new_q as u64);
                }
                stream.push(Msg {
                    act,
                    id,
                    q,
                    px: (rng.next() % 100_000) as u32,
                    locate: (rng.next() % 8907) as u32,
                    side: (rng.next() & 1) as u8,
                    new_id,
                    new_q,
                    new_px: (rng.next() % 100_000) as u32,
                });
            }
            let mut regs = std::collections::HashMap::new();
            let want = pull_resolve(&stream, &mut regs);
            let got = push_resolve(&stream);
            for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
                assert_eq!(g, w, "fuzz stream {t} pos {i}: {stream:?}");
            }
        }
    }

    /// Cell +25 (the U dual-register): chains where U's NEW id is
    /// referenced later in-batch — the new register's thread starts
    /// from the A-like row (M = 0, b = (s', π', q')), NOT from ⊥.
    /// (Covered inside the policy battery's hand cases 7/8 and the
    /// fuzz; this cell pins it in isolation with a long chain.)
    #[test]
    fn t_r38_u_dual_register() {
        let stream: Vec<Msg> = vec![
            Msg { act: ACT_A, id: 100, q: 500, px: 1_000, locate: 11, side: 1, new_id: 0, new_q: 0, new_px: 0 },
            Msg { act: ACT_E, id: 100, q: 100, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            Msg { act: ACT_U, id: 100, q: 0, px: 0, locate: 0, side: 0, new_id: 200, new_q: 350, new_px: 1_100 },
            Msg { act: ACT_E, id: 200, q: 50, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            Msg { act: ACT_C, id: 200, q: 25, px: 1_111, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            Msg { act: ACT_X, id: 200, q: 10, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
            Msg { act: ACT_D, id: 200, q: 0, px: 0, locate: 0, side: 0, new_id: 0, new_q: 0, new_px: 0 },
        ];
        let mut regs = std::collections::HashMap::new();
        let want = pull_resolve(&stream, &mut regs);
        let got = push_resolve(&stream);
        assert_eq!(got, want, "the U dual-register chain");
        // The new thread resolved from the A-like row: E(50) at px
        // 1_100 (E_SETS_LAST from the REPLACED register).
        assert_eq!(got[3].px, 1_100);
        assert_eq!(got[3].db, -50); // the inherited side: bid
    }

    /// The carry-in law (Theorem 30): two batches in sequence — the
    /// second batch's threads carry the first batch's terminal
    /// closed-form states as their o_entry (the pipeline register).
    /// The push driver's multi-batch twin: batch 1 resolves and
    /// updates the oracle; batch 2's entries = the register states
    /// at the batch boundary.
    #[test]
    fn t_r38_carry_in_two_batch() {
        let mut rng = Xs(0x5EED_C0DE_3838_0004);
        for t in 0..800 {
            let ids: Vec<u64> = (0..5).map(|_| rng.next() % 4 + 1).collect();
            let mut regs = std::collections::HashMap::new();
            for batch_no in 0..2 {
                let n = (rng.next() % 20 + 1) as usize;
                // Well-formed: no id re-Add after its D/U-away
                // (Assumption W).
                let mut retired: Vec<u64> = Vec::new();
                // The in-budget law (the same as the policy fuzz).
                let mut sim: std::collections::HashMap<u64, u64> =
                    std::collections::HashMap::new();
                let mut stream = Vec::with_capacity(n);
                for _ in 0..n {
                    let act = [ACT_A, ACT_E, ACT_C, ACT_X, ACT_D, ACT_U]
                        [(rng.next() % 6) as usize];
                    let mut id = ids[(rng.next() % ids.len() as u64) as usize];
                    if act == ACT_A && retired.contains(&id) {
                        id += 1000; // a fresh id (outside the 1..4 pool)
                    }
                    if act == ACT_D || act == ACT_U {
                        retired.push(id);
                    }
                    let q = match act {
                        ACT_A => (rng.next() % 250) as u32,
                        ACT_E | ACT_C | ACT_X => {
                            let rho = sim.get(&id).copied().unwrap_or(0);
                            (rng.next() % (rho + 1)) as u32
                        }
                        _ => 0,
                    };
                    match act {
                        ACT_A => {
                            sim.insert(id, q as u64);
                        }
                        ACT_E | ACT_C | ACT_X => {
                            if let Some(r) = sim.get_mut(&id) {
                                *r -= q as u64;
                            }
                        }
                        ACT_D | ACT_U => {
                            sim.remove(&id);
                        }
                        _ => {}
                    }
                    let new_id = rng.next() % 4 + 30;
                    let new_q = (rng.next() % 150) as u32;
                    if act == ACT_U {
                        sim.insert(new_id, new_q as u64);
                    }
                    stream.push(Msg {
                        act,
                        id,
                        q,
                        px: (rng.next() % 90_000) as u32,
                        locate: (rng.next() % 500) as u32,
                        side: (rng.next() & 1) as u8,
                        new_id,
                        new_q,
                        new_px: (rng.next() % 90_000) as u32,
                    });
                }
                // The SNAPSHOT before the pull (the pipeline
                // register: each id's state at the batch boundary).
                let snapshot: std::collections::HashMap<u64, PullReg> =
                    regs.clone();
                let want = pull_resolve(&stream, &mut regs);
                let got = push_resolve_with_entries(&stream, &snapshot);
                for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
                    assert_eq!(g, w, "carry-in {t} batch {batch_no} pos {i}: {stream:?}");
                }
            }
        }
    }

    /// The push driver with carry-in entries: the same bucketing and
    /// scan, but each thread whose id lives in `pre_state` gets
    /// o_entry = Some(state) at its first reference (and the born
    /// threads keep None — the A rows).
    fn push_resolve_with_entries(
        msgs: &[Msg],
        pre_state: &std::collections::HashMap<u64, PullReg>,
    ) -> Vec<Resolved> {
        use std::collections::HashMap;
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, m) in msgs.iter().enumerate() {
            match m.act {
                ACT_U => {
                    buckets.entry(m.id).or_default().push(i);
                    buckets.entry(m.new_id).or_default().push(i);
                }
                _ => {
                    buckets.entry(m.id).or_default().push(i);
                }
            }
        }
        let mut rows: Vec<RowOp> = Vec::with_capacity(msgs.len() + 16);
        let mut pos_of: HashMap<usize, usize> = HashMap::new();
        // (thread first row) -> the id (for the entry lookup).
        let mut thread_id_of_row0: HashMap<usize, u64> = HashMap::new();
        for (id, idxs) in &buckets {
            let first_row = rows.len();
            thread_id_of_row0.insert(first_row, *id);
            for &mi in idxs {
                let m = &msgs[mi];
                match m.act {
                    ACT_A => {
                        pos_of.insert(mi, rows.len());
                        rows.push(rowop_of(ACT_A, m.q, m.px, m.locate, m.side));
                    }
                    ACT_U => {
                        if *id == m.id {
                            pos_of.insert(mi, rows.len());
                            rows.push(rowop_of(ACT_D, 0, 0, 0, 0));
                        } else {
                            rows.push(rowop_of(ACT_A, m.new_q, m.new_px, 0, m.side));
                        }
                    }
                    act => {
                        pos_of.insert(mi, rows.len());
                        rows.push(rowop_of(act, m.q, m.px, m.locate, 0));
                    }
                }
            }
            for r in rows[first_row + 1..].iter_mut() {
                r.f = 0;
            }
        }
        let orig_f: Vec<u64> = rows.iter().map(|r| r.f).collect();
        let mut aos = rows.clone();
        order_scan(&mut aos);
        let scanned = aos;
        let mut out = vec![Resolved::default(); msgs.len()];
        // The thread of each row: the last thread-start at or below
        // the row (the pre-scan flags).
        let mut starts: Vec<usize> = Vec::new();
        for (i, &f) in orig_f.iter().enumerate() {
            if f != 0 {
                starts.push(i);
            }
        }
        for (mi, m) in msgs.iter().enumerate() {
            let ri = pos_of[&mi];
            let pre = if ri == 0 || orig_f[ri] != 0 {
                ROWOP_IDENTITY
            } else {
                scanned[ri - 1]
            };
            let (act, wq, wpx) = match m.act {
                ACT_U => (ACT_U, m.new_q, m.new_px),
                a => (a, m.q, m.px),
            };
            // The entry: the thread's id (the enclosing thread-start
            // at or below ri) looked up in the pre-batch state.
            let t0 = match starts.binary_search(&ri) {
                Ok(k) => starts[k],
                Err(k) => {
                    if k == 0 {
                        0
                    } else {
                        starts[k - 1]
                    }
                }
            };
            let ent = thread_id_of_row0
                .get(&t0)
                .and_then(|id| pre_state.get(id))
                .filter(|r| !r.dead)
                .map(|r| [r.s_b, r.pi, r.rho]);
            out[mi] = emit(&pre, ent.as_ref(), act, wq, wpx);
        }
        out
    }
}
