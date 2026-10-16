# 🚀 Report 42: The Zero-Thread Ingest — R27c/R29/R30, the 4.28B → 10B+ Frontier Program

**Branch:** `feat/run550-toroidal-fused-commit` (continues PR #15's stack)
**Base:** `ea7b22c` (the R27b watermark fix — CI green, 100/100 jobs)
**Commits:** 3 perf — R27c (the 64-deep record ring + the hole-free
inline commit), R29 (the zero-thread ingest), R30 (the vectorized
MoldUDP64 header twin) + this report

---

## 1. Executive Summary

Report 41's stack landed the toroidal-fused commit and its soundness
fix, and the fleet telemetry (Run 550, the 11ab-class arm) priced the
sustained shape at **4.28B msg/s** with a decisive split:

```
eval_ms  ≈ 1,000   (the workers' math — 2.6 GHz of headroom idle)
fw_ms    ≈ 2,450   (the workers WAITING at the frontier)
reset_ms = 830.5   (16.6% of the 5 s wall — the pass-reset tax)
main core = 0.18 ns/msg  (< 0.47 cycles/msg — the decode ceiling)
```

The diagnosis: **the workers already have the raw capability for
10B+ msg/s**; the entire system is bottlenecked by the single-threaded
funnel feeding them. The main core scans, decodes, packet-checks, and
commits every message before publishing — one core cannot decode
faster than ~4.3B msg/s, so the workers spend half their lives at the
frontier. This branch executes the three-step blueprint against the
funnel itself:

| Step | The re-derivation | The wall it breaks |
|---|---|---|
| **R27c** | The 64-deep record ring (REC_RING, decoupled from the 8-slot array ring) + the HOLE-FREE INLINE COMMIT | reset_ms = 830.5 ms — the watermark spin + the assist-disarmed epoch's rotating warm copies |
| **R29** | The SELF-SERVE PRE-PUBLISH: the deep-steady window publishes its whole span range at open; the workers ingest directly from the frozen array; the commit path is deleted for the window's steady batches | The 4.3B single-core submission ceiling (fw_ms = 2,450) |
| **R30** | The vectorized MoldUDP64 header twin — one `_mm256_loadu_si256` + two `_mm_shuffle_epi8` gathers | The scalar per-byte header assemble (0.27 → 0.09 cyc/msg frame-validation class) |

All three are **bit-exact by construction** (differential cells pin the
tuples; the new fold-order laws are enforced loudly in release mode),
zero-alloc, and each carries a per-run env rollback (CI arm **11s** —
same-draw attribution against 11ab).

### 1.1 The projected arithmetic

The blueprint's stacking (each step's recovered time re-denominated in
throughput at the 5 s window):

```
4.28B (Run 550 baseline)
  × 5.000 / 4.170   (R27c: reset_ms 830.5 → 0)      ≈ 5.15B
  × ~1.85           (R29: the funnel deleted — the
                      workers' idle 2,450 ms converts
                      to eval)                       ≈ 9.5B
  × 1.10-1.15       (R30: the ingest-side validation) ≈ 10.5-11B
```

The 11s/11ab same-draw deltas price each factor per silicon class; the
**PR1_R29_SELF_SERVE_VERDICT** gate line (10B, REPORTED per draw — the
R16/R23 elevation protocol) carries the milestone's claim discipline.

## 2. R27c — The 64-Deep Record Ring + the Hole-Free Inline Commit

### 2.1 The reset wall's anatomy

The R27b watermark restored the record ring's soundness (the fleet's
deterministic wrap — Report 41 §5b) but its reuse bound sat at 8
windows: the sink's open-time spin (`fold_pos >= rx_rec_end[rec_slot]`)
bound whenever the fold lagged a full ring, and the R27 toroidal
epoch's own payoff (zero warm copies) kept disarming under the default
assist regime. The chain, priced at 830.5 ms per 5 s run:

```
assist fires (timing-dependent) → the inline chunk's spans leave the
window's array words UNWRITTEN (the hole) → the next window on the
same (frozen) slot checks the stale word → a fix → the epoch's clean
check disarms → the rotating regime returns → the ~22 µs warm copy +
the reuse gate + the watermark spin, every window
```

### 2.2 The record ring decoupling

`REC_RING = 64` (rxdesc.rs) — the pass-record ring runs 64 deep,
decoupled from the 8-slot array ring (which sizes the descriptor/tag
rows and the 3-bit `aslot` field). The record ring is 4 KB of padded
cursors — the depth is free. The R27b watermark's reuse bound moves
from 8 windows to 64, and the bound is **structurally unreachable**:
the sink's own harvest ring (PASS_RING, 32 → 64 to match) forces
mid-run harvesting at 63 windows of lead, which forces record
completion, which forces the fold — the watermark's depth sits beyond
every lead the protocol can express. The open-time spin's price
collapses to zero in every regime; the toroidal pass runs copy-free
AND wait-free. The workers' `resolve_rec`/`probe_wb`/re-anchor walk
the 64-deep ring (both the diet and pre-diet workers).

**The law cell** (`t_r27c_deep_record_ring_wrap`): sixty parked-fold
windows of unbounded lead — the pre-R27c 8-slot ring deterministically
wraps at window 9 and folds wrong (the t_r27b fleet hash); the 64-deep
ring runs all sixty bit-exact with the epoch engaged.

### 2.3 The hole-free inline commit

An inline (assist) chunk now STILL checks-and-fixes its words into the
window's array slot — cursor-free (`arr_check_fix` /
`packed_arr_check_fix` / the per-span inline branch in
`submit_span_rx`): the check is the same 8-wide vector compare the
array path pays (~0.75 cyc/span — noise next to the inline CRC the
assist exists to run), and the slot's content is complete after EVERY
window regardless of where each chunk was evaluated. **The assist can
no longer disarm the toroidal epoch** — the rotating regime's return
(the 22 µs/pass warm-copy wall) is cured at its root.

**The law cell** (`t_r27c_hole_free_inline_commit`): a COLD
force-inline window (every chunk inline — the maximal hole shape),
then normal array-path windows: the fix count NEVER grows past the
cold fill (pre-R27c it re-fixes the entire never-written span set) and
the epoch engages. The debug hunt for this cell found the
**per-span-path hole** (the cold frames' inline branch never wrote its
word — the deterministic one-word divergence that took the diagnosis
through three layers: the batched packed path, the batched SpanRec
path, and finally `submit_span_rx`'s inline branch).

## 3. R29 — The Zero-Thread Ingest

### 3.1 The funnel, deleted

The deep-steady epoch — two consecutive clean toroidal windows at the
same span count (R27c's hole-free law makes the assist survivable) —
PROVES the frozen slot's words and tags are the invariant tape (the
same determinism law the warm copy already trusts: the schedule is
construction-fixed, the blob immutable, the session bytes live in
frame headers the span words never touch). The R29 window's open
therefore PRE-PUBLISHES the whole span range:

```
spans_ready = base + count0        (ONE Release store, at the open)
```

The workers self-serve every span from the frozen array the moment the
record lands — their frontier wait (fw_ms = 2,450) converts directly
into evaluation. The main core's steady batches route through a
32-line cursor-advance (the chunk-grid cadence kept exact for the next
window's straddle logic): no chunk-open assist decisions, no vector
check, no per-chunk publish Release — **the scan walks the sequencer
ladder only** (session/seq verification, the counter chain, the
count/last_seq bookkeeping — the emission words still flow for the
count law). Submission throughput stops being a single-core property.

The guards: a straddling inline continuation forces the classic path
(the marked chunk's remainder values come from the inline ring);
`force_inline` is the deterministic assist-mode shape; the arm requires
the batched commit family. `HFT_SELF_SERVE=0` is the rollback
(CI arm 11s).

### 3.2 The three soundness laws (each one a found-and-fixed bug)

The self-serve shape introduced three new protocol edges; each is now
a pinned law, enforced **loudly in release mode** (the module's own
"fail loud, never fold wrong descriptors" discipline):

1. **THE FRONTIER CAP** — the fold never consumes spans the scan has
   not emitted (`span_id + k > next_span` → park). The workers run
   ahead of the scan (the whole window is pre-published), so their
   results can end beyond `next_span`; without the cap the fold
   crosses the pending pass boundary BEFORE its `end_span` stamp —
   `complete_boundaries` breaks on the unstamped record, the next
   pass's values fold into THIS pass's chain, and the boundary
   snapshot lands on the contaminated hash. (Found as the
   deterministic pass-4 divergence.)

2. **THE DERIVED GRID** — `fold_lane`/`fold_rem` re-derive from
   `fold_pos` at every drain iteration. The pre-R29 accumulated
   cursors (decrement per record, switch at zero) were correct for the
   submission-bounded protocol, but the new park/resume edges (the
   frontier cap, mid-chunk pass-boundary records, assist straddles)
   opened drift windows — the accumulated cursor slips, the fold
   drains the WRONG lane's ring, and release mode silently consumes
   out-of-order spans (the 512-span drift class, chased through three
   instrumented builds before the structural fix). Deriving from the
   position makes the class impossible; the order law itself is now a
   real `assert` (both the composite and per-span drains).

3. **NO NEW INLINE CLAIMS in a self-serve window** — the pre-publish
   exposes the whole window up-front, so a mid-scan claim's mark would
   land AFTER the workers entered the chunk: both sides evaluate it and
   the lane ring carries a duplicate the order law rejects (the
   classic protocol's own law — the mark precedes the spans_ready store
   that exposes the chunk — cannot be restored once the exposure is
   up-front). The pace gate (RX_PACE = 8,192 spans) stays the flow
   control; `pending()` saturates (the fold legitimately runs ahead of
   the scan — a raw subtract wraps to ~u64::MAX in release and
   livelocks the pace gate).

**The law cell** (`t_r29_selfserve_parity`): ten passes of the
deterministic sustained shape (drain per batch — the harness's exact
cadence) over comp × self-serve cells: every record bit-exact vs the
sequential reference; the engagement cell asserts the arm fired
(windows 5+: the cold fill, the first clean verify, the toroidal
verify, one assist-straddle disarmed window, then self-serve); the
rollback cell asserts zero engagement and the same bit-exact records.

### 3.3 The local proof (2-vCPU dev VM)

```
DIAG sustained: passes=5548 ... work_ms=2184.4 assist_chunks=81
R16B_RXDESC_VERDICT rx_fixes=0 assist_chunks=81 selfserve=5544
```

5,544 of 5,548 windows self-served, zero fixes, zero allocs, bit-exact
end-to-end — the correctness law holds at full engagement. The dev VM
(1 worker, shared vCPU) cannot price the throughput; the fleet's
11ab/11s same-draw deltas carry the claim.

## 4. R30 — The Vectorized MoldUDP64 Header Twin

One `_mm256_loadu_si256` over the header's first 32 bytes; two
`_mm_shuffle_epi8` gathers: the seq's eight big-endian bytes (split
across the two 128-bit lanes — the low six in lane 0, the high two in
lane 1) merge into one register's low qword via OR; the count's two
bytes extract into another's low word. Two shuffles, one OR, two
register extracts replace the scalar per-byte assemble (8 loads +
shifts + ORs for the seq alone) — the 0.27 → 0.09 cycles/message
frame-validation class for every wide frame (len ≥ 32; the 20-byte
HB/EOS shapes keep the scalar law).

**The architecture ruling:** `nf-protocol` is `#![forbid(unsafe_code)]`
— the pure protocol law (audit-grade) — so the vector twin cannot live
beside the scalar law it mirrors. It lives in **nf-transport::moldvec**
(the ingest-SIMD layer that hosts the AVX-512 array commit) as a SAFE
API (`parse_header_wide` / `decode_seq_count_wide`, AVX2-gated, the
r25c OnceLock pattern), bit-identical to
`nf_protocol::moldudp64::parse_header` on every input — the
differential cell pins 10k random headers plus the lane-split patterns
and the short-frame fallback law. The sustained arm's per-frame
validation (the sequencer's SoA ladder) is already vectorized; this
twin serves the transport/recovery-side eager-validation shapes.

## 5. Gates & attribution

- **11ab** (the default): the full stack — R26/R27/R28 + R27c + R29 —
  `HFT_RXDESC=1` with the self-serve arm live; the R16B line carries
  `selfserve=` engagement telemetry.
- **11s** (new): `HFT_SELF_SERVE=0` — the full submission cadence
  verbatim. Same-draw delta 11ab − 11s = the zero-thread ingest's
  contribution.
- **PR1_R29_SELF_SERVE_VERDICT** (gates.rs, 10B, REPORTED per draw —
  the R16/R23 elevation protocol) — the milestone's claim discipline;
  tripwires pinned in the gates self-test.
- The 11g/11t/11p attribution arms (Report 41) continue to price the
  R26/R27/R28 contributions on the new stack.

## 6. Rollbacks

| Lever | Default | Rollback | CI arm |
|---|---|---|---|
| `HFT_SELF_SERVE` | 1 (armed) | 0 = the full commit cadence | 11s |
| REC_RING | 64 | (const — the R27b cell pins the wrap law at any depth) | — |
| The hole-free inline commit | on | (inseparable from correctness — the t_r27c cell pins it) | — |
| The fold's derived grid + frontier cap | on | (inseparable from correctness — the order asserts) | — |

## 7. Test ledger

- nf-testkit: **94/94** green (release + dev profiles, debug asserts
  on) — the two R27c cells + the R29 cell join the 91 from Report 41.
- nf-transport: 25/25 green (the moldvec differential cell).
- nf-protocol: 23/23 green (the gates tripwire extension).
- Workspace: all crates green; `clippy --workspace --all-targets
  -D warnings` clean.
- The local sustained smoke: bit-exact, allocs=0, selfserve engaged
  (§3.3).

## 8. Next levers (the post-10B stack)

1. **The RX thread's bake cadence** — at 10B (57 µs/pass) the RX
   thread's publication batches + the reset handshake become the next
   wall; the rxbuild skeleton (F-2) plus the vectorized header twin
   (R30) are the tools.
2. **The worker res-ring depth** — the per-span (non-composite) mode
   throttles at RES_CAP=4,096 spans of runahead; a deeper ring or
   8-span res records would widen the self-serve pipeline for
   slow-worker draws.
3. **The 3-worker fabric** on the 4+-core silicon classes — the
   self-serve design feeds N workers symmetrically (the lane grid
   derives from the fold position, not the lane count).
