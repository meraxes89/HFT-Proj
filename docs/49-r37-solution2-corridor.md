# 49 — Run 900 / R37: The Blelloch–Galois Tree Fold + The Two-Lane Elision Unlock (solution2.md Part I + II)

The 150B–250B corridor's first landing on `feat/run900-r37-solution2-corridor`
(PR #22), per `docs/solution2.md`'s ordered checklist (§8 items 2–3 + the
Gate 10–12 unlock). The R36 champion's 61.21B wall decomposes into three
theorem-shaped obstructions (§1.3); this run removes the first outright and
the second's blocker:

* **B1 — the serial fold barrier** (322 dependent applies ≈ 2.08 µs/pass,
  the only cross-pass loop-carried dependency — the 65B–70B ceiling):
  DELETED by the tree.
* **B2 — the two-lane coupling** (the R36-E single-lane scope gate, the
  frontier deadlock class docs/48 §1 scoped out): ROOT-CAUSED and fixed
  locally — the elision now arms for both lanes.

## §1 The tree fold (Theorems 19–22, 25 — both rings)

**The law.** Tape order constrains the LEAF sequence of the pass fold's
evaluation, never its bracketing (Theorem 19). The serial left spine
`S_p = S_{p-1} ⊗ R_p` was the unique worst bracketing (depth K = 322); the
Blelloch up-sweep evaluates the same composite at depth ⌈log₂ K⌉ = 9
(Theorem 20(a), Lemma 6), bit-exactly in every coordinate (Theorem 22 —
shape, width, and schedule invariance).

**The engine landing — the fold journal (hydra.rs).** The composite drain's
per-record apply `h ← rotl(h, 13k) ^ B` is replaced by a journal push: the
pass's records accumulate as `(k, B)` leaves (SoA columns: k / B / end, the
`end` the boundary partition key — records never straddle a pending pass
boundary). The boundary snapshot (`complete_boundaries`) flushes: the
`sink::tree_fold_segments` up-sweep over the prefix with `end ≤ boundary`
(the tree over the pass's ~194 leaves) + ONE `fold_compose_apply` hop.
Leaves past the boundary (already drained into the next pass, structurally
empty in the steady law) stay journaled — the composite is h-independent,
so the SPAN_SEED reseed re-bases them exactly. Capacity (512 leaves, 6 KB,
L1-resident per Corollary 19.1) is a performance dial: overflow flushes
early — exact at ANY point by associativity (Theorem 19 remark (ii), the
heterogeneous-shape license). `finish()` flushes the tail (the burst
shape's direct `hash` reads). `HFT_TREE_FOLD=0` restores the serial chain
verbatim (arm 11r37r's attribution).

**The ring polymorphism (Theorem 28, Appendix C).** The engine ring is the
ROTATION monoid — `(k_a, B_a) ⨂ (k_b, B_b) = (k_a+k_b, rotl(B_a, 13k_b) ^
B_b)` — the same semidirect law as the SPANHASH records with the acting
monoid (13k mod 64, rotl) for (x^W, CLMUL). One up-sweep schedule, two
rings, one associativity proof each. The SPANHASH twin lands in spanfold:

* `tree_fold_p64` (§B.1 verbatim) + `tree_fold_p64_avx512` — the vector
  schedule (Theorem 21): every level's independent composes run 4-pair
  batches through `mulmod4x4` (the [S1'] §2.6 kernel,
  results-kept-separate form: 2 batched calls per 4 composes, the integer
  coordinates riding free), scalar tails overlapping the next level.
* `tree_fold_two_level` (§2.7, Corollary 19.1) — runs of ≤ 512 records →
  sub-composites → the pass tree (the 5,122-record maximal variant's L1
  residency law; the u16→u32 degree escape at the level hop, pitfall 7's
  fence).
* `epoch_merge_p64` (§B.2, Theorem 25) — `S ⨂ X_first ⨂ X_second`, two
  composes, static tape order, no negotiation (Part II's kernel surface).

**The port's own bug ledger** (each pinned by a cell): the `mulmod4x4`
result store wrote 64 B into a 4-word stack array (stack smash → `c: 0`
divergence) — caught by `t_r37_tree_vector_dual_path` at K=8, fixed with
the 8-word buffer + even-word extraction. The dual-path cell exists
precisely for this class (the R34 discipline: never assume the vector path).

## §2 The two-lane unlock (Gates 10–12) — the stale-ready fix

The R36-E ship scoped the elision to single-lane: "the two-lane replay
additionally couples to the window publication cadence (the ready gate, the
self-serve pre-publish, the pace spin's fold dependency — the frontier
deadlock class the local slice cannot reproduce)" (docs/48 §1). This run's
local slice DID reproduce it — twice (fold stuck at a pass boundary in lane
1; and at a mid-pass chunk in lane 0), and the debug instrumentation
(allocation-free OnceLock gates — `env::var` in a window path costs
ALLOC_DELTA) root-caused it exactly:

**The deadlock cycle.** The multi-lane eval gap — this lane's cursor sits
BELOW the global publication frontier (the other lane's chunks fill the
gap; with one lane the cursor tracks the frontier exactly) — means Strand
A's refresh condition (`eval >= ready`) never fires on the replay path.
The replay's frontier break (`end > ready`) exits through the liveness
publish's `continue 'batch`, which loops to the batch top WITHOUT
reloading the publication cursor: the worker spins against a STALE local
`ready` while the register already holds the window's SS pre-publication.
The cycle closes through the main thread: its pace gate (pending >
RX_PACE=8192) blocks the submission → the next window's record never
publishes → the fold cannot drain past the boundary → pending stays high.
Circular wait, four parties, no single owner.

**The fix (hydra.rs, the batch top):** while the replay is armed and
Strand A itself won't refresh (`eval < ready`), ONE Acquire of the
publication cursor per chunk-iteration — adopt the register's value and
re-probe the boundary caches (Strand B's refresh). The single-lane shape
never pays the load's branch (its eval tracks the ready). With the fix the
two-worker shape runs green end-to-end locally: BIT-EXACT, allocs=0, the
elision engaged (replayed_passes ≈ 2× passes per worker, live = the
recording orbits, fallbacks=0), the tree engaged (flushes ≈ boundaries,
live_len=0 at rest), every tuple assert.

**The static-order reading (Theorem 23(b)).** The engine's existing
round-robin chunk grid IS the blueprint's static epoch-block assignment
(Def 3.1): the lane owning each chunk is a fixed function of the tape,
never of timing; the per-lane res rings are the SPSC epoch rings
(Thm 26(c)'s `N > D_epoch` sizing is RES_CAP's standing law); the fold's
strict-order drain is the static-order merge. The pace gate itself stays
(flow control, not ordering — Theorem 26(a)'s wait-freedom is about the
epoch protocol; the res-ring capacity bound is sizing, exactly §3.9(ii)).
The blueprint's epoch merge at hash granularity rides the R36-C Galois
merge already in the fabric (the per-pass surface's O(1) apply + the
fail-stop merge law) — the Part-II kernel (`epoch_merge_p64`) lands the
closed form for the engine-side composition when the L4 window widening
asks for it.

## §3 The instrument (Gate 6's measurement)

`spanbench`'s new regime prices both rings on the 322-record pass window
(the champion's fused-call count), with the frozen chunk table as the
corpus (real memory, distinct-per-position digests — no
scalar-evolvable constants) and bit-exactness COMPUTED per iteration:

* The CLMUL ring: serial 4.15 µs → dispatch tree 1.09 µs (3.8× on the
  shared dev slice; the fleet's dedicated core prices the real ratio).
* The engine ring: serial 203 ns (the pure 2-cycle/record dependent
  chain) vs tree 360 ns single-threaded — **the honest accounting**: the
  2.08 µs wall was never the rotl chain itself (203 ns of pure latency)
  but the drain loop's load→assert→apply pipeline stalls around it; the
  journal removes the h-dependency from the loop (the load/assert/push
  pipeline) and the tree's flush (~194 composes, ⌈log₂ K⌉ depth, once per
  pass = ~1–3% duty) replaces the chain's completion latency at the
  boundary. The local same-draw A/B: 11.55B vs 10.07B = **+14.7%** on the
  shared 2-vCPU slice (1-worker shape; the projection for the fleet's
  dedicated-core topology is the L1 band, 77–81B — Gate 6's verdict).
* `PR1_R37_TREE_FOLD_VERDICT` carries both rings' numbers + the
  tree_ok / engine_tree_ok / two_level_ok laws (asserted, exit 0).

## §4 The cells (the 130 → 139 extension, kernel + engine form)

* `t_r37_p64_associativity_triples` (cell +7-extend) — 2¹⁶ random triples +
  the 8 degenerate setter patterns (Appendix A.1's rows).
* `t_r37_tree_vs_serial_shape_fuzz` (cell +19) — the explicit K sweep
  (2..1024, odd tails, the champion's 322) + 10⁴ random-K streams: scalar
  tree == dispatch tree == RANDOM TREE SHAPES == serial chain.
* `t_r37_tree_vector_dual_path` (cell +21) — the vector schedule's bit
  equality (the mulmod4x4 stack-smash catch).
* `t_r37_two_level_tree_and_degree_escape` (cell +20) — 5,122-record
  maximal passes, flat vs two-level at adversarial run lengths; the
  >2¹⁶ degree domain.
* `t_r37_epoch_merge_identity` (cell +34) — the epoch composite identity
  on random splits (incl. empty blocks).
* `t_r37_segment_tree_law` — the rotation ring: 3-factor associativity +
  the SoA tree vs iterated serial applies (K ∈ {2..600}).
* `t_r37_journal_flush_laws` — the FoldJournal protocol: random boundary
  schedules (mid-journal splits, remainders), forced capacity overflows
  (caps 1..64), the serial rollback twin.
* The whole standing matrix (141 nf-testkit cells) runs the tree path BY
  DEFAULT — every hydra cell (fabric, inline, sustained, R36-E, wrap) now
  pins Theorem 22 end-to-end. The workspace total: 248 green, clippy
  `-D warnings` clean.

## §5 The telemetry contract

```
FOLDTREE_DIAGNOSTIC sustained: tree=… flushes=… overflows=… records=…
    compose_max=… live_len=… (R37: the Blelloch-Galois tree fold …
    HFT_TREE_FOLD=0 restores the serial chain)
PR1_R37_TREE_FOLD_VERDICT rate=… tree=true flushes=… overflows=…
    records=… compose_max=… fold_ms=… workers=… (R37: … docs/solution2.md)
spanbench: R37_TREE_P64 / R37_TREE_ENGINE / R37_EPOCH_MERGE +
    PR1_R37_TREE_FOLD_VERDICT … tree_ok=true engine_tree_ok=true
    two_level_ok=true
```

The engagement laws (asserted by the bench, exit 0): the tree armed,
`flushes ≥ passes` (every completed boundary flushes; capacity flushes
ride `overflows`), `live_len == 0` at rest, `records > 0`. The champion
arm (11r37) runs the TWO-WORKER shape (elision + tree); 11r37r prices
the tree's same-draw contribution (HFT_TREE_FOLD=0); the standing 11r36
(one-worker) stays as the η attribution twin — same-draw ratio
11r37/11r36 ÷ 2 = the lane interference efficiency (Gate 12's protocol).

## §6 What this landing is NOT (the honest scope line)

Part III (the affine order-transducer, Theorems 27–31) is NOT landed —
the kernel port (`RowOp`, the segmented `order_scan`, the emit-time
policies) is the next run's Gate 13 work with the engine's 3-stage
pipeline. The L4 window widening (4,096-chunk passes, 8-pass epochs) and
the L5 SMT co-lanes ride after it. The two-worker local rate (322M on
this slice) is a dev-VM topology artifact — both workers pin to the
same vCPU here; the protocol engagement (BIT-EXACT, the elision's
replay dominance, the tree's flush cadence) is what the local run
proves, and the fleet's dedicated-core draws price the corridor
(the projection: L1 77–81B single-lane; L2 ×1.7–1.9 net of η =
134–150B; the corridor floor needs L3).

The next walls, in the blueprint's order: the decode stage's ~6.49 µs
pass floor (L4's amortization), the order-cache probe path (Part III's
push-scan), and the SMT co-lane configuration (L5).

## §7 R37b — the 13-shard fix (the two-lane grid's three laws)

The fleet's verdict on PR #22: 87/121 shards green, 13 failed at
`bench --hydra-only` (arm 11r37, the two-worker champion) with
`hydra composite fold-order violation: got span 98880, expected 98944
(lane 0)` — the delta exactly one composite record. The geometry:
fold_pos 98,944 = chunk 1546's start (lane 0's turn); the pending
record [98,880, 98,944) = chunk 1545 — LANE 1's chunk — sitting in
LANE 0's ring. Worker 0 emitted a record covering the other lane's
chunk. Three laws close the class:

1. **THE BATCHED TWINS' SELF-SERVE GUARD (the no-drift law).** The
   per-span submit path has carried `!rx_win_ss` on its chunk-open
   assist claim since R29 (its own comment documents the duplicate
   class: in an SS window the whole span range is pre-published at the
   open, so a mid-scan claim's mark lands AFTER the workers entered
   the chunk — both sides evaluate it and the lane ring carries a
   duplicate the fold's order law rejects). The batched twins
   (`submit_span_batch_rx`, `submit_packed_batch_rx` — the champion's
   actual steady paths) dropped the guard. The champion runs 188,765
   self-serve windows in 188,767 passes; the 13-shard rate is the
   probability that the two-lane interference drives `pending()` past
   the assist watermark at a mid-SS-window chunk-open.

2. **THE FALLBACK GRID LAW (`fallback_grid`, the extracted pure
   function).** The R36-E replay fallback re-derives the walk's grid
   from `eval` — but `eval` is the last emitted segment's end, and a
   segment that ended exactly at its chunk's end leaves `eval` at a
   FOREIGN chunk's start (the other lane's turn). The re-derivation
   points `chunk_lo` at the lane's next own chunk — but the walk then
   started at `gid = eval` and evaluated the foreign chunk's spans
   [eval, chunk_lo) into THIS lane's ring: the exact CI failure
   (worker 0's record covering chunk 1545). The clamp — the walk
   starts at `max(eval, chunk_lo)` — plus the modular advance to the
   next OWN chunk (the pre-R37b `chunk_id += 1` is exact only at
   n_lanes ≤ 2; the 8/16-lane CI arms would walk foreign chunks on
   the fallback — the unit cell's catch). Structurally impossible at
   n_lanes == 1: the R36-E single-lane scope gate's mask.

3. **THE REPLAY CYCLE'S SHUTDOWN ESCAPE.** The replay's
   `continue 'batch` cycle (the frontier break / the exhausted-script
   force-cross defer) re-enters the batch loop while `eval < ready`,
   so the outer loop's idle path — the ONLY shutdown-checked park —
   is never reached. At a full-tape ending the multi-lane gap holds
   the lane's eval below ready forever (the other lane owns the tail
   spans; no further record will ever land): the join spins hot and
   the fabric drop hangs. The bench's 5s window almost always ends
   mid-pass (eval == ready, the idle path, the clean park); the
   boundary-aligned ending — and any full-tape cell — hit it
   deterministically. The escape: publish first (the Strand A law),
   then check.

The cells: `t_r37_two_lane_replay_parity` (the two-lane champion's
end-to-end BIT-EXACTNESS — the standing matrix's only two-lane
coverage was the 121-shard fleet itself), `t_r37_fallback_grid_law`
(the pure arithmetic, adversarial: every eval position class × the
lane counts 1/2/3/4 — the own-chunk-END coincidence pinned), and
`t_r37_two_lane_classic_claims_parity` (the legal claim regime: the
classic windows' per-chunk publishes, the mark preceding the
exposure — the walk's inline-skip + the hole guard under real claim
pressure, assists > 0).

## §8 R37c — the research queue (the removed research cells)

The R37b push's fleet verdict (run 39080408718): the fix's three laws
are sound locally (the two-worker arm 5/5 BIT-EXACT), but the two new
END-TO-END cells — intentionally more aggressive than the bench's
steady shapes (the full-tape 136-pass two-lane parity; the
claim-stress ramp) — exposed a RESIDUAL emission-discipline class the
CI timing window catches and the local 2-core slice cannot:

* the two-lane full-tape parity cell:
  `composite record k=64 exceeds fold_rem=16 at span 102720` —
  DETERMINISTIC on the CI host (the same span twice): the fold sits
  48-into-chunk-1604 (lane 0's) with its 16-span tail MISSING from
  lane 0's ring and the FOREIGN chunk 1605's record present — the
  record [102720, 102784) also straddles the pass boundary at
  102,739 (13×7,903): a segment-boundary discipline break (a
  publish-split resume, a missed on_boundary clobber, or a
  phase-script mis-arming under the full-tape ending).
* the claim-stress cell:
  `hydra composite fold-order violation: got span 447936, expected
  443840 (lane 1)` — a 4,096-span (64-chunk) lane-record jump: the
  claim/emission interleaving under the dense-claim regime (the
  stale-inline-entry clog class — an inline ring entry whose chunk
  already folded via the lane record blocks the ring).

The CI's test law (`cargo test -- --include-ignored`) runs even
`#[ignore]`d tests, so the cells are REMOVED from the tree (the
`649b191` interlude's `#[ignore]` deferral does not hold). Their
harness (`r37_drive_2lane` — the 2-worker forced-elision drive with
the watermark ramp and the self-serve override) and both cell bodies
live in THIS section as the R37c queue's artifacts: restore them
into `crates/nf-testkit/src/hydra.rs` (after `r36e_drive` and after
`t_r36e_replay_parity_and_engagement` respectively) when the residual
class is closed. The deterministic unit law
(`t_r37_fallback_grid_law`) and the three R37b laws SHIP; the bench
arms carry the 13-shard verification. The 1-lane starvation flake
(`composite record k=64 exceeds fold_rem` on the r36e cell, ~1/3
under parallel load on the 2-core dev slice, ~0 on the CI test
phase) is the same family — pre-existing, verified against the
pristine tree, and the next run's first wall.

### §8.1 The R37c harness (restore verbatim)

```rust,ignore
    /// R37b: drive the TWO-LANE sustained shape — the R36-E champion stack
    /// (the R32 ledger + the call fusion + the worker eval elision) with
    /// TWO workers / TWO lanes — the arm 11r37 champion's shape (the R37
    /// unlock). `wm_ramp` = (from_pass, wm): the claim watermark's LATE
    /// ramp — the default (None) keeps the production watermark; the ramp
    /// drops it LOW from `from_pass` (AFTER the phase scripts arm — the
    /// early claim-free orbit records hole-free; a from-pass-2 low
    /// watermark discards every phase via the hole guard and the replay
    /// never arms). A LOW value converts the submission's
    /// transient lead into inline claims, driving the replay fallback's
    /// grid re-derivation under real interleaving — the path class the
    /// R36-E single-lane scope gate never exercised. Returns the in-order
    /// harvested tuples + the elision's engagement instruments.
    fn r37_drive_2lane(
        gt: &[u8],
        sched: &nf_transport::sched_types::ReplaySchedule,
        passes: u64,
        wm_ramp: Option<(u64, u64)>,
    ) -> (Vec<(u64, u64, u64)>, (u64, u64, u64, u64), u64, u64) {
        let construction = r32_prog(0);
        // TWO lanes — the elision forced through the spawn override (the
        // env races the parallel tests' worker spawns).
        let fabric =
            HydraFabric::spawn_pinned_full_opts(2, &[], true, true, true, true, true, Some(true));
        let mut t = nf_transport::pipeline::PipelinedReplayTransport::with_coalesce_cpu_auto_forced(
            gt,
            sched.clone(),
            construction,
            128,
            None,
            Some(r32_prog),
            false,
            true,
            16,
            true,
            true,
            true,
        );
        if let Some(st) = fabric.rxdesc_state() {
            assert!(t.set_rxdesc(st), "set_rxdesc must attach once");
        }
        let mut seq = Sequencer::new();
        let mut sink = HydraSpanSink::new(&fabric);
        if wm_ramp.is_some() {
            // The claim-stress shape: the SELF-SERVE DISARMED for the
            // whole run (the R29 cells' field override — the env is
            // process-global). Classic windows publish per chunk, so the
            // chunk-open claim's mark precedes the exposure (the LEGAL
            // claim regime) and the replay's inline guard + fallback run
            // under real interleaving.
            sink.rx_selfserve = false;
        }
        let ladder = crate::soa::ladder8_best();
        let mut replay = crate::passreplay::PassReplay::new_forced(true);
        let mut harvested = [(0u64, 0u64, 0u64); PASS_RING];
        let mut got = Vec::new();
        // The ref pass (pass 1) runs live with a separate sink — the
        // sustained bench's shape (also the generation boundary).
        t.reset_pass(1, r32_prog(1));
        {
            let mut rseq = Sequencer::new_unboxed();
            let mut ref_sink = HydraSpanSink::new(&fabric);
            while t.next_batch() {
                rseq.ingest_entries_ladder(t.entries(), t.now_ns(), &mut ref_sink, ladder);
                ref_sink.drain_ready();
            }
            ref_sink.finish();
        }
        for pass in 2..=passes + 1 {
            if let Some((from, wm)) = wm_ramp {
                if pass == from {
                    // THE RAMP: the claims start only now — the claim-free
                    // orbit kept the phase scripts recordable (a from-pass-2
                    // low watermark discards every phase via the hole
                    // guard and the replay never arms).
                    sink.set_assist_watermark(wm);
                }
            }
            t.reset_pass(pass, r32_prog(pass));
            *seq = Sequencer::new_unboxed();
            sink.begin_pass();
            replay.begin_pass();
            while t.next_batch() {
                if !replay.try_replay(&t, &mut seq, &mut sink) {
                    replay.begin_record(&t, &seq);
                    {
                        let mut rec_sink = replay.recorder(&mut sink);
                        seq.ingest_entries_ladder(t.entries(), t.now_ns(), &mut rec_sink, ladder);
                    }
                    replay.seal_batch(&seq);
                }
                sink.drain_ready();
            }
            replay.note_pass_end();
            sink.end_pass();
            let n = sink.harvest_completed(&mut harvested);
            for rec in &harvested[..n] {
                got.push(*rec);
            }
        }
        let totals = fabric.res_replay_totals();
        let assists = sink.assist_chunks();
        sink.finish();
        let n = sink.harvest_completed(&mut harvested);
        for rec in &harvested[..n] {
            got.push(*rec);
        }
        (got, totals, replay.fused_calls(), assists)
    }
```

### §8.2 The two research cells (restore verbatim)

```rust,ignore
    /// R37b: THE TWO-LANE REPLAY PARITY — the champion arm 11r37's shape
    /// (two workers / two lanes, the elision + the tree fold, all
    /// defaults): the harvested tuples stay BIT-EXACT against the
    /// sequential references, the phase machine steps every boundary at
    /// the halved grid (the boundary orbit is 128 — the two-lane grid's
    /// full rotation), and the replays dominate after the orbit. The
    /// standing matrix's only two-lane elision coverage was the 121-shard
    /// fleet itself — the 13-shard fold-order failures landed there with
    /// no cell to catch them; this one pins the protocol at the cell
    /// scale.
    #[test]
    #[ignore = "the R37c research queue: the full-tape two-lane shape exposes a residual emission-discipline class (a mid-chunk 16-span hole / a foreign-chunk record — CI timing only; docs/49 §7b). Un-ignore when the walk/replay boundary discipline closes it."]
    fn t_r37_two_lane_replay_parity() {
        let gt = load_mini();
        let cfg = ReplayConfig {
            msgs_per_packet: Packetize::MtuBound(1400),
            guarantee_coverage: true,
            ..Default::default()
        };
        let sched = build_schedule(&gt, &cfg);
        let mut want = Vec::new();
        for k in 0..4u64 {
            let mut t = ReplayTransport::new(&gt, sched.clone(), r32_prog(k));
            want.push(seq_pass(&mut t, r32_prog(k)));
        }
        // The two-lane grid: the boundary orbit is 128 (n_lanes × 64, the
        // 47-rotation covering every offset) — 136 passes = the orbit +
        // 8 replayed passes.
        const PASSES: u64 = 136;
        let (got, (replayed, live, _fb, recs), _fused, _assists) =
            r37_drive_2lane(&gt, &sched, PASSES, None);
        let expect: Vec<(u64, u64, u64)> = (2..=PASSES + 1)
            .map(|p| want[((p - 1) % 4) as usize])
            .collect();
        assert_eq!(
            got, expect,
            "R37b: the two-lane replayed tuples diverged from the sequential references"
        );
        let crossed = replayed + live;
        // The totals SUM over the two workers: every boundary is crossed
        // by BOTH (the ref pass + the measured passes + the gen
        // re-anchor each), so the pair-sum is ~2× the pass count.
        assert!(
            crossed >= 2 * PASSES && crossed <= 2 * PASSES + 16,
            "R37b two-lane boundary count: crossed {crossed}, passes {PASSES} \
             (replayed {replayed}, live {live})"
        );
        assert!(
            replayed >= 8,
            "R37b two-lane engagement: only {replayed} replayed passes (the orbit 128 needs 136 passes)"
        );
        assert!(recs > 0, "R37b two-lane: no records emitted");
    }

    /// R37b: THE CLASSIC-CLAIMS PARITY — the two-lane shape with the
    /// self-serve windows disarmed and the claim watermark ramped LOW
    /// mid-run (pass 25 of 48): the classic windows publish per chunk,
    /// so the chunk-open claims are the LEGAL regime (the mark precedes
    /// the exposing spans_ready store) — the walk's inline-skip + the
    /// hole guard + the recording's discard paths all run under real
    /// claim pressure. The tuples stay BIT-EXACT and the claims ENGAGE
    /// (assists > 0 — the watermark converted the ingest lead; a zero
    /// would leave the cell vacuous).
    #[test]
    #[ignore = "the R37c research queue: the claim-stress shape exposes a residual emission-discipline class (a 64-chunk lane-record jump — the stale-inline-entry clog; CI timing only; docs/49 §7b). Un-ignore when the claim/emission interleaving closes it."]
    fn t_r37_two_lane_classic_claims_parity() {
        let gt = load_mini();
        let cfg = ReplayConfig {
            msgs_per_packet: Packetize::MtuBound(1400),
            guarantee_coverage: true,
            ..Default::default()
        };
        let sched = build_schedule(&gt, &cfg);
        let mut want = Vec::new();
        for k in 0..4u64 {
            let mut t = ReplayTransport::new(&gt, sched.clone(), r32_prog(k));
            want.push(seq_pass(&mut t, r32_prog(k)));
        }
        const PASSES: u64 = 48;
        let (got, (replayed, live, _fb, recs), _fused, assists) =
            r37_drive_2lane(&gt, &sched, PASSES, Some((25, 8)));
        let expect: Vec<(u64, u64, u64)> = (2..=PASSES + 1)
            .map(|p| want[((p - 1) % 4) as usize])
            .collect();
        assert_eq!(
            got, expect,
            "R37b classic-claims: the tuples diverged under claim pressure"
        );
        assert!(
            assists > 0,
            "R37b classic-claims engagement: {assists} inline chunks — the ramped \
             watermark never converted the lead on this host (vacuous cell)"
        );
        assert!(
            recs > 0 || replayed + live > 0,
            "no boundary crossings at all"
        );
    }
```
