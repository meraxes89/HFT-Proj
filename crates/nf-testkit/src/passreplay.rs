//! R32: the steady-pass VERDICT REPLAY — the main-core scan elision.
//!
//! THE WALL (the Run-600 fleet verdict, docs/42): the session elision
//! deleted the reset bake (reset_ms 1,561 → 340, zero master patches),
//! and the profile inverted — `scan_ms ≈ 4.65s` of the 5s window is now
//! the main core's `ingest_entries_ladder` walk itself (work_ms ≈ 3.9s):
//! the AoS gathers, the `SoaLadder8` kernels, the per-entry elig ladder,
//! the counter folds and the packed emission math, re-run for EVERY pass
//! over a frame array that — under the elision regime — is provably
//! byte-identical every pass (the master is construction-frozen; the RX
//! publishes the same deterministic `(rx_start, len)` slice sequence; the
//! per-pass sequencer starts fresh and adopts the same construction
//! session; the schedule has no session split by the regime gate).
//!
//! THE LEVER: a pure function re-run on identical inputs has a
//! memoizable verdict. The ledger records ONE live pass's per-batch
//! observable surface — every sink call in order (`on_msg`, `on_event`,
//! `on_span`, `on_span_batch`, `on_span_batch_packed`), the post-batch
//! counters, the watermark, the progress clock — and every later pass
//! in the same regime REPLAYS it: the recorded calls re-issue to the
//! live sink (which re-drives the submit bookkeeping, the self-serve
//! windows, the worker submissions — the workers still fold every
//! message of every pass; only the redundant RE-VALIDATION walk is
//! elided), and the sequencer lands in the recorded state via
//! [`nf_arbitrator::Sequencer::replay_apply`].
//!
//! SOUNDNESS (the guard chain — every replay is proved, never trusted):
//! 1. **Regime**: `PipelinedReplayTransport::pass_replay_regime()` —
//!    publish-by-reference master AND session elision (zero master/blob
//!    writes exist anywhere in the run's code paths).
//! 2. **Master write barrier**: the transport's `master_version`
//!    (Release-bumped at every master write site) must equal the version
//!    the template was recorded under — any bump truncates the template
//!    and forces a fresh live recording (never a stale replay).
//! 3. **Per-batch identity**: the published `(rx_start, len)` bounds AND
//!    the entry watermark must equal the recorded ones — a mid-replay
//!    divergence is a broken determinism contract and FAILS LOUD
//!    (panic; the same discipline as the session-elision contract cell:
//!    the program is a schedule, not a trust substitute).
//! 4. **Emission order**: the replay re-issues calls strictly in
//!    recorded order between live `begin_pass`/`end_pass` brackets, so
//!    the sink's own per-pass reset (`begin_pass`) and fold pump
//!    (`drain_ready`) — which the harness runs verbatim — observe the
//!    identical call sequence.
//!
//! The replayed pointers (`body`, `blocks`) captured from live emission
//! slices point exclusively into the construction-frozen blob and the
//! construction-built master (the recording wrapper never captures
//! scan-driver stack buffers: `on_span_batch`'s recs and the packed
//! words are copied by value), so re-slicing them under guards 1+2 is
//! sound. `HFT_PASS_REPLAY=0` is the rollback (the live scan verbatim —
//! the attribution arm's cell); capacity overflow poisons the ledger to
//! permanent live mode (fail-safe, never wrong).

use nf_arbitrator::types::SpanRec;
use nf_arbitrator::{Counters, Event, LiveFeedProof, Sequencer, Sink};
use nf_transport::pipeline::PipelinedReplayTransport;

/// The recorded call log's capacities — one-time construction
/// allocations (BEFORE the measured window; the allocs=0 gate covers
/// the window only). A pass that overflows any of them poisons the
/// ledger to permanent live mode (the sustained steady pass sits orders
/// of magnitude below; per-message sinks (`wants_spans() == false`)
/// overflow [`MAX_CALLS`] immediately and degrade to the live scan —
/// correct by construction, the ledger is the packed/span shape's lever).
const MAX_BATCHES: usize = 2048;
const MAX_CALLS: usize = 16_384;
const MAX_WORDS: usize = 262_144;
const MAX_RECS: usize = 16_384;
/// The `on_span_batch` replay's stack staging depth (the scan drivers'
/// emitter buffers flush at 32 — 8× headroom; a recorded batch beyond
/// it fails loud rather than ever replaying a truncated recs set).
const SPAN_STAGE: usize = 256;

/// One recorded sink call. Variants carry everything the replay needs to
/// re-issue the call bit-identically; slice arguments are captured as
/// `(ptr, len)` pairs into the frozen blob/master, arrays (`words`,
/// `SpanRec` batches) are copied by value into the ledger's own buffers.
#[derive(Clone)]
enum Call {
    Event(Event),
    Msg {
        gen: u64,
        seq: u64,
        ptr: usize,
        len: u32,
    },
    Span {
        gen: u64,
        first: u64,
        count: u16,
        ptr: usize,
        len: u32,
        bptr: usize,
        blen: u32,
    },
    SpanBatch {
        gen: u64,
        rec_lo: u32,
        n: u32,
    },
    Packed {
        gen: u64,
        first: u64,
        total: u64,
        base: usize,
        w_lo: u32,
        n: u32,
    },
}

/// One recorded `on_span_batch` rec (the contents of a `SpanRec` — the
/// struct itself rides the scan driver's stack buffer, so the ledger
/// copies the value fields).
#[derive(Clone, Copy)]
struct RecEntry {
    first: u64,
    count: u16,
    ptr: usize,
    len: u32,
    bptr: usize,
    blen: u32,
}

/// One batch's recorded verdict: the identity guards, the call range, and
/// the post-batch sequencer state.
struct BatchRec {
    rx_start: u32,
    len: u32,
    w_in: u64,
    call_lo: u32,
    call_hi: u32,
    counters_after: Counters,
    w_out: u64,
    /// The post-batch progress clock (the live drivers' repeated
    /// `progress_vt = now_ns` assignment's surviving value; replaying
    /// each batch's recorded value reproduces the pass's final clock
    /// exactly — the last batch's record IS the pass-end state).
    vt_out: u64,
}

/// R32: the pass-replay ledger. Construct ONCE per run (before the
/// measured window); `begin_pass` at every pass boundary; `try_replay`
/// per batch — returning true means the harness SKIPS the live scan for
/// this batch (the recorded surface was re-issued); returning false
/// means the harness MUST run the live scan bracketed by
/// `begin_record`/`recorder`/`seal_batch` so this batch's verdict lands
/// in the template.
pub struct PassReplay {
    /// The env arm (`HFT_PASS_REPLAY` — default ON; read once at
    /// construction, outside every measured window).
    enabled: bool,
    /// Capacity/protocol poison — permanent live mode once set.
    poison: bool,
    /// The template's master version (the write-barrier guard's operand).
    version: u64,
    batches: Vec<BatchRec>,
    calls: Vec<Call>,
    words: Vec<u64>,
    recs: Vec<RecEntry>,
    /// The current pass's batch cursor (replay index AND record cursor).
    pass_batch_idx: usize,
    /// This pass's live-scanned batch count (the pass attribution).
    pass_live: u64,
    /// The in-flight recording's batch record.
    recording: Option<BatchRec>,
    /// Telemetry: replayed batches, live-scanned batches, fully-replayed
    /// passes, regime-fallback re-records.
    replayed_batches: u64,
    live_batches: u64,
    replayed_passes: u64,
    re_records: u64,
    /// R36-E: the call-fusion arm (`HFT_CALL_FUSION` — default ON; read
    /// once at construction, outside every measured window).
    fusion: bool,
    /// R36-E: the count of packed calls absorbed into their predecessor
    /// (the main-side re-issue work elided).
    fused_calls: u64,
    /// R36-E: the pass bracket's fusion barrier — the first packed call
    /// of each pass NEVER fuses into the previous pass's last call (the
    /// emission-order law: calls re-issue strictly inside the live
    /// begin/end brackets; a cross-pass fusion would submit the next
    /// pass's spans before its begin_pass opens the window/record —
    /// the boundary record would never carry the pass-start flag and
    /// the workers' phase machine would stall).
    fusion_barrier: bool,
}

impl Default for PassReplay {
    fn default() -> Self {
        Self::new()
    }
}

impl PassReplay {
    /// Read the env arm and preallocate the buffers (construction-time —
    /// the one-time allocations sit outside every measured window).
    #[allow(clippy::disallowed_types)] // construction-time only (the capacity preallocation)
    pub fn new() -> Self {
        let enabled = std::env::var("HFT_PASS_REPLAY").as_deref() != Ok("0");
        Self::new_forced(enabled)
    }

    /// The tests'/local A/B's explicit arm (the env is process-global and
    /// racy across parallel test transports — the forced-constructor
    /// discipline).
    #[allow(clippy::disallowed_types)] // construction-time only (the capacity preallocation)
    pub fn new_forced(enabled: bool) -> Self {
        Self {
            enabled,
            poison: false,
            version: 0,
            batches: Vec::with_capacity(MAX_BATCHES),
            calls: Vec::with_capacity(MAX_CALLS),
            words: Vec::with_capacity(MAX_WORDS),
            recs: Vec::with_capacity(MAX_RECS),
            pass_batch_idx: 0,
            pass_live: 0,
            recording: None,
            replayed_batches: 0,
            live_batches: 0,
            replayed_passes: 0,
            re_records: 0,
            fusion: std::env::var("HFT_CALL_FUSION").as_deref() != Ok("0"),
            fused_calls: 0,
            fusion_barrier: true,
        }
    }

    /// R36-E: THE CALL FUSION — try to absorb a packed call into its
    /// recorded predecessor. TRUE means the ledger was extended in place
    /// (no new call pushed; the words are REBASED onto the predecessor's
    /// base and appended contiguously onto its tail). The guard chain:
    /// same generation, the contiguous span sequence
    /// (`first == prev.first + prev.total`), the predecessor's words at
    /// the ledger's tail, the successor's base ahead of the
    /// predecessor's within one u32, and the capacity law. The base
    /// changes per emitter flush (each flush's first body pointer), so
    /// the fusion rebases the successor's offsets — a ONE-TIME recording
    /// cost; the replay then re-issues ONE call whose words are exact
    /// against the fused base (both sink paths stay correct: the
    /// self-serve counting branch never reads the words, the classic
    /// check path reads the rebased offsets — identical to the unfused
    /// sequence's semantics).
    fn try_fuse_packed(
        &mut self,
        gen: u64,
        first: u64,
        total: u64,
        base: usize,
        words: &[u64],
    ) -> bool {
        if !self.fusion || self.words.len() + words.len() > MAX_WORDS {
            return false;
        }
        // The pass bracket's barrier: the pass's first call starts a
        // fresh script (never fuses into the previous pass's tail).
        if self.fusion_barrier {
            self.fusion_barrier = false;
            return false;
        }
        let Some(Call::Packed {
            gen: pgen,
            first: pfirst,
            total: ptotal,
            base: pbase,
            w_lo: pw_lo,
            n: pn,
        }) = self.calls.last()
        else {
            return false;
        };
        if *pgen != gen
            || *pfirst + *ptotal != first
            || (*pw_lo as usize + *pn as usize != self.words.len())
            || base < *pbase
            || (base - *pbase) > u32::MAX as usize
        {
            return false;
        }
        // FUSE: rebase the successor's words onto the predecessor's base
        // (the blob advance is a plain add on the low-32 offset field;
        // the len/count fields ride untouched — the carry law is the
        // batch's own u32 offset budget, asserted at the sink).
        let delta = (base - *pbase) as u64;
        self.words
            .extend(words.iter().map(|w| w.wrapping_add(delta)));
        let last = self.calls.last_mut().expect("checked above");
        if let Call::Packed {
            total: ptotal,
            n: pn,
            ..
        } = last
        {
            *ptotal += total;
            *pn += words.len() as u32;
        }
        self.fused_calls += 1;
        true
    }

    /// The env arm's state (the rollback telemetry).
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Telemetry: `(replayed_batches, live_batches, replayed_passes,
    /// re_records)` — the DIAG line's instruments.
    pub fn stats(&self) -> (u64, u64, u64, u64) {
        (
            self.replayed_batches,
            self.live_batches,
            self.replayed_passes,
            self.re_records,
        )
    }

    /// R36-E: the call-fusion telemetry — the count of packed calls
    /// absorbed into their predecessors (the main-side re-issue work
    /// elided). The verdict line's engagement instrument.
    pub fn fused_calls(&self) -> u64 {
        self.fused_calls
    }

    /// Whether the ledger is live and armed for this transport (the
    /// regime gate + env + poison). The harness consults this once per
    /// run; `try_replay` re-checks cheaply per batch.
    pub fn armed_for(&self, t: &PipelinedReplayTransport) -> bool {
        self.enabled && !self.poison && t.pass_replay_regime()
    }

    /// Pass boundary: reset the per-pass cursors. Call right after the
    /// harness's `sink.begin_pass()` (the recorded calls replay strictly
    /// inside the live begin/end brackets — emission-order law 4).
    pub fn begin_pass(&mut self) {
        self.pass_batch_idx = 0;
        self.pass_live = 0;
        self.recording = None;
        // R36-E: the bracket barrier — this pass's calls may not fuse
        // into the previous pass's tail (the emission-order law 4).
        self.fusion_barrier = true;
    }

    /// R36-E: the batch barrier's twin — a transport batch's first
    /// recorded call starts a fresh script too. The fusion is an
    /// INTRA-BATCH lever: the emitter's per-32-span flushes within one
    /// batch merge; the batch boundary restores the per-batch call
    /// script (the harness's per-batch work/guard rhythm and the
    /// window-open cadence ride the batch edges — keeping the fused
    /// surface strictly inside one batch's bracket keeps every
    /// downstream law structural).
    fn begin_batch_barrier(&mut self) {
        self.fusion_barrier = true;
    }

    /// Try to replay batch `pass_batch_idx` from the template. TRUE ⇒
    /// the harness skips the live scan for this batch entirely; FALSE ⇒
    /// the harness MUST run `begin_record` → scan with `recorder` →
    /// `seal_batch`. The guards are the soundness proof (see the module
    /// doc); a mid-template divergence fails LOUD.
    pub fn try_replay<S: Sink>(
        &mut self,
        t: &PipelinedReplayTransport,
        seq: &mut Sequencer,
        sink: &mut S,
    ) -> bool {
        if !self.enabled || self.poison || !t.pass_replay_regime() {
            return false;
        }
        let idx = self.pass_batch_idx;
        if idx >= self.batches.len() {
            // No record for this batch index: the first live pass (the
            // template fills), or this regime's pass runs longer than
            // the recorded one (an abandon-truncated template extends).
            return false;
        }
        // Guard 2 — the master write barrier: a bump is a NEW regime;
        // truncate the template and fall to the live scan (which
        // re-records from batch 0 under the new version). NEVER a stale
        // replay.
        if t.master_version() != self.version {
            self.batches.clear();
            self.calls.clear();
            self.words.clear();
            self.recs.clear();
            self.version = t.master_version();
            self.re_records += 1;
            return false;
        }
        let (rx_start, len) = (self.batches[idx].rx_start, self.batches[idx].len);
        let w_in = self.batches[idx].w_in;
        // Guard 3 — the per-batch identity proof. A mismatch here means
        // the deterministic-regime contract broke mid-replay: fail loud,
        // never silently wrong (the session-elision contract discipline).
        let bounds = t.batch_bounds();
        assert_eq!(
            (rx_start, len),
            bounds,
            "R32 replay guard: batch {idx} bounds diverged (recorded \
             ({rx_start}, {len}), live {bounds:?}) — the frozen-master \
             determinism contract is broken"
        );
        assert_eq!(
            seq.watermark(),
            w_in,
            "R32 replay guard: batch {idx} watermark diverged (recorded \
             {w_in}, live {}) — the verdict chain is broken",
            seq.watermark()
        );
        // REPLAY: re-issue the recorded calls in order, then land the
        // sequencer in the recorded post-batch state.
        let (call_lo, call_hi) = (self.batches[idx].call_lo, self.batches[idx].call_hi);
        for i in call_lo as usize..call_hi as usize {
            self.replay_call(&self.calls[i], sink);
        }
        let rec = &self.batches[idx];
        seq.replay_apply(rec.w_out, &rec.counters_after, rec.vt_out);
        self.pass_batch_idx += 1;
        self.replayed_batches += 1;
        true
    }

    /// Open the recording bracket for a batch the harness will scan
    /// live. Snapshots the guards' operands and the pre-batch state.
    pub fn begin_record(&mut self, t: &PipelinedReplayTransport, seq: &Sequencer) {
        if !self.enabled || self.poison {
            return;
        }
        // R36-E: the batch bracket — the batch's first call starts a
        // fresh script (the fusion is an intra-batch lever).
        self.begin_batch_barrier();
        // The version latches with the FIRST recorded batch of the
        // template (the re-record path clears the template first, so
        // this is the new regime's version).
        if self.batches.is_empty() {
            self.version = t.master_version();
            self.calls.clear();
            self.words.clear();
            self.recs.clear();
        }
        let (rx_start, len) = t.batch_bounds();
        let call_lo = self.calls.len() as u32;
        self.recording = Some(BatchRec {
            rx_start,
            len,
            w_in: seq.watermark(),
            call_lo,
            call_hi: call_lo,
            counters_after: Counters::default(),
            w_out: seq.watermark(),
            vt_out: 0,
        });
    }

    /// The recording sink wrapper — tees every sink call the live scan
    /// makes into the ledger while delivering it to the real sink
    /// unchanged. Drop before `seal_batch`.
    pub fn recorder<'a, S: Sink>(&'a mut self, inner: &'a mut S) -> Recorder<'a, S> {
        Recorder { ledger: self, inner }
    }

    /// Seal the recording bracket: snapshot the post-batch sequencer
    /// state into the batch record and advance the template.
    pub fn seal_batch(&mut self, seq: &Sequencer) {
        let Some(mut rec) = self.recording.take() else {
            return;
        };
        rec.call_hi = self.calls.len() as u32;
        rec.counters_after = seq.counters();
        rec.w_out = seq.watermark();
        rec.vt_out = seq.progress_vt();
        // Capacity gates: any overflow poisons to permanent live mode
        // (fail-safe — the partial template is discarded).
        if self.batches.len() >= MAX_BATCHES
            || self.calls.len() > MAX_CALLS
            || self.words.len() > MAX_WORDS
            || self.recs.len() > MAX_RECS
        {
            self.poison = true;
            self.batches.clear();
            self.calls.clear();
            self.words.clear();
            self.recs.clear();
            return;
        }
        if self.pass_batch_idx < self.batches.len() {
            self.batches[self.pass_batch_idx] = rec;
        } else {
            self.batches.push(rec);
        }
        self.pass_batch_idx += 1;
        self.pass_live += 1;
        self.live_batches += 1;
    }

    /// The pass-end accounting: a pass whose batches ALL replayed (zero
    /// live-scanned) is a fully-replayed pass. Call right before the
    /// harness's `sink.end_pass()`.
    pub fn note_pass_end(&mut self) {
        if self.pass_batch_idx > 0 && self.pass_live == 0 {
            self.replayed_passes += 1;
        }
    }

    /// Re-issue one recorded call to the live sink.
    fn replay_call<S: Sink>(&self, call: &Call, sink: &mut S) {
        match call {
            Call::Event(ev) => sink.on_event(ev),
            Call::Msg { gen, seq, ptr, len } => {
                let proof = LiveFeedProof::replay_from_gen(*gen);
                let msg = unsafe_body(*ptr, *len as usize);
                sink.on_msg(&proof, *seq, msg);
            }
            Call::Span {
                gen,
                first,
                count,
                ptr,
                len,
                bptr,
                blen,
            } => {
                let proof = LiveFeedProof::replay_from_gen(*gen);
                let body = unsafe_body(*ptr, *len as usize);
                let blocks = unsafe_blocks(*bptr, *blen as usize);
                sink.on_span(&proof, *first, *count, body, blocks);
            }
            Call::SpanBatch { gen, rec_lo, n } => {
                let proof = LiveFeedProof::replay_from_gen(*gen);
                let lo = *rec_lo as usize;
                let hi = lo + *n as usize;
                assert!(
                    *n as usize <= SPAN_STAGE,
                    "R32 replay: recorded span batch of {n} recs exceeds the \
                     staging depth ({SPAN_STAGE}) — fail loud, never truncate"
                );
                // The staged recs are a STACK array rebuilt from the
                // recorded contents (zero allocation — the allocs=0 law);
                // the sink observes the same values the live scan's
                // RecEmitter delivered.
                let mut staged = [SpanRec {
                    first_seq: 0,
                    count: 0,
                    body: &[],
                    blocks: &[],
                }; SPAN_STAGE];
                for (k, r) in self.recs[lo..hi].iter().enumerate() {
                    staged[k] = SpanRec {
                        first_seq: r.first,
                        count: r.count,
                        body: unsafe_body(r.ptr, r.len as usize),
                        blocks: unsafe_blocks(r.bptr, r.blen as usize),
                    };
                }
                sink.on_span_batch(&proof, &staged[..*n as usize]);
            }
            Call::Packed {
                gen,
                first,
                total,
                base,
                w_lo,
                n,
            } => {
                let proof = LiveFeedProof::replay_from_gen(*gen);
                let lo = *w_lo as usize;
                let hi = lo + *n as usize;
                sink.on_span_batch_packed(&proof, *first, *total, *base, &self.words[lo..hi]);
            }
        }
    }

    /// Record-side: the call-log appender (the Recorder's tee). Overflow
    /// poisons immediately (the seal discards the partial template).
    fn push_call(&mut self, call: Call) {
        if self.recording.is_none() || self.poison {
            return;
        }
        if self.calls.len() >= MAX_CALLS {
            self.poison = true;
            return;
        }
        self.calls.push(call);
    }
}

/// SAFETY (the re-slice): the recorded pointer/length pair was captured
/// from a live emission slice whose target is the construction-frozen
/// blob (packet bodies) or the construction-built master (block
/// triples) — the regime gate (elision ⇒ no writer exists) and the
/// master write barrier (version equality) prove the bytes are the
/// exact recorded ones. Empty slices avoid the raw-parts constructor.
#[inline(always)]
fn unsafe_body(ptr: usize, len: usize) -> &'static [u8] {
    if len == 0 {
        &[]
    } else {
        // SAFETY: see above — frozen-blob target, recorded length.
        unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }
    }
}

/// SAFETY: the block-triple re-slice — same law as [`unsafe_body`]
/// (the master's boxed triple arrays are construction-frozen and
/// aligned; the count is the recorded one).
#[inline(always)]
fn unsafe_blocks(ptr: usize, n: usize) -> &'static [(u64, u32, u32)] {
    if n == 0 {
        &[]
    } else {
        // SAFETY: see above — frozen-master target, recorded count.
        unsafe { std::slice::from_raw_parts(ptr as *const (u64, u32, u32), n) }
    }
}

/// The recording sink wrapper — the live scan's tee.
pub struct Recorder<'a, S: Sink> {
    ledger: &'a mut PassReplay,
    inner: &'a mut S,
}

impl<S: Sink> Sink for Recorder<'_, S> {
    #[inline(always)]
    fn on_msg(&mut self, proof: &LiveFeedProof, seq: u64, msg: &[u8]) {
        self.ledger.push_call(Call::Msg {
            gen: proof.gen(),
            seq,
            ptr: msg.as_ptr() as usize,
            len: msg.len() as u32,
        });
        self.inner.on_msg(proof, seq, msg);
    }

    #[inline(always)]
    fn on_event(&mut self, ev: &Event) {
        self.ledger.push_call(Call::Event(ev.clone()));
        self.inner.on_event(ev);
    }

    #[inline(always)]
    fn wants_spans(&self) -> bool {
        self.inner.wants_spans()
    }

    #[inline(always)]
    fn on_span(
        &mut self,
        proof: &LiveFeedProof,
        first_seq: u64,
        count: u16,
        body: &[u8],
        blocks: &[(u64, u32, u32)],
    ) {
        self.ledger.push_call(Call::Span {
            gen: proof.gen(),
            first: first_seq,
            count,
            ptr: body.as_ptr() as usize,
            len: body.len() as u32,
            bptr: blocks.as_ptr() as usize,
            blen: blocks.len() as u32,
        });
        self.inner.on_span(proof, first_seq, count, body, blocks);
    }

    #[inline(always)]
    fn on_span_batch(&mut self, proof: &LiveFeedProof, recs: &[SpanRec<'_>]) {
        if self.ledger.recording.is_some() && !self.ledger.poison {
            if self.ledger.recs.len() + recs.len() > MAX_RECS
                || self.ledger.calls.len() >= MAX_CALLS
            {
                self.ledger.poison = true;
            } else {
                let rec_lo = self.ledger.recs.len() as u32;
                for r in recs {
                    self.ledger.recs.push(RecEntry {
                        first: r.first_seq,
                        count: r.count,
                        ptr: r.body.as_ptr() as usize,
                        len: r.body.len() as u32,
                        bptr: r.blocks.as_ptr() as usize,
                        blen: r.blocks.len() as u32,
                    });
                }
                self.ledger.push_call(Call::SpanBatch {
                    gen: proof.gen(),
                    rec_lo,
                    n: recs.len() as u32,
                });
            }
        }
        self.inner.on_span_batch(proof, recs);
    }

    #[inline(always)]
    fn wants_packed_spans(&self) -> bool {
        self.inner.wants_packed_spans()
    }

    #[inline(always)]
    fn on_span_batch_packed(
        &mut self,
        proof: &LiveFeedProof,
        first_seq: u64,
        total: u64,
        base_ptr: usize,
        words: &[u64],
    ) {
        if self.ledger.recording.is_some() && !self.ledger.poison {
            if self.ledger.words.len() + words.len() > MAX_WORDS
                || self.ledger.calls.len() >= MAX_CALLS
            {
                self.ledger.poison = true;
            } else {
                // R36-E: THE CALL FUSION — consecutive packed calls of the
                // same transport batch (the same gen/blob base, contiguous
                // span sequence, the previous words at the ledger's tail)
                // merge into ONE call. The ladder's emitter flushes at 32
                // spans, so ONE pass's surface is ~344 calls × ~31 ns of
                // replay dispatch — fusing them collapses the main side's
                // re-issue to ~22 fused calls per pass (the sink's
                // self-serve counting branch is granularity-invariant:
                // the counters, the chunk-grid cadence, and the window
                // opens are identical for any split of the same span
                // run). HFT_CALL_FUSION=0 restores the verbatim
                // per-flush calls (the attribution arm).
                let fused = self.ledger.try_fuse_packed(
                    proof.gen(),
                    first_seq,
                    total,
                    base_ptr,
                    words,
                );
                if !fused {
                    let w_lo = self.ledger.words.len() as u32;
                    self.ledger.words.extend_from_slice(words);
                    self.ledger.push_call(Call::Packed {
                        gen: proof.gen(),
                        first: first_seq,
                        total,
                        base: base_ptr,
                        w_lo,
                        n: words.len() as u32,
                    });
                }
            }
        }
        self.inner
            .on_span_batch_packed(proof, first_seq, total, base_ptr, words);
    }
}
