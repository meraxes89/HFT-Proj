# 🧲 Report 39: The Chunk-Composite Fold — Killing the Consumer's Serial Chain

**Branch:** [`feat/run500-algebraic-rediscovery`](https://github.com/zephyr4289/HFT-Proj/tree/feat/run500-algebraic-rediscovery)
**Base:** `20ec61a` (PR #14 head, post-Report 38)
**Commits:** 2 — R25a (the composite fold) + R25b (the SMT flip)

---

## 1. Executive Summary

The first Run 500+ fleet draw (CI run 38683262938, 120-shard matrix,
EPYC 9V45 class) priced the four Report-38 tracks on target silicon:

| Instrument | Reading |
|---|---|
| `affine_sub_1t` (worker kernel, single core) | **151.13 GB/s = 4.72B msg/s** |
| Sustained full-verify, 2 workers, R23c armed | **2.049B msg/s** (best shards 2.14B) |
| Worker utilization (eval_ms/wall) | ~98% busy |
| Main core ("scan") utilization | ~90% busy |
| `rx_actual_cpu` | **3** — the RX thread on worker 1's SMT sibling |

The worker kernel runs at 2.3× the rate the pipeline delivers. The
decomposition (the Run 500+ diagnosis) found the fundamental culprit
in `hydra.rs`'s consumer loop: **a serial, one-span-at-a-time fold
over the harvested result ring** — `h ← rotl(h,13) ⊕ v; h ← h·P` per
span, one 16-byte `Res` record per span, span-id asserts per record,
cacheline ping-pong per 4 records between the worker core that wrote
them and the consumer core that drains them. The per-span chain is a
5-cycle dependent latency (rotl→xor→IMUL) that cannot be pipelined
away — and the result-ring MESI traffic stalls the worker side too.

This report ships the fix as an **algebraic re-association, not a
micro-optimization**:

1. **R25a — the chunk-composite affine fold:** the fold law is
   re-derived to the exactly-composable form
   `h ← rotl(h,13) ⊕ (v·P)`, and a whole ≤64-span segment composes
   into ONE 16-byte composite record `(k, B)`. The consumer applies it
   in **two dependent instructions** (`rotl` + `xor`) per segment
   where it previously spent k × 5 serial cycles plus k ring reads.
   Result-ring traffic collapses ~46× (per-lane records per pass:
   ~3,951 → ~85).
2. **R25b — the hyperthread hypothesis, priced and refuted:** the
   scalars-stacked placement (main+rx on one sibling, worker 1's
   sibling idle) measured **−32.5% median across 29 target-silicon
   shards** — the system is main-bound; the default reverted to the
   R16d distinct layout, with the stack kept as a counterfactual
   arm. The composite fold itself measured **+2.36% median
   (27/29 shards positive)** — kept as the default.

## 2. The Culprit, Precisely

The consumer's fold loop (pre-R25a, `fold_available`'s lane drain):

```rust
let slots = lane.res_slots_read();
for i in 0..n as usize {
    let r = &slots[((tail + i as u64) & RES_MASK) as usize];
    let v = r.value;          // 16B record load per SPAN
    self.fold_value(v);       // rotl(13) ^ v; then * 0x9e3779b97f4a7c15
}
```

Three costs compound per span: (a) the serial dependency chain —
rotate (1c) → xor (1c) → IMUL (3c) — unhideable because each step
consumes the previous step's `self.hash`; (b) the 16-byte result
record every span, whose writes bounce cachelines between the worker
core (Modified) and the consumer core (Shared pull) at the rate of
one 64B line per 4 spans ≈ 9.6M line transfers per second at the
armed rate; (c) the ring-protocol bookkeeping (cursor math, boundary
caps, lane-grid tracking) riding each record.

## 3. R25a — The Composition Law

### 3.1 Why the old law could not compose

The R23 law `h ← (rotl(h,13) ⊕ v)·P` mixes two incompatible algebras:
integer multiply (linear over ℤ₂₆₄ with `+`) and rotate/XOR (linear
over GF(2)⁶⁴). Integer multiply does **not** distribute over XOR
(`3·P ≠ P ⊕ 2P` on any odd P), so the composition of two folds is not
expressible as any single `(A, B)` pair — there is no chunk composite,
which is exactly why the fold had to run span-by-span on the consumer.

### 3.2 The re-derivation

Re-associate the same three mixing primitives so the multiply rides
the INDEPENDENT span values instead of the serial chain:

```text
h_j = rotl(h_{j-1}, 13) ⊕ (P·v_j)         (the law; P = 0x9e3779b97f4a7c15)
```

Then over k consecutive spans from h₀, writing uᵢ = P·vᵢ (induction on
the rotation count — each term's rotation depends only on its distance
from the segment END):

```text
H_k = rotl(h₀, 13k mod 64) ⊕ ⊕ᵢ rotl(uᵢ, 13(k−i) mod 64)  =: rotl(h₀, 13k) ⊕ B
```

Every term of B is **independently computable** (a RORX + XOR pair,
throughput-bound), and the composite `(k, B)` applies as two dependent
ops. The classic identity lands for free: `13·64 ≡ 0 (mod 64)` — a
full 64-span chunk applies as a **pure XOR** (`h ^= B`).

Both mixers survive: the P-avalanche still diffuses every value before
it enters the XOR tree, and the position-dependent rotations keep the
chain order-sensitive. (A bare `rotl⊕` chain would cancel duplicates
at distance 64·k — the premultiply breaks that: a repeated span value
enters with the same u, but at a different position its rotated term
differs unless u is a fixed point of the rotation, and the sequence-
continuity asserts remain the primary order/dup detector exactly as
before.)

### 3.3 The machinery

* **Worker side (`SegAcc`):** per span, the premultiplied term
  `u = fold_term(v)` lands in an 8-byte L1 slot (replacing the 16-byte
  `Res` store); at each segment break the flush composes
  `B = fold_compose_b(terms)` and writes ONE record
  `{first_span: u32, k: u16, B: u64}`. Segment breaks:
  **grid-chunk ends** (the lane grid switches there — required by the
  consumer's fold_rem bookkeeping), **batch/frontier publishes** (the
  resume starts a fresh segment), and **pass-start records**.
* **Pass-start flag (`R25a` protocol bit):** the pass boundary is
  where the consumer snapshots the pass hash and re-seeds the chain —
  a composite may NEVER straddle it. Window records gain a pass-start
  bit (bit 47 of the packed `(gen, base)` record word;
  `rx_pack_record`/`read_record`). A window opening exactly at the
  current pass's first span (`begin_pass`'s eager open — tracked as
  `pass_start_span`) carries the flag; the workers' record walks
  (`resolve_rec`) return it, and the span loop splits the segment
  exactly at that span. Soundness is release-ordered for free: the
  record is published before any of its spans' `spans_ready` store,
  and the walk lands on it exactly when `gid == base`. A split at any
  OTHER point is merely an extra record — the composition law is exact
  for every k, so the mechanism is fail-safe, not fail-loud.
* **Consumer side:** the lane drain consumes records —
  `h = fold_compose_apply(h, k, B)`, `fold_pos += k`, cursor advance,
  boundary snapshot per record. Two dependent ops per ≤64 spans. The
  per-span span_id assert becomes a per-record first-span assert; the
  pass-boundary cap (`n.min(end_span - fold_pos)`) drops to a debug
  assert (records never straddle); the lane-grid switch fires exactly
  at record ends because segments end at grid-chunk ends.
* **Inline/assist path:** unchanged shape — the sink folds its own
  chunks per span via `fold_value` (the new law); the composition of
  per-span steps equals the composite by the law itself.
* **Ring path (`HFT_RXDESC=0`):** verbatim per-span emission and
  per-span consumer fold under the new law — the rollback semantics
  of the R17 ruling stay pinned by the parity matrix.

### 3.4 The gates

* `t_r25_composite_law` — 10,000 random vectors: step-fold vs one
  composite for every k ∈ 1..=64 (all rotation residues), PLUS the
  split discipline (a segment split at any interior m, applied as two
  records, equals the unsplit chain — exactly the pass-boundary /
  batch-end split shape), PLUS the k=64 pure-XOR identity.
* The standing matrix, re-swept: `t_rxdesc_parity_matrix` now runs
  composite AND per-span × diet AND pre-diet × 1..3 workers × chaos
  and steady × check-and-fix / forced-inline / pipelined — all
  bit-exact against the sequential reference.
* `t_rxdesc_sustained_multipass_parity` — 400 passes × both formats:
  7903 spans per pass, 7903 mod 64 = 47, so ~every pass boundary
  straddles a grid chunk mid-chunk — the split law gets hammered
  ~800 times per run under slot cycling, frontier partial publishes
  and generation re-anchors.
* Workspace: **181 tests, 0 failures**; clippy `--workspace
  --all-targets` clean. Golden span-chain hash values change with the
  law (by design — the sequential reference and hydra change together;
  bit-PARITY is the pinned invariant, and no test pins the constant).

### 3.5 Rollback

`HFT_COMPOSITE_FOLD=0` — per-span Res emission + per-span consumer
fold verbatim under the new law (CI arm **11comp**; the law itself is
not a knob — bit-parity is its gate, the composite is the perf shape).

## 4. R25b — The Hyperthread Hypothesis, Priced and Refuted

Run 38683262938's DIAG: `rx_actual_cpu=3`. The R16d "distinct"
placement puts the RX ingest thread on cpu3 — the SMT sibling of
worker 1 (cpu2). The Run 500+ diagnosis ("the hyperthread trap")
held that the RX's publication loop (28% busy) robs worker 1 of issue
ports and L1/L2 — and that stacking BOTH scalars on cpu1 (worker 0's
sibling) would free a full physical core for worker 1.

`fabric_placement_scalars_stacked` implemented exactly that (workers
(0,2); main+rx timesharing cpu1; cpu3 idle). The fleet priced it
(run 38702884119, 29 target-silicon shards, paired same-shard deltas
against the distinct layout):

```
stacked vs distinct: median −32.5%   min −35.6%   max −29.2%   (29/29 negative)
composite vs per-span: median +2.36% min −0.3%   max +4.9%    (27/29 positive)
```

**The hypothesis is rejected — decisively.** The system is
MAIN-BOUND, not worker-SMT-bound: main (~90% busy) timeshared with
the RX (~28%) on one logical cpu runs at ~118% combined demand —
main loses roughly a quarter of its own throughput, and the RX sits
inside main's critical loop (`next_batch`), compounding every stall.
Meanwhile the RX's SMT residency on cpu3 costs worker 1 an estimated
10–15% of ONE worker — the far cheaper theft. The default placement
reverted to the R16d distinct layout; `scalars_stacked` stays armed
as the counterfactual instrument (`HFT_FABRIC_PLACE=scalars_stacked`,
CI arm **11smt**) because the tradeoff shape differs on ≥3-physical-
core silicon, where a future fleet may re-price it.

The honest residue of the diagnosis: the consumer-side fold WAS a
real cost (the composite's +2.36% is its removal), but it was ~6% of
the main core's time (fold_ms=262 of work_ms=4126 in the baseline
DIAG) — a supporting actor, not the strangler. The binding
constraint is the main core's SUBMISSION path
(`ingest_entries_ladder` + the per-span array check-and-fix + tag
sidecar writes: work_ms=4.1s of the 5s window) — that is where the
next 2B→3.5B must come from (R25c below).

## 5. What the Fleet Priced (run 38702884119, 29 target-silicon shards)

The composite's paired same-shard attribution (11ab vs 11comp):
**+2.36% median** (min −0.3%, max +4.9%, 27/29 positive). The
consumer-side structural wins, measured and prospective:

* **Consumer per-span work → ~0**: two dependent ops + cursor math
  per ≤64 spans (≈0.03 cycles/span at the 4.7B target rate — the
  serial chain that capped the consumer at ~⅓ of a worker's span
  rate is gone).
* **Result-ring traffic ~46× lower** (85 records/lane/pass vs 3,951):
  the worker-side emit stores halve (8B term vs 16B record), and the
  cross-core cacheline ping-pong collapses to ~0.2M lines/s.
* **Worker 1 unthrottled** (R25b): the ~28% sibling demand leaves its
  physical core.
* **The `Res` slot layout is dual-interpretation** (one 16-byte
  format, protocol-owned: `{span_id, aux, value}` per-span /
  `{first_span, k, B}` composite) — no struct change, no unsafe
  union, `size_of::<Res>() == 16` still compile-pinned.

Local dev-VM smoke (1 worker, indicative only — this VM's topology
gave a 1-worker fabric): BIT-EXACT through 5,149 sustained passes
with straddling boundaries, `payload_fallbacks=0`, `allocs=0`,
`fold_ms=48.3`. The 120-shard fleet draw is the pricing instrument.

## 6. The Residual (next tracks)

* **R25c — zero-thread ingest:** the RX producer folds into the
  workers' own window advance (no RX thread, no sibling demand at
  all); the transport refactor is the prerequisite.
* **Submission-side per-span cost:** `ingest_entries_ladder` +
  `submit_span_rx` still do per-span array check-and-fix, tag sidecar
  writes and ledger walks on the main core — the next consumer-side
  decomposition once the fold's share is re-priced.
* **Vectorized composite flush:** `fold_compose_b`'s RORX+XOR loop is
  throughput-bound but scalar; a VPROLVQ+VPXOR 8-lane flush would cut
  the worker's per-chunk compose cost ~2.5× (0.6 cyc/span of the
  worker's ~23 — only worth it if the fleet re-prices the worker as
  the binding constraint).

## 7. Files

* `crates/nf-testkit/src/sink.rs` — the law + the compose helpers
  (`FOLD_MIX_P`, `FOLD_ROT`, `fold_term`, `fold_compose_b`,
  `fold_compose_apply`, `fold_step`).
* `crates/nf-transport/src/rxdesc.rs` — the pass-start record bit
  (`RX_REC_PASS_BIT`, `rx_pack_record`/`rx_unpack_record`;
  `publish_record`/`read_record` signatures).
* `crates/nf-testkit/src/hydra.rs` — `SegAcc`, the dual-`Res` layout,
  composite emission in both array workers, the composite drain in
  `fold_available`, `pass_start_span` tracking, `spawn_pinned_full`'s
  composite flag, 128B lane alignment, the re-swept parity matrix +
  multipass soak + `t_r25_composite_law`.
* `crates/nf-testkit/src/affinity.rs` —
  `fabric_placement_scalars_stacked` + the R25b default flip.
* `scripts/ci.sh` — arms **11comp** (composite rollback) and **11smt**
  (placement rollback).
