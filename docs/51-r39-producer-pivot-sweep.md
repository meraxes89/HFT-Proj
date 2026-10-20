# R39 — The Producer Pivot: the Coarse-Grained Submission Sweep (Option A)

Run 900 continues on `feat/run900-r37-solution2-corridor`. The fleet's
verdict on the two-lane elision (run 39168431150, 41/41 green) proved
the workers are NO LONGER the bottleneck: the champion at 60.3B msg/s
with 99.998% of both lanes' boundary crossings replayed runs at the
one-lane rate — the main producer core is the binding constraint. This
run lands the pivot's first lever: **the coarse-grained submission
sweep** — the main loop's per-batch ceremony amortizes over groups of
already-published batches. The attribution discipline is unchanged: the
champion arm carries the lever, the rollback arm (11r39r,
`HFT_BATCH_GROUP=1`) restores the verbatim per-batch cadence, and the
same-draw delta prices it.

## §1 The measured decomposition (the fleet's own numbers)

The champion-class draws of run 39168431150 (the 58.6B/66.6B runs, the
2596MHz silicon) decompose the 5,000ms window on the MAIN core as:

| phase | ms | share | content |
|---|---|---|---|
| scan (the whole inner loop) | 4,834 | 96.7% | wait + work + fold + instrumentation |
| — wait | 1,248 | 25.0% | the per-batch lockstep with the RX: freed `fetch_add` + parked check + filled spin per batch (12.7M batches) |
| — work | 1,111 | 22.2% | try_replay dispatch + `submit_packed_batch_rx` (a per-call `Arc::clone`+drop — two locked RMWs on the shared refcount line) + bookkeeping |
| — fold | 1,860 | 37.3% | `drain_ready`: the per-record protocol — a runtime `% n_lanes` DIV at every loop top (~99.5M divides), a `complete_boundaries()` CALL per record (103M calls for 578,887 true boundaries), per-batch drain setup |
| — unattributed | ~615 | 12.3% | the per-batch TSC instrumentation (6 `_rdtsc` × 12.7M batches) |
| reset + end | 137 | 2.7% | the pass boundary |

The per-batch cadence is the common factor: 22 batches per pass ×
578,887 passes, each paying the full wait/free/wake/guard/submit/drain/
instrument ceremony. The user's directive (Option A): widen the
submission granularity 4×–8×.

## §2 The grouped consume (`next_group` + `select_group_batch`)

**The change.** `PipelinedReplayTransport` grows a grouped consume:
`next_group(k)` waits ONCE for the group's first batch (the verbatim
spin/park protocol, extracted into `wait_filled_turn`), then PEEKS the
already-published consecutive turns into the group — each peeked member
costs one `filled[]` Acquire load and one `pubs.len` read; no free, no
wake check, no wait — and returns the group. The harness replays the
members back-to-back (`select_group_batch(j)` points the guards' `cur`
at each member — `try_replay`'s identity assert, the live scan's
`entries`/`now_ns`, ALL per-member verbatim), then ONE `drain_ready`
at the group's end. `HFT_BATCH_GROUP` (default 8; 1 = the verbatim
cadence) is the arm.

**The deferred-free FIFO.** The group's buffer returns amortize
exactly like the per-batch path's one-batch deferral: all but the last
member ride a bounded FIFO of consumed data turns (`pending_data`,
MAX_GROUP=32), the last rides `cur` (the legacy deferral verbatim), and
`free_pending` — called at the next group's entry, at every reset, and
at `next_batch`'s entry (the mixed-mode defense) — drains the cluster
with ONE producer-wake check (the R33-B both-RMW cycle proof holds:
the frees are Release RMWs, the flag check follows them in the same
call). EOS marker turns NEVER enter the FIFO: their own consumption
frees them (free + wake, verbatim — the auto-advance trigger's timing
law). At a clean pass end the EOS-fetching call's entry drains the last
group (FIFO + cur), then the marker free + wake fires — the free
coverage through the marker is EXACTLY the per-batch shape's.

**The sweep's soundness.** A peek that finds `filled != turn+j` stops
the sweep (a stale `filled[]` value can never equal a future turn —
turns are unique per slot use); a peek that finds a published EOS
marker stops WITHOUT consuming it (the next call's wait finds it
already published and runs the EOS path verbatim — the sweep never
crosses a pass boundary). The ledger's per-batch identity guards run
per member, unchanged — the sweep changes only the SCHEDULING of the
checks, never their strength. The mid-pass abandon drains the FIFO at
`reset_pass`'s top (the same release the legacy cur-free provided,
plus the group's earlier members) — the unstick loop's forward frees
are untouched.

**The transport cell.** `t_r39_group_sweep_parity_vs_batch` drives the
armed frozen-master shape (rxbuild + elision + sched replay + prearm,
NBUF=16) through passes of ~8 publications each — the ring laps within
every pass, so a leaked deferred turn stalls the RX's bufwait at NBUF
and the test hangs: the FIFO's discipline is exercised at depth. The
grouped sweep's observable stream must equal the per-batch stream
member-for-member ((rx_start, len) + clocks, EOS alignment, frame
counts); then the mixed-mode pass (a `next_batch` stretch inside a
grouped pass) and the mid-pass abandon (reset after a partial group)
must complete with the same identity stream.

## §3 The submit-path diet (the Arc clone, the lane DIV, the boundary call)

Three fixed costs leave the main core's hot loops — none changes a
single observable:

* **The cached borrow (`rx_ref`).** The sink's `rx` Arc is
  construction-frozen; the hot submit paths paid an `Arc::clone` +
  drop per call (two locked RMWs on the shared refcount line — ~25-40
  cycles each, on the champion's every-batch cadence and the
  per-span fallback paths) for a pointer the sink already owns. The
  cached `&'a RxdescState` (the fabric's own Arc, dereferenced once at
  construction; the lifetime rides the fabric's `'a`, independent of
  any `&mut self` method borrow) deletes the pair with zero unsafe —
  a copy of a shared reference; all the state's methods are `&self`
  with interior/atomic mutability.

* **The exact lane modulus (`LaneMod`).** The fold grid's derived-lane
  re-derivation paid a runtime `% n_lanes` DIV at every drain loop top
  (~99.5M divides on the champion's window — the divisor is runtime,
  so the compiler must emit DIV). `LaneMod` precomputes: one AND when
  the lane count is a power of two (the champion's 2 lanes — the
  modulo vanishes), else the Granlund–Montgomery exact invariant
  multiplication (m = ⌊2^N/d⌋+1, N = 32+⌊log₂ d⌋; q = (u·m) ≫ N,
  exact for every dividend u < 2^32 — the fold grid is 2^28.3 at the
  200B target, six orders of margin). The derivation LAW is unchanged
  (drift-immune re-derive at every loop top).

* **The boundary hoist (`pending_boundary_end`).** The composite
  drain called `complete_boundaries()` after EVERY record — ~12-18
  cycles of loads + branch for a condition that fires once per ~178
  records (103M calls for 578,887 true boundaries). The hoist reads
  the oldest pending pass's `end_span` once per record-loop entry; the
  per-record call becomes a local compare (`>=` — records never
  straddle a pending boundary by the worker's break discipline, and
  the full function still runs at the loop-top / break / park sites,
  so the pop-and-advance shapes and the multi-cross resumes stay
  exact; the refresh re-reads after each true firing).

The per-batch TSC instrumentation amortizes with the group (6 reads
per GROUP, not per batch — the R36-E clock diet's completion).

## §4 What the fleet prices (the honest projection)

The local dev slice (2 vCPUs, both workers on one core) is
worker-bound — it verifies correctness (BIT-EXACT, allocs=0, the
engagement laws: mean_width 3.6-3.7 vs the rollback's 1.00, wait_ms
542.7 → 311.7) but cannot price the lever. On the fleet's main-bound
draws the projection: wait −70% (the lockstep sync amortizes ~8×),
work −45% (the per-batch fixed costs + the Arc pair), fold −35% (the
DIV + the boundary call + the per-batch drain setup), instrumentation
−85% — a projected scan share ~2.2s of the 5s window, a rate in the
110-130B class on the 60.3B draws' silicon. The residual after the
sweep (the per-record fold chain + the irreducible submit work + the
pass boundary) is Option B's surface: the dual-producer dispatch (the
SMT co-lanes) or the self-supplying main (the schedule table consumed
without the RX round trip — the R33 table is RX-internal today; the
Theorem-25 epoch-slot machinery is the merge surface). The fleet's
same-draw 11r37 - 11r39r delta is the next session's denominator.

## §5 The arms

* **11r37 (the champion)**: the sweep rides the default (k=8). The
  engagement greps: `PR1_R39_GROUP_SWEEP_VERDICT` presence,
  `armed=true`, `mean_width=[2-9].` (the sweep actually widened — the
  R38L2 baseline at the per-batch cadence was groups == batches). The
  hard floor unchanged: BIT-EXACT, allocs=0, the R36-E/R37/R38/L2
  engagement laws all still pinned.
* **11r39r (the rollback)**: `HFT_BATCH_GROUP=1` — the verbatim
  per-batch cadence on the identical shape; the same-draw delta prices
  the sweep. `armed=false` pinned.
* **t_r39** (the transport cell) runs before the champion, the
  R37c ship's-gate pattern.
