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
//! * **THE ARRIVAL PROTOCOL**: P1 publishes `(pass, count_1)` at every
//!   pass close (Release); P0's completion fires when BOTH lanes'
//!   cursors crossed the boundary (`lane_end_at`) and P1's arrival
//!   landed — the lazy law (a lagging P1 defers the completion; P0
//!   never blocks mid-pass; the deferred boundary folds during the
//!   next pass exactly as the mono's tail does).
//! * **THE POISON LAW**: any regime break (a live re-scan, a non-SS
//!   window) poisons the engine permanently — P1 stops dispatching at
//!   the next batch check, publishes its final count + cursor, and P0
//!   computes the deterministic overlap correction from the script
//!   (the odd batches in `[poison_batch, p1_cursor]`) so the merged
//!   count stays exact. The pass completes under the merge (the
//!   insertion-sorted path — the post-poison journal concatenation is
//!   not sorted), and every later pass runs the MONO loop verbatim.
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

/// The shared leaf ring's capacity (leaves). One steady pass journals
/// ~86 lane-1 leaves; the backpressure headroom (256) bounds P1's lap
/// over P0's boundary retire at (2048 − 256)/86 ≈ 20 passes — inside
/// the 64-slot arrival ring's validity window (ARR_SLOTS ≥ 2× the
/// bound; the script's batches ≤ 64 → ≤ ~200 leaves/pass at the
/// fleet's shape LOWERS the bound further; degenerate shapes clamp at
/// the push gate and park P1 behind P0's retire).
pub const J1_CAP: usize = 2048;
/// The push gate's headroom: P1 parks when `len − consumed` exceeds
/// this (the margin keeps P0's in-flight merge reads stable — the
/// disjoint-slot law).
pub const J1_HEADROOM: u64 = 256;
/// The arrival ring's slots (a power of two ≥ 2× the push-gate's
/// lead bound). The push gate parks P1 at (J1_CAP − J1_HEADROOM)
/// in-flight leaves ≈ 20.8 passes at the steady shape (~86
/// leaves/pass) — the arrival lead over P0's completion can reach
/// that bound, so the ring must hold MORE than it: 16 slots (the
/// original arithmetic claimed "20 well inside 16" — wrong, a lap
/// would overwrite an unconsumed arrival and either trip the
/// slot-pass debug assert or silently hand P0 the WRONG count).
/// 64 slots = 3× the bound; the degenerate shapes (~200 leaves/pass)
/// lower the bound further (≈ 9 passes) — wider still.
pub const ARR_SLOTS: usize = 64;

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
}

/// R40: one arrival slot — P1's per-pass completion publication.
struct Arrival {
    pass: AtomicU64,
    count: AtomicU64,
}

impl Arrival {
    const fn new() -> Self {
        Self {
            pass: AtomicU64::new(u64::MAX),
            count: AtomicU64::new(0),
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

    /// P1's arrival publication (its pass close).
    pub fn publish_arrival(&self, pass: u64, count_1: u64) {
        let a = &self.arrivals[(pass & (ARR_SLOTS as u64 - 1)) as usize];
        a.pass.store(pass, Ordering::Relaxed);
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

/// R40: P1's per-thread state (stack-local, zero alloc).
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
    /// This pass's count (P1's dispatched totals).
    pub count_1: u64,
    /// This pass's drain target (the script's pass end, global).
    pub pass_end: u64,
    pub done: bool,
}

/// R40: P1's lane-1 drain — the per-lane twin of the sink's composite
/// drain. Consumes lane 1's res ring into the shared journal,
/// asserting the per-lane contiguity law (`span_id == fold_pos`) and
/// the chunk cap (`k ≤ fold_rem`); the LANE SKIP (a chunk-end record
/// advances the cursor across the sibling lane's interposed chunk —
/// `+CHUNK`) keeps the cursor on lane-1 territory. The wrap-aware
/// frontier cap parks records that end beyond the current pass target
/// (they stay buffered — the same law as the mono's R29 cap).
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
    let target32 = st.pass_end as u32;
    let mut consumed: u64 = 0;
    while tail + consumed < head {
        let r = &slots[((tail + consumed) & mask) as usize];
        let k = r.aux as u64;
        // R29's wrap-aware frontier cap (the serial-number compare —
        // records ending beyond the pass target stay buffered).
        let end32 = r.span_id.wrapping_add(k as u32);
        let dcap = end32.wrapping_sub(target32);
        if dcap != 0 && dcap < 0x8000_0000 {
            break;
        }
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
        // finish retires j1 in lockstep: its wait loop completes every
        // arrival-gated boundary each poll); a bail here would strand
        // the sweep mid-record and freeze the cursor exactly where the
        // old deadlock did. The gate parks only on P0's retire lag —
        // a dead P0 hangs visibly (the fail-loud law) rather than
        // exiting with a lying arrival.
        let mut sb = 0u32;
        while st.j1_len - gate.j1.consumed() > (J1_CAP as u64) - J1_HEADROOM {
            crate::affinity::polite_spin(&mut sb);
            gate.p1_parks.fetch_add(1, Ordering::Relaxed);
        }
        gate.j1.push(&mut st.j1_len, k as u32, r.value, st.fold_pos + k);
        st.fold_pos += k;
        st.fold_rem -= k;
        if st.fold_rem == 0 {
            // THE LANE SKIP: the record ended at a chunk end — the
            // next lane-1 record starts past the sibling's chunk.
            st.fold_pos += CHUNK;
            st.fold_rem = CHUNK;
        }
        consumed += 1;
    }
    st.res_tail += consumed;
    lane.r40_res_tail_store(st.res_tail);
    gate.p1_fold.store(st.fold_pos, Ordering::Release);
    consumed > 0
}

/// R40: P1's engine loop. Spins on the arm word, then cycles the
/// script: the odd batches' dispatch (the count adds + the poison
/// checks + the cursor publications), the interleaved lane-1 drain,
/// the pass-end drain to the lane projection, and the arrival
/// publication. Exits on the shutdown word (the harness's finish) or
/// the poison (the permanent fallback — the final state posted).
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
            return;
        }
        crate::affinity::polite_spin(&mut sb);
    };
    let mut st = Lane1State {
        pass: first_pass,
        j1_len: 0,
        fold_pos: gate.fold_init_load(),
        fold_rem: CHUNK - (gate.fold_init_load() % CHUNK),
        res_tail: gate.res_tail_init_load(),
        count_1: 0,
        pass_end: 0,
        done: false,
    };
    // R40: THE PASS-GRID ANCHOR — the TRUE boundary at the arm (the
    // sink's next_span), NOT the fold projection (which may sit inside
    // the previous pass's deferred tail — the drift class the parity
    // cell caught). P1's cursor starts at the projection and drains
    // the tail on its way to the first target.
    let mut pass_base = gate.pass_base_load();
    let script = gate.script();
    'outer: loop {
        if st.done {
            break;
        }
        // R40c: THE SHUTDOWN SWEEP — the harness's clean finish (all
        // passes driven by P0; `initiate_shutdown(frontier)` posted
        // the end BEFORE the word). P1 does NOT exit on the word
        // alone: its remaining lane-1 territory is still P1's to
        // drain (nobody else owns the cursor pre-poison), and P0's
        // finish waits on `dual_lanes_past(frontier)` — a bare exit
        // freezes the cursor mid-stream and hangs the finish (the
        // parity cell's 2-of-6 deadlock face). The sweep processes
        // the remaining passes — the accounting from the script (the
        // completions' count operand — identical to the dispatch's
        // accumulation), the FULL pass-end drain (the records all
        // exist: P0 submitted everything before the word), and the
        // per-pass arrival — until the pass base reaches the
        // frontier. The legacy bare-shutdown (end == 0) breaks at the
        // top without sweeping (the pre-R40c call-site shape).
        let sweeping0 = gate.shutdown.load(Ordering::Acquire);
        let mut sweeping = sweeping0;
        if sweeping {
            let end = gate.shutdown_end_load();
            if end == 0 || pass_base >= end {
                break;
            }
        }
        // The per-pass count resets FIRST (the pre-pass poison's
        // final publication must carry the POISON pass's dispatched
        // count — zero when nothing was dispatched).
        st.count_1 = 0;
        // The pre-pass poison: a break at an earlier pass (or this
        // pass's batch 0) — P1 contributes nothing more.
        if let Some((pp, _pb)) = gate.poison_at() {
            if pp <= st.pass {
                gate.p1_final.store(st.count_1, Ordering::Release);
                gate.p1_done.store(true, Ordering::Release);
                break;
            }
        }
        st.pass_end = pass_base + script.pass_spans;
        if sweeping {
            // ── the sweep's accounting: the script's owned totals ──
            // (the dispatch already happened on P0's clock; the
            // counts are the boundary completions' operand — the
            // same accumulation the dispatch loop performs, minus
            // the pacing.)
            for j in 0..script.n {
                if script.p1_owned[j] {
                    st.count_1 += script.totals[j];
                    gate.p1_batches.fetch_add(1, Ordering::Relaxed);
                }
            }
        } else {
            // ── the dispatch: the odd batches ──
            for j in 0..script.n {
                if !script.p1_owned[j] {
                    continue;
                }
                // The mid-pass poison check (one Acquire per owned batch).
                if let Some((pp, pb)) = gate.poison_at() {
                    if pp < st.pass || (pp == st.pass && pb <= j as u64) {
                        // Stop AT the poison batch: count_1 holds exactly
                        // the odd totals dispatched so far (j's add has not
                        // run) — the completion's overlap formula
                        // (count_1 − odd_totals_before(poison_batch),
                        // saturating) is exact for ANY progress here.
                        gate.p1_final.store(st.count_1, Ordering::Release);
                        gate.p1_done.store(true, Ordering::Release);
                        break 'outer;
                    }
                }
                st.count_1 += script.totals[j];
                gate.p1_batches.fetch_add(1, Ordering::Relaxed);
                // The drain interleave (every 4th owned batch — the
                // records arrive with the window's walk).
                if (j & 3) == 3 {
                    drain_lane1(gate, fabric, &mut st);
                }
            }
        }
        // ── the pass-end drain: to the lane projection ──
        // R40c: NO unconditional shutdown break — the drain MUST
        // complete before the arrival publishes (an arrival with a
        // short cursor is a LIE: P0's completion would partition j1 at
        // the boundary and compose an incomplete leaf set — silent
        // corruption). Under the harness's law the shutdown word
        // implies every record of THIS pass exists (P0 drove it before
        // the word). THE MID-DRAIN RE-CAP: P1 may have entered this
        // pass in NORMAL mode (the phantom-pass race — P1's grid runs
        // one ring-id ahead of P0's driven passes at the tail, so the
        // pass beyond the frontier targets records that never exist);
        // observing the word mid-drain re-caps the target to the
        // frontier's projection — the sweep's own law, applied at the
        // earliest observation point.
        let mut lane1_target = if sweeping {
            lane_end_at(gate.shutdown_end_load().min(st.pass_end), 1)
        } else {
            lane_end_at(st.pass_end, 1)
        };
        if std::env::var("R40_CELL_TRACE").is_ok() {
            eprintln!(
                "R40P1 pass={} target={} cursor={} j1len={} consumed={} arrived_seq={}",
                st.pass, lane1_target, st.fold_pos, st.j1_len,
                gate.j1.consumed(),
                gate.arrival_seq.load(Ordering::Relaxed)
            );
        }
        let mut sb2 = 0u32;
        let drain_start = std::time::Instant::now();
        let mut logged = false;
        while st.fold_pos < lane1_target {
            if !sweeping && gate.shutdown.load(Ordering::Acquire) {
                sweeping = true;
                let end = gate.shutdown_end_load();
                let capped = if end == 0 {
                    break;
                } else {
                    lane_end_at(end.min(st.pass_end), 1)
                };
                if st.fold_pos >= capped {
                    break;
                }
                lane1_target = capped;
                continue;
            }
            if !drain_lane1(gate, fabric, &mut st) {
                // R40c: the stall diagnostic — a healthy drain spin is
                // microseconds; 250ms without progress means the records
                // are not coming (the fleet's 11r40 first draw: P1
                // frozen at its target, the whole rendezvous dead behind
                // it). One line per pass — the state pins WHERE the
                // records stopped.
                if !logged && drain_start.elapsed() >= std::time::Duration::from_millis(250) {
                    logged = true;
                    let lane_dbg = fabric.lane(1);
                    eprintln!(
                        "R40_P1_STALL pass={} target={} cursor={} res_tail={} res_head={} j1_len={} j1_consumed={} count_1={} sweeping={} (R40c: P1's pass-end drain stuck — the lane-1 records beyond the cursor are not arriving)",
                        st.pass,
                        lane1_target,
                        st.fold_pos,
                        st.res_tail,
                        lane_dbg.r40_res_head(),
                        st.j1_len,
                        gate.j1.consumed(),
                        st.count_1,
                        sweeping
                    );
                }
                crate::affinity::polite_spin(&mut sb2);
            }
        }
        // ── the arrival ──
        gate.publish_arrival(st.pass, st.count_1);
        pass_base = st.pass_end;
        st.pass += 1;
    }
    // Every exit path posts the terminal state (the finish's wait and
    // the poison's count read depend on it).
    gate.p1_done.store(true, Ordering::Release);
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
    #[test]
    fn t_r40_lane_end_law() {
        assert_eq!(lane_end_at(0, 0), 0);
        assert_eq!(lane_end_at(0, 1), 0);
        // Mid-chunk on lane 1 (chunk 15 = [960, 1024)): lane 1 projects
        // to s, lane 0 to the chunk start.
        assert_eq!(lane_end_at(1000, 1), 1000);
        assert_eq!(lane_end_at(1000, 0), 960);
        // A chunk-aligned s on lane-0 territory (chunk 14 ends at 960):
        assert_eq!(lane_end_at(960, 0), 960);
        assert_eq!(lane_end_at(960, 1), 896);
        // A chunk-aligned s ending a lane-1 chunk (chunk 15 ends 1024):
        assert_eq!(lane_end_at(1024, 1), 1024);
        assert_eq!(lane_end_at(1024, 0), 960);
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
    #[test]
    fn t_r40_arrival_ring() {
        let gate = DualGate::new();
        assert!(gate.arrival(5).is_none());
        for p in 0..8u64 {
            gate.publish_arrival(p, p * 100);
        }
        assert_eq!(gate.arrival(7), Some(700));
        assert_eq!(gate.arrival(8), None);
        // A later arrival makes earlier ones addressable (the seq gate
        // is the monotone cursor; the slot reuse is lap-bounded).
        gate.publish_arrival(8, 800);
        assert_eq!(gate.arrival(5), Some(500));
        assert_eq!(gate.arrival(8), Some(800));
        assert_eq!(gate.arrival(9), None);
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
