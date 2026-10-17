# 48 — Run 800 / R36-E: The Worker Eval Elision (the Res-Record Replay)

The 50–75B band's lever named by docs/47 §8, landed on PR #21's branch
(`feat/run800-r36-res-fold`). The run-39008889406 fleet verdict (Shard
10, 9V45) priced the walls precisely: 29.79B msg/s with the workers at
89% duty (15.13 µs of eval per pass per lane — the per-span walk: the
arr word loads, the SoA value loads, the fold_term multiplies, the
ring mechanics) and the main thread saturated at 99.6% (344 recorded
sink calls × ~31 ns re-issued per pass + 2.4 µs of `Instant::now`
diag pairs + the fold's 2.08 µs). This run deletes the redundancy on
all three fronts — the memoized regime (solution1.md §6.1 operating
point B) made structural.

## §1 The worker res-record replay (the elision)

**The law.** In the frozen-master regime (the R32 proof upstream: every
re-issued batch is byte-identical to the recorded one) the workers'
emitted Res stream is a pure function of the frozen tape — the
re-derivation is redundant. Per lane, a PHASE-SEGMENT LOG memoizes it:
the composite records `(rel, k, B)` — the segment's first span
RELATIVE to the pass boundary, its span count, its composed value —
recorded during the first passes, re-emitted verbatim with the
boundary offset every later pass. The per-pass worker cost collapses
from the span walk (~15 µs) to ~86 record emissions (~0.3 µs).

**The phase law (why one log per phase, not one log).** The composite
records must break at the lane grid's chunk ends (the fold consumes
lane `(fold_pos/CHUNK) % n_lanes`'s ring — a record crossing a chunk
end double-folds) and at the pass boundaries (the fold snapshots
there). The pass length T is NOT a multiple of the grid period
(`n_lanes × CHUNK`): the boundary's offset within the grid advances by
`T mod grid` each pass, so the SEGMENTATION repeats with period
`grid / gcd(T, grid)`. The golden tape: T = 10,992 spans, grid = 128 →
the orbit is 4 (the boundary phases 0, 96, 64, 32). One log per phase,
keyed by `rb mod grid`, fills over the orbit's first passes; from then
on every pass finds its phase's script and replays. (The mini tape's
orbit is the FULL 64 — the adversarial rotation; the parity cell runs
it.)

**Soundness (the guard chain — every replay is proved, never trusted):**
1. The frozen-master regime (the R32 guards upstream — the batch
   bounds/watermark identity, the master write barrier).
2. THE LENGTH LAW: each boundary's `rb` must advance the learned pass
   length — a stream change resets the whole log and re-records from
   the live walk.
3. THE ALIGNMENT LAW: the replay offset (`rb − the phase's recorded
   boundary`) must be a multiple of the pass length — the tape-phase
   mapping is exact, never rotated.
4. THE HOLE GUARD: an inline-claimed chunk skipped during a recording
   (the warm-up windows' rare assist) leaves a hole the replay cannot
   honor — the fold's inline interleaving is timing-dependent — so the
   phase's log is discarded and re-recorded hole-free on the next orbit
   (the steady self-serve windows carry no assist at all).
5. THE INLINE GUARD: a chunk claimed mid-replay falls back to the live
   walk for the pass's remainder (the derived-grid re-derivation).
6. THE TUPLE ASSERT (the output gate, unchanged): the fold's per-pass
   tuple must equal the golden reference EVERY pass — a corrupt log, a
   wrong offset, or a mis-segmented replay diverges the tuple and
   fails loud the same pass. The SPANHASH surface (the O(1) P64
   application + the R36-C merge law's fail-stops) shadows it.

Arming: OPT-IN (`HFT_RES_REPLAY=1`) — the elision's frozen-regime
premise holds on the R32 champion shape (the sustained loop's
frozen-master template); the pre-R32 multipass shapes (the direct
live ingest, no pass-replay ledger) keep the verbatim walk — their
inline-assist interleaving is timing-dependent and the fleet's faster
silicon exposed the divergence class there (shard 59's draw:
`t_rxdesc_sustained_multipass_parity`, the replay vs the mid-walk
inline claims). The CI's champion arm (11r36) sets the flag
explicitly; `HFT_RES_AUDIT=1` is the live-walk differential twin. The log's 1 MB segment store
allocates at worker spawn (pre-window; the allocs=0 gate law); the gen
re-anchor clears it in place.

**The landing's own bug ledger** (each pinned by a cell or a guard):
the exhausted-script boundary spin (the phase script's end is not the
pass's end — the lane's last span sits short of the boundary; the
boundary check now runs on the PENDING position, the force-cross at
the record itself); the clean handoff (the ready overlap lets the live
walk run PAST an armed boundary — the walk now stops at the armed
boundary, the replay owns the new pass's spans); the gen's base-0
record is the first boundary (the phase machine steps at the re-anchor
or the phase-0 class records one orbit late); the fallback's record
re-derivation (the force-cross adopts records ahead of the cursor —
the fallback re-finds the covering record by scan).

## §2 The main-side call fusion

The R32 replay re-issues the recorded sink calls: the ladder's
PackedEmitter flushes at 32 spans, so one pass's surface is ~344 calls
× ~31 ns of dispatch — ~10.7 µs of the main thread's 14.9 µs/pass. The
fusion merges the consecutive same-batch packed calls into one
(the words REBASED onto the first's base — a one-time recording cost;
the sink's self-serve counting branch never reads the words, the
classic check path reads the rebased offsets — both stay exact). The
batch bracket is a fusion barrier (the first call of each transport
batch starts a fresh script — the harness's per-batch rhythm and the
window-open cadence ride the batch edges), as is the pass bracket
(the emission-order law: calls re-issue strictly inside the live
begin/end brackets — a cross-pass fusion would submit the next pass's
spans before its begin_pass opens the window, and the boundary record
would never carry the pass-start flag). `HFT_CALL_FUSION=0` restores
the per-flush calls (the attribution arm).

## §3 The diag clock diet

The per-batch `Instant::now` pairs (~2.4 µs/pass at the 22-batch
cadence — 17% of the projected post-elision main-thread budget) became
calibrated-TSC reads: one `_rdtsc` (~6 ns) + one multiply-shift (the
Q32 reciprocal of the calibrated frequency). Only the DIAG SPLITS
changed instruments — the verdict rate's denominator keeps the
monotonic-raw clock.

## §4 The span-id wrap

At the elision's own target rates (50B+ msg/s × 5 s = 5–8B spans) the
`Res.span_id: u32` domain wraps mid-window — the fold's naive u64
compares would fire at ~40% into the window. The order asserts became
masked compares (`span_id == fold_pos as u32` — order violations are
local, never 2^31 spans apart) and the R29 frontier cap became the
RFC-1982 serial compare (`end >_ser next ⟺ (end − next) mod 2^32 ∈
(0, 2^31)` — the serial window is nine orders beyond any real fold
lag). Pinned by the drain-level wrap cell (`t_r36w_span_id_wrap`).

## §5 The telemetry contract

```
RESREPLAY_DIAGNOSTIC <label>: workers=… elision=true replayed_passes=…
    live_passes=… fallbacks=… records=…
    (R36-E: the worker eval elision — the res-record replay over the
    phase-segment logs, docs/48; HFT_RES_REPLAY=0 restores the live
    walk, HFT_RES_AUDIT=1 the differential twin)
DIAG worker[i] … res_replays=… res_recs=… res_fb=… res_live=…
PR1_R36_EVAL_ELISION_VERDICT rate=… elision=true replayed_passes=…
    live_passes=… fallbacks=… res_records=… fused_calls=…
```

The engagement laws (asserted by the bench — the exit-0 discipline):
on an armed run every loop pass replays (the per-worker boundary count
== the pass count + the gen re-anchors; the recording orbit and the
fallbacks are the only live passes), the fallbacks stay at the assist's
noise floor, and the fusion absorbs the flush granularity. The
`tag_hits` counter's semantics shift under the elision (it counts the
recording passes' live tag evaluations — the steady replay emits
records, not tag evaluations); `payload_fallbacks=0` keeps its meaning.

## §6 The cells

`t_r36e_replay_parity_and_engagement` — the full-64 orbit (the mini
tape, one worker): 72 passes = the recording orbit + 8 replays; the
tuples bit-exact vs the sequential references; the boundary-count,
replay-dominance, fusion-engagement laws. `t_r36e_phase_machine_laws`
— the unit-level state machine: the orbit fill, the arming at the
length-aligned repeat, the length law's trip (the full reset), the
alignment law's trip (the rotated offset), the hole guard's discard,
the audit/disabled twins. `t_r36w_span_id_wrap` — the drain-level wrap
(seed the fold below 2^32, records crossing it, the cursor landing
past it). The fusion's parity rides the existing R32 cell (the live
arm vs the fused replay arm — the tuples and the sequencer state
identical) + the 11r36er bench arm.

## §7 What this landing is NOT (the honest scope line)

The fold stays LIVE (2.08 µs/pass — the composite application is the
per-pass verification surface; a fold-side replay would make the tuple
assert vacuous). The RX stays live (the publication replay + the
self-serve windows). The next walls, in the fleet's own order: the
fold's serial chain at the higher pass cadence, the worker wake
latency per publication (the strand-A cadence), and the boundary
machinery's fixed residue (~0.6 µs/pass). The blueprint's Problem 2
deferred-queue protocol (solution1.md §3.5) remains the deeper
restructure if those walls bind.
