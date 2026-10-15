# 🧲 Report 40: The Batched Array Commit — Draining the Main Core's Per-Span Tax

**Branch:** [`feat/run500-algebraic-rediscovery`](https://github.com/zephyr4289/HFT-Proj/tree/feat/run500-algebraic-rediscovery)
**Base:** `d4398f2` (the placement revert + Report 39 §4 fleet postmortem)
**Commits:** 1 — R25c (this report) + the 11smt CI arm filename fix

---

## 1. Executive Summary

The R25b postmortem (Report 39 §4-5) left the system at **2.05B msg/s
healthy-window sustained** with one verdict and one open door:

* **Verdict (confirmed here):** the consumer-side composite fold is
  worth ~+1%, the SMT stack is refuted — the system is **main-bound**.
* **Open door:** `DIAG sustained` splits the main core's 5-second wall
  as `scan_ms≈4.16s (83%) / reset_ms≈0.83s (17%)`, with
  `work_ms=3.78-4.19s` — the **commit path**: the scan ladder plus the
  per-span array check-and-fix. At the healthy draw's span rate
  (~40M spans/s at 2.1B msg/s, ~54 msgs/span) that is a
  **~90-cycle-per-span budget on the binding core**, and roughly half
  of it is fixed per-span overhead — not work.

R25c re-derives the commit path at its natural batching granularity
(the sink's `on_span_batch`, ≤32 spans per call — the R8 batch emission
that already existed on the scan side):

| Per-span cost (array path, steady state) | Pre-R25c | R25c |
|---|---|---|
| `Arc<RxdescState>` clone + drop (two locked RMWs) | every span | **once per batch** |
| Flow-control gate (`pending()` → cross-core `fold_pos` load) | every span | **once per batch** |
| Window-open / blob-base / path-select branches | every span | once per batch |
| Check-and-fix compare | 8 × (load+cmp+br) | **1 × (64B load + vector cmp)** per 8 |
| Chunk-boundary arithmetic | per span | per run (≤64) |
| Publish cadence (`spans_ready` Release) | per 64 spans | unchanged — per 64 spans |

Local 2-vCPU smoke (1 worker, noisy): `work_ms` 3164-3350 →
2920-2960 (**−8%**), tuples BIT-EXACT, `allocs=0`. Fleet pricing rode
the new **11bs** attribution arm (same-draw delta 11ab − 11bs) — and
**landed far beyond the model**, see §5.

## 2. The Confirmation Run (and the Typo That Fought Back)

Two CI runs executed on `d4398f2` (38707616922, 38707647529; 120-shard
matrix, 33 and 40 target-silicon draws respectively). Both went red —
**every draw that reached the repurposed 11smt arm died at a stale
filename**: the arm's `tee` writes `/tmp/bench_smtstack.txt` while its
greps read `/tmp/bench_smtback.txt` (the pre-repurpose name); grep
exits 2, `set -e` kills the shard. One-line fix (this commit). The
failure is AFTER every perf arm — the battery's data is intact:

| Instrument (paired, same draw) | Run 38707616922 (33 draws) | Run 38707647529 (40 draws) |
|---|---|---|
| Composite vs per-span fold (distinct placement) | **+1.41% median** (26/33 positive, −2.6..+10.9%) | **+0.93% median** (28/40 positive) |
| Distinct vs stacked placement (composite ON) | **+48.8% median** (stacked = −32.8%, min +41.3%) | median (stacked −35.8% by medians) |
| Healthy-window default (11ab p75) | 2.049B | 2.055B (max 2.102B) |

The R25b verdicts hold on the reverted default: **composite ON
(+~1%), placement distinct** (the stack costs a third of the system —
re-confirmed independently of the 9094f32 draws).

## 3. The Culprit, Precisely

The array commit path, pre-R25c (`hydra.rs`, `submit_span_rx`), runs
once **per span** on the main core:

```rust
fn submit_span_rx(&mut self, body: &[u8]) {
    let st = self.rx.clone().expect(...);      // (1) locked xadd …
    ...                                          //     … and its xsub at scope exit
    if !self.rx_win_open { self.rx_open_window(); }
    while self.rx_assist_wm > 0
        && self.pending() > Self::RX_PACE { ... } // (2) fold_pos load per span
    if self.submit_rem == CHUNK { ... }           // (3) chunk-open branch
    let idx = (self.next_span - self.rx_win_base) as usize;
    let w = desc8_pack_span(off, len) | flag;
    if st.get_arr(self.rx_win_slot, idx) != w {    // (4) 8B load + cmp per span
        ...fix...                                   //     (cold path)
    }
    self.submit_rem -= 1;                          // (5) tracker per span
    if chunk_done { ...; self.rx_publish_ready(); }
}
```

Five observations re-derive the cost:

1. **(1) is indefensible at span granularity.** The `Arc` refcount
   line is uncontended (only the sink clones it), but a locked RMW is
   still ~15-20 cycles of fully-serialized pipeline on Zen5 — and the
   path pays **two** per span (clone + drop). At 40M spans/s that is
   ~1.2-1.6G serialized cycles per second on the binding core.
2. **(2) reads the consumer's line every span.** `pending()` is
   `next_span - fold_pos`; `fold_pos` is written by the consumer
   thread (Shared-state line). Post-R25a the consumer advances it ~85
   times per pass — the load is almost always a hit — but it is still
   a cross-domain dependency evaluated at span rate for a gate whose
   threshold (8192 spans) is 0.4% of one pass.
3. **(4) is the named bottleneck — and it is already almost free in
   the steady state.** The R16b law: `rx_fixes=0` — the RX prefill
   plus the window warm start leave every entry already exact, so the
   compare is the whole cost, and eight consecutive compares hit ONE
   64-byte cacheline. The per-span FORM (eight load+cmp+branch
   triples) is what's left to drain.
4. **The batch already exists.** `steady_scan_ladder` buffers ≤32
   `SpanRec`s and calls `sink.on_span_batch(recs)` — the R8 emission
   batching. The sink then promptly re-serializes it into 32
   per-span `submit_span` calls. The batching boundary was crossed
   and thrown away.
5. **The remaining per-span work is genuinely per-span** (build the
   expected word from the body's ptr/len; the mismatch fix) — R25c
   keeps exactly that and nothing more.

## 4. R25c — The Batched Array Commit

### 4.1 The shape

`on_span_batch`'s fabric branch now calls
`submit_span_batch_rx(recs)` (default ON; `HFT_BATCH_SUBMIT=0` is the
documented rollback — the per-span loop, verbatim):

* **One `Arc` clone per batch**, held across the whole batch.
* **One flow-control gate per batch** (the drain loop itself is
  unchanged — fold + deep spin; the overshoot bound tightens by at
  most the batch length, ≤32 spans, 0.4% of RX_PACE).
* **Run-based chunk arithmetic:** the batch is walked as a sequence of
  runs — each run is min(64, remaining) spans in ONE mode (array or
  inline; a chunk never mixes them). The chunk-open assist decision,
  the tracker wrap, the inline seal and the `spans_ready` publish all
  fire at exactly the same grid boundaries as the per-span path.

### 4.2 The 8-wide check (`RxdescState::check_arr8`)

The check half of the check-and-fix, in `rxdesc.rs`:

```rust
let a = _mm512_loadu_si512(words);     // the array's 8 words — ONE 64B load
let b = _mm512_loadu_si512(expect);    // the 8 expected packed descriptors
let bad = !_mm512_cmpeq_epi64_mask(a, b); // mismatch bitmask
```

Bit i set ⇔ word `idx+i` differs — the mask routes exactly the
divergent lanes through the scalar fix path (`rx_affine_tag_for` +
`set_affine_tag` + `set_arr` + `rx_fixes`), in the same ascending
order. The vector compare is **bit-identical in strength** to eight
scalar compares (it IS eight u64 equality compares); there is no env
gate because there is no semantic to roll back — only the AVX-512F
feature gate, with a scalar fallback building the same mask. Sub-8
tails run the per-span path verbatim.

The groups live strictly inside runs, so a group never straddles a
chunk boundary — every array store still lands before the publish
that exposes its span (a group that would straddle is simply a
shorter run + tail).

### 4.3 The law (why this is safe)

The R16b protocol's ordering contract is per-span: *store(entry_i)
happens-before publish(ready ≥ i+1)*. The batch path preserves it
exactly — all stores for a run precede the run's boundary publish;
the warm start, the reuse gate, the pass-start record flags, the
inline straddle re-claim (`rx_open_window`) and the generation mark
clear are untouched. The check itself is the correctness (a wrong
prefill costs stores, never a wrong answer) — and the vector check
proves exactly as much per span as the scalar one did.

### 4.4 The gates

* `t_r25c_batch_submit_parity` (new): steady × chaos × composite ×
  per-span res format —
  **(a) the law cell** (no prefill, assists disabled — deterministic):
  both modes must reproduce the sequential reference AND **fix
  identical span counts** (the 8-wide compare must fix exactly the
  per-span mode's spans); **(b) the full-stack cell** (pipelined +
  RX prefill + assists): both modes bit-exact vs the reference.
* The full `t_rxdesc_parity_matrix` + the 400-pass multipass soak now
  run the batched default end-to-end (both res formats, both worker
  shapes).
* Workspace: **182 tests, 0 failures**; clippy clean.
* Local sustained smoke: `HYDRA_BITPARITY → BIT-EXACT`,
  `payload_fallbacks=0`, `allocs=0`, `work_ms −8%`.

### 4.5 Rollback

`HFT_BATCH_SUBMIT=0` (read once at sink construction — per-run
constant, never in a window). The CI **11bs** arm runs it on every
draw; same-draw delta 11ab − 11bs is the lever's fleet price.

## 5. What the Fleet Priced (runs 38716351075 + 38716405744, `06acc8c`)

Both runs: 99/100 shards green (the single failure in each is a kbench
CV-noise gate on a co-tenant draw — `stddev 2.71c > 2.5c` / `cv 42.6% >
25%`, no correctness involvement); every perf arm completed; bit-parity
+ `allocs=0` + `payload_fallbacks=0` held on every draw.

| Instrument (paired, same draw) | Run 38716351075 (38 draws) | Run 38716405744 (33 draws) |
|---|---|---|
| **11ab (batched) vs 11bs (per-span) — the R25c lever** | **+82.6% median, 38/38 positive** (min +38.1%) | **+55.7% median, 33/33 positive** (healthy draws +85-91%) |
| 11ab healthy window (p75 / max) | **3.799B / 4.061B** | 3.753B / 4.053B |
| 11bs (the rollback) p75 / max | 2.070B / 2.160B | 2.040B / 2.167B |
| Composite vs per-span fold | +0.76% (23/38) | −0.41% (14/33) — noise-level |
| Distinct vs stacked placement | +91.3% median | +68.0% median |
| R23 2.5B verdict (11ab) | **27 shards PASS at 3.7-4.06B** | (draws noisier; top rates equal) |

The healthy sustained window moved **2.05B → 3.75-4.06B msg/s** — the
2.0B→3.5B program goal is passed with margin, bit-exact and zero-alloc.

The mechanism (the 3.768B draw's own `DIAG sustained`):

```
passes=37244 reset_ms=833.8 scan_ms=4161.0 work_ms=3893.4 fold_ms=112.7
pending_max=3920 assist_chunks=615 crc_demand_gb_s=104.20
```

* **`assist_chunks=615`** (was 176k-428k on d4398f2): the R10 assist is
  nearly DORMANT — the freed main cycles did not convert into inline
  CRC, they converted into SUBMISSION. Main was the strangler; with
  the per-span tax drained, the workers absorb the load themselves
  (104 GB/s CRC demand across 2 workers ≈ 52 GB/s each — ~1/3 of the
  151 GB/s single-core kernel peak, the walk/protocol residue).
* Per-span main budget: ~86c (2.05B, 40M spans/s) → **~48c at 74.5M
  spans/s** — the commit machinery roughly halved while the span rate
  ~doubled; throughput ∝ 1/(per-span cost) held exactly.
* `fold_ms=112.7`, consumer parks 62 (was 1389): the R25a composite
  fold has headroom to spare at 2× the rate.
* The model's +8-15% was the conservative floor — the assist
  equilibrium flipped instead (main no longer needs to subsidize the
  workers), so the lever priced at the full strangler share.

## 6. The Residual (next tracks)

The wall is rebalanced: `scan_ms 4.16s (83%) / reset_ms 0.83s (17%)`
with main ~78% busy in pure ladder+batched-commit work at 74.5M
spans/s, and the workers at ~1/3 of kernel peak. The next levers, in
expected-value order:

1. **The worker-side walk (the new binding side):** 104 GB/s demand vs
   151 GB/s/core kernel peak — ~2/3 of the workers' cycles are the
   record walk, tag reads and res/composite protocol. Batching the
   worker's per-span walk the way R25c batched main's is the natural
   R26 (the record-resolution diet already batches the wake).
2. **`reset_ms` (17% of wall):** ~22µs/pass at 37k passes — the pass
   boundary's bake tail + window warm copy. Overlapping it with the
   tail drain (or amortizing the record protocol across passes) is
   worth up to +17% at the new rate.
3. **The ladder's per-span residue on main:** the steady_step/
   ladder_group_apply SpanRec build is now the dominant per-span cost
   (~48c/span includes it) — a SoA-native batch emission (ladder →
   sink without the per-entry SpanRec materialization) attacks it.
4. **The pace/assist equilibrium re-tune:** with assists dormant,
   `RX_PACE=8192` and `HFT_ASSIST_WATERMARK=2048` were tuned for the
   main-bound regime; re-price them at the worker-bound equilibrium.

## 7. Files

| File | Change |
|---|---|
| `crates/nf-transport/src/rxdesc.rs` | `check_arr8` (AVX-512F + scalar fallback), `r25c_avx512f` OnceLock gate |
| `crates/nf-testkit/src/hydra.rs` | `submit_span_batch_rx` + `submit_arr_run` (run-based, 8-wide groups), `batch_submit` field (env `HFT_BATCH_SUBMIT`), `on_span_batch` dispatch, `hydra_pass_piped_mode` test helper, `t_r25c_batch_submit_parity` |
| `scripts/ci.sh` | the 11smt filename fix (`smtback` → `smtstack`) + the 11bs rollback arm |
| `docs/40-r25c-batched-commit.md` | this report |
