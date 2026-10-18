//! R38 (Run 900, Gate 13 — THE L3 WIRING): the 𝒜₅.₀ bake's PUSH
//! resolve path — docs/solution2.md Part III's Theorem-29 kernel
//! ([`crate::orderscan`]) wired into the fabric's Phase R as the
//! ACTIVE engine resolution (the `HFT_ORDER_SCAN` arm; `=0` restores
//! the Phase-R [`OrderCache`]-pull path verbatim — the attribution /
//! rollback arm).
//!
//! # What this module is
//!
//! The bake's resolve stage (`bake_sig_fabric`'s pass 2) produced the
//! per-message token tuples `(seq, px, db, da)` through the
//! single-owner hash-table pull (one probe per E/C/X/D/U reference).
//! This module produces THE IDENTICAL TUPLES through the push model:
//! a per-batch id-bucketed segmented scan ([`order_scan_dispatch`])
//! whose closed forms carry each register's prefix state TO its
//! reference positions — the probe's serial dependency chain (depth
//! up to B per thread) re-associated to ⌈log₂ B⌉ = 9 levels WHATEVER
//! the id distribution (Theorem 29(b)). The bake is untimed, so this
//! is not itself a rate lever; it is the [S1] §6.4 prohibition's
//! ALGEBRAIC LIFT made real in the engine (Theorem 31's prerequisite:
//! the push model is the dual-lane resolution's soundness condition)
//! and the B3 wall's dataflow inversion proven at the fabric level.
//!
//! # The port's laws (each pinned by the parity cell
//! `t_r38_bake_push_pull_parity` — the differential twin vs the pull
//! path, the project's own no-drift discipline)
//!
//! 1. **ROW-BUDGET BATCHING** — a batch holds ≤ [`SCAN_B_MAX`] = 512
//!    ROWS (each book message contributes one row; a U contributes
//!    two — the D-like on the old id's thread and the A-like
//!    run-start on the new id's thread). A message never splits
//!    across batches: the u_old/u_new pair must scan together (the
//!    u_new entry is the old thread's closed form at the u_old row).
//! 2. **ID-BUCKETING** — within a batch, each id's rows are
//!    contiguous (first-appearance bucket order — deterministic);
//!    the scan's segmented operator makes per-thread composites
//!    bucket-order independent, and the closed forms are exact for
//!    ANY batch split (Theorem 30's carry-in).
//! 3. **THE RUN-START LAW** — `f = 1` on every bucket's first row,
//!    every `A` row, and every u_new row; interior ref/D rows are
//!    cleared. An A as a run start is the pull's insert-overwrite
//!    semantics (a re-birth discards the thread's history exactly as
//!    `regs.insert` does); a u_new run start is the new register's
//!    thread boundary.
//! 4. **THE ENTRY LAW** — the closed form `o = e·M_pre + b_pre`
//!    evaluates against the thread's ENTRY: the carry-in register
//!    for continuing threads (the batch-boundary state, Theorem 30);
//!    `(0,0,0)` for A-born threads (the A's zero masks make the
//!    entry irrelevant); the OLD thread's state at the U for u_new
//!    threads (the s_b flows through `rowop_u_new`'s M = (1,0,0));
//!    `None` for absent threads (the anomaly check keys on the
//!    prefix masks + the missing entry, exactly as the C reference
//!    does — [S1] §5.6's absent-id anomaly with NO token).
//! 5. **THE PHANTOM LAW** — a U whose old register is absent/dead at
//!    the U births NO new register (the pull's insert only fires on
//!    a live probe): the u_new thread's entry is `None`, and the
//!    A-like row's kept m[0] keeps the anomaly check armed for the
//!    thread's later references.
//! 6. **THE CLAMP RE-ANCHOR** (Theorem 29(a)'s policy arm) — a
//!    violating position (ok = 2, the over-cancel clamp) re-anchors
//!    its thread's continuation with the position-local overwrite
//!    row-op `(M = 0, b = (σ_b, π, 0))` — the [S1] register's ρ
//!    never goes below zero; each round fixes at least one violating
//!    position exactly and the loop is monotone (the fresh-violation
//!    set is the rewrite frontier; the position's own ok = 2 output
//!    is the CORRECT clamp and is idempotent across rounds).
//! 7. **THE TELEMETRY** — anomalies / clamps / zero_rem_keeps counted
//!    on the FINAL re-anchor round only (the re-emit rounds
//!    re-count; the round-local counters bank at the break).
//!    `probes` / `rehashes` / `max_chain` / `max_lifecycle` are
//!    pull-path-specific instruments — the push path reports 0 (the
//!    attribution arm; the parity cell excludes them by design).
//!
//! The algorithm was validated bit-exactly against the pull path on
//! the full 505,849-message golden tape plus the adversarial fuzz
//! corpus (≈1.9k anomalies / clamps per 3k-message draw: over-cancels,
//! post-D references, never-born references, batch straddles, U
//! chains) in the pre-port twin — the differential discipline the
//! cells carry forward in Rust.

use crate::orderscan::{
    emit, order_scan_dispatch, rowop_of, rowop_u_new, ScanSoa, RowOp, ROWOP_IDENTITY, ACT_A,
    ACT_C, ACT_D, ACT_E, ACT_U, ACT_X, SCAN_B_MAX,
};
use std::collections::HashMap;

/// The resolve stage's product: the four token columns in MESSAGE
/// order (one entry per decoded message — `len ≥ 3`, stream order —
/// the SAME indexing the bake's shared epilogue walks) plus the §5.6
/// telemetry and the pull cache's own instruments (the push path
/// reports zeros for those four — its attribution law). Both layouts
/// (the locate-bucketed SoA + the R36 message-order columns) are
/// written downstream from these tuples — the no-drift law.
pub(crate) struct ResolveOut {
    pub sv: Vec<u64>,
    pub pv: Vec<u64>,
    pub d0: Vec<u64>,
    pub d1: Vec<u64>,
    pub anomalies: u64,
    pub clamps: u64,
    pub zero_rem_keeps: u64,
    pub probes: u64,
    pub rehashes: u64,
    pub max_chain: u64,
    pub max_lifecycle: u64,
}

/// One decoded wire message (the bake's field offsets, verbatim).
struct Dec {
    /// The wire type byte (`msg[0]`).
    t: u8,
    /// The orderscan act code, or `NONE_ACT` for the non-book types
    /// (P/Q/⊥ — self-contained or tokenless).
    act: u8,
    /// The stock locate (`msg[1..3]`).
    loc: u16,
    /// The referenced (or born) order id (`msg[11..19]`).
    id: u64,
    /// The reference quantity / birth shares.
    q: u32,
    /// The price (C: the exec px; A/F: the resting px; P/Q: the print
    /// px; E/X/D/U: unused at decode).
    px: u32,
    /// The raw wire side byte (`msg[19]`, A/F only — b'B'/b'S').
    side: u8,
    /// The C printable flag (`msg[31]`).
    printable: u8,
    /// U's new register (`msg[19..27]`, `msg[27..31]`, `msg[31..35]`).
    new_id: u64,
    new_q: u32,
    new_px: u32,
    /// The 1-based wire position (0 collides with ⊥, T3 — counts
    /// every unit INCLUDING the sub-3 degenerates, exactly as the
    /// pull loop's `pos`).
    pos: u64,
}

/// The non-book sentinel for `Dec::act`.
const NONE_ACT: u8 = u8::MAX;

/// The carry-in register (the pipeline state at a batch boundary):
/// `(s_b, pi, rho)` + the dead flag (a D'd or never-born id — later
/// references are anomalies, exactly as the pull's probe-miss).
#[derive(Clone, Copy)]
struct Reg {
    s_b: u64,
    pi: u64,
    rho: u64,
    dead: bool,
}

/// The row role within a bucket.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A book message's own row (A/E/C/X/D).
    Ref,
    /// A U's D-like row on the OLD id's thread (the emit row).
    UOld,
    /// A U's A-like run-start on the NEW id's thread.
    UNew,
}

/// THE PUSH RESOLVE (the bake's Phase-R push path). `bodies` are the
/// frame message blocks in stream order (the bake's own contract).
pub(crate) fn resolve_push(bodies: &[&[u8]]) -> ResolveOut {
    // ---- the decode walk (one pass; the field law is the bake's) ----
    let mut dec: Vec<Dec> = Vec::new();
    let mut pos: u64 = 0;
    for &body in bodies {
        let mut p = 0usize;
        while p + 2 <= body.len() {
            let l = u16::from_be_bytes([body[p], body[p + 1]]) as usize;
            p += 2;
            if p + l > body.len() {
                break; // a truncated trailing unit (pass 1's law)
            }
            let msg = &body[p..p + l];
            p += l;
            pos += 1;
            if msg.len() < 3 {
                continue; // the degenerate unit: counted, no token
            }
            let t = msg[0];
            let loc = u16::from_be_bytes([msg[1], msg[2]]);
            let mut d = Dec {
                t,
                act: NONE_ACT,
                loc,
                id: 0,
                q: 0,
                px: 0,
                side: 0,
                printable: 0,
                new_id: 0,
                new_q: 0,
                new_px: 0,
                pos,
            };
            match t {
                b'A' | b'F' => {
                    d.act = ACT_A;
                    d.id = u64::from_be_bytes([
                        msg[11], msg[12], msg[13], msg[14], msg[15], msg[16], msg[17], msg[18],
                    ]);
                    d.side = msg[19];
                    d.q = u32::from_be_bytes([msg[20], msg[21], msg[22], msg[23]]);
                    d.px = u32::from_be_bytes([msg[32], msg[33], msg[34], msg[35]]);
                }
                b'E' => {
                    d.act = ACT_E;
                    d.id = u64::from_be_bytes([
                        msg[11], msg[12], msg[13], msg[14], msg[15], msg[16], msg[17], msg[18],
                    ]);
                    d.q = u32::from_be_bytes([msg[19], msg[20], msg[21], msg[22]]);
                }
                b'C' => {
                    d.act = ACT_C;
                    d.id = u64::from_be_bytes([
                        msg[11], msg[12], msg[13], msg[14], msg[15], msg[16], msg[17], msg[18],
                    ]);
                    d.q = u32::from_be_bytes([msg[19], msg[20], msg[21], msg[22]]);
                    d.printable = msg[31];
                    d.px = u32::from_be_bytes([msg[32], msg[33], msg[34], msg[35]]);
                }
                b'X' => {
                    d.act = ACT_X;
                    d.id = u64::from_be_bytes([
                        msg[11], msg[12], msg[13], msg[14], msg[15], msg[16], msg[17], msg[18],
                    ]);
                    d.q = u32::from_be_bytes([msg[19], msg[20], msg[21], msg[22]]);
                }
                b'D' => {
                    d.act = ACT_D;
                    d.id = u64::from_be_bytes([
                        msg[11], msg[12], msg[13], msg[14], msg[15], msg[16], msg[17], msg[18],
                    ]);
                }
                b'U' => {
                    d.act = ACT_U;
                    d.id = u64::from_be_bytes([
                        msg[11], msg[12], msg[13], msg[14], msg[15], msg[16], msg[17], msg[18],
                    ]);
                    d.new_id = u64::from_be_bytes([
                        msg[19], msg[20], msg[21], msg[22], msg[23], msg[24], msg[25], msg[26],
                    ]);
                    d.new_q = u32::from_be_bytes([msg[27], msg[28], msg[29], msg[30]]);
                    d.new_px = u32::from_be_bytes([msg[31], msg[32], msg[33], msg[34]]);
                }
                b'P' => {
                    d.px = u32::from_be_bytes([msg[32], msg[33], msg[34], msg[35]]);
                }
                b'Q' => {
                    d.px = u32::from_be_bytes([msg[27], msg[28], msg[29], msg[30]]);
                }
                _ => {}
            }
            dec.push(d);
        }
    }

    let n = dec.len();
    let mut sv = vec![0u64; n];
    let mut pv = vec![0u64; n];
    let mut d0 = vec![0u64; n];
    let mut d1 = vec![0u64; n];
    let mut anomalies = 0u64;
    let mut clamps = 0u64;
    let mut zero_rem_keeps = 0u64;
    // Theorem 7(iv)'s L: the per-id lifecycle event count (the
    // insert + every resolved touch), accumulated across batches —
    // the tracked invariant (single-digit on real feeds).
    let mut events: HashMap<u64, u64> = HashMap::new();
    // The carry-in register file (the batch-boundary state).
    let mut carry: HashMap<u64, Reg> = HashMap::new();

    // ---- the batch loop (ROW-BUDGET: <= SCAN_B_MAX rows, messages
    //      atomic — a U's two rows never split) ----
    let mut base = 0usize;
    while base < n {
        let mut rows_in = 0usize;
        let mut end = base;
        while end < n {
            let add = if dec[end].act == ACT_U {
                2
            } else if dec[end].act == NONE_ACT {
                0
            } else {
                1
            };
            if rows_in + add > SCAN_B_MAX && end > base {
                break;
            }
            rows_in += add;
            end += 1;
        }
        resolve_batch(
            &dec[base..end],
            base,
            &mut sv,
            &mut pv,
            &mut d0,
            &mut d1,
            &mut anomalies,
            &mut clamps,
            &mut zero_rem_keeps,
            &mut events,
            &mut carry,
        );
        base = end;
    }

    ResolveOut {
        sv,
        pv,
        d0,
        d1,
        anomalies,
        clamps,
        zero_rem_keeps,
        probes: 0,
        rehashes: 0,
        max_chain: 0,
        max_lifecycle: events.values().copied().max().unwrap_or(0),
    }
}

// The helpers below are plain (capture-free) fns: the round state is
// thread-local to `resolve_batch`'s loop and the borrow phases stay
// clean (u_ent is MUTATED between the emit and the u_ent-chain walk —
// a closure over it would fight the two-phase borrow law).

/// The exclusive prefix at a row (the identity at a run start).
fn pre_of(scanned: &[RowOp], orig_f: &[u64], ri: usize) -> RowOp {
    if ri == 0 || orig_f[ri] != 0 {
        ROWOP_IDENTITY
    } else {
        scanned[ri - 1]
    }
}

/// THE ENTRY LAW (the closed form's `e`): u_ent for u_new-born
/// threads, the carry-in for continuing threads, `(0,0,0)` for A-born
/// threads, `None` for absent threads.
#[allow(clippy::too_many_arguments)]
fn ent_of(
    spec: &[(usize, Role, u64)],
    carry: &HashMap<u64, Reg>,
    batch: &[Dec],
    gi_off: usize,
    t0: usize,
    u_ent: &HashMap<usize, Option<[u64; 3]>>,
) -> Option<[u64; 3]> {
    if let Some(e) = u_ent.get(&t0) {
        return *e;
    }
    let (_, role, tid) = spec[t0];
    match carry.get(&tid) {
        Some(r) if !r.dead => Some([r.s_b, r.pi, r.rho]),
        _ => {
            if role == Role::Ref && batch[spec[t0].0 - gi_off].act == ACT_A {
                Some([0, 0, 0])
            } else {
                None
            }
        }
    }
}

/// The closed-form state at a row (the register's value there).
#[allow(clippy::too_many_arguments)]
fn state_at(
    scanned: &[RowOp],
    orig_f: &[u64],
    thread_start: &[usize],
    spec: &[(usize, Role, u64)],
    carry: &HashMap<u64, Reg>,
    batch: &[Dec],
    gi_off: usize,
    ri: usize,
    u_ent: &HashMap<usize, Option<[u64; 3]>>,
) -> [u64; 3] {
    let pre = pre_of(scanned, orig_f, ri);
    let e = ent_of(spec, carry, batch, gi_off, thread_start[ri], u_ent).unwrap_or([0, 0, 0]);
    [
        (e[0] & pre.m[0]).wrapping_add(pre.b[0]),
        (e[1] & pre.m[1]).wrapping_add(pre.b[1]),
        (e[2] & pre.m[2]).wrapping_add(pre.b[2]),
    ]
}

/// One batch's resolve: bucket → rows → scan (with the re-anchor
/// rounds) → emit → the carry-in update. `gi_off` is the batch's
/// first decoded index (the output columns' base).
#[allow(clippy::too_many_arguments)]
fn resolve_batch(
    batch: &[Dec],
    gi_off: usize,
    sv: &mut [u64],
    pv: &mut [u64],
    d0: &mut [u64],
    d1: &mut [u64],
    anomalies: &mut u64,
    clamps: &mut u64,
    zero_rem_keeps: &mut u64,
    events: &mut HashMap<u64, u64>,
    carry: &mut HashMap<u64, Reg>,
) {
    // ---- 1. the id buckets (first-appearance order — deterministic;
    //         the scan's composites are bucket-order independent) ----
    let mut bucket_idx: HashMap<u64, usize> = HashMap::new();
    let mut bucket_ids: Vec<u64> = Vec::new();
    let mut buckets: Vec<Vec<(usize, Role)>> = Vec::new();
    for (i, g) in batch.iter().enumerate() {
        let gi = gi_off + i;
        match g.act {
            ACT_U => {
                let bi_old = *bucket_idx.entry(g.id).or_insert_with(|| {
                    buckets.push(Vec::new());
                    bucket_ids.push(g.id);
                    bucket_ids.len() - 1
                });
                buckets[bi_old].push((gi, Role::UOld));
                let bi_new = *bucket_idx.entry(g.new_id).or_insert_with(|| {
                    buckets.push(Vec::new());
                    bucket_ids.push(g.new_id);
                    bucket_ids.len() - 1
                });
                buckets[bi_new].push((gi, Role::UNew));
            }
            ACT_A | ACT_E | ACT_C | ACT_X | ACT_D => {
                let bi = *bucket_idx.entry(g.id).or_insert_with(|| {
                    buckets.push(Vec::new());
                    bucket_ids.push(g.id);
                    bucket_ids.len() - 1
                });
                buckets[bi].push((gi, Role::Ref));
            }
            _ => {}
        }
    }

    // ---- 2. the rows (the bucket-contiguous row array) ----
    let mut rows: Vec<RowOp> = Vec::with_capacity(SCAN_B_MAX);
    // (gi, role, thread_id) — the thread id for the entry lookups
    // (the NEW id for u_new rows, the bucket id otherwise).
    let mut spec: Vec<(usize, Role, u64)> = Vec::with_capacity(SCAN_B_MAX);
    let mut emit_row: HashMap<usize, usize> = HashMap::new(); // gi -> the emit row
    let mut u_new_row: HashMap<usize, usize> = HashMap::new(); // gi -> the A-like row
    // The bucket row ranges (the carry update's per-bucket spans).
    let mut bucket_range: Vec<(usize, usize)> = Vec::with_capacity(bucket_ids.len());
    for (bi, &id) in bucket_ids.iter().enumerate() {
        let first = rows.len();
        for &(gi, role) in &buckets[bi] {
            let g = &batch[gi - gi_off];
            match role {
                Role::UNew => {
                    rows.push(rowop_u_new(g.new_q, g.new_px, g.loc as u32));
                    spec.push((gi, Role::UNew, g.new_id));
                    u_new_row.insert(gi, rows.len() - 1);
                }
                Role::UOld => {
                    rows.push(rowop_of(ACT_D, 0, 0, 0, 0));
                    spec.push((gi, Role::UOld, id));
                    emit_row.insert(gi, rows.len() - 1);
                }
                Role::Ref => {
                    if g.act == ACT_A {
                        // The wiring's side conversion: the wire byte
                        // (b'B'/b'S') → the row-op's σ_b (1 = bid,
                        // 0 = ask — emit's convention).
                        let side_u8 = u8::from(g.side != b'S');
                        rows.push(rowop_of(ACT_A, g.q, g.px, g.loc as u32, side_u8));
                    } else {
                        rows.push(rowop_of(g.act, g.q, g.px, g.loc as u32, 0));
                    }
                    spec.push((gi, Role::Ref, id));
                    emit_row.insert(gi, rows.len() - 1);
                }
            }
        }
        // THE RUN-START LAW: clear the interior flags EXCEPT the A
        // rows and the u_new rows (the rebirth/overwrite semantics).
        for j in first + 1..rows.len() {
            let (gi, role, _) = spec[j];
            if role == Role::UNew {
                continue;
            }
            if role == Role::Ref && batch[gi - gi_off].act == ACT_A {
                continue;
            }
            rows[j].f = 0;
        }
        bucket_range.push((first, rows.len()));
    }

    // ---- 3. the re-anchor loop (Theorem 29(a)'s policy arm) ----
    let mut rewritten: Vec<bool> = vec![false; rows.len()];
    loop {
        // Round-local telemetry + the lifecycle events (banked at
        // the final round's break — the re-emit rounds re-count).
        let mut r_anoms = 0u64;
        let mut r_clamps = 0u64;
        let mut r_zrk = 0u64;
        let mut r_events: HashMap<u64, u64> = HashMap::new();
        // The scan on the CURRENT rows (the re-anchored ones on later
        // rounds) — the dispatch picks the AVX-512 twin when present
        // (HFT_ORDER_SCAN_VEC=0 forces the scalar SoA — the
        // attribution arm; the cells pin both paths equal).
        let mut soa = ScanSoa::with_capacity(rows.len());
        soa.load(&rows);
        order_scan_dispatch(&mut soa);
        let scanned = soa.store();
        let orig_f: Vec<u64> = rows.iter().map(|r| r.f).collect();
        // The thread-start map: the largest run start <= ri.
        let mut thread_start = vec![0usize; rows.len()];
        {
            let mut cur = 0usize;
            for ri in 0..rows.len() {
                if ri == 0 || orig_f[ri] != 0 {
                    cur = ri;
                }
                thread_start[ri] = cur;
            }
        }

        // THE U-ENT CHAIN (message order): the U-new threads' entries —
        // the OLD thread's state at the U (None = the phantom case:
        // the old register absent/dead at the U). A later U may
        // reference a register birthed by an earlier U's u_new row in
        // the same batch — the chain resolves in message order.
        let mut u_ent: HashMap<usize, Option<[u64; 3]>> = HashMap::new();
        let mut u_order: Vec<usize> = u_new_row.keys().copied().collect();
        u_order.sort_unstable();
        for &gi in &u_order {
            let ri_new = u_new_row[&gi];
            let d_ri = emit_row[&gi];
            let t0 = thread_start[d_ri];
            let pre = pre_of(&scanned, &orig_f, d_ri);
            let valid =
                pre.dead == 0 && ent_of(&spec, carry, batch, gi_off, t0, &u_ent).is_some();
            let entry = if valid {
                Some(state_at(
                    &scanned, &orig_f, &thread_start, &spec, carry, batch, gi_off, d_ri, &u_ent,
                ))
            } else {
                None
            };
            u_ent.insert(ri_new, entry);
        }

        // ---- 4. the emit (per message, at its row) ----
        let mut violations: Vec<(usize, usize)> = Vec::new(); // (gi, ri)
        for (i, g) in batch.iter().enumerate() {
            let gi = gi_off + i;
            if g.act == NONE_ACT {
                // P/Q: the self-contained print tokens; the rest: ⊥.
                if g.t == b'P' || g.t == b'Q' {
                    sv[gi] = g.pos;
                    pv[gi] = g.px as u64;
                }
                continue;
            }
            if g.act == ACT_A {
                // Self-contained: the birth's own volume delta (+ the
                // lifecycle insert).
                *r_events.entry(g.id).or_insert(0) += 1;
                if g.side == b'S' {
                    d1[gi] = g.q as u64;
                } else {
                    d0[gi] = g.q as u64;
                }
                continue;
            }
            let ri = emit_row[&gi];
            let pre = pre_of(&scanned, &orig_f, ri);
            let t0 = thread_start[ri];
            let ent = ent_of(&spec, carry, batch, gi_off, t0, &u_ent);
            let (act, wq, wpx) = if g.act == ACT_U {
                (ACT_U, g.new_q, g.new_px)
            } else {
                (g.act, g.q, g.px)
            };
            let out = emit(&pre, ent.as_ref(), act, wq, wpx);
            if out.ok == 0 {
                // The absent-id anomaly: NO token (§5.6).
                r_anoms += 1;
                continue;
            }
            if out.ok == 2 {
                r_clamps += 1;
                if act == ACT_E {
                    r_zrk += 1; // the over-execute's zero-rem keep
                }
            }
            match act {
                ACT_E => {
                    sv[gi] = g.pos;
                    pv[gi] = out.px as u64; // E_SETS_LAST: the order px
                }
                ACT_C => {
                    let is_print = g.printable == b'Y' || g.printable == 1;
                    if is_print {
                        sv[gi] = g.pos;
                        pv[gi] = g.px as u64; // the WIRE's px
                    }
                }
                _ => {}
            }
            // The lifecycle touch: the resolved reference +1 (the
            // pull's probe-hit); a resolved U touches the old id (the
            // probe+erase) AND births the new register (the insert).
            *r_events.entry(g.id).or_insert(0) += 1;
            if act == ACT_U {
                *r_events.entry(g.new_id).or_insert(0) += 1;
            }
            d0[gi] = out.db as u64;
            d1[gi] = out.da as u64;
            // Only the CLAMP POLICY's arms (E/C/X — the kernel's own
            // doc: "U's removal is exact and D's d is the rho itself —
            // neither clamps") enter the re-anchor set. A D's transient
            // ok=2 is the kernel's SIGNED-comparison quirk on the
            // WRAPPED rho (a pre-re-anchor over-cancel leaves rho with
            // the top bit set; `rho as i64 < 0` fires): re-anchoring a
            // D would rewrite it with (M=0, b=(sigma, pi, 0), dead=0) —
            // WIPING the thread's dead terminator and resurrecting the
            // register (post-D references would resolve instead of
            // anomaly). The D's transient token is re-emitted by the
            // next round (the upstream E/C/X re-anchor zeroes the wrap)
            // and no wrap can survive into the final round.
            let clamp_arm = act == ACT_E || act == ACT_C || act == ACT_X;
            if out.ok == 2 && clamp_arm && !rewritten[ri] {
                violations.push((gi, ri));
            }
        }

        if violations.is_empty() {
            // Bank the round's telemetry + the lifecycle events.
            *anomalies += r_anoms;
            *clamps += r_clamps;
            *zero_rem_keeps += r_zrk;
            for (id, c) in r_events {
                *events.entry(id).or_insert(0) += c;
            }
            // ---- 5. THE CARRY-IN UPDATE (the batch-boundary state) ----
            for (bi, &id) in bucket_ids.iter().enumerate() {
                let (lo, hi) = bucket_range[bi];
                if hi <= lo {
                    continue; // an empty bucket (defensive; cannot happen)
                }
                let last = hi - 1;
                let t0 = thread_start[last];
                let ent = ent_of(&spec, carry, batch, gi_off, t0, &u_ent);
                let comp = scanned[last];
                match ent {
                    None => {
                        // ABSENT (a ref-only/anomaly thread, or a
                        // phantom u_new birth): the pull's probes miss
                        // the id later too.
                        carry.insert(id, Reg { s_b: 0, pi: 0, rho: 0, dead: true });
                    }
                    Some(e) => {
                        let st = [
                            (e[0] & comp.m[0]).wrapping_add(comp.b[0]),
                            (e[1] & comp.m[1]).wrapping_add(comp.b[1]),
                            (e[2] & comp.m[2]).wrapping_add(comp.b[2]),
                        ];
                        carry.insert(
                            id,
                            Reg {
                                s_b: st[0],
                                pi: st[1],
                                rho: st[2],
                                dead: comp.dead != 0,
                            },
                        );
                    }
                }
            }
            break;
        }
        // ---- the re-anchor: the overwrite row-op with the position's
        //      pre-state identity (σ_b, π) and ρ = 0 ----
        for &(_gi, ri) in &violations {
            let st = state_at(
                &scanned, &orig_f, &thread_start, &spec, carry, batch, gi_off, ri, &u_ent,
            );
            let f = rows[ri].f;
            rows[ri] = RowOp {
                f,
                m: [0; 3],
                b: [st[0], st[1], 0],
                dead: 0,
            };
            rewritten[ri] = true;
        }
    }
}
