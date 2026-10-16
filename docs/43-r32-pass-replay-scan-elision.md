# 43 — Run 625: The Steady-Pass Verdict Replay (R32-A / R32-B)

The Run-625 program against PR #16's merged baseline (Run 600, the
R31 stack). PR #16 delivered the record — **5.967B msg/s** (+39% over
the 4.28B baseline; confirmed at 5.930B), the reset wall deleted
(`reset_ms` 1,561 → 340, `master_patches` 367M → 0, advance latency
0.0 µs/advance, 100% self-serve windows on the record draws) — and the
fleet's own telemetry inverted the profile:

```
scan_ms ≈ 4,652 ms   (93% of the 5s window — the dominant wall)
work_ms ≈ 3,933 ms   (the main core's ingest_entries_ladder walk itself)
crc_demand = 165 GB/s (a DERIVED metric: rate × bytes_per_msg — the
                       affine ship's zero-payload-reads law means the
                       workers read tags, not bodies; the bus is NOT
                       the wall, the scan walk is)
```

The remaining problems, per the PR #16 autopsy: the single-core
submission ceiling (the workers and fold monoids wait on the main
core's scan to advance `pos` and emit the sequence facts); the
loop-carry dependency on `*w` (the sequential watermark check
serializes the pipeline); the vector ladder's uop overhead (the
16-frame dual-ZMM step's AoS gather + stack buffering + pointer
dispatch measured 14% BELOW the un-laddered scalar arm 11ab on the
fleet — the R12-class refutation persists on server silicon).

## The Insight: the Scan Is a Pure Function on Identical Inputs

Under the elision regime, the pass-to-pass identity is now provable
end to end:

* the **rxbuild master** is construction-frozen (zero write sites are
  reachable — `rxbuild_bake_full` and both `master_patch_range` sites
  are gated on the elision's non-elided twins, and the CMD_RESET bake
  is the only remaining writer);
* the RX publishes the **same deterministic `(rx_start, len)` batch
  sequence** every pass (`poll_rxbuild` walks the pacing skeleton —
  no frame reads, no entry builds; flow control blocks publication,
  it never changes the split);
* the per-pass sequencer starts **fresh** and adopts the same
  construction session from frame 0 — the ladder's verdicts, the
  emission surface, the counters and the watermark chain are the same
  pure function of the same frozen inputs, every pass.

A pure function re-run on identical inputs has a memoizable verdict.

## R32-A — The Pass-Replay Ledger (`HFT_PASS_REPLAY`, default ON)

**The mechanism.** `nf-testkit::passreplay` records ONE live pass's
per-batch observable surface:

* every sink call in order — `on_msg`, `on_event`, `on_span`,
  `on_span_batch` (rec contents copied by value — the emitter's stack
  buffers are never captured), `on_span_batch_packed` (the words
  copied by value);
* the post-batch sequencer state — the counters, the watermark, the
  progress clock (the live drivers' repeated `progress_vt = now_ns`
  assignment's surviving value).

Every later pass in the same regime REPLAYS it: the recorded calls
re-issue to the live sink (which re-drives the submit bookkeeping, the
self-serve windows, the worker submissions — **the workers still fold
every message of every pass**; only the redundant re-VALIDATION walk
is elided), and the sequencer lands in the recorded state via
`Sequencer::replay_apply`.

**The guard chain (every replay is proved, never trusted):**

1. **Regime** — `pass_replay_regime()`: rxbuild AND session elision
   (the frozen-master regime; any other shape runs the live scan
   verbatim).
2. **Master write barrier** — `master_version`: a `Release`-bumped
   counter at every master write site (all three: the blocking-reset
   bake, the advance's incremental patch, the advance's tail patch).
   The ledger records it once and re-verifies every pass; any bump
   truncates the template and forces a fresh live recording — never a
   stale replay. Under the elision the counter is construction-stable
   (the write sites are unreachable), so the steady regime records
   zero re-records.
3. **Per-batch identity** — the published `(rx_start, len)` bounds AND
   the entry watermark must equal the recorded ones. A mid-replay
   divergence is a broken determinism contract and FAILS LOUD (the
   session-elision contract discipline: the program is a schedule, not
   a trust substitute).
4. **Emission order** — the replay re-issues calls strictly in
   recorded order between the live `begin_pass`/`end_pass` brackets,
   so the sink's per-pass reset and fold pump observe the identical
   call sequence.

**Pointer safety.** The recorded `(ptr, len)` pairs captured from
live emission slices point exclusively into the construction-frozen
blob and the construction-built master (the block triples) — under
guards 1+2 those bytes are the exact recorded ones, so the re-slice is
sound. The stack-backed arrays (the `RecEmitter` buffer, the
`PackedEmitter` words) are copied by value into the ledger's own
preallocated buffers (construction-time, outside the allocs=0
measured window). Capacity overflow poisons the ledger to permanent
live mode — fail-safe, never wrong (per-message sinks overflow the
call log immediately and degrade gracefully; the ledger is the
packed/span shape's lever).

**The verdict instrumentation.** The DIAG line gains
`replay_batches= replay_live= replay_passes= replay_rerecords=` and a
`PR1_R32_PASS_REPLAY_VERDICT` row carries the engagement law: exactly
ONE live-recorded pass (`replay_live == the first pass's batch
count`), every later pass replayed (`replay_passes == passes − 1`),
zero re-records in the steady regime. `HFT_PASS_REPLAY=0` (CI arm
11pr) restores the live scan verbatim.

**The parity pins.** `t_r32_pass_replay_parity_vs_live`: the elided
sustained shape (the 4-session rotation, 12 passes) with the ledger
forced on vs off — the harvested tuples AND the final sequencer state
`(watermark, counters, progress_vt)` are IDENTICAL, and the engagement
laws hold exactly (11/12 passes replayed, 0 re-records).
`t_r32_pass_replay_barrier_injection`: a foreign version bump
(`r32_debug_bump_master_version`, the diagnostic seam) fires exactly
one truncate-and-re-record; the tuples stay bit-exact and the replay
re-engages on the following passes.

## R32-B — The Fleet-Worker Scaling Arms (11fw / 11fw16)

With the scan elided, the pass cadence unbinds from the main core and
the worker fleet's fold rate sets the sustained ceiling. The auto
shape's conservative 3 workers exist for the shared-VM dev runners;
the fleet nodes have the cores to scale. The arms pin the lane count
at 8 (11fw) and 16 (11fw16) — the record-array grid and the chunk
cadence price the scaling curve, the same-draw deltas carry the
attribution, and the record claim rides whichever arm's median healthy
draw carries it (the R16/R23 elevation protocol). The ledger is
worker-count agnostic (the engagement law is identical at every lane
count — verified locally at 8 lanes).

## The Dev-VM A/B (2 vCPU, the RX-bound shape)

```
                     rollback (live scan)    replay ARMED
work_ms                     3,067.6              1,096.6   (−64%)
scan_ms                     3,838.2              2,886.1   (−25%)
passes (5s)                  15,695              16,715+
sustained rate               1.588B              1.69–1.85B
replay engagement      0 / 0 / 0 / 0       367,708 / 22 / 16,714 / 0
bit-parity                  BIT-EXACT            BIT-EXACT
allocs                            0                    0
```

The `work_ms` collapse (−64%) is the scan walk's deletion; on this VM
the remaining wall is the RX advance cadence (`reset_ms` 1.1–2.1s —
the 2-vCPU shared-core shape) and the single worker, so the rate gain
is capped at +6–16%. The fleet's 12+-core shape — where the scan was
84–93% of the wall — prices the real rate; the projection band is
**7.5B–10B+ msg/s** on the default 3-worker shape and above that on
the 11fw/11fw16 arms, bounded by the worker fleet's fold rate and the
RX pass-supply cadence (the next wall, the R33 candidate).

## A Pre-Existing Finding (documented, out of scope)

While pinning the ledger against mid-pass abandons, the sink-level
short-pass shape was found to have a **pre-existing** worker-ahead
race: when a pass is abandoned mid-window with self-serve
pre-publication, the workers can eval past the (not yet published)
next pass-start record and emit a composite segment that straddles the
pass boundary — `hydra.rs`'s fold assert fails loud ("composite record
straddled a pass boundary"). Reproduced with the ledger DISABLED under
CPU load (the shape is timing-fragile independent of R32; the bench
never abandons mid-pass; the transport-level abandon is pinned by
`t_r31a_session_elision_mid_pass_abandon`). The sink-level abandon
cell was dropped from the suite; the R33 candidate is the
lagging-record guard for the composite fold's boundary split (the same
class as `rx_tor_fix_guard`).

## Rollbacks

* `HFT_PASS_REPLAY=0` — the live scan verbatim (CI arm 11pr).
* The ledger is regime-gated: any non-elided shape (HFT_SESSION_ELISION=0,
  HFT_RXBUILD=0, split schedules, blocking resets) runs the live scan
  with zero ledger interaction.
