# 47 — Run 800: The R36 Program — the Res-Fold, the Boundary Transparency & the Galois Merge

The program against PR #20's merged baseline (Run 750, the R35
pass-boundary collapse + the 𝒜₅.₀ fabric wiring). The math team's
`docs/solution1.md` solved the three remaining theoretical problems;
this run lands their executable form. The port found and fixed THREE
defects in the reference kernel (the R34 discipline: the blueprint's
algebra is better than its code — every fix is pinned by a differential
cell).

## §1 The reference kernel's three defects (found before landing)

1. **The compress mask** (§2.6 `mulmod4`): `_mm512_maskz_compress_
   epi64(0xAA, r)` selects the ODD words, but the products of
   even-word operands land in the EVEN words — the correct mask is
   `0x55`. Probed on-silicon before the port; the dual-path fuzz
   (cell `t_r36_mulmod_dual_path_fuzz`, 2²⁰ pairs × 3 oracles) keeps
   it pinned.
2. **The two-level table's bounds** (§4.5 `shift_const`): the init
   reads `T_pow[65536]` — past the reference's own `MAXSPAN = 22,017`
   bound. The port's base table covers the full u16 range (512 KB,
   L2-resident; the chunk LAW stays 22,017).
3. **The irreducibility witness's constants** (§2.2 Theorem 9's
   example): the quoted `x^{43} ⊕ x^{20}` does not follow from the
   document's own Lemma 5 — the correct pair is `x^{43} ⊕ x` vs
   `x^{19} ⊕ x` (distinctness, the theorem's content, is unaffected;
   the cell pins the corrected values against the sequential oracle).

The port's own kernel restructure (measured on the dev VM): the
reference's `_mm512_set_epi64` operand pairs cost ~16 scattered uops
per 4 messages — replaced by one 32-byte dense load + one `VPERMQ`
per operand (the SoA law), the per-4 stack round-trip for the
multiplier column (a store-forwarding stall on Intel) replaced by a
one-pass column precompute, and the per-iteration `VPCOMPRESSQ`
replaced by a full-ZMM accumulate with ONE deferred mask (XOR is
order-free — bit-identical, the dual-path law holds).

## §2 R36-A: the Res-fold (Theorems 9–11, `spanfold.rs`)

**The kernel.** `Q(x) = x⁶⁴+x⁴+x³+x+1` (QLOW `0x1B`); `mulmod` is the
3-CLMUL reduction (scalar `_mm_clmulepi64_si128` + the 4-lane
VPCLMULQDQ `mulmod4` twin), the schoolbook shift-XOR multiply and a
fully independent MSB-first long-division oracle ship as the fuzz
stack's judges. The `T_pow` init (65,536 entries) exhaustively
verifies `ord(x) > 2²¹` (Lemma 4's no-collision law — the assert
fails loudly at init).

**The records.** The 36-byte packed **R36** (`V` hash fold, Δb/Δa,
the tagged price setter, span/count/flags; `sizeof == 36` is a
load-bearing static assert) and the 48-byte **P64** accumulator (the
degree leaves u16 at pass scale — `C` stored as a field element, the
two-level scheme). Composition is ONE carry-less multiply + ONE XOR
(Corollary 10.1); the non-book message types are the identity class
(width 0 ∧ digest 0 — the fold runs the full stream in place).

**The fold.** Theorem 10b: `V = ⊕ᵢ vᵢ ⊗ x^{dᵢ}` — the L terms are
independent, no serial dependency, the type's only role is a table
index (no switch). `fold_cols` is the column-view API (zero
materialization — the fabric's Tier-F law); `fold_stream` composes
the 512-block records into the P64 at any length.

**The cells (13 in spanfold + 3 in the fabric).** The mulmod
3-oracle fuzz, the ord/table distinctness, the token cross-invariant,
the type-order adversary (Theorem 9's irreducibility — a multiset-V
implementation fails exactly here), the all-one-type extremes, the
empty-segment identity edges, the R36/P64 associativity fuzz, the
degree-overflow escape (3,000 max-span chunks, u16 wrap into P64),
the partition-invariance sweep (granularity 1..4096 × random offsets
— Theorem 12's executable form), the split-degree sweep + deg-
convention trap detector + merge trees 2–5 workers, the discipline
mix, the 500k compose law, and the golden-tape span layer.

## §3 R36-C: the Galois shift (Theorem 17)

`hash_merge(h_early, h_late, w_late) = h_early ⊗ x^{w_late} ⊕ h_late`
— the shift is by the LATER segment's span (Appendix D item 1's
convention is structural in the argument names; the corpus-level trap
detector proves the sweep is not vacuous). The multiplier is ONE
field element — the 64×64 matrix is never materialized (Theorem 16
kills the fixed-Λ claim; §4.4's representation remark kills the
matrix form). Pass/worker-level degrees use the two-level table
(`T_hi[d≫16] ⊗ T_lo[d&0xFFFF]` — 2 loads + 1 mulmod).

**The instrument** (`spanbench`): the cold fold, the serialized-chain
baseline, the memoized 976-composition replay, and the merge
micro-rate, with `bitexact`, `dual_path`, `replay_ok`, `merge_ok` all
COMPUTED and asserted (never priced on faith). Dev-VM draws: the
replay at **13.7 cycles per chunk record** (the blueprint's ~15), the
merge at **6.0 ns**, the cold fold at 15.6 cyc/msg (the 2-vCPU shared
Intel slice's price — the 9V45's CLMUL ports and wide issue will
price their own constants; the regimes, not the constants, are the
contract).

## §4 The fabric wiring (the span layer, live)

The bake (`bake_sig_fabric`) now also emits the message-order span
columns (type → W_T index, the payload digest, the volume deltas, the
setter flags — one tuple per message, the shared epilogue writes BOTH
the locate-bucketed SoA and the message-order columns from ONE match
body: the no-drift law) and folds them into **988 frozen chunk
records** on the golden tape (505,849 messages; `span_deg =
11,544,836` — the byte-span law), the composed P64, the global
SPANHASH, and the frozen two-worker split at the record midpoint.

**The per-pass surface (the memoized regime — §6's operating point
B):** every window boundary in Worker 0's walk runs, alongside the
R34 Σ walk: the O(1) P64 application (1 ring multiply + XOR, ~15
cycles) and the **R36-C merge law** (3 CLMUL + XOR) — two fail-stops
per pass, ~20 ns total. `HFT_SIG_AUDIT=1` adds the 977-hop record
re-composition (the audit twin). The commutative coordinates agree
with the R34 Σ on both the mini corpus and the golden tape (the two
layers are one monoid's coordinates).

## §5 R36-B: the boundary transparency (Theorem 12)

**The measurement** (the new `open_split` probe in
BOUNDARY_DIAGNOSTIC): the PR #20 fleet's remaining `reset_ms = 204`
was `reset_wait` 12.4 ms + `pass_open` ~192 ms — and **82% of
pass_open was the sequencer's per-pass 64 KiB arena zero + full-struct
copy** (`*seq = Sequencer::new_unboxed()` at every pass head: 620
ns/pass on the 9V45's ~100 GB/s memset, 1.26 µs on the dev VM).

**The law that kills it (the write-before-read discipline):** the
arena's bytes are read ONLY through `lens[slot] != 0`
(`window::drain` checks the mark first), and every mark is written
AFTER the slot's bytes are staged (`window::stage_msg`). Zeroing the
1 KiB presence bitmap — never the 64 KiB arena — is sufficient: no
stale slot can be read, and every readable slot was written since the
last re-anchor. **The arena zero was pure overhead — exactly the
blueprint's "the re-anchor scan has zero semantic content."**

**The fix:** `Sequencer::reanchor()` — the O(1) re-anchor (11.6 ns
measured in isolation), `HFT_REANCHOR=0` (CI arm `11r36r`) restores
the full construction for the same-draw attribution. The
differential cell (`test_r36_reanchor_differential`) pins the
observable equality against fresh construction across six adversarial
pass programs (out-of-order staging, unsticking drains, window-ring
wraps, gaps, heartbeats, session re-adoption) — bit-exact messages
and events.

```
LOCAL A/B (2-vCPU dev VM, reanchor on vs off):
reset_ms        68.7 -> 16.1     (-77%)
per_pass_open  1531 -> 274 ns    (-82%)
open_split[sequencer]  1257 -> 56 ns (the arena zero's whole cost)
rate            4.24B -> 4.22B   (the shared-core noise floor; the
                                 fleet's dedicated cores price the win)
```

## §6 The pre-bake (PR #20's ALLOC_DELTA gate failure, fixed)

The fleet's `ALLOC_DELTA != 0` gate failure (the 116 diagnostic
allocations): the lazy bake fires at the first **lane-0-owned** pass
boundary — a chunk-grid alignment race that lands it 1–2 passes INTO
the timed window, so the bake's ~130 one-time table allocations
(mutable Vecs, the order cache's growth rehashes, the Arc) polluted
the steady state's zero-allocation gate. The fix: **the pre-bake**
(`HydraFabric::prebake_sig_fabric`) — the harness bakes from the SAME
ledger walk (the extracted `bake_from_ledger` — one body, both
callers, no drift) BEFORE the window opens, respects
`HFT_SIG_FABRIC=0`, and warms the R36 two-level shift table (the lazy
512 KB `T_hi` OnceLock must never build inside a window). The ref
pass also now runs the full pass shape (begin/end — the worker's
boundary fires there; its tuple comes from the harvest ring, the
authoritative surface under the pass shape). Every arm exits 0 with
`allocs=0` restored.

## §7 The telemetry contract

```
BOUNDARY_DIAGNOSTIC <label>: reset_wait_ms=… pass_open_ms=…
    per_pass_reset_wait_ns=… per_pass_open_ns=…
    open_split[sequencer=… begin_pass=… replay=…]
SIGFABRIC_DIAGNOSTIC <label>: baked=true msgs=… locates=… chunks=…
    L=… digest=… sig_replays=… sig_replay_us=… span_hash=0x…
    span_records=… span_deg=… span_merge_ok=…
PR1_R36_RESFOLD_VERDICT rate=… baked=true span_records=988
    span_deg=… span_merge_ok=true reset_ms=… per_pass_open_ns=…
    prebaked=true reanchor=true
spanbench: PR1_R36_RES_FOLD_VERDICT fold_cyc_per_msg=…
    seq_chain_ns_per_512=… kernel_speedup=… replay_ns_per_500k=…
    replay_cyc_per_record=… merge_ns=… bitexact=true dual_path=true
    replay_ok=true merge_ok=true
```

The engagement laws on an armed sustained run: `baked=true`,
`span_records == 988` (the 505,849-message grid), the byte-span law
`span_deg == 11,544,836`, `span_merge_ok=true` (the frozen
two-worker split's merge law — Theorem 17 on the live path),
`prebaked=true`, `reanchor=true`, `sig_replays ≈ passes` (every loop
pass is pure replay), `open_split[sequencer]` collapsed from the
~1257 ns arena-zero class to the tens-of-ns re-anchor class, and the
alloc/parity floor holds: `allocs=0`, BIT-EXACT per pass.
Rollbacks: `HFT_REANCHOR=0` (arm `11r36r`), `HFT_SIG_FABRIC=0` (the
fabric), `HFT_SIG_AUDIT=1` (the deep audit).

## §8 What this landing is NOT (the honest scope line)

The worker **eval elision** (the res-record replay replacing the
per-span value computation on the steady path) remains the next run's
candidate: the R35/R36 fabric verifies every pass's SPANHASH and Σ
from frozen tables, but the workers still evaluate each span's CRC
value live. The scan/decode stage is the next wall (the blueprint's
§6.2 honest ceiling: CLMUL-throughput- and parse-bound at ~2.5–4 G
msg/s fresh from wire; the memoized regime is where the 10 G+ class
lives). The R36-C merge is wired at the fabric level (the frozen
split + the live per-pass law); the per-worker lane merge (the live
two-lane fold composition) prices with the eval-elision run.
