# 🌀 Report 41: The Toroidal-Fused Commit — R26–R28, the 4.0B → 6-8B Frontier Program

**Branch:** `feat/run550-toroidal-fused-commit`
**Base:** `10f9134` (main — the Run 500+ squash, R25c + Report 40)
**Commits:** 3 — R26 (worker 8-span vertical walk), R27 (zero-reset toroidal pass), R28 (fused ZMM ladder-commit) + the CI attribution arms

---

## 1. Executive Summary

Report 40 landed the batched array commit at **3.799B / 4.061B msg/s
(p75 / max)** and split the remaining wall precisely:

```
passes=37244 reset_ms=833.8 scan_ms=4161.0 work_ms=3893.4 fold_ms=112.7
pending_max=3920 assist_chunks=615 crc_demand_gb_s=104.20
```

Two structural walls: the **worker efficiency gap** (52 GB/s per worker
against the 151 GB/s single-core kernel peak — ~2/3 of the walk's
cycles are record-walk tax, not Galois Field math) and the
**pass-boundary stop-the-world tax** (reset_ms = 17% of wall — the
bulk warm copies). This branch executes the R26–R28 program against
both, plus the main core's residual SpanRec materialization:

| Track | The re-derivation | The wall it breaks |
|---|---|---|
| **R26** | SoA tag sidecar + the vertical 8-span worker group + spray auto-mute | The 52 GB/s worker walk |
| **R27** | The steady epoch: frozen array slot, rotating epoch-tagged records | reset_ms=833.8ms |
| **R28** | The packed word at span discovery + the fused transform-compare | The ~48c/span main core |

All three are **bit-exact by construction** (every track's differential
pins the tuples and, where comparable, the fix counts against its
verbatim rollback), zero-alloc, and each carries a per-run env rollback
(CI arms **11g / 11t / 11p** — same-draw attribution against 11ab).

## 2. R26 — The Worker 8-Span Vertical SIMD Walk

### 2.1 The diagnosis (Report 40 §6.1)

The worker's steady span loop, pre-R26: one AoS `get_affine_tag`
(3-word slice copy where the release path needs exactly ONE — the
value), one `get_arr` word, one flag branch, one `Res`/`seg.push`
buffer store, one **LOCK RMW on `affine_hits` per span**, and — in the
all-tagged steady regime — the prefetch spray's entirely dead walk
(~2-3 cycles/span walking tagged spans it will never prefetch).

### 2.2 The re-derivation

* **The SoA sidecar.** The R24 half-cacheline layout stored
  `[t0, t1, t2]` interleaved (24 B/span). R26 splits it at the hot/cold
  seam: `tagvals[slot][idx]` — the golden VALUE, 8 B/span, **one
  64-byte line per 8 spans** — and `tagraws[slot][2i..2i+2]` — the
  (t0, t1) integrity pair, 16 B/span, debug/fix-path only. The
  `[u64; 3]` pack API is preserved verbatim (a storage re-derivation,
  not a protocol change).
* **The vertical group.** Eight consecutive spans: ONE 64 B arr load
  (the AND-reduce of the 8 `RX_AFFINE_FLAG` bits is the all-tagged
  test), ONE 64 B values load, `SegAcc::push8` (8 independent
  fold_term multiplies — no serial chain), 8 unrolled `Res` stores in
  per-span mode, **one** `fetch_add(8)` for the counter. Preconditions
  (chunk interior, one record window, wbatch reservation, cap) each
  fall back to the verbatim scalar ladder.
* **The spray auto-mute.** 64 consecutive tagged spans disarm the
  spray; the first untagged span (the payload fallback's own prefetch
  need) re-arms it. The mute is exactly the group's arming threshold —
  the steady regime pays zero spray cycles.

### 2.3 The gates

`t_r26_worker_group_parity` — the law cell (no prefill, no assists:
both shapes bit-exact AND identical fix counts — the group must fix
exactly the per-span walk's spans) + the full-stack cell (prefill +
assists: bit-exact). The full parity matrix and the 400-pass soak run
the group default end-to-end. Rollback: `HFT_WORKER_GROUP=0`
(arm **11g**).

## 3. R27 — The Zero-Reset Toroidal Pass

### 3.1 The diagnosis (Report 40 §5-6)

reset_ms = 833.8 ms / 37,244 passes ≈ 22.4 µs per pass — the window
open's **bulk warm copies**: the arr row (~80 KB, NT-streamed) plus
the tag sidecar (~240 KB, RFO-heavy) per window, ~320 KB per pass ≈
2.4 GB/s of pure copy at the fleet's pass rate. The old paradigm
treats a window as a bounded lifecycle: rotate slot → gate → copy →
publish.

### 3.2 The re-derivation (the epoch-tagged virtual tape)

Repeating a deterministic pass over an invariant frame buffer is
walking a torus. The array slot's content is **window-invariant**:
same schedule, same blob ⇒ same `(off, len)` words, same tags. A
window that verifies **fix-free at the expected span count** PROVES
the slot already holds the next window's entries — and the
check-and-fix that already runs every span stays the WHOLE safety net
(rx_fixes=0 in the steady state is the law this elision rides).

* **The steady epoch** (≥1 clean window): the window open reuses the
  array slot with **zero copies and zero bulk reuse gate**. The
  first window of a generation always rotates (cold fill); any fix or
  count change disarms — the next window rotates back and the
  warm-copy chain re-seeds the fresh slot from the toroidal slot's
  verified content.
* **The record IS the epoch tag.** The record ring keeps rotating
  every window — now carrying a 3-bit `aslot` field
  (`gen:16 | pass:1 | aslot:3 | base:44`): a lagging walker finds
  every window's record in ring order (the walk's adoption law is
  untouched) and the aslot points it at the frozen array.
* **Divergence safety.** Toroidal fix writes are pre-exposure by the
  ready-store law (`store(entry_i)` happens-before
  `publish(ready ≥ i+1)`) — but the SHARED slot also serves the
  previous window's lagging readers: every toroidal fix carries the
  **per-fix lagging-reader guard** (`rx_tor_fix_guard`: wait for the
  fold to pass the previous window's span at the same word — the
  worker's read completes exactly as its eval cursor crosses it).
  Bounded, cold-path only (fixes fire only on schedule divergence;
  the epoch disarms at that window's close anyway).

### 3.3 The gates

`t_r27_toroidal_multipass_parity` — 12 passes on one sink (cold fill →
warm copy → 10 toroidal), both regimes bit-exact, the epoch's
**ENGAGEMENT asserted** (≥2), the chaos schedule exercising the
re-earning law. The 400-pass multipass soak and the parity matrix run
the toroidal default. Rollback: `HFT_TOROIDAL=0` (arm **11t**).

## 4. R28 — The Fused ZMM Ladder-Commit

### 4.1 The diagnosis (Report 40 §6.3)

The main core's ~48 c/span includes the SpanRec materialization tax:
the ladder stores a 48 B struct per span into the batch slice; the
sink re-reads it from L1D, re-runs the `(off, len)` arithmetic +
`desc8_pack_span` per span into an `expect[8]` stack buffer, then
loads that buffer into a ZMM inside `check_arr8`. The R25c batch
crossed the batching boundary — and then re-serialized the
representation.

### 4.2 The re-derivation (zero-materialization SWAR/SIMD emission)

* **The packed word at discovery.** The steady scan's span-emission
  buffer becomes a generic `SpanEmit` (ONE source of truth per ladder
  step, monomorphized twice): the classic `RecEmitter` (verbatim —
  every non-opted sink) and the `PackedEmitter` —
  `(off_batch: u32 | len: u16 | count: u16)` at the exact moment the
  span boundary is discovered, ONE 8 B store + 3 ALU ops. The count
  rides the flag half (the MoldUDP64 message count is a u16 wire
  field); the offset is batch-RELATIVE (`body_ptr − base_ptr`).
* **The fused transform-compare.** `check_arr8_packed`: one 512-bit
  load of the 8 packed words, `vpandq` LOW48 (the count field drops),
  `vpaddq` with the **batch-constant delta** (`base_ptr − blob_base` —
  the batch-relative → blob-global translation in ONE add; the
  off-fits-u32 law excludes the carry), `vporq` the flag, `vpcmpeqq`
  against the array's own 512-bit load. The mismatch mask routes the
  cold fixes, which re-transform the lane's expected word and
  reconstruct the body from `(base_ptr, word)` — the same slices the
  SpanRec path referenced.
* **The Sink contract.** `wants_packed_spans()` (default false —
  classic sinks keep the exact SpanRec emission) +
  `on_span_batch_packed(proof, first_seq, total, base_ptr, words)`:
  the G-INV assert, ONE continuity assert at the batch head (the
  ladder's chain law proves the words consecutive — steady_step emits
  only contiguous-apply frames), the count/last_seq bookkeeping from
  the batch header, then the fused commit. The preamble, chunk-open
  assist, inline branches and publish cadence are R25c's, verbatim.

### 4.3 The gates

`t_r28_packed_ladder_commit_parity` — the law cell (both modes
bit-exact AND identical fix counts: the fused transform+compare
proves exactly as much per span as the expect[]-build compare) + the
full-stack cell. The entire existing rx-mode suite (matrix, soak,
R26/R27 cells) runs the packed default. Rollback:
`HFT_PACKED_EMISSION=0` (arm **11p**).

## 5. Local smoke (2-vCPU, 1 worker, noisy — correctness only)

The dev VM cannot price the levers (3 threads on 2 vCPUs; the assist
regime flips locally — assist_chunks ~10^5 vs the fleet's 615 — and
reset_ms is RX-starvation dominated). What it CAN gate, it gates:

* `HYDRA_BITPARITY → BIT-EXACT` (count/hash/msg_hash identical).
* `allocs=0` on every arm shape.
* Workspace: **90 tests, 0 failures**; clippy `--workspace
  --all-targets -D warnings` clean.

## 6. The fleet pricing program

Arms 11g/11t/11p ride the same 120-shard matrix as 11ab/11bs; the
same-draw deltas price each lever per draw:

| Delta | Prices |
|---|---|
| 11ab − 11g | the worker 8-span group + SoA sidecar (the 52→125-140 GB/s walk) |
| 11ab − 11t | the toroidal elision (the 833.8 ms reset wall → ~0) |
| 11ab − 11p | the fused commit (the ~48c → ~22-25c/span main core) |

The projection arithmetic from the master blueprint (worker ceiling
>9.5B msg/s at 125-140 GB/s/core, the 5.0s fully spent scanning, the
main core at ~24c/span) stacks to the **6.8-7.5B+ msg/s** class on
healthy draws — the fleet verdict line (`PR1_GIGAHFT_VERDICT`) and the
DIAG sustained split (reset_ms should collapse toward the auto-advance
handshake residue; crc_demand_gb_s should rise toward the workers' new
ceiling) are the instruments.

## 7. Files

| File | Change |
|---|---|
| `crates/nf-transport/src/rxdesc.rs` | SoA sidecar (tagvals/tagraws), `get_affine_value`/`get_values8`/`get_arr8`, `copy_tags` both rows; the aslot record packing (`rx_pack_record`/`rx_unpack_record`, `publish_record`, `read_record`, `last_slot_value`); `check_arr8_packed` + `RX_PACKED_LOW48` |
| `crates/nf-arbitrator/src/lib.rs` | `SpanEmit`/`RecEmitter`/`PackedEmitter`; `steady_step`/`ladder_group_apply`/`steady_scan_ladder` generic over the emitter; `ingest_entries_ladder`'s packed branch |
| `crates/nf-arbitrator/src/types.rs` | `Sink::wants_packed_spans` + `Sink::on_span_batch_packed` (the packed contract) |
| `crates/nf-testkit/src/hydra.rs` | The worker vertical group + spray auto-mute + `SegAcc::push8`; the epoch/toroidal window open + `rx_tor_fix_guard` + the epoch update at close; `submit_packed_batch_rx`/`submit_packed_arr_run`/`packed_body`; the sink's packed overrides; tests `t_r26_worker_group_parity`, `t_r27_toroidal_multipass_parity`, `t_r28_packed_ladder_commit_parity` |
| `scripts/ci.sh` | arms 11g / 11t / 11p (the three rollbacks' attribution) |
| `docs/41-r26-r28-toroidal-fused-commit.md` | this report |
