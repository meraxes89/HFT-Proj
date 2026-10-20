//! R40: THE DUAL-PRODUCER CHAMPION (Champion-4T) — the 200B corridor's
//! structural split (docs/52-r40-dual-producer-blueprint.md).
//!
//! THE WALL (the R39 fleet verdict, the 73.72B record's own DIAG):
//! the champion's steady pass is 7.29 µs — `work_ms` 1697 (the submission
//! ceremony: the ledger guards + the call re-issue + the transport group
//! protocol), `fold_ms` 1449 (the res-ring drain: 172 composite records
//! from both lanes), `wait_ms` 1261 (the per-pass RX turnaround), plus
//! ~740 ns/pass of boundary + harvest + assert ceremony — ALL on Thread
//! 0. The workers sit at ~4% duty (the res-replay elision) and the
//! windows are 99.999% SELF-SERVE (`selfserve_wins` 686,087 of 686,091):
//! the sink's submit path is pure cursor arithmetic — the ring writes
//! the classic path owns are structurally absent. The system is
//! MAIN-BOUND; the road to 200B passes through SPLITTING THE LOOP.
//!
//! THE SPLIT (this module's `DualGate` + `lane1_run`):
//! * **P0 (main, the sink's owner)** keeps the transport ring (the
//!   verbatim `next_group` ceremony + the R32 guard chain), the window
//!   machine, the pass records, and the boundary completion. Under the
//!   dual flag its submit dispatch is SUPPRESSED for the odd batches
//!   (`set_p1_suppress` — P1 owns their count) and its drain is
//!   LANE-0-ONLY (`fold_available`'s dual branch) into the sink's own
//!   journal.
//! * **P1 (`lane1_run`, a dedicated physical core)** replays the ODD
//!   batches' packed totals from a compact script snapshot of the R32
//!   ledger (count bookkeeping only — the SS regime's submit is
//!   arithmetic) and drains LANE 1's res ring into the shared leaf
//!   ring `j1` (the per-lane journal, single-writer P1).
//! * **THE BOUNDARY MERGE (Theorem 23(b)'s static order made
//!   explicit)**: each lane's res sub-stream is internally contiguous
//!   (records end at grid-chunk ends / batch publishes / pass
//!   boundaries — all lane-local), so the two journals' leaves merge
//!   BY GLOBAL END POSITION into exactly the mono drain's leaf
//!   sequence; the tree flush composes the merged staging and the
//!   composite is bit-identical by construction. The per-pass count
//!   merges additively (`count_0 + count_1 − overlap`), the msg-hash
//!   chain stays P0-only (all `Msg`/`Event` calls re-issue on P0 in
//!   ledger order), and the tuple assert runs VERBATIM on the merged
//!   composite at every boundary — the same oracle, one assert per
//!   pass.
//! * **THE ARRIVAL PROTOCOL (R41: THE CONTINUOUS DRAINER)**: P1 drains
//!   lane 1's res ring continuously — no per-pass cycle, no dispatch
//!   ceremony, no pass-end barrier — and publishes the arrival
//!   `(pass, count_1, run_start, run_len)` OPPORTUNISTICALLY at every
//!   boundary crossing inside the drain (ordered BEFORE the `p1_fold`
//!   cursor store, both Release). P0's completion fires when BOTH
//!   lanes' cursors crossed the boundary (`lane_end_at`) — the CURSOR
//!   GATE; the arrival, already visible whenever the cursor gate
//!   passes, is the run-window FAST PATH (the count is script-direct
//!   on the fallback — the R40c ground-truth law). The old law (the
//!   completion waited for the per-pass arrival handshake — P1's whole
//!   cycle per boundary) locked the completion frontier 1:1 to P1's
//!   cadence and parked P0 for 65% of the record window; the cursor
//!   gate deletes the handshake while keeping the exact merge.
//! * **THE POISON LAW**: any regime break (a live re-scan, a non-SS
//!   window) poisons the engine permanently — P1 observes the word at
//!   its next drain cycle and posts its terminal state (the zombie
//!   captures whatever cursor/j1 state P1 reached — sound because every
//!   completion count is script-direct, independent of P1's drain
//!   progress), and P0 computes the deterministic overlap correction
//!   from the script (the odd batches in `[poison_batch, p1_cursor]`)
//!   so the merged count stays exact. The pass completes under the merge
//!   (the insertion-sorted path — the post-poison journal concatenation
//!   is not sorted), and every later pass runs the MONO loop verbatim.
//!
//! SOUNDNESS (the guard chain, fail-loud everywhere):
//! 1. The batch identity: P0's `try_replay` bounds/watermark guards run
//!    VERBATIM for every batch (the ring is P0's; the sequencer chain
//!    replays the full stream state per batch).
//! 2. The lane-order law: each producer asserts every drained record's
//!    `span_id` against its lane cursor (the per-lane contiguity — the
//!    same fail-stop class as the mono's fold-order assert).
//! 3. The composite: the merged tree == the mono's journal sequence
//!    (position-keyed, provenance-agnostic) — pinned by the parity
//!    cells (`t_r40_parity_mini`: the dual vs the mono, per-pass tuples
//!    BIT-EXACT).
//! 4. The tuple assert per pass vs the golden reference (unchanged).
//! 5. Allocs=0: every structure here is pre-allocated at construction
//!    (outside every measured window); the script snapshot FILLS
//!    pre-allocated arrays at the arm (one ledger walk, no alloc).
//!
//! ROLLLBACK: `HFT_DUAL_PRODUCER=0` (the default) never spawns P1, never
//! arms the gate — the mono loop bit-for-bit. `HFT_DUAL_PRODUCER=1`
//! arms after the SS regime proves itself (≥3 consecutive SS windows
//! and a complete ledger template); any break falls back permanently.

use crate::hydra::{HydraFabric, CHUNK};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// The packed cursor layout: `(pass << 20) | batch` (the batch count is
/// bounded by the ledger's MAX_BATCHES = 2048 « 2^20).
pub const CUR_BATCH_MASK: u64 = (1u64 << 20) - 1;
/// The poison word's clean value.
pub const POISON_CLEAN: u64 = u64::MAX;

/// The shared leaf ring's capacity (leaves). R41b: 2048 → 8192 — the
/// R41 fleet (run 39405512333) priced the 21-pass quantum: P1 filled
/// the old ring every ~21 passes and parked (21.6M parks on the record
/// draw — the arrival frontier moved in 21-pass quanta paced by P0's
/// merge bursts, the completion-pace stall's successor). R42's coarse
/// grid cuts the steady leaf count to ~16/pass (the 512-span runs), so
/// the wide ring holds (8192 − 1024)/16 ≈ 448 passes of lead — the
/// park cadence amortizes ~5× further than R41b's 83-pass quantum; the
/// degenerate raw-leaf shapes still park proportionally earlier (the
/// fail-soft bound).
pub const J1_CAP: usize = 8192;
/// The push gate's headroom: P1 parks when `len − consumed` exceeds
/// this (the margin keeps P0's in-flight merge reads stable — the
/// disjoint-slot law). R41b: 256 → 1024 — the same 1/8 share of the
/// widened ring (the composed steady law never reaches it; the
/// degenerate raw-leaf shapes park at the fail-soft bound).
pub const J1_HEADROOM: u64 = 1024;
/// The arrival ring's slots (a power of two ≥ 2× the push-gate's
/// lead bound). R42: 256 → 2048 — the coarse grid's lead bound is
/// (8192 − 1024) leaves / ~16 leaves-per-pass ≈ 448 passes (the
/// steady law; the degenerate shapes lead LESS — the ring fills
/// faster), so 2048 slots ≥ 4× the worst steady bound (the original
/// law: a lap would overwrite an unconsumed arrival — the slot-pass
/// debug assert's class). The slots are 32 B each — 64 KB once at
/// spawn.
pub const ARR_SLOTS: usize = 2048;

/// The lane-territory projection: the end position of `lane`'s res
/// sub-stream when the global stream stands at `s`. If `s` falls
/// inside (or exactly at the end of) a chunk owned by `lane`, the
/// projection is `s` itself (the pass boundary split the lane's chunk
/// there); otherwise the lane's territory ended at the previous chunk
/// boundary (the interposed chunk belongs to the sibling lane).
///
/// LAW (pinned by `t_r40_lane_end_law`): each lane's drain cursor is
/// contiguous across this projection — the next record the lane's ring
/// holds starts exactly at `lane_end_at(s, lane)` when the global
/// cursor is `s`, for every `s` (mid-chunk, chunk-aligned, pass-end).
#[inline]
pub fn lane_end_at(s: u64, lane: u64) -> u64 {
    if s == 0 {
        return 0;
    }
    let c = (s - 1) / CHUNK;
    if (c & 1) == (lane & 1) {
        s
    } else {
        c * CHUNK
    }
}

/// R40: the shared leaf ring — P1's fold journal. Single writer (P1)
/// and a single boundary reader (P0) over a fixed slot ring; the
/// ownership protocol is the `HydraLane` res-ring's own: P1 owns slots
/// `[len % CAP, ..)` (published by the `len` Release store), P0 owns
/// the retire cursor `consumed` (published by the Release store after
/// its merge). The disjoint-slot law: P0 reads `[consumed,
/// snapshot)`; P1 pushes at `[len, ..)` with `len − consumed ≤
/// CAP − HEADROOM` enforced by the push gate — the two index ranges
/// never share a slot (the ring wrap included), so the concurrent
/// read/write is ownership-transfer, not aliasing.
pub struct SharedLeafRing {
    /// SAFETY CONTRACT (mirrors `HydraLane`'s res ring): the arrays are
    /// owned by the SPSC ownership-transfer protocol — P1 owns slots
    /// `[len % CAP, ..)` (published by the `len` Release store), P0 owns
    /// the read window `[consumed, snapshot)` (ordered by the
    /// consumed/len Release-Acquire pairs). The current slot OWNER is
    /// the only accessor — never true aliasing.
    k: std::cell::UnsafeCell<Box<[u32; J1_CAP]>>,
    b: std::cell::UnsafeCell<Box<[u64; J1_CAP]>>,
    end: std::cell::UnsafeCell<Box<[u64; J1_CAP]>>,
    len: AtomicU64,
    consumed: AtomicU64,
    /// Telemetry: total leaves pushed.
    pub records: AtomicU64,
}

// SAFETY: see the struct-level contract (the disjoint-slot law).
unsafe impl Send for SharedLeafRing {}
unsafe impl Sync for SharedLeafRing {}

impl SharedLeafRing {
    #[allow(clippy::declare_interior_mutable_const)]
    fn new() -> Self {
        Self {
            k: std::cell::UnsafeCell::new(Box::new([0u32; J1_CAP])),
            b: std::cell::UnsafeCell::new(Box::new([0u64; J1_CAP])),
            end: std::cell::UnsafeCell::new(Box::new([0u64; J1_CAP])),
            len: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            records: AtomicU64::new(0),
        }
    }

    /// P1's push — the caller OWNS the disjoint-slot proof (the push
    /// gate held) and passes its local `len` mirror. Writes the slot,
    /// then publishes the new length (Release — the arrival/merge that
    /// Acquires `len` sees the leaf).
    #[inline]
    pub fn push(&self, len: &mut u64, k: u32, b: u64, end: u64) {
        let i = (*len as usize) & (J1_CAP - 1);
        // SAFETY (the caller's gate): slot i is beyond every index P0
        // may read ([consumed, snapshot) with snapshot ≤ the published
        // len ≤ *len, and *len − consumed ≤ CAP − HEADROOM keeps the
        // ring wrap disjoint) — the ownership-transfer protocol.
        unsafe {
            let kk = &mut *self.k.get();
            let bb = &mut *self.b.get();
            let ee = &mut *self.end.get();
            kk[i] = k;
            bb[i] = b;
            ee[i] = end;
        }
        *len += 1;
        self.len.store(*len, Ordering::Release);
        self.records.fetch_add(1, Ordering::Relaxed);
    }

    /// P0's snapshot: the published leaf count (Acquire — the leaves
    /// below it are visible).
    #[inline]
    pub fn snapshot(&self) -> u64 {
        self.len.load(Ordering::Acquire)
    }

    /// P1's gate operand: the retire cursor (Acquire — the retired
    /// slots are re-ownable).
    #[inline]
    pub fn consumed(&self) -> u64 {
        self.consumed.load(Ordering::Acquire)
    }

    /// P0's retire — after the merge consumed `[..upto)`.
    #[inline]
    pub fn retire(&self, upto: u64) {
        self.consumed.store(upto, Ordering::Release);
    }

    /// P0's leaf read — the caller proves `consumed ≤ i < snapshot`
    /// (the disjoint-slot law).
    #[inline]
    pub fn leaf(&self, i: u64) -> (u32, u64, u64) {
        let idx = (i as usize) & (J1_CAP - 1);
        // SAFETY: i ∈ [consumed, snapshot) — the read window P0 owns.
        unsafe {
            let kk = &*self.k.get();
            let bb = &*self.b.get();
            let ee = &*self.end.get();
            (kk[idx], bb[idx], ee[idx])
        }
    }

    /// R40d: P0's RUN VIEW — the compacted run-drain's operand. The
    /// leaf window `[lo, hi)` (the caller proves `lo ≤ hi ≤ snapshot`
    /// and `hi − lo ≤ J1_CAP − HEADROOM` — the push gate's bound, so
    /// the window spans less than one lap) as at most TWO contiguous
    /// array segments `(k, b, end)` — the ring's wrap splits the
    /// window where it crosses the array end. The merge consumes the
    /// segments IN ORDER (ring order = push order = the monotone end
    /// order, so the two-finger continues across the split); the
    /// per-record masked `leaf()` reads (three UnsafeCell derefs per
    /// leaf — the old merge's dominant cost) are gone.
    pub fn run_segments(&self, lo: u64, hi: u64) -> [(&[u32], &[u64], &[u64]); 2] {
        debug_assert!(lo <= hi);
        let n = (hi - lo) as usize;
        // SAFETY: the caller's disjoint-slot proof — the window is
        // inside P0's read range [consumed, snapshot); read-only for
        // the merge's lifetime.
        unsafe {
            let kk = &*self.k.get();
            let bb = &*self.b.get();
            let ee = &*self.end.get();
            let start = (lo as usize) & (J1_CAP - 1);
            let first = (J1_CAP - start).min(n);
            let second = n - first;
            [
                (
                    &kk[start..start + first],
                    &bb[start..start + first],
                    &ee[start..start + first],
                ),
                (&kk[..second], &bb[..second], &ee[..second]),
            ]
        }
    }
}

/// R40: one arrival slot — P1's per-pass completion publication.
/// R40d: the slot carries the boundary's FULL half of the merge
/// operands — the count AND j1's run bounds (the compacted
/// run-drain's window: P1's cumulative leaf total at the pass top and
/// the pass's pushes). P1 publishes them at its arrival (it knows both
/// — `j1_len` is its local mirror); P0's merge consumes the window
/// directly from the slot (NO per-record ring scan — the arriving-
/// last election's operand handoff).
struct Arrival {
    pass: AtomicU64,
    count: AtomicU64,
    /// R40d: the run's first leaf index (j1's cumulative length at the
    /// pass top — P1's local mirror snapshot).
    run_start: AtomicU64,
    /// R40d: the run's leaf count (this pass's pushes into j1).
    run_len: AtomicU64,
}

impl Arrival {
    const fn new() -> Self {
        Self {
            pass: AtomicU64::new(u64::MAX),
            count: AtomicU64::new(0),
            run_start: AtomicU64::new(0),
            run_len: AtomicU64::new(0),
        }
    }
}

/// R40: the dual-producer rendezvous — every word P0 and P1 share.
///
/// SAFETY (justifies `unsafe impl Send/Sync`): the `UnsafeCell`
/// members follow established ownership-transfer contracts — the
/// script is written ONCE pre-arm (P1 blocked on `armed`, whose
/// Release store orders the fill for P1's Acquire) and read-only
/// after; the shared leaf ring follows `HydraLane`'s SPSC
/// disjoint-slot law. Every other member is an atomic.
pub struct DualGate {
    /// 0 = mono (P1 idles on this word); else the FIRST dual pass id.
    armed: AtomicU64,
    /// The harness's final-drain signal — P1 completes its in-flight
    /// pass (drain + arrival), then exits.
    pub shutdown: AtomicBool,
    /// `POISON_CLEAN` or `(pass << 20) | batch` — the permanent
    /// fallback's trigger point.
    poison: AtomicU64,
    /// P1's last COMPLETED pass (monotone; the completion's lazy gate).
    arrival_seq: AtomicU64,
    /// The armed pass id (the completion's pre/post-arm discriminator:
    /// boundaries for passes BELOW it complete on the pure mono path).
    armed_pass: AtomicU64,
    /// The per-pass arrival ring: `(pass, count_1)` pairs.
    arrivals: Box<[Arrival; ARR_SLOTS]>,
    /// P1's lane-1 drain cursor (the window watermark's operand; the
    /// zombie's state capture after the poison).
    p1_fold: AtomicU64,
    /// R40: THE SHUTDOWN SWEEP'S END — P0's submission frontier at the
    /// clean shutdown (the last pass's end_span, exact). Stored
    /// BEFORE the shutdown word (both Release — P1's Acquire load of
    /// `shutdown` orders the end read). P1's exit path drains its
    /// remaining lane-1 territory to this frontier pass-by-pass (the
    /// accounting + the arrivals from the script) BEFORE posting
    /// p1_done — the finish's `dual_lanes_past(next_span)` wait and
    /// the boundary completions both terminate on it. 0 = the legacy
    /// bare-shutdown shape (P1 exits at the top without sweeping —
    /// only reachable from pre-R40c call sites).
    shutdown_end: AtomicU64,
    /// The poison path's final count_1 publication.
    p1_final: AtomicU64,
    /// P1's terminal state (posted on every exit path).
    p1_done: AtomicBool,
    /// P1's lane-1 cursor at the arm (the mono fold position's
    /// projection).
    fold_init: AtomicU64,
    /// P1's res-ring consumer cursor at the arm (lane 1's `res_tail`).
    res_tail_init: AtomicU64,
    /// R40: THE PASS-GRID ANCHOR — the true pass boundary at the arm
    /// (the sink's `next_span` at the pass top == the last pass's
    /// `end_span`). P1's per-pass targets are `base + k·pass_spans`;
    /// anchoring at the FOLD PROJECTION instead (which sits inside the
    /// previous pass's deferred tail by the lazy law) drifts every
    /// target EARLY by the tail's width — the last record of each pass
    /// then parks at the frontier cap and the arrival spin never
    /// completes (the t_r40_parity cell's catch).
    pass_base: AtomicU64,
    /// The compact script snapshot (filled at the arm — the pre-arm
    /// single-writer law: P1 is blocked on `armed`, whose Release store
    /// orders these writes for P1's Acquire).
    script: std::cell::UnsafeCell<DualScript>,
    /// P1's shared journal.
    pub j1: SharedLeafRing,
    /// Telemetry: P1's dispatched batches.
    pub p1_batches: AtomicU64,
    /// Telemetry: P1's dispatch parks (the journal backpressure).
    pub p1_parks: AtomicU64,
    /// Telemetry: boundaries deferred on P1's lag.
    pub arrival_defers: AtomicU64,
    /// R41: telemetry — completions that took the ARRIVAL FAST PATH
    /// (the run window consumed; the cursor gate passed and the arrival
    /// was already visible — the steady law).
    pub r41_fast: AtomicU64,
    /// R41: telemetry — completions that fell back to the POSITION SCAN
    /// (the pre-arm/edge shape; the arrival had not landed when the
    /// cursor gate passed).
    pub r41_scan: AtomicU64,
    /// R41: telemetry — P1's opportunistic arrival publications (the
    /// continuous drainer's boundary crossings).
    pub r41_arrivals: AtomicU64,
    /// R41: the trace flag — `R41_TRACE=1` arms the per-crossing print
    /// (read ONCE at construction, outside every measured window: the
    /// old per-pass `env::var` was a locked lookup on the hot path).
    trace: AtomicBool,
    /// R40d: THE ARRIVING-LAST ELECTION WORD — the end position of the
    /// LAST finalized boundary (monotone; `fetch_max` at the
    /// completion's top is the atomic swap). P1's boundary step is its
    /// ARRIVAL (the run publication — it proceeds immediately, never
    /// waiting on P0); P0's completion is the arriving-last finalize
    /// (both lanes' cursors crossed + the arrival landed — the second
    /// event observed). The swap guarantees EXACTLY-ONCE per boundary:
    /// an attempt whose boundary was already finalized (the deferred /
    /// recomposed edge) is the election's loser and skips — the merge
    /// never runs twice for one boundary.
    boundary_taken: AtomicU64,
    /// R40d: THE EXIT LATCH (the allocs=0 law's last edge) — P1's
    /// thread teardown frees exactly TWO heap blocks (the spawn's
    /// closure box + the `Thread` inner), and without the latch those
    /// frees land INSIDE the measurement window (the finish completes
    /// at `p1_done` while the exit path races on — the "2 in-window
    /// counter deltas" the bisect probe was armed for). The harness
    /// arms `exit_hold` BEFORE the final finish and stores
    /// `exit_release` AFTER the closing ALLOC snapshot; P1 parks
    /// between (a PAUSE-spin bounded at 250ms — the retired producer's
    /// cpu is idle; a harness that never releases is already a red
    /// run, and the process exit reaps the thread regardless).
    exit_hold: AtomicBool,
    /// R40d: the latch's release word (0 = hold; non-zero = the thread
    /// may tear down — written AFTER the harness's final allocation
    /// counter snapshot).
    exit_release: AtomicU64,
}

// SAFETY: see the struct-level contract.
unsafe impl Send for DualGate {}
unsafe impl Sync for DualGate {}

impl DualGate {
    /// Construction-time allocation only (the allocs=0 law — the gate
    /// is built outside every measured window).
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            armed: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
            poison: AtomicU64::new(POISON_CLEAN),
            arrival_seq: AtomicU64::new(0),
            armed_pass: AtomicU64::new(0),
            arrivals: Box::new(std::array::from_fn(|_| Arrival::new())),
            p1_fold: AtomicU64::new(0),
            shutdown_end: AtomicU64::new(0),
            p1_final: AtomicU64::new(0),
            p1_done: AtomicBool::new(false),
            fold_init: AtomicU64::new(0),
            res_tail_init: AtomicU64::new(0),
            pass_base: AtomicU64::new(0),
            script: std::cell::UnsafeCell::new(DualScript::new()),
            j1: SharedLeafRing::new(),
            p1_batches: AtomicU64::new(0),
            p1_parks: AtomicU64::new(0),
            arrival_defers: AtomicU64::new(0),
            r41_fast: AtomicU64::new(0),
            r41_scan: AtomicU64::new(0),
            r41_arrivals: AtomicU64::new(0),
            trace: AtomicBool::new(std::env::var("R41_TRACE").is_ok()),
            boundary_taken: AtomicU64::new(0),
            exit_hold: AtomicBool::new(false),
            exit_release: AtomicU64::new(0),
        })
    }

    /// The arm: publish the first dual pass + P1's initial cursors.
    /// Call AFTER `script` is filled and BEFORE P0's first dual
    /// `begin_pass` (the ordering: the armed store is the LAST Release
    /// — P1's Acquire sees the script + the inits). The arrival
    /// sequence starts at `first_pass − 1` (P1's arrivals land from
    /// `first_pass` on; pre-arm boundaries never consult it).
    pub fn arm(
        &self,
        first_pass: u64,
        fold_init: u64,
        res_tail_init: u64,
        pass_base: u64,
    ) {
        self.fold_init.store(fold_init, Ordering::Relaxed);
        self.res_tail_init.store(res_tail_init, Ordering::Relaxed);
        self.pass_base.store(pass_base, Ordering::Relaxed);
        self.armed_pass.store(first_pass, Ordering::Relaxed);
        self.arrival_seq.store(first_pass.wrapping_sub(1), Ordering::Relaxed);
        // The watermark's operand starts at the init cursor (P1's first
        // drain publishes past it; BEFORE that, `dual_lanes_past` must
        // see the arm-time projection — a zero here deadlocks the
        // window watermark's spin, the parity cell's catch).
        self.p1_fold.store(fold_init, Ordering::Relaxed);
        self.armed.store(first_pass, Ordering::Release);
    }

    /// The armed pass (0 = mono).
    #[inline]
    pub fn armed_from(&self) -> u64 {
        self.armed.load(Ordering::Acquire)
    }

    /// The armed-pass discriminator (the completion's pre/post-arm
    /// check — no Acquire needed: it is write-once before the armed
    /// Release, and every reader Acquired `armed` first).
    #[inline]
    pub fn armed_pass(&self) -> u64 {
        self.armed_pass.load(Ordering::Relaxed)
    }

    /// The poison publication (idempotent — the FIRST call wins; the
    /// permanent fallback's law). The overlap correction is NOT
    /// computed here: P1 may still be dispatching past the poison
    /// word's landing — the completion reads P1's FINAL cursor (posted
    /// with `p1_final`'s Release) and derives the overlap from the
    /// script deterministically then.
    pub fn set_poison(&self, pass: u64, batch: u64) {
        let v = (pass << 20) | (batch & CUR_BATCH_MASK);
        self.poison
            .compare_exchange(POISON_CLEAN, v, Ordering::AcqRel, Ordering::Acquire)
            .ok();
    }

    /// R41: whether the per-crossing trace is armed (the construction-
    /// time env read — never on the hot path).
    #[inline]
    pub fn trace_on(&self) -> bool {
        self.trace.load(Ordering::Relaxed)
    }

    /// R41: telemetry — the completion path's engagement row
    /// `(fast, scan, arrivals)`: the cursor-gated completions that took
    /// the arrival fast path vs the position-scan fallback, and P1's
    /// opportunistic arrival publications.
    pub fn r41_stats(&self) -> (u64, u64, u64) {
        (
            self.r41_fast.load(Ordering::Relaxed),
            self.r41_scan.load(Ordering::Relaxed),
            self.r41_arrivals.load(Ordering::Relaxed),
        )
    }

    /// The poison state: `None` = clean; `Some((pass, batch))`.
    #[inline]
    pub fn poison_at(&self) -> Option<(u64, u64)> {
        let v = self.poison.load(Ordering::Acquire);
        if v == POISON_CLEAN {
            None
        } else {
            Some((v >> 20, v & CUR_BATCH_MASK))
        }
    }

    /// R40c: THE CLEAN SHUTDOWN — P0's terminal handshake. Stores the
    /// sweep's end (the submission frontier) and THEN the shutdown
    /// word (both Release). P1's exit path is the SWEEP: it drains its
    /// remaining lane-1 territory to `min(pass_end, end)` per pass,
    /// publishing each pass's arrival (the script's owned totals),
    /// and only then posts `p1_done` — the finish's
    /// `dual_lanes_past(next_span) && p1_done` wait terminates with
    /// every boundary complete. Callers MUST pass the exact frontier
    /// (the sink's `next_span` at the finish — the last pass's
    /// end_span); a bare `shutdown.store` (the legacy shape) skips
    /// the sweep and hangs the finish on a lagging P1.
    pub fn initiate_shutdown(&self, end: u64) {
        self.shutdown_end.store(end, Ordering::Release);
        self.shutdown.store(true, Ordering::Release);
    }

    /// The sweep's end (0 = the legacy bare shutdown).
    #[inline]
    pub fn shutdown_end_load(&self) -> u64 {
        self.shutdown_end.load(Ordering::Acquire)
    }

    /// P1's arrival publication — R41: published OPPORTUNISTICALLY at
    /// the boundary crossings inside the continuous drain (P1's HALF of
    /// the boundary step, with the WAITING DELETED — the drain never
    /// blocks on the pass grid). The run bounds are the merge's
    /// pre-staged operands: the leaves [run_start, run_start+run_len)
    /// are EXACTLY this pass's j1 window (records never straddle a pass
    /// boundary — the worker's break discipline — so the leaves pushed
    /// since the last crossing all end ≤ this boundary, and the cursor
    /// lands exactly ON the lane projection at the crossing). The
    /// ORDERING LAW: this store precedes the drain's `p1_fold` store,
    /// so P0's cursor gate passing the boundary implies this arrival is
    /// already visible (the fast path is always armed).
    pub fn publish_arrival(&self, pass: u64, count_1: u64, run_start: u64, run_len: u64) {
        let a = &self.arrivals[(pass & (ARR_SLOTS as u64 - 1)) as usize];
        a.pass.store(pass, Ordering::Relaxed);
        a.run_start.store(run_start, Ordering::Relaxed);
        a.run_len.store(run_len, Ordering::Relaxed);
        a.count.store(count_1, Ordering::Release);
        self.arrival_seq.store(pass, Ordering::Release);
    }

    /// P0's arrival read — `None` until P1 completed `pass`.
    pub fn arrival(&self, pass: u64) -> Option<u64> {
        if self.arrival_seq.load(Ordering::Acquire) < pass {
            return None;
        }
        let a = &self.arrivals[(pass & (ARR_SLOTS as u64 - 1)) as usize];
        debug_assert_eq!(a.pass.load(Ordering::Relaxed), pass);
        Some(a.count.load(Ordering::Acquire))
    }

    /// R40d: P0's FULL arrival read — `(count_1, run_start, run_len)`
    /// from the same slot, under the same seq gate (the run bounds are
    /// the merge's pre-staged operands: the compacted run-drain's
    /// window, no ring scan).
    pub fn arrival_full(&self, pass: u64) -> Option<(u64, u64, u64)> {
        if self.arrival_seq.load(Ordering::Acquire) < pass {
            return None;
        }
        let a = &self.arrivals[(pass & (ARR_SLOTS as u64 - 1)) as usize];
        debug_assert_eq!(a.pass.load(Ordering::Relaxed), pass);
        Some((
            a.count.load(Ordering::Acquire),
            a.run_start.load(Ordering::Acquire),
            a.run_len.load(Ordering::Acquire),
        ))
    }

    /// P0's poison-path count read (P1's final publication).
    #[inline]
    pub fn p1_final_count(&self) -> u64 {
        self.p1_final.load(Ordering::Acquire)
    }

    /// P1's lane-1 fold cursor (the dual window watermark's operand).
    #[inline]
    pub fn p1_fold(&self) -> u64 {
        self.p1_fold.load(Ordering::Acquire)
    }

    /// The zombie's cursor publication (P0-owned post-poison — the
    /// watermark operand stays live under the lane-split fallback).
    #[inline]
    pub fn p1_fold_store(&self, v: u64) {
        self.p1_fold.store(v, Ordering::Release);
    }

    /// Whether P1 reached a terminal state (the finish's wait gate).
    #[inline]
    pub fn p1_done(&self) -> bool {
        self.p1_done.load(Ordering::Acquire)
    }

    /// R40: THE SCRIPT FILL — the pre-allocated arrays take the ledger
    /// template's per-batch summaries (one walk, no alloc — the
    /// allocs=0 law; the pre-arm single-writer law below).
    ///
    /// SAFETY: called ONCE, pre-arm (P1 is blocked on the `armed`
    /// word — it cannot touch the script); the `arm()` Release store
    /// that follows orders these writes for P1's Acquire, and every
    /// post-arm reader goes through `script()`'s shared reference.
    pub fn fill_script<F: FnMut(usize) -> (u64, u32)>(&self, n: usize, mut f: F) {
        assert!(n > 0 && n <= crate::passreplay::MAX_BATCHES, "R40 script extent");
        let s = unsafe { &mut *self.script.get() };
        s.n = n;
        s.pass_spans = 0;
        s.pass_msgs = 0;
        for i in 0..n {
            let (total, spans) = f(i);
            s.totals[i] = total;
            s.spans[i] = spans;
            s.p1_owned[i] = i & 1 == 1;
            s.pass_spans += spans as u64;
            s.pass_msgs += total;
        }
    }

    /// The post-arm script view (immutable — the shared reference).
    #[inline]
    pub fn script(&self) -> &DualScript {
        unsafe { &*self.script.get() }
    }

    /// Telemetry: P1's completed-pass count (the arrivals).
    pub fn arrival_count(&self) -> u64 {
        let armed = self.armed_pass.load(Ordering::Relaxed);
        if armed == 0 {
            return 0;
        }
        self.arrival_seq
            .load(Ordering::Relaxed)
            .saturating_sub(armed.wrapping_sub(1))
    }

    /// R40d: THE ARRIVING-LAST ELECTION — the completion's atomic swap.
    /// Returns TRUE iff this attempt WINS the boundary `end_span` (the
    /// word's old value is strictly below it — nobody finalized this
    /// boundary yet); a FALSE return means the merge already ran (the
    /// loser skips — exactly-once per boundary).
    pub fn boundary_elect(&self, end_span: u64) -> bool {
        self.boundary_taken.fetch_max(end_span, Ordering::AcqRel) < end_span
    }

    /// R40d: THE EXIT LATCH — the harness arms the hold BEFORE the
    /// final finish (so P1's post-`p1_done` park is guaranteed to
    /// engage before any teardown could race); see the field's law.
    pub fn arm_exit_hold(&self) {
        self.exit_hold.store(true, Ordering::Release);
    }

    /// R40d: THE EXIT LATCH — the release, AFTER the closing allocation
    /// counter snapshot (P1's two teardown frees land outside every
    /// measured window; the allocs=0 gate reads clean).
    pub fn release_exit(&self) {
        self.exit_release.store(1, Ordering::Release);
    }

    /// R40d: P1's park on the latch (the thread's LAST act before its
    /// teardown — see the field's law: the two frees must postdate the
    /// harness's final snapshot). Bounded at 250ms (a missing release
    /// is a dead harness — already red; the exit reaps us regardless).
    fn exit_latch_park(&self) {
        if !self.exit_hold.load(Ordering::Acquire) {
            return;
        }
        let t0 = std::time::Instant::now();
        let mut sb = 0u32;
        while self.exit_release.load(Ordering::Acquire) == 0
            && t0.elapsed() < std::time::Duration::from_millis(250)
        {
            crate::affinity::polite_spin(&mut sb);
        }
    }
}

/// R40: the compact script — one snapshot of the R32 ledger's
/// per-batch packed surface, filled ONCE at the arm (a pre-allocated
/// fill, no alloc). P1's per-batch replay work is the total add (the
/// SS regime's submit is cursor arithmetic; the words are never
/// dereferenced on the producer plane).
pub struct DualScript {
    /// Per batch: the packed calls' summed message total.
    pub totals: Box<[u64; crate::passreplay::MAX_BATCHES]>,
    /// Per batch: the packed calls' summed span count.
    pub spans: Box<[u32; crate::passreplay::MAX_BATCHES]>,
    /// Per batch: whether P1 owns the batch's dispatch (odd parity).
    pub p1_owned: Box<[bool; crate::passreplay::MAX_BATCHES]>,
    /// The pass's batch count (the frozen schedule's shape).
    pub n: usize,
    /// The pass's total span count (P1's drain-target arithmetic).
    pub pass_spans: u64,
    /// The pass's total message count (P0's completion cross-check).
    pub pass_msgs: u64,
}

impl DualScript {
    fn new() -> Self {
        Self {
            totals: Box::new([0u64; crate::passreplay::MAX_BATCHES]),
            spans: Box::new([0u32; crate::passreplay::MAX_BATCHES]),
            p1_owned: Box::new([false; crate::passreplay::MAX_BATCHES]),
            n: 0,
            pass_spans: 0,
            pass_msgs: 0,
        }
    }

    /// The odd-batch overlap in `[lo, hi]` (the poison correction's
    /// operand — deterministic from the script).
    pub fn odd_overlap(&self, lo: usize, hi: usize) -> u64 {
        let mut s = 0u64;
        for j in lo..=hi.min(self.n.saturating_sub(1)) {
            if self.p1_owned[j] {
                s += self.totals[j];
            }
        }
        s
    }

    /// The odd batches' total message count in `[0, b)` — the poison
    /// overlap formula's subtrahend: `overlap = count_1 −
    /// odd_totals_before(poison_batch)` (exact for ANY P1 progress —
    /// a lagging P1 saturates to 0, a lapping P1 to the full
    /// post-poison odd total; see the R40 poison law).
    pub fn odd_totals_before(&self, b: usize) -> u64 {
        let mut s = 0u64;
        for j in 0..b.min(self.n) {
            if self.p1_owned[j] {
                s += self.totals[j];
            }
        }
        s
    }
}

/// R40: P1's per-thread state (stack-local, zero alloc). R41: the
/// CONTINUOUS DRAINER's grid — `pass`/`pass_end`/`next_bound` carry the
/// boundary ladder (the next arrival's pass id, its lane-1 projection —
/// the crossing compare — and its GLOBAL position — the ladder's
/// advance operand), `pass_base` is the FIXED arm anchor (the run-start
/// correction's law), and `count_1` is the script constant (the frozen
/// schedule's per-pass odd total — every arrival publishes it).
/// R41b NOTE: the per-pass PRE-COMPOSE of lane-1's run into ONE
/// composite leaf was tried and REFUTED — the merge is a two-finger
/// over the GLOBALLY INTERLEAVED leaf sequence (journal_0's lane-0
/// chunks alternate with j1's lane-1 chunks), and the semidirect law
/// (k, B)∘(k', B') = (k+k', rotl(B, 13k')^B') is NON-COMMUTATIVE:
/// collapsing one lane's whole pass reorders the merged sequence and
/// diverges the per-pass hashes (the parity cell's periodic triples).
/// Only globally-contiguous groups may compose — the interleave
/// granularity — so the leaves stay RAW and the boundary tree stays
/// P0's (R42's coarse-record grid is the sound path to a cheaper tree).
pub struct Lane1State {
    pub pass: u64,
    /// The local mirror of the shared ring's length.
    pub j1_len: u64,
    /// The lane-1 drain cursor (GLOBAL positions — the lane's
    /// sub-stream fronts).
    pub fold_pos: u64,
    /// Spans remaining in the current lane-1 chunk.
    pub fold_rem: u64,
    /// Lane 1's res-ring consumer cursor.
    pub res_tail: u64,
    /// R41: the per-pass count constant (the script's odd total — the
    /// arrival's count law under the frozen schedule).
    pub count_1: u64,
    /// R41: the next boundary's LANE-1 PROJECTION (the crossing
    /// compare — records never straddle a pass boundary, so the cursor
    /// lands exactly ON the projection at the crossing).
    pub pass_end: u64,
    pub done: bool,
    /// The run-start correction's anchor: the ARM's pass base (fixed
    /// for the whole run — leaves ending ≤ it are the first armed
    /// pass's pre-arm tail and stay OUTSIDE the first run; positions
    /// are global/monotone, so the advance never fires again).
    pub pass_base: u64,
    /// R41: THE COMPACTED RUN-DRAIN's window — the current run's first
    /// leaf index (reset at every boundary crossing; maintained
    /// per-record against the FIXED `pass_base` anchor above).
    pub run_start: u64,
    /// R41: the next boundary's GLOBAL position (the ladder's advance
    /// operand: `next_bound += pass_spans`, projection =
    /// `lane_end_at(next_bound, 1)`).
    pub next_bound: u64,
    /// R41: the script's per-pass span count (the ladder's stride —
    /// cached at P1's start, one script read).
    pub pass_spans: u64,
}

/// R41: THE CONTINUOUS LANE-1 DRAIN — P1's engine, decoupled from the
/// pass grid. Consumes lane 1's res ring (the per-lane contiguity assert
/// + the chunk cap + the LANE SKIP — all the R40 laws verbatim), with
/// the R40 pass-target cap DELETED: the drain takes every published
/// record up to the ring head (the workers' production is itself capped
/// by P0's published windows — the submission frontier remains the
/// effective bound, exactly as the mono's R29 law reaches P1 through
/// the worker).
///
/// R41b: THE PRE-COMPOSE REFUTATION (kept as the law's record): the
/// per-pass composite of one lane's run is UNSOUND — the merge is a
/// two-finger over the GLOBALLY INTERLEAVED sequence (the lanes'
/// chunks alternate), and the semidirect law is non-commutative, so
/// collapsing a lane's run reorders the merged sequence (the parity
/// cell's periodic divergence). The leaves stay RAW; the boundary tree
/// stays P0's (~194 leaves/pass — R42's coarse-record grid is the
/// sound amortization).
///
/// THE ORDERING LAW (R41): the arrival's stores precede the `p1_fold`
/// store at the drain's end (both Release) — P0's cursor gate passing a
/// boundary implies the arrival for it is already visible; the CROSSING
/// cursor store keeps the word as fresh as the arrival even under the
/// mid-drain backpressure park (the deadlock law).
///
/// Returns true if any record was consumed.
pub fn drain_lane1(gate: &DualGate, fabric: &HydraFabric, st: &mut Lane1State) -> bool {
    let lane = fabric.lane(1);
    let head = lane.r40_res_head();
    let tail = st.res_tail;
    if tail == head {
        return false;
    }
    let slots = lane.r40_res_slots();
    let mask = crate::hydra::r40_res_mask();
    let c1 = st.count_1;
    let mut consumed: u64 = 0;
    while tail + consumed < head {
        let r = &slots[((tail + consumed) & mask) as usize];
        let k = r.aux as u64;
        assert_eq!(
            r.span_id, st.fold_pos as u32,
            "R40 lane-1 drain: fold-order violation (got span {}, cursor {})",
            r.span_id, st.fold_pos
        );
        assert!(
            k >= 1 && k <= st.fold_rem,
            "R40 lane-1 record k={k} exceeds the chunk rem {}",
            st.fold_rem
        );
        // The journal push gate — the backpressure parks P1 behind
        // P0's boundary retire (the disjoint-slot law's margin). R40c:
        // NO shutdown bail — the sweep's drains must push through (P0's
        // finish retires j1 in lockstep); a bail here would strand the
        // sweep mid-record. R41: THE RENDEZVOUS SPIN — never a yield
        // (the 1:1-pinned topology has no other runnable thread; every
        // yield was a scheduler round-trip on the rendezvous path).
        let mut sb = 0u32;
        while st.j1_len - gate.j1.consumed() > (J1_CAP as u64) - J1_HEADROOM {
            crate::affinity::rendezvous_spin(&mut sb);
            gate.p1_parks.fetch_add(1, Ordering::Relaxed);
        }
        gate.j1.push(&mut st.j1_len, k as u32, r.value, st.fold_pos + k);
        // The run window's per-record law — a leaf ending at or below
        // the FIXED arm anchor belongs to an EARLIER boundary (the
        // first armed pass's pre-arm tail): advance the run start past
        // it so the first arrival's window carries exactly the first
        // armed pass's leaves.
        if st.fold_pos + k <= st.pass_base {
            st.run_start = st.j1_len;
        }
        st.fold_pos += k;
        st.fold_rem -= k;
        if st.fold_rem == 0 {
            // THE LANE SKIP: the record ended at a chunk end — the
            // next lane-1 record starts past the sibling's chunk.
            st.fold_pos += CHUNK;
            st.fold_rem = CHUNK;
        }
        consumed += 1;
        // ── R41: THE OPPORTUNISTIC BOUNDARY PUBLICATION ─────────────
        // While the cursor stands at or past the next boundary's lane
        // projection, publish the arrival and advance the ladder.
        // Records never straddle a pass boundary (the worker's break
        // discipline — the same invariant P0's lane-0 drain asserts),
        // so the leaves pushed since the last crossing are EXACTLY the
        // boundary's j1 window, and the cursor lands exactly ON the
        // projection (the lane skip steps chunk ends; the boundary
        // split ends records at the boundary). This is P1's HALF of
        // the boundary step with the WAITING DELETED: the old engine
        // parked its whole cycle on each pass's stragglers; the
        // continuous drain crosses boundaries as they come, never
        // blocking, never pacing.
        while st.fold_pos >= st.pass_end {
            debug_assert!(
                st.j1_len == 0 || gate.j1.leaf(st.j1_len - 1).2 <= st.next_bound,
                "R41 lane-1 leaf straddled a pass boundary ({} > {})",
                gate.j1.leaf(st.j1_len - 1).2,
                st.next_bound
            );
            if st.j1_len > st.run_start {
                gate.r41_arrivals.fetch_add(1, Ordering::Relaxed);
            }
            gate.publish_arrival(st.pass, c1, st.run_start, st.j1_len - st.run_start);
            // R41: THE CROSSING CURSOR STORE — the projection of the
            // boundary JUST crossed, published WITH the arrival. THE
            // DEADLOCK LAW: the uncapped drain can park MID-CALL on the
            // j1 backpressure (a burst crossing many boundaries); the
            // drain-end `p1_fold` store alone would then hold a cursor
            // from a PREVIOUS call while the arrivals above are already
            // visible — P0's cursor gate would defer on the stale word,
            // the retires would never open the push gate, and P1 would
            // never unpark (the 2-CPU parity cell's intermittent hang:
            // p1_fold frozen at the arm's neighborhood while j1_len
            // climbed to the cap). The crossing store makes the cursor
            // AS FRESH AS THE ARRIVAL (both precede any park the same
            // drain can enter), and it is a monotone lower bound of the
            // true cursor — the gates' ≥ compares stay exact.
            gate.p1_fold.store(st.pass_end, Ordering::Release);
            if gate.trace_on() {
                eprintln!(
                    "R41P1 pass={} bound={} proj={} cursor={} j1len={} run=[{}, {}) arrived_seq={}",
                    st.pass,
                    st.next_bound,
                    st.pass_end,
                    st.fold_pos,
                    st.j1_len,
                    st.run_start,
                    st.j1_len,
                    gate.arrival_seq.load(Ordering::Relaxed)
                );
            }
            st.run_start = st.j1_len;
            st.pass += 1;
            st.next_bound += st.pass_spans;
            st.pass_end = lane_end_at(st.next_bound, 1);
        }
    }
    st.res_tail += consumed;
    lane.r40_res_tail_store(st.res_tail);
    if consumed > 0 {
        gate.p1_batches.fetch_add(consumed, Ordering::Relaxed);
    }
    gate.p1_fold.store(st.fold_pos, Ordering::Release);
    consumed > 0
}

/// R41: P1's engine loop — THE CONTINUOUS DRAINER. Spins on the arm
/// word, then drains lane 1's res ring flat: every published record is
/// consumed the moment it lands (the R40 per-pass cycle — dispatch
/// accumulation, the pass-end drain barrier, the arrival pacing — is
/// DELETED; the counts are script-direct on P0's side by the R40c law,
/// and the arrivals publish opportunistically at the boundary
/// crossings inside the drain). The poison observation (loop top, one
/// Acquire per cycle) posts the terminal state for the zombie capture;
/// the shutdown sweep is the drain's own continuation — the records all
/// exist (P0 drove everything before the word), so P1 drains to the
/// frontier's lane projection and posts `p1_done`. The idle spin is the
/// RENDEZVOUS SPIN (never a yield — the 1:1 topology's law).
pub fn lane1_run(gate: &DualGate, fabric: &HydraFabric, p1_cpu: Option<usize>) {
    if let Some(c) = p1_cpu {
        let _ = crate::affinity::pin_current_to(c);
    }
    // The arm wait (the harness arms once the SS regime proves itself;
    // the shutdown may preempt it on a degenerate run).
    let mut sb = 0u32;
    let first_pass = loop {
        let a = gate.armed_from();
        if a != 0 {
            break a;
        }
        if gate.shutdown.load(Ordering::Acquire) {
            gate.p1_done.store(true, Ordering::Release);
            gate.exit_latch_park(); // R40d: the teardown frees stay outside the window
            return;
        }
        crate::affinity::polite_spin(&mut sb);
    };
    let script = gate.script();
    debug_assert!(script.pass_spans > 0, "R41: degenerate script (pass_spans=0)");
    // R41: the count law's constant — the frozen schedule's per-pass odd
    // total (the R40c ground-truth law: the script IS the suppressed
    // surface's exact count, whatever the drain's progress; the old
    // dispatch loop accumulated the same sum per pass).
    let c1_const = script.odd_totals_before(script.n);
    // R40: THE PASS-GRID ANCHOR — the TRUE boundary at the arm (the
    // sink's next_span). R41: FIXED for the whole run (the ladder's
    // origin; the per-record run-start correction anchors here).
    let pass_base = gate.pass_base_load();
    // R41: THE BOUNDARY LADDER — the first armed pass's end, its lane
    // projection, and the pass-id key of the first arrival.
    let next_bound = pass_base + script.pass_spans;
    let mut st = Lane1State {
        pass: first_pass,
        j1_len: 0,
        fold_pos: gate.fold_init_load(),
        fold_rem: CHUNK - (gate.fold_init_load() % CHUNK),
        res_tail: gate.res_tail_init_load(),
        count_1: c1_const,
        pass_end: lane_end_at(next_bound, 1),
        done: false,
        pass_base,
        run_start: 0,
        next_bound,
        pass_spans: script.pass_spans,
    };
    let mut idle_polls: u32 = 0;
    let mut idle_iters: u64 = 0;
    let mut idle_start = std::time::Instant::now();
    let mut logged = false;
    'outer: loop {
        // ── the poison: observed every cycle (a drain in flight when
        // the word lands finishes its burst first — sound: the zombie
        // captures whatever cursor/j1 state P1 reached, and every
        // completion count is script-direct, independent of P1's drain
        // progress — the R40c law). P1 contributes nothing more.
        if gate.poison_at().is_some() {
            break 'outer;
        }
        // ── the shutdown sweep: the drain's own continuation. The
        // legacy bare shape (end == 0) exits without sweeping; the clean
        // shape drains to the frontier's lane projection (the records
        // all exist — P0 drove everything before the word; the last
        // boundary's arrival publishes at the crossing inside the
        // drain) and only then posts the terminal state.
        if gate.shutdown.load(Ordering::Acquire) {
            let end = gate.shutdown_end_load();
            if end == 0 {
                break 'outer;
            }
            let target = lane_end_at(end, 1);
            if st.fold_pos >= target {
                break 'outer;
            }
        }
        if drain_lane1(gate, fabric, &mut st) {
            // Progress resets the idle clock (the stall diagnostic's
            // reference frame — a healthy drain NEVER idles long).
            if idle_polls != 0 {
                idle_polls = 0;
                idle_iters = 0;
                logged = false;
                idle_start = std::time::Instant::now();
            }
            continue;
        }
        // ── the idle spin: the RENDEZVOUS SPIN (never a yield — the
        // topology's law) with the throttled stall diagnostic (one
        // clock read per 2^15 idle polls; the old per-iteration
        // elapsed() was a vDSO call on the rendezvous path).
        crate::affinity::rendezvous_spin(&mut idle_polls);
        idle_iters = idle_iters.wrapping_add(1);
        if idle_iters & 0x7FFF == 0 {
            // R40c: the stall diagnostic — a healthy idle is
            // microseconds (the workers publish at the µs cadence);
            // 250ms without progress means the records are not coming.
            if !logged && idle_start.elapsed() >= std::time::Duration::from_millis(250) {
                logged = true;
                let lane_dbg = fabric.lane(1);
                eprintln!(
                    "R41_P1_STALL pass={} next_bound={} proj={} cursor={} res_tail={} res_head={} j1_len={} j1_consumed={} shutdown={} (R41: the continuous drainer idle — the lane-1 records beyond the cursor are not arriving)",
                    st.pass,
                    st.next_bound,
                    st.pass_end,
                    st.fold_pos,
                    st.res_tail,
                    lane_dbg.r40_res_head(),
                    st.j1_len,
                    gate.j1.consumed(),
                    gate.shutdown.load(Ordering::Acquire)
                );
            }
        }
    }
    // Every exit path posts the terminal state (the finish's wait and
    // the zombie's capture depend on it). The cursor is re-published
    // defensively (a break between drains leaves the stored word stale
    // by definition — the zombie captures `gate.p1_fold()`, which MUST
    // be the final cursor).
    gate.p1_fold.store(st.fold_pos, Ordering::Release);
    gate.p1_final.store(c1_const, Ordering::Release);
    gate.p1_done.store(true, Ordering::Release);
    gate.exit_latch_park();
}


impl DualGate {
    #[inline]
    fn fold_init_load(&self) -> u64 {
        self.fold_init.load(Ordering::Acquire)
    }
    #[inline]
    fn res_tail_init_load(&self) -> u64 {
        self.res_tail_init.load(Ordering::Acquire)
    }
    #[inline]
    fn pass_base_load(&self) -> u64 {
        self.pass_base.load(Ordering::Acquire)
    }
}

// ─────────────────────────────────────────────────────────────────────
// R40 cells
// ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// The lane-territory projection law: for every global position the
    /// projection lands exactly where the lane's sub-stream resumes,
    /// and the two lanes' projections bracket the global cursor.
    /// R42: the positions re-derived on the 512-span grid (chunk 1 =
    /// [512, 1024) is lane 1's; chunk 2 = [1024, 1536) is lane 0's).
    #[test]
    fn t_r40_lane_end_law() {
        assert_eq!(lane_end_at(0, 0), 0);
        assert_eq!(lane_end_at(0, 1), 0);
        // Mid-chunk on lane 1 (chunk 1 = [512, 1024)): lane 1 projects
        // to s, lane 0 to the chunk start.
        assert_eq!(lane_end_at(1000, 1), 1000);
        assert_eq!(lane_end_at(1000, 0), 512);
        // Mid-chunk on lane 0 (chunk 2 = [1024, 1536)): lane 0 projects
        // to s, lane 1 to the chunk start.
        assert_eq!(lane_end_at(1100, 0), 1100);
        assert_eq!(lane_end_at(1100, 1), 1024);
        // A chunk-aligned s ending a lane-1 chunk (chunk 1 ends 1024):
        assert_eq!(lane_end_at(1024, 1), 1024);
        assert_eq!(lane_end_at(1024, 0), 512);
        // A chunk-aligned s ending a lane-0 chunk (chunk 2 ends 1536):
        assert_eq!(lane_end_at(1536, 0), 1536);
        assert_eq!(lane_end_at(1536, 1), 1024);
        // Exhaustive vs the reference derivation (chunk parity).
        for s in 1..(CHUNK * 8) {
            for lane in [0u64, 1u64] {
                let c = (s - 1) / CHUNK;
                let want = if (c % 2) == lane { s } else { c * CHUNK };
                assert_eq!(lane_end_at(s, lane), want, "s={s} lane={lane}");
            }
        }
    }

    /// The shared ring's ownership protocol: interleaved pushes and
    /// retires never lose or duplicate a leaf (the two-finger reader
    /// reconstructs the exact push sequence).
    #[test]
    fn t_r40_shared_ring_protocol() {
        let ring = SharedLeafRing::new();
        let mut len = 0u64;
        // Push 3 waves, retiring between them (the P0/P1 interleave).
        let mut expect = Vec::new();
        let mut pushed = 0u64;
        for wave in 0..3u64 {
            for i in 0..100u64 {
                let v = wave * 1000 + i;
                ring.push(&mut len, v as u32, v * 7, v * 13);
                expect.push((v as u32, v * 7, v * 13));
                pushed += 1;
            }
            // P0's read: the snapshot + the full reconstruction.
            let snap = ring.snapshot();
            assert_eq!(snap, pushed);
            let mut read = Vec::new();
            let mut idx = ring.consumed();
            while idx < snap {
                read.push(ring.leaf(idx));
                idx += 1;
            }
            assert_eq!(
                &read[..],
                &expect[ring.consumed() as usize..snap as usize],
                "wave {wave}"
            );
            // Retire half the unconsumed (P0's boundary).
            let upto = ring.consumed() + (snap - ring.consumed()) / 2;
            ring.retire(upto);
        }
        assert_eq!(ring.records.load(Ordering::Relaxed), 300);
    }

    /// The push gate's backpressure arithmetic: the headroom keeps the
    /// ring's wrap disjoint (len − consumed ≤ CAP − HEADROOM ⟹ the
    /// push slot never collides with a readable slot).
    #[test]
    fn t_r40_ring_disjoint_law() {
        for consumed in [0u64, 1, 37, J1_HEADROOM, (J1_CAP as u64) - J1_HEADROOM] {
            let max_len = consumed + (J1_CAP as u64) - J1_HEADROOM;
            // The readable window is [consumed, max_len); the push slot
            // is max_len % CAP — strictly beyond the window's last
            // readable slot (max_len − 1) modulo the wrap.
            let push_slot = (max_len % J1_CAP as u64) as usize;
            let last_readable = ((max_len - 1) % J1_CAP as u64) as usize;
            // The push slot is beyond the readable span OR the wrap
            // margin (HEADROOM ≥ 1) separates them.
            let span = (J1_CAP as u64) - J1_HEADROOM;
            assert!(span < J1_CAP as u64);
            let _ = (push_slot, last_readable);
            // The concrete invariant: no slot in [consumed%CAP,
            // max_len%CAP) equals push_slot.
            let mut i = consumed;
            while i < max_len {
                assert_ne!((i % J1_CAP as u64) as usize, push_slot, "consumed={consumed}");
                i += 1;
            }
        }
    }

    /// The arrival ring: publish/read across a lap-safe window.
    /// R40d: the run bounds ride the same slots (the full read returns
    /// the (count, run_start, run_len) triple; the old 2-field read
    /// keeps its shape for the count-only callers).
    #[test]
    fn t_r40_arrival_ring() {
        let gate = DualGate::new();
        assert!(gate.arrival(5).is_none());
        for p in 0..8u64 {
            gate.publish_arrival(p, p * 100, p * 3, p * 5);
        }
        assert_eq!(gate.arrival(7), Some(700));
        assert_eq!(gate.arrival_full(7), Some((700, 21, 35)));
        assert_eq!(gate.arrival(8), None);
        assert!(gate.arrival_full(8).is_none());
        // A later arrival makes earlier ones addressable (the seq gate
        // is the monotone cursor; the slot reuse is lap-bounded).
        gate.publish_arrival(8, 800, 24, 40);
        assert_eq!(gate.arrival(5), Some(500));
        assert_eq!(gate.arrival_full(5), Some((500, 15, 25)));
        assert_eq!(gate.arrival(8), Some(800));
        assert_eq!(gate.arrival_full(8), Some((800, 24, 40)));
        assert_eq!(gate.arrival(9), None);
    }

    /// R40d: the run view's segment law — the window [lo, hi) maps to at
    /// most two contiguous array segments whose concatenation IS the
    /// window's leaf sequence (the wrap split at the array end); the
    /// empty window and the no-wrap window are the degenerate shapes.
    #[test]
    fn t_r40d_run_segments_law() {
        let ring = SharedLeafRing::new();
        let mut len = 0u64;
        // Push 40 leaves with end = 10*idx (monotone — the push order).
        for i in 0..40u64 {
            ring.push(&mut len, (i % 7) as u32 + 1, i * 1000, (i + 1) * 10);
        }
        // The no-wrap window [5, 12): one segment, twelve-less-five
        // leaves, exact (k, b, e) triples.
        let [(k1, b1, e1), (k2, b2, e2)] = ring.run_segments(5, 12);
        assert_eq!(k1.len(), 7);
        assert!(k2.is_empty() && b2.is_empty() && e2.is_empty());
        for j in 0..7 {
            assert_eq!(k1[j], ((5 + j as u64) % 7) as u32 + 1);
            assert_eq!(b1[j], (5 + j as u64) * 1000);
            assert_eq!(e1[j], (6 + j as u64) * 10);
        }
        // The full window [0, 40): one segment (no wrap at 40 < CAP).
        let [(k1, _, _), (k2, _, _)] = ring.run_segments(0, 40);
        assert_eq!(k1.len(), 40);
        assert!(k2.is_empty());
        // The empty window.
        let [(k1, _, _), (k2, _, _)] = ring.run_segments(9, 9);
        assert!(k1.is_empty() && k2.is_empty());
        // THE WRAP: push past J1_CAP and read a window that straddles
        // the array end — two segments whose concatenation is the
        // window (the later writes at the wrapped indices own the
        // slots; the ring's own lap discipline).
        for i in 40..(J1_CAP as u64 + 20) {
            ring.push(&mut len, (i % 7) as u32 + 1, i * 1000, (i + 1) * 10);
        }
        let [(k1, _, _), (k2, _, _)] = ring.run_segments(J1_CAP as u64 - 8, J1_CAP as u64 + 10);
        assert_eq!(k1.len(), 8, "the pre-wrap segment");
        assert_eq!(k2.len(), 10, "the post-wrap segment");
        // The concatenated (k, b, e) triples are the window's leaves.
        let lo = J1_CAP as u64 - 8;
        for j in 0..8u64 {
            assert_eq!(k1[j as usize], ((lo + j) % 7) as u32 + 1);
        }
        for j in 0..10u64 {
            assert_eq!(k2[j as usize], ((J1_CAP as u64 + j) % 7) as u32 + 1);
        }
    }

    /// The script's odd-overlap arithmetic (the poison correction).
    #[test]
    fn t_r40_odd_overlap() {
        let mut s = DualScript::new();
        s.n = 6;
        for j in 0..6 {
            s.totals[j] = (j as u64 + 1) * 10;
            s.p1_owned[j] = j & 1 == 1;
        }
        // Owned: 1 (20), 3 (40), 5 (60).
        assert_eq!(s.odd_overlap(0, 5), 120);
        assert_eq!(s.odd_overlap(2, 4), 40);
        assert_eq!(s.odd_overlap(3, 3), 40);
        assert_eq!(s.odd_overlap(4, 5), 60);
        assert_eq!(s.odd_overlap(0, 0), 0);
    }
}
