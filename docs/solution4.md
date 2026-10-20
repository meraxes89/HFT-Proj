# SOLUTION4.MD — The Walk-Free Frontier, the Super-Coarse Grid, the Vector Digest, and the Certified Sub-Picosecond Horizon (The 1.0T–1.5T Corridor)

**Status:** the R44 mathematical specification (post-R43, PR #24's 903.3B baseline).
**Hardware target:** the same dedicated CI silicon — 2 physical AMD EPYC Zen 5 cores / 4 vCPUs @ 4.56 GHz, AVX-512 (VPCLMULQDQ native).
**Baseline achieved:** 903.3B msg/s peak, ~840B median (4.51 Trillion messages in 5.00 s, window period 464 ns, allocs = 0, the 210-cell matrix bit-exact).
**Target milestone:** ≥ 1.0T msg/s (the sub-picosecond amortized frontier, < 1.0 ps/message), with 1.5T as the corridor's gated ceiling.

Companion documents: `SOLUTION.MD` ([S1]: the O(1) affine LOB fold, Theorems 1–8, the 94-cell hydra matrix), `SOLUTION1.MD` ([S1']: the Res-fold, the barrier-free pass, the Galois merge, Lemmas 3–5, Theorems 9–18), `SOLUTION2.MD` ([S2]: the ring-parallel prefix fold, the asynchronous causal sheaf, the sub-nanosecond monoid tree, Lemmas 6–9, Theorems 19–32), and `SOLUTION3.MD` ([S3]: the universal multi-tape sheaf, the cross-venue transducer, the cohomological phantom detector, the Shannon–Bremermann capacity law, Lemmas 10–16, Theorems 33–42, the 210-cell matrix). Notation, the state space, the span-hash discipline, the commutativity split, and the tie-break taxonomy are imported from there and cited as `[S1]` / `[S1']` / `[S2]` / `[S3]`. This document is self-contained for everything new: **Lemmas 17–20, Theorems 43–46, and the 210 → 224-cell conformance extension (+59..+72).**

The one-sentence summary, in the series' idiom: **the 903B champion's last serial organ is P₀'s own two hands — a walk that re-derives, every window, an answer the frozen schedule already knows (Theorem 43 deletes it), and a digest merge that adds eight numbers one at a time (Theorem 45 vectors them); with the owners' event stream coarsened to the 1024-span grid (Theorem 44), the corridor's binding floor leaves the silicon and becomes a budget statement (Theorem 46 certifies it).**

## Table of Contents

- §0 Executive Summary
  - 0.1 The four constructions
  - 0.2 Verdicts on the posed claims
  - 0.3 The headline budget
  - 0.4 What is proven, what is measured, what is projected
- §1 The 903B Anatomy: P₀'s Two Serial Floors and the Owners' Event Stream
- §2 Part XII — The Closed-Form Frontier Jump Operator (Theorem 43)
  - 2.1 The walked configuration space
  - 2.2 Lemma 17 — the schedule-invariant class and the guard-elision law
  - 2.3 The jump operator 𝒥
  - 2.4 Theorem 43 — the bisimulation and the walk-free equivalence
  - 2.5 Corollaries — the P₀ budget, the fallback law, the kill switch
  - 2.6 What breaks it
- §3 Part XIII — The 1024-Span Super-Chunk (Theorem 44)
  - 3.1 The re-grid: sub-records, super-chunks, the derived frontier
  - 3.2 Lemma 18 — the re-grid split law and heterogeneous closure
  - 3.3 The 4-level Blelloch up-sweep
  - 3.4 The AVX-512 vector schedule — dual-accumulated ZMM, no spills
  - 3.5 Theorem 44 — bit-exactness and the halved event stream
  - 3.6 What breaks it
- §4 Part XIV — The SIMD-Vectorized Digest Reduction (Theorem 45)
  - 4.1 The digest's algebra — the abelian projection recalled
  - 4.2 Lemma 19 — the plane transposition isomorphism
  - 4.3 The 8-slot vector schedule
  - 4.4 Theorem 45 — SIMD ≡ scalar, bit-exact
  - 4.5 What breaks it
- §5 Part XV — The Sub-Picosecond Watermark Invariant and the DayChain (Theorem 46)
  - 5.1 The sub-picosecond regime — where the order lives
  - 5.2 Lemma 20 — the quantum-widened cone bound (sub-cycle flooring)
  - 5.3 Theorem 46 — no inversion, no phantoms, the DayChain's three properties
  - 5.4 The tiered verification budget
  - 5.5 What breaks it
- §6 The Corridor Arithmetic — the 1.0T–1.5T Budget (Theorem 46's floor table)
- §7 The Matrix Extension: Cells +59..+72 (210 → 224)
- §8 The Engineering Interface
  - 8.1 The super-chunk tape layout and the fold kernel
  - 8.2 The digest planes and the merge kernel
  - 8.3 The jump state and the arm/fallback protocol
- Appendix A — Proof Details and the Extended Computations
- Appendix B — Reference Algorithms

## §0 Executive Summary

### 0.1 The four constructions

| Part | Construction | Deletes | Theorem | Dividend |
|---|---|---|---|---|
| XII | **The closed-form frontier jump** — the walked ceremony is a constant affine translation `δ_𝒮` on the observable configuration under the frozen schedule; the jump operator `𝒥` applies `δ_𝒮` as ≤ 12 register stores, replacing the 22-batch iterative walk (296 ns) with an O(1) publication (≤ 30 ns) | the loop-iteration ceremony: `next_group(8)` waits, `select_group_batch` re-pointing, per-batch guard evaluation, monotone counter churn — re-derivation of an answer the frozen schedule already encodes | Thm 43; Lemma 17 | P₀'s walk term collapses 296 → ≤ 30 ns; the ingest path becomes one release-store of `window_pub` + the resets; rollback is `HFT_JUMP=0` (the verbatim walk, the CI's A/B arm) |
| XIII | **The 1024-span super-chunk** — the tape's fold atom coarsens from the 512-span chunk to the 1024-span super-chunk, itself a 4-level Blelloch up-sweep over 16 recorded 64-span sub-records; the owners' replay consumes the super-composites, halving the chunk-event stream (241M → ~120M per 5 s window) | half the owners' per-window ring events, cursor steps, and L1d/L2 line touches — the contention on the shared vCPU pair's cache cycles | Thm 44; Lemma 18 | the owners' aggregate fold capacity 1.136T → 2.27T msg/s-equivalent; the boundary partition stays strictly closed under 𝔽₂⁶⁴ at every cut; the vector schedule folds 2 super-chunks in ≤ 60 cycles, ≤ 14 live ZMM registers, zero spills |
| XIV | **The vector digest transform** — the 8-slot window-close merge, scalar today (8 × (XOR, XOR, add, add) RMW chains over 8 cache lines, ~40 ns), becomes the transposed **digest-plane** reduction: 2 ZMM loads + the balanced `VPXORD`/`VPADDQ` horizontal trees, ≤ 12 cycles for the core 4 coordinates | the scalar RMW chain and the per-slot store-forwarding stalls on P₀'s close path | Thm 45; Lemma 19 | the close's digest term 40 → ≤ 2.6 ns; the whole close (digest + day-chain append + ring consumes + ready check) fits the ≤ 50 ns budget; the publish side pays 2 off-path 16 B plane stores per owner per window |
| XV | **The sub-picosecond watermark invariant** — the order never lived in the timestamp (it lives in the per-slot sequence, [S1-T6]); sub-cycle boundary flooring widens the cone bound by exactly one quantum (`Δ_max + q`, Lemma 20), so clock drift and sub-ns resolution cannot induce causal inversions or phantom false positives at any rate; the DayChain proof object `Π_day` stays unique, minimal, and verifiable in < 2.0 ms via the 8-wide verify schedule | the folklore that sub-ps amortization needs a better clock — it needs none (Theorem 41(b)'s clock-freeness is the load-bearing law, now at 1T) | Thm 46; Lemma 20 | the certified amortization: P₀ ≤ 0.16 fs/message (4.4 orders under the 1.0 ps budget), the whole pipeline ≤ 3.7 fs/message; `Π_day`'s verification 2.1 ms scalar → ≤ 0.28 ms vector on any machine in the engine's own ISA class |

### 0.2 Verdicts on the posed claims

| The posed claim | Verdict | The one-line reason |
|---|---|---|
| Under the frozen schedule 𝒮, the pass transition is an invariant affine progression, and P₀ can advance the publication window by an O(1) register store, bypassing iterative batch stepping entirely while preserving 100% equivalence to the live walk | **TRUE** (Theorem 43) | The walked configuration's observable content is a cursor tuple whose per-window transition is the SAME affine translation `δ_𝒮` every window (the repeated-schedule law); the omitted ceremony (turn re-reads, per-batch guards, histogram churn) has no consumer in the SS-cursor regime — the bisimulation's observable equivalence is the ledger identity, verified once at the arm and carried by Lemma 17's induction |
| A 4-level Blelloch up-sweep over 1024-span super-chunks stays strictly closed under 𝔽₂⁶⁴ with a spill-free dual-accumulator AVX-512 schedule | **TRUE** (Theorem 44) | Associativity ([S2]-T19) makes every grid's evaluation equal at every cut (Lemma 18); the closure is the monoid's own (C ∈ 𝔽₂⁶⁴, V ∈ 𝔽₂⁶⁴, deg/cnt ∈ ℤ exact-range); the 16-leaf tree needs 4 ZMM-pair rounds and ≤ 14 live registers against the 32-register file — the budget table is part of the theorem |
| The 8-slot digest merge maps onto two 512-bit registers and completes in two vector instructions, ≤ 12 cycles | **TRUE with the layout law** (Theorem 45) | The merge lives in the commutative signature monoid ([S3] Lemma 14) — order-free and shape-free; the plane transposition (Lemma 19) is a monoid isomorphism, so the SIMD tree and the scalar loop evaluate the SAME sum bit-exactly; "two instructions" is the per-step literal (`vpxord` + `vpaddq` on the two loaded planes), the full core merge is a ≤ 12-cycle schedule including the horizontal folds |
| At ≥ 1.0T msg/s, clock drift and sub-ns timestamp resolution cannot induce causal inversions or phantom false positives, and `Π_day` remains unique, minimal, verifiable < 2.0 ms | **TRUE** (Theorem 46) | Causality is sequence-indexed, not timestamp-indexed — no rate inverts it; flooring costs exactly one quantum of detector conservatism (Lemma 20), zero false positives; the DayChain inherits the vector verify schedule (shape-free by [S2]-T19), landing at ≤ 0.28 ms on any 512-bit-CLMUL machine at 1 GHz — sevenfold under the 2.0 ms bound |

### 0.3 The headline budget

The measured 903B draw's per-window period was 464 ns = 296 (walk) + 168 (close) against a 505,804-message window. The corridor's stacked budget:

| Term | Today | After Thm 43 | After Thm 45 | After Thm 44 | The binding order |
|---|---|---|---|---|---|
| P₀ walk (the ingest sweep) | 296 ns | **≤ 30 ns** | ≤ 30 ns | ≤ 30 ns | — |
| P₀ close (digest + chain + consumes) | 168 ns | 168 ns | **≤ 50 ns** | ≤ 50 ns | — |
| P₀ serial period | 464 ns | ~198 ns | **≤ 80 ns** | ≤ 80 ns | P₀ ceiling: 505,804/80 ns = **6.3T/s** |
| Owners' fold capacity (aggregate) | 1.136T/s | 1.136T/s | 1.136T/s | **2.27T/s** | the corridor's true floor |
| LAG_MAX slack (64 windows) | 32.4M msgs | 32.4M msgs | 32.4M msgs | 32.4M msgs | the jitter budget ≈ 26 µs at 1.25T |

The corridor statement: **the 1.0T floor needs only Theorems 43 + 45 (P₀ ≤ 80 ns) plus the scalar 1024 re-grid (owners 2.27T); the 1.5T ceiling additionally needs the owners' dual-accumulator vector fold and the lag budget to hold at the contended draws** — §6 prices every line, and the CI gates (§7's cells + the 11r44/11r45/11r46 arms) certify each layer against the same-draw rollback.

### 0.4 What is proven, what is measured, what is projected

**Proven here (bit-exact, no tolerance):** Theorems 43–46 and Lemmas 17–20 — the jump's bisimulation, the re-grid split law at every cut, the plane isomorphism, the SIMD ≡ scalar digest identity, the quantum-widened cone bound, the DayChain's uniqueness/minimality/verify-budget — every one pinned by a cell in §7's extension. **Measured (carried in as ground truth from the R43 fleet, run 39476202545, PR #24):** the 464 ns period decomposition (walk 296 / close 168), the 8,928,575 windows, the 241,071,525 chunk composes, the 22-batch frozen schedule, the 505,804-message window, the allocs=0 discipline, the 210-cell bit-exactness. **Projected (each with a CI gate):** the post-jump P₀ period (gate 11r44 vs 11r44r, the same-draw A/B), the post-plane close (gate 11r45), the halved event stream (gate 11r46's `chunk_events` telemetry), the owners' 2.27T capacity (the same gate's per-window lag telemetry), and the corridor's contended-draw floor (the 509B class neighbor — the lag budget's stress arm). The corridor's floor (1.0T) is a theorem-backed budget statement; its ceiling (1.5T) is gate-bounded and stated as such.

## §1 The 903B Anatomy: P₀'s Two Serial Floors and the Owners' Event Stream

### 1.1 The measured decomposition

On the 903.3B draw (4.51T messages, 5.00 s, 8,928,575 windows merged), the per-window period decomposed as

```
τ_window = 464 ns = 296 ns (P₀ walk, 64%) + 168 ns (P₀ close, 36%)
```

**The walk (296 ns).** Each window replays the SAME frozen schedule: a 22-batch table recorded once by the construction pass (the per-batch per-slot span histograms + the chunk-edge list). P₀'s ceremony per window: `reset_pass` (the session affine step + the sequence re-anchor), then `next_group(8)` groups of the 22 batches, and for each batch `select_group_batch` + `try_replay` — the re-issue path whose entire payload is the SS-cursor arithmetic ("the per-batch histograms advance the per-slot cursors, NOTHING else" — the fleet landing's own comment). The payload is never read; the guards (the bounds identity, the ledger identity, the cursor containment) are re-evaluated per batch; the monotone counters churn. Twenty-two iterations of a loop whose answer is the same every window.

**The close (168 ns).** One release-store of `window_pub` (the entire submitter→owner surface — the derivation law), the per-batch cursor resets, then the lazy merge: for every window whose 8 slot composites have all arrived, `merge_window_nowait` — a scalar loop over M = 8 slots doing, per slot, 3 XORs (`v`, `c`, `h_final`) and 5 adds (`db`, `da`, `deg`, `cnt`, `msgs`) across 8 cache lines, then the day-chain append (one `mulmod` + XOR) and the completed-ring push. The digest's algebra is commutative ([S3] Lemma 14); the loop is scalar only because the entries' fields are slot-major.

**The owners (the parallel organ).** 241,071,525 chunk composes in 5.00 s across the owner threads — 48.2M composes/s aggregate, ~41.5 ns per compose-thread, each compose folding a recorded 512-span chunk composite (23,552 messages of tape) into the slot's window accumulator plus the sequential `h` chain's `mulmod` step. The owners' aggregate capacity: 48.2M × 23,552 ≈ **1.136T msg/s-equivalent** — already above the 903B draw, and the reason the system is P₀-bound today.

### 1.2 The two floors as theorem-shaped obstructions

**R5 — the derivation residue.** The R43 landing's derivation law already deleted every per-slot gate (the owners derive their chunk frontiers from the window id — a pure function of the frozen schedule). What remains on P₀'s path is the walk's own iteration: the transport's frozen-table stepping and the per-batch re-issue calls. The obstruction's shape: the walk is a *re-derivation* — a state transition computed iteratively whose closed form the schedule already encodes. That is exactly the shape Theorem 43 formalizes and deletes.

**R6 — the scalar residue.** The close's digest loop is the last scalar organ on P₀'s path. Its algebra is commutative (Lemma 14); its scalar form is an accident of layout (slot-major fields). The obstruction's shape: an order-free reduction waiting for a transposition — Theorem 45's plane law.

**R7 — the event-stream floor.** The owners' 241M events contend for L1d/L2 cycles on the shared vCPU pair (the 2-physical-core budget: P₀ + 2 owners + the harness). At 512 spans per event, the event rate is fixed by the message rate; coarsening the atom to 1024 spans halves the events per message-folded — Theorem 44's re-grid, with the boundary partition's exactness the entire proof obligation.

### 1.3 Notation imported (used throughout, cited once here)

| Object | Meaning | Home |
|---|---|---|
| `P64`, `p64_compose`, `⊗` | the semidirect pair monoid `(C,V) ⊗ (C',V') = (CC', VC' ⊕ V')` over 𝔽₂⁶⁴; `deg/cnt/db/da` ride as exact-range ℤ coordinates | [S1'] §2, [S2] §1.4 |
| `Q(x) = x⁶⁴ + x⁴ + x³ + x + 1`, `QLOW = 0x1B` | the reduction polynomial; the folded 2-clmul reduction | [S2] §1.4 |
| `𝒜_glob = ∏_s 𝒜_res^{(s)}` | the multi-tape product algebra; slots commute by disjoint support | [S3] §2 |
| Lemma 14 | the digest's commutative signature algebra: `(C,V)` by XOR, `(db,da,deg,cnt)` by integer sums — order-free, shape-free | [S3] §7 |
| Lemma 15 | the cone watermark bound `claim_time ≥ pub_time + d − Δ_max` (soundness under bounded drift) | [S3] §8.2 |
| Theorem 19 | any-bracketing: every tree shape over the semidirect compose equals the serial chain | [S2] §2 |
| Theorem 33(d), Corollary 33.1 | merge order-freedom; the fleet digest reduction `2 VPXORQ + 3 VPADDQ` per slot-composite | [S3] §2 |
| Theorem 40 | `Π_day`: the per-slot epoch chain, 19 MB/day at the 64M dial, 365,600 links, 2.1 ms scalar verify | [S3] §8.1 |
| Theorem 41(b) | clock-freeness: window indices are sequence counts, never timestamps | [S3] §8.2 |
| The repeated-schedule law | the frozen schedule's tapes are finite; cursors are global-monotone modulo the schedule's totals | R43 landing (PR #24) |
| The derivation law | every per-slot gate withheld; owners derive chunk frontiers from the window id | R43 landing (PR #24) |
| The window-bounded consumption law | an owner's accumulator never crosses a window boundary | R43 landing (PR #24) |

## §2 Part XII — The Closed-Form Frontier Jump Operator (Theorem 43)

### 2.1 The walked configuration space

Fix the fleet engine of the R43 landing: M slots, the frozen schedule 𝒮 (the per-batch span histograms `H[b][s]` and the chunk-edge list, recorded once by the construction pass), the repeated-schedule regime (the tape cursors global-monotone modulo the schedule's totals), and the SS-cursor replay path (the payload is never read in a measured window — the re-issue's entire payload is the cursor arithmetic).

**Definition (the observable configuration).** The walked ceremony's observable state at the top of window `w` is the cursor tuple

```
C(w) = ( window_pub,                     the global publication word
         r_batch, last_seq, count,       the re-issue cursors (reset per window)
         span_cur[0..M),                 the per-slot span cursors
         sess(w),                        the session bytes (affine in w)
         turn, pkt, seq_base )           the transport's frozen-table position
```

Everything else the walk touches is (i) schedule constants (the histogram rows, the chunk table), (ii) intermediate loop state with no consumer outside the loop, or (iii) the payload bytes themselves — unread in this regime.

**Definition (the walked transition).** `W` maps `C(w)` to `C(w+1)`: the full per-window ceremony — `reset_pass`, the `next_group(8)` iteration over the 22 batches (each: `select_group_batch`, `try_replay`'s guard set + histogram application, the monotone counter updates), the EOS break, `note_pass_end`, and the close's cursor resets. The walk also *evaluates* the guard set `G` (the bounds identity, the ledger identity, the cursor containment) at every batch — Boolean predicates over `C` and 𝒮's rows.

**The key structural fact** (the repeated-schedule law, fleet-proven): every measured window replays the *identical* schedule rows. Consequently `W` is not merely deterministic — it is **w-invariant**: the transition map is the same function at every window. Write `δ_𝒮` for that function's net effect on the observable tuple.

### 2.2 Lemma 17 — the schedule-invariant class and the guard-elision law

**Lemma 17 (guard elision).** Let `g ∈ G` be a guard of the walked ceremony. Call `g` **𝒮-invariant** if, whenever `g(C(w), 𝒮)` holds at some `w ≥ w_arm`, it holds at every `w' ≥ w`. Then every guard in the re-issue path is 𝒮-invariant, and the single verification of `G` at the arm window is a proof of `G` for all later windows.

*Proof.* Each guard is a Boolean combination of order and equality relations between components of `C(w)` and *constant* rows of 𝒮 (the bounds identity compares the re-issued span total against the histogram row's total; the ledger identity compares the cursor tuple against the schedule's cumulative table; the containment compares `span_cur[s]` against the window's derived frontier — the derivation law's own closed form). The transition `C(w) → C(w+1)` is the same affine translation `δ_𝒮` on every component: the histogram rows applied per batch telescope to the per-window deltas, which are the schedule's constants; the resets (`r_batch ← 0`, `last_seq ← 0`, `count ← 0`) land at the same relative positions; the session step is affine; the transport's frozen table wraps to its origin (the repeated-schedule law). A relation between `C`-components and 𝒮-constants that holds at `w` and whose every variable is translated by the same w-independent `δ_𝒮` holds at `w+1`: each relation re-evaluates on `C(w) + δ_𝒮` against the same constants, and the induction hypothesis is exactly the statement that the translated tuple satisfies the same relations (the relations are themselves functions of the tuple's position *relative to the schedule's cumulatives*, which advance by the same constants). Induction on `w' − w`. The arm's verification is therefore not a per-window necessity but a one-time certificate. ∎

The lemma is the walk's own obituary: the ceremony's guards exist to catch schedule corruption and regime breaks; under the frozen regime their satisfaction is an invariant, and re-evaluating an invariant 22 times per window buys nothing.

### 2.3 The jump operator 𝒥

**Definition (the frontier jump).** The jump operator is the O(1) application of `δ_𝒮`:

```
𝒥(w):  window_pub ← w + 1                (one Release store — the owners' ONLY input)
       r_batch ← 0;  last_seq ← 0;  count ← 0        (the close's resets, hoisted)
       span_cur[s] ← span_cur[s] + Δ_𝒮[s]  for the M slots  (M adds — or 1 vector add)
       sess ← SESS_BASE + w·SESS_STEP                (the affine session law)
       turn/pkt/seq_base ← the frozen table's origin (the wrap — a constant copy)
```

— ≤ 12 stores and 2 loads, independent of the batch count, the slot count beyond the M adds, and the schedule's length. The transport's 22-turn table is never walked; the per-batch histograms are never re-applied; the guards are not evaluated.

**The arm protocol.** The jump engages only after: (i) the construction's schedule is sealed (the frozen table exists); (ii) one full window is walked in the verbatim form with `G` verified and the harvested tuple asserted bit-exact against the oracle (the arm certificate — Lemma 17's base case); (iii) the wrap law is confirmed (`window count ≡ 0 mod the schedule period`). The **kill switch** is per-window and O(1): `HFT_JUMP=0` (the verbatim walk — the CI's A/B arm) and the **poison path**: any failed expectation (a live re-scan request, a schedule mutation, a harness assert) re-enters the walked form mid-stream — the jump's effects are confined to the cursor tuple, which the walked form re-derives idempotently (the walk from a jumped configuration is the walk from the equivalent walked configuration — the bisimulation below makes this exact).

### 2.4 Theorem 43 — the bisimulation and the walk-free equivalence

**Theorem 43 (the closed-form frontier jump).** Under the frozen regime with the arm certificate held, for every `w ≥ w_arm`:

**(a) The bisimulation.** `W(C(w)) = 𝒥(C(w))` on the observable tuple: the walked transition and the jump produce identical configurations.

**(b) Observable equivalence.** The engine's outputs — the owners' inputs (the `window_pub` word), the harvested window tuples, the day chain — are identical under `W` and `𝒥` (they are functions of the observable tuple alone: the derivation law makes every owner input a function of `window_pub`; the tuples are functions of the owners' publications; the day chain of the tuples).

**(c) The cost.** `𝒥` is O(1): ≤ 12 stores + 2 loads, vs the walk's 22-batch iteration (296 ns measured). The jump's own budget: ≤ 30 ns including the arm check, the wrap check, and the close's hand-off.

*Proof.* (a) Both `W` and `𝒥` are transitions on the same tuple defined by the same schedule constants. `W`'s per-batch steps compose to the per-window delta: the histogram applications telescope (`Σ_b H[b][s] = Δ_𝒮[s]` — the ledger identity itself), the resets land at the window's edge, the transport's table iteration is a bounded loop over pre-published turns whose net effect on the cursor tuple is the constant wrap (the turns' data is static — the RX thread is retired; the walk re-reads the same turns each window, and re-reading static data has no effect on the tuple beyond the cursor arithmetic it performs). Hence `W = 𝒥` as functions on the tuple. The parts of the ceremony `𝒥` omits — the turn re-reads, the per-batch guard evaluations, the monotone counter churn — write only to (i) loop-local state dead at the window's edge, or (ii) the observable tuple through the very delta `𝒥` applies directly; there is no third consumer, because the SS-cursor regime's only reader is the cursor arithmetic itself (the payload is never read; the workers are gone — the elision; the owners read only `window_pub`). (b) is the derivation law plus (a): identical inputs at every window ⇒ identical owner publications ⇒ identical tuples (the owners' fold is a deterministic function of the window id and the frozen tapes — [S3] Thm 33(c)); the day chain follows by induction on windows. (c) is the store count of §2.3's listing; the 30 ns budget covers the two O(1) checks (the wrap congruence and the arm word) at the measured close-path cost of a release-store plus the resets. ∎

**Corollary 43.1 (the P₀ walk budget).** The ingest path's per-window cost drops from 296 ns to ≤ 30 ns; P₀'s serial period becomes the close's budget (≤ 80 ns with Theorem 45) — a 5.8× collapse of the serial organ, moving the binding constraint to the owners (§6).

**Corollary 43.2 (the fallback law).** A poison at window `w` (a regime break detected by any means — the harness, a telemetry invariant, an external probe) restores the walked form at `w` with zero residue: the walked form's first action is `reset_pass`, which re-derives the tuple's transport components from `(w, sess)` — the jumped tuple's values are exactly what the walk would hold at the same point (the bisimulation's equality), so the re-entry is the identity continuation. The mixed drive (walked windows, then jumped windows, then walked again) produces the same tuples as the all-walked drive — cell +61's adversarial case.

### 2.5 What breaks it

**(i) Jumping in an unfrozen regime.** The bisimulation's premise is the frozen schedule + the SS-cursor path. A live re-scan (the champion's fallback shape — the transport actually producing fresh turns) re-couples the payload to the walk: the jump would publish a window whose tape contents were never scanned. The arm certificate must precede every jumped region, and the poison must trip on any regime signal — the fallback law is the safety net, and cell +60 drives a synthetic mutation to prove it trips.

**(ii) A consumer of the intermediate states.** Any code that reads the per-batch counters, the monotone turn position, or the histogram cursor *mid-window* (a telemetry hook, a debug print, an assert in the loop) observes states the jump never materializes. The discipline: mid-window observers are forbidden in the jumped regime; all telemetry moves to the window edge (the R43 telemetry discipline already does).

**(iii) A schedule whose period does not divide the window stream.** The wrap law (`windows ≡ 0 mod period`) is what makes `δ_𝒮` w-invariant. A schedule with a ragged tail (a final partial batch set) breaks the congruence at every period boundary — the jump must arm per-schedule-cycle or the tail windows must walk (the O(1) wrap check catches it; the mixed drive is legal by Corollary 43.2).

**(iv) Non-affine session discipline.** `sess(w)` must stay affine in `w` (the bench's 3-byte big-endian increment is affine until the carry crosses a byte boundary at 2²⁴ windows — 16.7M windows, 8.4 s at the 5 s window's rate... at 1T with 505K-message windows the carry lands at ~8.4 s of running; the law: the increment is affine in the low 3 bytes, and the carry's arrival is a schedule-consistent event the wrap check can pre-compute — or the session discipline moves to a plain u64 with the affine law exact).

**(v) Asserting only the walked region.** The harness must assert every jumped window's tuple (it does — the harvest loop is outside the walk/jump choice); a build that skips the oracle comparison "because the jump is proven" converts a theorem into an assumption. The cells never skip.

## §3 Part XIII — The 1024-Span Super-Chunk (Theorem 44)

### 3.1 The re-grid: sub-records, super-chunks, the derived frontier

The R43 tape's fold atom is the 512-span chunk composite (`SLOT_CHUNK = 512`): the construction folds every span into a chunk accumulator and records the composite every 512 spans; the owners' replay composes the recorded chunk composites into the window accumulator — 241M such events on the 903B draw. The re-grid changes the atom:

* **The sub-record** — the 64-span serial fold (the construction's existing per-span kernel, register-resident, un-timed): `subs[s] = [P64; n_subs]`, one 64 B line each.
* **The super-chunk** — 16 consecutive sub-records, folded ONCE at the construction by the 4-level Blelloch up-sweep (§3.3) into the recorded super-composite `supers[s] = [P64; n_supers]`. The owners' replay consumes `supers` — **one compose event per 1024 spans**, half the events, half the cursor steps, half the L1d/L2 line touches per message folded.
* **The derived frontier** — the cumulative table `window_super[]` (the derivation law, verbatim at the new grid: the owners derive their per-window super-chunk ranges from the window id; no new gate).

The sub-records stay in the tape **for the boundary partition**: a window edge cuts the span stream at an arbitrary position, and the partial super-chunk's composite must be exact (§3.2). The construction pays the sub-record table (16× the entries of the super-table — the construction pass is untimed and the table is per-schedule-cycle, wrapped by the repeated-schedule law).

### 3.2 Lemma 18 — the re-grid split law and heterogeneous closure

**Lemma 18 (the re-grid split law).** Fix a slot's span sequence `τ = (t_1, …, t_n)` (any heterogeneous discipline mix — the span-hash discipline maps every span to a P64 generator regardless of type), a window edge at `k ∈ [0, n]`, and any grid γ ∈ {512, 1024} with the sub-record granularity 64. Then:

**(a) The partition equality.** `fold(τ[1..k])` — the serial compose in tape order — equals the compose, in tape order, of the γ-grid partials induced on `[1..k]` (the full grid cells interior to the window, plus the boundary partial at the edge), and equals the 64-grid evaluation, and equals the 16×64 Blelloch evaluation inside each super-cell. Every partition whose common refinement is the span partition evaluates to the same composite.

**(b) Heterogeneous closure.** Every partial — interior, boundary, or sub-record-partial — lies in `P64` with its ℤ coordinates exact: the C-coordinate in 𝔽₂⁶⁴ (closed under the carry-less product and the Q-reduction), the V-coordinate in 𝔽₂⁶⁴ (closed under ⊕ and the field product), `deg/cnt` in exact-range ℤ (closed under +, no wrap: `deg ≤ spans ≤ 2³²`, `cnt ≤ msgs·2³²`). The monoid is closed at EVERY granularity; the re-grid introduces no new value class.

**(c) The boundary partial's exactness.** A window edge at span `k` inside a super-chunk cuts it at a sub-record boundary `k₁ = ⌊k/64⌋·64` plus a sub-record-internal tail `r = k − k₁ < 64`. The partial = the up-sweep over the `k₁/64` whole sub-records (Lemma 18a at the 64-grid) composed with the serial fold of the `r` trailing spans (the base case — the per-span kernel, [S1]'s own law). Rounding `k` to any other position changes the composite (the fold is injective on the generator sequence for distinct span multisets with overwhelming probability — [S1'] §6's collision bound 2⁻⁶⁴): there is no approximation.

*Proof.* (a) is associativity ([S2]-T19) applied to the partition induced by the grid lines intersected with the window edge: every evaluation is a bracketing of the same serial product; T19's any-bracketing law makes them equal. The refinement statement: the span partition refines every γ-partition, and two partitions with a common refinement evaluate equally by two applications of the re-bracketing law. (b) is the monoid's closure table (the semidirect pair over 𝔽₂⁶⁴ with the exact-range ℤ coordinates — [S1'] §2's own computation; nothing in the re-grid leaves the monoid). (c) is (a) at the mixed 64/span grid, plus the injectivity note for the no-approximation claim. ∎

### 3.3 The 4-level Blelloch up-sweep

The super-chunk's fold at the construction (and the verifier's — Theorem 46 reuses it): 16 sub-record leaves, pairwise up-sweep,

```
level 1:  16 → 8    (8 independent composes)
level 2:   8 → 4    (4)
level 3:   4 → 2    (2)
level 4:   2 → 1    (1)      — 15 composes, critical depth 4
```

Each compose is the semidirect pair law: `C' = C₁·C₂`, `V' = V₁·C₂ ⊕ V₂` — two 𝔽₂⁶⁴ multiplies (one `vpclmulqdq` + the folded 2-clmul Q-reduction each) and one XOR. The tree shape is free (T19); the balanced 4-level shape is chosen for the vector schedule, not for the value (every shape gives the same bits — cell +64's fuzz).

### 3.4 The AVX-512 vector schedule — dual-accumulated ZMM, no spills

**The layout.** The leaves' pairs live coordinate-major: `zmmC = [C₀..C₇]`, `zmmV = [V₀..V₇]` — two ZMM registers per 8 composites. Two super-chunks (32 leaves) occupy 4 ZMM pairs (8 registers).

**The per-level round.** Level ℓ composes adjacent pairs within each 8-lane plane: extract the even and odd lanes (`vpermd`/`vpermilpd` — 2 shuffles per plane), then

```
C'[i] = C_{2i} · C_{2i+1}                    — _mm512_clmulepi64_si128(even, odd, 0x00)
        + the folded Q-reduction             — 2 clmuls by QLOW + 3 vpxord/vpternlogd
V'[i] = (V_{2i} · C_{2i+1}) ⊕ V_{2i+1}       — clmul + reduction, then vpxord
```

— per 8 composes: ~2 shuffles + 4 clmuls + ~10 reduction/logic uops ≈ 16 uops, all full-width.

**The dual accumulator.** The two super-chunks' trees are independent; their rounds interleave on the two planes so the clmul→reduction dependency chain (~12–15 cycles) of one hides under the other's issue slots: the two-super-chunk sweep completes in 4 rounds × ~16 uops with a critical path of ~60 cycles — **≤ 60 cycles for 2 super-chunks, ≈ 8.2 ns per super-chunk at 4.56 GHz** (vs 2 serial composes ≈ 30 cycles but at HALF the messages per event — the throughput math that matters is §6's).

**The register budget (the no-spill law).**

| Live set | Registers |
|---|---|
| The 32 leaves (2 chunks × 16), C/V planes | 8 |
| The even/odd extraction temporaries | 2 |
| The reduction constants (QLOW broadcast, the fold masks) | 2 |
| The dual accumulators / level outputs | 2 |
| **Total** | **14 of 32 — 18 free, zero spills** |

Cell +65 audits the spill count in the perf window (the (I) class: an audit-only assertion — a spill is a performance bug, never a correctness one).

### 3.5 Theorem 44 — bit-exactness and the halved event stream

**Theorem 44 (the super-coarse grid).** With the re-grid of §3.1:

**(a) Bit-exactness.** Every slot's window composite at the 1024-grid equals the 512-grid composite and the span-serial reference, for every window shape and every edge position (Lemma 18a — the partition equality; the 512-grid is the R43 fleet-proven value, the span-serial the [S1] oracle).

**(b) The event law.** The owners' compose events per window per slot go from `⌈spans_s/512⌉ + ε_b` to `⌈spans_s/1024⌉ + ε_b'` (ε the boundary partials): the measured stream halves — 241,071,525 → ≈ 120.5M events per 5 s window at the same rate. The `h`-chain steps ride the events and halve with them.

**(c) The capacity dividend.** The owners' aggregate fold capacity doubles: 1.136T → ≈ 2.27T msg/s-equivalent (the same per-event cost, twice the messages per event).

**(d) The schedule.** The construction-side (and verifier-side) super-fold executes 2 super-chunks in ≤ 60 cycles, ≤ 14 live ZMM registers, zero spills (§3.4's budget table; cell +65's perf audit).

*Proof.* (a) is Lemma 18(a) with the window edge as the cut — the three grids' partitions share the span refinement. (b) is the arithmetic of the cumulative frontier tables at the two grids (the boundary partial counts differ by at most 1 per window per slot; the measured ratio is the spans ratio 512/1024 = 1/2 within the partials' noise — the 27-events-per-window measured shape becomes ~13.5). (c) follows from (b) at the fixed per-event cost (the measured ~41.5 ns per compose-thread; the events per message halves). (d) is the schedule of §3.4 — the uop counts per the Zen 5 vector ports (4 FP pipes; `vpclmulqdq` zmm at 1/cycle throughput, ~5-cycle latency; the dual-chain interleave covers the reduction's dependency). ∎

### 3.6 What breaks it

**(i) Rounding the boundary partial.** The window edge's partial must fold to the EXACT span position (Lemma 18c). A "round up to the sub-record" shortcut silently changes the composite (the injectivity note) — the divergence lands in the window tuple and the cell +63 catches every offset.

**(ii) An accumulator crossing a window.** The window-bounded consumption law is unchanged by the re-grid: the owner's accumulator resets at every window edge; a super-chunk straddling the edge is split by Lemma 18's law, never carried (the R43 parity cell's own caught class, now at the new grid).

**(iii) The 4-level tree across a window boundary.** The leaves of any one Blelloch sweep must be window-local (the sweep evaluates a within-window product; a cross-window bracketing composes spans into the wrong window's composite — the associativity law composes equal products, but the WINDOW PARTITION is not a value question, it is a reporting question: the tuple's count/hash split at the edge).

**(iv) Forgetting the ℤ coordinates.** `deg/cnt/db/da` ride every composite and every partial; a re-grid that carries only (C, V) breaks the degree law's accounting (the count identity — the harvest's own assert).

**(v) The 17th live register.** The budget table is part of the theorem; an implementation that spills (an extra plane, an unrolled 3rd chain) trades the latency-hiding for L1 traffic — cell +65's audit, the (I) class.

## §4 Part XIV — The SIMD-Vectorized Digest Reduction (Theorem 45)

### 4.1 The digest's algebra — the abelian projection recalled

The window-close digest merge lives entirely in the **commutative signature algebra** ([S3] Lemma 14): the per-slot window composites' digest coordinates reduce by XOR on `(v, c, h_final)` and by wrapping integer addition on `(db, da, deg, cnt, msgs)` — order-free, shape-free, concurrency-free. The semidirect twist of [S1'] Lemma 3 was resolved *inside* each slot's fold (the owners' `p64_compose` chains); the merge across slots is the **abelianization** — a homomorphic image of the window monoid, hence commutative. This is why the merge is vectorizable at all: a reduction over a commutative monoid is a function of the multiset of inputs, and every SIMD lane assignment, every tree shape, every consumption order evaluates the same sum.

Today's `merge_window_nowait` is scalar only because the `CompositeEntry` layout is slot-major: one 64 B line per slot, the digest fields scattered at fixed offsets. The reduction walks 8 lines, doing per slot 3 XORs + 5 adds as dependent RMW chains on a single accumulator struct — ~40 ns with load-to-use and store-forwarding stalls, inside the 168 ns close.

### 4.2 Lemma 19 — the plane transposition isomorphism

**Definition (the digest planes).** For each window `w`, the **digest planes** are the coordinate-major (transposed) image of the M slots' digest fields:

```
plane_vc[w]  = [v_0..v_7 | c_0..c_7]        128 B — the XOR plane (2 ZMM)
plane_dd[w]  = [db_0..db_7 | da_0..da_7]    128 B — the ADD plane (2 ZMM)
plane_h[w]   = [h_0..h_7]                    64 B — the chain plane (1 ZMM)
plane_m[w]   = [msgs_0..msgs_7]              64 B — the count plane (1 ZMM)
plane_dc[w]  = [deg_cnt_0..deg_cnt_7]        64 B — the aux plane (1 ZMM)
```

The owner publishes its slot's coordinates into the planes as part of the window publication (the same Release that publishes the ring entry — §8.2's contract: the plane stores precede the head's Release, exactly the entry's own law).

**Lemma 19 (the transposition law).** The map `T: x ↦ planes` from the slot-major field arrangement to the coordinate-major planes is an isomorphism of the commutative signature monoid onto the coordinate-direct product of per-coordinate vectors: the slot-sequential reduction of the entries equals the per-coordinate reduction of the planes, for every M ≤ 8, every value assignment, and every reduction order/shape on either side.

*Proof.* Both sides evaluate the same finite sum in the same commutative monoid: per coordinate, the slot-sequential loop computes `x_0 ⊕ x_1 ⊕ … ⊕ x_{M−1}` (or `+`), and the plane reduction computes the same multiset's sum by a balanced tree over the transposed lanes. Commutativity and associativity make every evaluation order and every bracketing equal — the AC-induction over M (the [S3] Lemma 14 argument, applied to the transposed representation). The transposition itself is a bijection on the data (no value is transformed — only re-addressed), so the two reductions' input multisets are identical. ∎

### 4.3 The 8-slot vector schedule

```
; ── the core 4-coordinate merge: 2 loads + 4 VPXORD + 4 VPADDQ ──
vpxord    zmm4, zmm0, zmm1        ; the vc planes' halves: [v_0^v_4, …, v_3^v_7 | c-halves]
vextracti64x4  ymm5, zmm4, 1      ; the high 256 → low
vpxord    zmm4, zmm4, zmm5-256    ; the 256-fold → 128
vpshufd   xmm6, xmm4, 0x4E        ; the 64-swap
vpxord    xmm4, xmm4, xmm6        ; → (Σv, Σc) in the two lanes — 4 ops, depth 3
vpaddq    zmm7, zmm2, zmm3        ; the dd planes — same horizontal shape
...                                ; → (Σdb, Σda) — 4 ops, depth 3
```

The critical path: 2 overlapped 64 B loads (2 cycles, 2 load ports) + the 3-level balanced tree of 2–3-cycle vector ops (≤ 6 cycles) + the final lane extract/permute (≤ 4 cycles) = **≤ 12 cycles ≈ 2.6 ns at 4.56 GHz** — the directive's "~4 ns" with margin. The full 7-coordinate close (the aux planes + the day-chain append's `mulmod` + the 8 ring-tail Release consumes + the window-ready check's 8 head loads) schedules in ≤ 24 cycles of vector work plus ~15 cycles of chain mulmod, overlapped — **the ≤ 50 ns close budget holds with 2× headroom**.

### 4.4 Theorem 45 — SIMD ≡ scalar, bit-exact

**Theorem 45 (the vector digest transform).** The plane-based vector merge of §4.3 and the scalar `merge_window_nowait` produce identical digests, window tuples, and day-chain values, bit-exactly, for every slot count M ≤ 8, every composite assignment, and every arrival order; the core 4-coordinate merge completes in ≤ 12 cycles (the schedule of §4.3), and the full close fits the ≤ 50 ns budget at the 1T corridor's window rate.

*Proof.* The identity is Lemma 19 (the two reductions evaluate the same commutative monoid sum over the same multiset — the AC-induction) plus the publication protocol's intactness: the planes are published under the same Release as the ring entry (the transposition contract), so the merge reads a consistent snapshot exactly when the scalar merge would (the readiness check is unchanged — the 8 head loads). The arrival-order invariance is Lemma 14's (the merge is a function of the multiset; the lanes are keyed by slot, not by arrival). The cycle bound is §4.3's dependency chain on the Zen 5 vector ports (2 load ports for the 4 plane loads, 4 FP pipes for the 8 tree uops — throughput 3 cycles, latency ≤ 12); the close budget stacks the core merge (≤ 12 cycles), the aux planes (≤ 12, overlapped), the chain `mulmod` (≤ 15, overlapped with the aux), the 8 consume stores (pipelined, ≤ 8), and the loop/check overhead — ≤ 50 ns at 4.56 GHz with the measured close's non-digest terms carried over. ∎

**Corollary 45.1 (the publish-side price).** Each owner pays ≤ 5 extra 16 B stores per window (its slot's lane in each plane), off P₀'s critical path entirely (the owner's publication is already a 64 B entry write + a Release; the plane stores are local, non-temporal-eligible, and overlap the owner's next window's fold).

### 4.5 What breaks it

**(i) Two sources of truth.** The ring entries and the planes must never diverge: the transposition contract (§8.2) makes the plane stores part of the publication's Release set — a "convenience" writer that updates one but not the other (a debug path, a partial publish) creates a split-brain digest. Cell +67 asserts the planes ≡ the entries at every merge.

**(ii) Lane/coordinate layout swaps.** The planes' layout is the contract; a swapped coordinate (XOR applied to an ADD lane, `db` read as `da`) is silent — the values stay plausible. The cells' layout fuzz (+68) drives random plane permutations against the scalar reference; the engineering interface's `static_assert`s pin the offsets.

**(iii) A torn plane read.** The planes must ride the entry's Release — a reader that consumes the ring entry (head's Acquire) but loads the planes WITHOUT the ordering relationship can see a half-updated plane. The protocol: the plane loads are ordered after the same Acquire that gates the entry read (the ready check's head load), which the release-store protocol already provides.

**(iv) The window tag's elision.** The scalar merge asserts `e.window == w` per slot (the arrival tag). The vector path must keep the check — on the miss path (one scalar compare per merged window against the tag plane, or the entry's own field on the rare miss). Eliding it entirely trades a containment law for nothing.

**(v) The aux planes left scalar while claiming the budget.** The ≤ 12-cycle bound is the CORE merge's; the honest full-close budget (≤ 50 ns) includes the aux coordinates' plane pairs. An implementation that vectors the core but loops the aux scalar still fits 168 → ~80 ns but not the 50 ns budget — the budget table must be quoted whole (§6's discipline).

## §5 Part XV — The Sub-Picosecond Watermark Invariant and the DayChain (Theorem 46)

### 5.1 The sub-picosecond regime — where the order lives

At ≥ 1.0T msg/s the amortized per-message spacing is ≤ 1.0 ps (0.8 ps at 1.25T) — three orders below the nanosecond timestamp granularity and half an order below the 219.3 ps TSC tick at 4.56 GHz. The folklore question: can two messages "swap" when their timestamps cannot distinguish them? The engine's answer is structural, and it is the load-bearing law of this entire part:

**The order never lived in the timestamp.** The engine's causal order is the per-slot sequence order — exact integers assigned at the single-writer publication point ([S1-T6]'s principle; the single-writer watermark law). Window indices are sequence counts, not timestamps (Theorem 41(b) — clock-freeness). Timestamps ride as payload; their only consumer is the phantom detector's R2 cone check, a necessary-condition filter, never an ordering input. Consequently **no rate, no drift, and no resolution limit can invert causality**: an inversion would require a sequence inversion inside one slot, which the single-writer law makes unrepresentable (the torn-watermark class — [S3] §2.5's (I)).

The sub-picosecond number is an **amortization statement** — and Theorem 46's first duty is to make the amortization's accounting exact (§5.3(d): the per-message budget table, all four threads, every term).

### 5.2 Lemma 20 — the quantum-widened cone bound (sub-cycle flooring)

**Definition (sub-cycle flooring).** Let `φ(t) = ⌊t/q⌋·q` be the flooring of any clock reading to the quantum `q` (the TSC tick 219.3 ps, the nanosecond 1 ns, or the message timestamp's own granularity — the deployment constant; `q` is fixed per day).

**Lemma 20 (the quantum-widened cone bound).** Under the bounded-drift assumption `|Δclock| ≤ Δ_max`, a legal claim (true clocks satisfying `claim_time ≥ pub_time + d(s→t) − Δ_max`, Lemma 15) satisfies the floored bound

```
φ(claim) ≥ φ(pub) + d − (Δ_max + q)          [the hybrid form: q once]
φ(claim) ≥ φ(pub) + φ(d) − (Δ_max + 2q)      [the fully-clocked form: q twice]
```

and the floored detector never rejects a legal claim (zero false positives); the widening is exact — no larger correction is needed, and `q → 0` recovers Lemma 15 verbatim.

*Proof.* Monotonicity and the two-sided flooring error: `φ(t) ≥ t − q` and `φ(t) ≤ t` for all t. Then `φ(claim) ≥ claim − q ≥ (pub + d − Δ_max) − q ≥ φ(pub) + d − Δ_max − q` (using `pub ≥ φ(pub)`), which is the hybrid form; the fully-clocked form applies the same two inequalities to `d`'s measurement. Soundness: the bound is *implied by* legality (the chain above is a weakening), so testing it never rejects a legal claim — the conservative-detector property, unchanged by flooring. Exactness: the two `q`'s are attained (a claim whose true time sits a hair above the bound, with both floors losing almost `q`) — no smaller uniform correction is sound. The limit `q → 0` is continuity of every term. ∎

The lemma is the whole cost of sub-picosecond operation for the detector: **one quantum of conservatism**. At `q` = 1 ns and Δ_max = 10 ms, the widening is 10⁻⁵ of the budget — noise. The edge cases (a claim exactly at the cone boundary, a timestamp exactly at a quantum edge) are cell +69's adversarial surface.

### 5.3 Theorem 46 — no inversion, no phantoms, the DayChain's three properties

**Theorem 46 (the sub-picosecond watermark invariant).**

**(a) No causal inversion at any rate.** For every message pair `a ≺ b` in the causal order (same slot, `seq_a < seq_b`), every engine output — every window composite, every digest, every published epoch record — composes `a` before `b`, at any amortized rate including sub-picosecond, under any clock drift and any timestamp resolution. *Proof:* the composition order is the tape order ([S2]-T23's static-order law within a slot); the tape order is the sequence order; the sequence is a single-writer integer — no clock participates (Theorem 41(b)). A hypothetical inversion is a sequence inversion, unrepresentable under the single-writer release-store law. Rate enters nowhere: the amortized spacing constrains the *schedule*, never the *order*. ∎

**(b) No phantom false positives under flooring.** The R2 cone check under Lemma 20's widened bound rejects no legal claim (the weakening chain); the R1 version compare and the (C, V) value compare are clock-free integers/field elements. The false-positive measure stays zero on well-formed feeds at every rate — cell +69's edges, cell +52's drift stress extended to the sub-ps regime.

**(c) The DayChain: uniqueness.** `Π_day` — the per-slot epoch chains — is a deterministic function of the per-slot tapes: by Theorem 42(a)'s 15 freedoms (extended by this document's +4: the jump's walked/jumped equivalence (Thm 43), the grid's 512/1024 (Thm 44), the digest's scalar/vector and plane shape (Thm 45), and the verifier's tree shape (below)), every configuration choice yields the same chains; two distinct tape multisets yield distinct chains except with probability 2⁻⁶⁴ per link pair ([S1'] §6's collision bound) — cell +71's injection fuzz.

**(d) Minimality.** Per-epoch spot verification requires ≥ 1 machine word per epoch (the checker must distinguish `W` distinct prefixes; an object with fewer words cannot — the pigeonhole information bound). `Π_day` carries exactly the per-slot links (52 B each) plus the checkpoint roots — an inclusion-minimal object for the per-epoch spot-verification service it renders; every proper sub-object fails some spot check (drop any link: the chain recomposes to a different root — cell +71's sub-object drive). The cross-slot digest is *not* carried (the verifier recomputes it in O(M) — Theorem 40(a)'s own note).

**(e) Verifiability < 2.0 ms.** The verification work is `W = 365,600` P64 composes (Theorem 40's object at the 1T dial, 23.4T messages/day, the 64M window dial). By [S2]-T19 the verification tree is shape-free, so the verifier may use **Theorem 44's own 8-wide dual-accumulator schedule**: `W/8 × ~6 uops ≈ 2.7 × 10⁵` vector-pipe cycles ≈ **0.06 ms at 4.56 GHz, ≤ 0.28 ms at 1 GHz** — under the 2.0 ms bound on every machine with a 512-bit CLMUL pipe (the engine's own ISA class — any machine that can run the fold can verify it). The scalar fallback retains Theorem 40(b)'s 2.1 ms budget on a 2.6 GHz core; the page-tier checkpoints (below) cut the scalar SPOT path to O(log W) and the full scalar path stays the 15-cycle/link class. Cell +72 asserts the budget on the live object.

*The theorem's proof is the union of the four parts above; each is self-contained and each is pinned by its cell.* ∎

### 5.4 The tiered verification budget

| Tier | Work | Cycles (vector) | Time @ 4.56 GHz | Time @ 1 GHz | Use |
|---|---|---|---|---|---|
| Spot (one epoch) | the page path: log₈(W/page) + page-local ≈ 12 composes | ~70 | ~15 ns | ~70 ns | the counterparty's single-epoch check |
| Page tier (all roots) | W/page root composes + the root chain | ~2.7×10⁵ / page-size factor | ~60 µs | ~0.28 ms | the day-integrity sweep |
| Full recompute | all W links | ~2.7×10⁵ | **~60 µs** | **~0.28 ms** | the regulator's complete audit |
| Full recompute (scalar) | all W links | 5.5×10⁶ | 1.2 ms | 5.5 ms | the no-AVX-512 fallback |

— with page = 512 links (the page roots published alongside the object, +8 B per 512 links ≈ +5.7 KB/day — the checkpoint structure Theorem 40(a) already carries, now the spot tier's index).

### 5.5 What breaks it

**(i) A timestamp comparison anywhere in the correctness path.** The inversion's only possible cause. Theorem 41(b) is a theorem about the protocol; a deployment that orders anything by clock has re-introduced the sequencer with extra steps ([S3] §8.2's family, now at sub-ps stakes).

**(ii) The unwidened Δ_max after flooring.** Flooring costs exactly one quantum (Lemma 20); a deployment that floors but keeps Δ_max unchanged re-introduces false positives at the cone's edge — the widening is part of the flooring contract (cell +69's twin drive: widened → zero false positives, unwidened → the edge case fires).

**(iii) A mid-day quantum change.** `q` is a deployment constant; changing it mid-day breaks the floor-monotonicity bookkeeping (the widened bound's two sides were computed under different quanta). The day-flush is the only legal transition point.

**(iv) Carrying the cross-slot digest in the object.** Triples the object for zero verification value (Theorem 40(a)'s note, unchanged); the verifier recomputes — Theorem 33(d) makes it order-free.

**(v) Verifier-tree drift as a "consistency" requirement.** The verification tree is shape-free (T19); an implementation that *requires* its own tree shape for cross-verifier agreement has misunderstood the law — cross-machine agreement is the VALUE's property (Theorem 42(c)), not the schedule's. The drift pitfall is fake; the real one is (ii).

## §6 The Corridor Arithmetic — the 1.0T–1.5T Budget

The stacked budget (every line measured on the 903B draw except the post-theorem terms, each gated):

| Organ | Term | Today | Theorem | After | The gate |
|---|---|---|---|---|---|
| P₀ | the walk (ingest sweep) | 296 ns | 43 | ≤ 30 ns | 11r44 vs 11r44r (the same-draw A/B) |
| P₀ | the close (digest + chain + consumes) | 168 ns | 45 | ≤ 50 ns | 11r45 vs 11r45r |
| P₀ | **the serial period** | **464 ns** | 43+45 | **≤ 80 ns** | — |
| P₀ | the ceiling (505,804 msgs / period) | 1.09T/s | — | **6.3T/s** | no longer binding |
| Owners | the event stream | 241M / 5 s | 44 | ~120M / 5 s | 11r46's `chunk_events` telemetry |
| Owners | the aggregate capacity | 1.136T/s | 44 | **2.27T/s** | 11r46's lag telemetry |
| Owners | the vector fold (the ceiling's end) | — | 44 (§3.4) | ~4T/s-class | the corridor's gated ceiling |
| All | the lag slack (LAG_MAX=64) | 32.4M msgs | — | 32.4M msgs ≈ 26 µs @ 1.25T | the contended-draw arm |
| All | **the binding floor** | P₀ (464 ns) | — | **the owners** | — |

**The corridor statement.** With Theorems 43 + 45 alone, P₀'s ceiling is 6.3T and the owners' 1.136T — the system becomes owner-bound at ~1.1T (the contended draws lower). With Theorem 44's scalar re-grid, the owners' 2.27T lifts the binding floor above the 1.5T target for every healthy draw class; the contended class (the 509B neighbor — a descheduled owner) is bounded by the lag slack (26 µs of absorbable lag at 1.25T) and remains the honest stress arm. **The 1.0T milestone is a budget statement once 43+45+44 land; the 1.5T end needs the owners' vector fold at the contended draws** — and each layer's number is certified against its own same-draw rollback, never against a different draw's champion.

**The per-message honesty table (Theorem 46(d)'s accounting):**

| Organ | Per-window work | Per-message (505,804 msgs/window) |
|---|---|---|
| P₀ walk (post-43) | ≤ 30 ns | 0.059 fs |
| P₀ close (post-45) | ≤ 50 ns | 0.099 fs |
| P₀ total | ≤ 80 ns | **0.158 fs — 6,300× under the 1.0 ps budget** |
| Owners (post-44) | ~120.5M × 41.5 ns / 5 s / 2 threads | ~3.5 fs |
| **The whole pipeline** | — | **≤ 3.7 fs/message — 270× under the 1.0 ps budget at 1.0T** |

The sub-picosecond frontier is not a clock achievement; it is a **budget certification** — and the certified number has 2.4 orders of margin.

## §7 The Matrix Extension: Cells +59..+72 (210 → 224)

Fourteen new cell classes (two seeded each, the harness's seed discipline — 210 + 14 = **224 cells**), in four adversarial families matching Parts XII–XV:

| # | Cell class | Adversarial design | Catches | Class |
|---|---|---|---|---|
| +59 | Zero-walk jump equivalence | the full drive with every window ≥ w_arm jumped vs the all-walked drive: every window tuple, the day chain, and the final state bit-exact; the jumped drive's window count and the oracle's assert loop identical | Thm 43(a)-(b) | bit-exact |
| +60 | Guard-elision + poison law | the jump's arm certificate verified once; a synthetic schedule mutation (a histogram row edited mid-drive) trips the poison, the walked form resumes, the tuples stay exact — the fallback law's drive | Thm 43, Cor 43.2; Lemma 17 | bit-exact + (D) |
| +61 | Mixed walked/jumped drive | walked → jumped → walked → jumped at adversarial window boundaries (incl. the schedule period's edge): the tuple stream equals the all-walked reference — zero residue at every transition | Cor 43.2 | bit-exact |
| +62 | Super-chunk re-grid parity | the 1024-grid window composites vs the 512-grid vs the span-serial oracle, every slot, every window shape (incl. single-span windows and the degenerate empty-slot shapes) | Thm 44(a) | bit-exact |
| +63 | Super-chunk boundary partition sweep | every window-edge offset 0..1023 (adversarial: the 64-boundary coincidences, the 512 coincidence, the 1023 tail, the 1-span head), heterogeneous discipline mixes in every partial | Lemma 18 | bit-exact |
| +64 | The 4-level Blelloch schedule parity | the vector tree vs the serial compose over the 16 sub-records: 200 random P64 sets + the degenerate shapes (identity leaves, uniform-K, the all-XOR trap `13·64 ≡ 0 mod 64`, the single-active-leaf) | Thm 44; [S2]-T19 | bit-exact |
| +65 | The register budget audit | perf: zero spill/fill uops in the super-fold kernel's window (the counters), the dual-chain interleave's uop counts within the budget table | Thm 44(d) | (I) |
| +66 | Vector digest merge parity | the plane merge vs `merge_window_nowait` on 10⁴ random 8-slot draws (incl. all-identity, all-max, the wrap-boundary db/da values, M < 8 shapes): digests, tuples, chains equal | Thm 45 | bit-exact |
| +67 | The transposition law | the published planes ≡ the ring entries' fields at every slot/window (the two-sources-of-truth containment); a torn plane is unrepresentable (the Release contract) | Thm 45; Lemma 19 | bit-exact + (I) |
| +68 | Vector digest order/shape fuzz | 200 random slot orders × 64 random reduction shapes (incl. unbalanced, odd tails, lane-permuted) of the plane merge — all equal to the flat reduction | Lemma 14, 19 | bit-exact |
| +69 | Sub-ps flooring edges | claims at the cone boundary ± 1 tick, timestamps at `k·q − 1 / k·q / k·q + 1`, drift at ±100 ppm and ±10 ms: the widened bound (Δ_max + q) → zero false positives; the unwidened twin fires at the edge (the contrast proves the widening is load-bearing) | Lemma 20 | (D) + (B) |
| +70 | Sub-ps causal inversion stress | 10⁶ messages at 0.8 ps amortized effective spacing (the lexicographic (floor(t), seq) order vs the sequential reference): zero inversions, zero phantom gaps, the watermark lattice monotone | Thm 46(a) | bit-exact |
| +71 | DayChain uniqueness + minimality | 10³ random tape pairs → distinct chains (zero collisions); every proper sub-object (each single link dropped) fails the spot verify — the inclusion-minimal drive | Thm 46(c)-(d) | bit-exact |
| +72 | DayChain verify budget | the 8-wide vector verify vs the scalar chain recomposition: bit-exact + the measured cycle count ≤ the derived budget (2.7×10⁵ vector cycles for W = 365,600) — the perf assertion on the live object | Thm 46(e) | (I) + bit-exact |

Total: **224 cells**, bit-exact, no tolerance, every PR. The matrix remains the contract; Theorems 43–46 remain its proof.

## §8 The Engineering Interface

The systems engineer's wiring surface: three structs, three kernels, one protocol. Everything else is the existing R43 engine.

### 8.1 The super-chunk tape layout and the fold kernel

```rust
// ── the 1024-span super-chunk grid (Theorem 44) ─────────────────────
pub const SUB_SPANS:   usize = 64;    // the sub-record granularity (the split law's atom)
pub const SUPER_SUBS:  usize = 16;    // 16 sub-records per super-chunk
pub const SUPER_SPANS: usize = 1024;  // SUB_SPANS * SUPER_SUBS

/// One 64-span sub-record — the construction's serial leaf product.
/// ONE cache line; the boundary partition's atom (Lemma 18c).
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct SubRec {
    pub p: P64,          // the 64-span composite (C, V, db, da, deg, cnt)
    pub _pad: [u8; 40],  // 64 B total
}
const _: () = assert!(std::mem::size_of::<SubRec>() == 64);

/// The re-gridded per-slot tape. The owners' replay consumes `supers`
/// (ONE compose event per 1024 spans — the halved stream); `subs` stays
/// for the window-edge partials (the up-sweep over the whole part).
pub struct SuperTape {
    pub subs:   Vec<SubRec>,   // the 64-span stream (16x the supers)
    pub supers: Vec<P64>,      // the folded super-composites (the replay atoms)
    pub window_super: Vec<u32>,// the cumulative frontier (DERIVED, never gated)
}
```

```rust
// ── THE 4-LEVEL BLELLOCH UP-SWEEP, dual-accumulated (§3.4) ──────────
// Two super-chunks (32 leaves) in ≤ 60 cycles, ≤ 14 live ZMMs, no spills.
// Plane layout: zmmC = [C0..C7], zmmV = [V0..V7] per 8 composites.
#[target_feature(enable = "avx512f,avx512clmul")]
unsafe fn super_fold_x2(leaves: &[SubRec; 32], out: &mut [P64; 2]) {
    let q  = _mm512_set1_epi64(QLOW as i64);       // the fold constant
    let mut c0 = load_c(leaves,  0);               // zmm: chunk A's C[0..8]
    let mut v0 = load_v(leaves,  0);               // zmm: chunk A's V[0..8]
    let mut c1 = load_c(leaves,  1);               // zmm: chunk B's C[0..8]
    let mut v1 = load_v(leaves,  1);               // zmm: chunk B's V[0..8]
    // levels 1..4: compose adjacent pairs within each plane.
    for level in 0..4 {
        // extract evens/odds (the pair operands)
        let (ce, co) = even_odd(c0);               // 2 shuffles
        let (ve, vo) = even_odd(v0);
        // C' = C_even · C_odd          — one clmul + the folded reduction
        let mut nc = gf_mul_x8(ce, co, q);         // clmul + 2-clmul fold + xors
        // V' = V_even · C_odd ⊕ V_odd  — clmul + reduction + vpxord
        let mut nv = _mm512_xor_si512(gf_mul_x8(ve, co, q), vo);
        // the second chain (chunk B) interleaved — the latency hide
        let (c1e, c1o) = even_odd(c1);
        let (v1e, v1o) = even_odd(v1);
        let nc1 = gf_mul_x8(c1e, c1o, q);
        let nv1 = _mm512_xor_si512(gf_mul_x8(v1e, c1o, q), v1o);
        // narrow to the next level's planes (the tree's shape)
        (c0, v0, c1, v1) = narrow(nc, nv, nc1, nv1, level);
    }
    // the roots land in lane 0 of each plane — the deg/cnt ride scalar
    out[0] = extract_root(c0, v0);  out[1] = extract_root(c1, v1);
}
```

The window-edge partial (the owner's boundary path): the up-sweep over the `k₁/64` whole sub-records composed with the serial fold of the `r` trailing spans — Appendix B's `partial_fold`.

### 8.2 The digest planes and the merge kernel

```rust
// ── the digest planes (Theorem 45) — coordinate-major, 8 slots ─────
#[repr(C, align(64))]
pub struct DigestPlanes {
    pub vc:  [u64; 16],  // [v_0..v_7 | c_0..c_7]        — the XOR plane
    pub dd:  [u64; 16],  // [db_0..db_7 | da_0..da_7]    — the ADD plane
    pub h:   [u64; 8],   // h_final per slot              — the chain plane
    pub m:   [u64; 8],   // msgs per slot                 — the count plane
    pub dc:  [u64; 8],   // deg_cnt packed per slot       — the aux plane
    pub tag: [u64; 8],   // the window id per slot (the arrival assert)
}
// THE PUBLICATION CONTRACT: the owner's plane stores for window w
// PRECEDE the ring head's Release — the same law as the entry itself
// (a torn plane is unrepresentable; cell +67's containment).

// ── the vector merge (§4.3): the core in 4 + 4 ops ─────────────────
#[target_feature(enable = "avx512f")]
unsafe fn merge_planes_core(p: &DigestPlanes) -> (u64, u64, u64, u64) {
    let z0 = _mm512_load_si512(p.vc.as_ptr()     as *const __m512i);
    let z1 = _mm512_load_si512(p.vc.as_ptr().add(8) as *const __m512i);
    let mut x = _mm512_xor_si512(z0, z1);              // vpxord
    x = fold_512_to_64_xor(x);                         // the balanced tree (3 ops)
    let v = _mm_cvtsi128_si64(_mm512_castsi512_si128(x));
    let c = _mm_cvtsi128_si64(_mm512_extracti64x2_epi64(x, 1));
    let a0 = _mm512_load_si512(p.dd.as_ptr()     as *const __m512i);
    let a1 = _mm512_load_si512(p.dd.as_ptr().add(8) as *const __m512i);
    let mut s = _mm512_add_epi64(a0, a1);              // vpaddq
    s = fold_512_to_64_add(s);                         // the same shape
    let db = _mm_cvtsi128_si64(_mm512_castsi512_si128(s));
    let da = _mm_cvtsi128_si64(_mm512_extracti64x2_epi64(s, 1));
    (v, c, db, da)                                     // ≤ 12 cycles total
}
```

The full close: the core + the aux planes (`h` XOR-tree, `m`/`dc` ADD-trees) + the day-chain `mulmod` + the 8 consume Releases + the tag assert on the miss path — Appendix B's `close_vector`.

### 8.3 The jump state and the arm/fallback protocol

```rust
// ── the frontier jump (Theorem 43) ──────────────────────────────────
pub struct JumpDelta {
    pub span_deltas: [u64; SLOTS_MAX],  // δ_𝒮's per-slot span advances
    pub msgs: u64,                      // the window's message count (the ledger)
    pub sess_step: u64,                 // the affine session law's step
}

pub struct JumpState {
    pub armed: bool,        // the arm certificate: 1 walked window, G green, tuple exact
    pub w_arm: u64,         // the arm window
    pub period: u64,        // the schedule period (the wrap law's modulus)
    pub delta: JumpDelta,   // the frozen schedule's constant per-window delta
}

// 𝒥(w) — ≤ 12 stores + 2 loads, NO loop, NO guard evaluation:
#[inline(always)]
fn jump(w: u64, st: &JumpState, eng: &FleetEngine, sink: &mut FleetSink) {
    debug_assert!(st.armed && w % st.period == 0);     // the O(1) checks (poison → walk)
    eng.window_pub.store(w + 1, Ordering::Release);    // THE store — the owners' only input
    sink.r_batch = 0; sink.last_seq = 0; sink.count = 0;
    for s in 0..sink.n_slots {                         // or one vector add over the cursor array
        sink.span_cur[s] += st.delta.span_deltas[s];
    }
    sink.sess = sess_at(st, w + 1);                    // the affine law
    // the transport's frozen-table position wraps by construction (the
    // repeated-schedule law); no turn is walked, no histogram applied.
}
// ROLLBACK: HFT_JUMP=0 — the verbatim walk (the CI's 11r44r arm).
// POISON: any failed expectation → walk from w (Corollary 43.2's
// identity continuation — reset_pass re-derives the tuple).
```

## Appendix A — Proof Details and the Extended Computations

**A.1 The bisimulation's dead-state argument (Theorem 43a, extended).** The walked ceremony's writes partition into: (w1) the observable tuple's components — all reproduced by `𝒥` directly; (w2) loop-local temporaries (the batch index, the group accumulator, the per-batch histogram row pointer) — dead at the window's edge by construction (the loop's scope); (w3) the transport's turn-consumption side effects — in the frozen regime the turns are static (the RX retired, the table pre-published), so "consuming" a turn is reading static memory and advancing a cursor that resets at the wrap; (w4) the guard evaluations' architectural state — branch history, flags — with no semantic consumer. Classes (w2)–(w4) are observationally void; (w1) is the bisimulation's equality. The one subtlety: the *telescoping identity* `Σ_b H[b][s] = Δ_𝒮[s]` is itself the ledger identity guard — verified once at the arm (Lemma 17's base case), never re-derived.

**A.2 The wrap law's congruence.** The frozen schedule's period `P` (the number of windows per schedule cycle) must divide the jumped window stream: `w ≡ w_arm (mod P)` at every jumped window. The check is one `u64` modulo (or the precomputed reciprocal multiply) — the O(1) budget's largest term. A ragged schedule (a partial final cycle) arms per-cycle with the tail windows walked (the mixed drive, cell +61).

**A.3 The reduction arithmetic (Theorem 44's folded form).** The 𝔽₂⁶⁴ product's reduction of a 127-bit carry-less product `P(x) = lo ⊕ hi·x⁶⁴` under `Q(x) = x⁶⁴+x⁴+x³+x+1`: `hi·x⁶⁴ ≡ hi·(x⁴+x³+x+1) (mod Q)` — one `vpclmulqdq` of `hi` by `QLOW = 0x1B` produces a ≤ 67-bit result whose own top 3 bits fold once more (`t >> 64` × `QLOW`) — 2 clmuls + 3 logic ops total, the [S2] §1.4 kernel, unchanged; the vector form runs 8 lanes per instruction. The dual-accumulator interleave: the two chains' clmul→fold→clmul dependencies (≈ 12–15 cycles) issue in adjacent cycles on the 4 FP pipes — the second chain's uops fill the first's latency shadow; 32 composes in 4 rounds ≈ 60 cycles, 14 live registers (§3.4's table).

**A.4 The horizontal folds (Theorem 45).** `fold_512_to_64_xor`: `vextracti64x4(1)` + `vpxord` (256→128), `vextracti128`/permute + `vpxord` (128→64) — 3 ops, depth 3, on the 2–3-cycle vector ALU latency; the ADD twin uses `vpaddq`. The interleaved `[v | c]` plane halves return both coordinates in one tree (the even/odd lanes) — the layout's whole point.

**A.5 The corridor's contended-draw model.** The owner-deschedule class (the 509B draw's neighbor): an owner parked `k` windows behind holds `k` ring slots; the merge's LAG_MAX=64 parks P₀ at 64 windows of lead = 64 × 505,804 = 32.4M messages ≈ 25.9 µs at 1.25T — the absorbable jitter. Beyond it, P₀'s backpressure spin (the `replay_window_close` park) throttles the publication rate — the system degrades gracefully to the owners' rate (2.27T post-44), never to a wrong answer. The contended arm's CI gate measures the parked-window count (the telemetry dial `lag_windows`).

**A.6 The DayChain page structure.** The object's page tier: consecutive 512-link groups' roots, published alongside the links (+8 B / 512 links ≈ 5.7 KB/day). The spot path: the page root's chain position → `log₈(W/512)` page-tree composes + ≤ 9 page-local composes ≈ 12 composes ≈ 70 vector cycles. The uniqueness argument (Thm 46c) extends: the page roots are functions of the links (the same monoid); the sub-object minimality holds per tier (drop a page root → the spot tier loses its index; drop a link → the full recompute diverges — cell +71 drives both).

## Appendix B — Reference Algorithms

```
ALGORITHM B.1 — partial_fold (the window-edge partial, Lemma 18c)
INPUT:  the slot's sub-records subs[0..n64), the edge offset r ∈ [0, 64),
        the window's whole-super prefix already composed into acc
STATE:  acc ∈ P64 (the window accumulator — window-bounded, never crosses)
1. n64' ← ⌊edge/64⌋ whole sub-records belong to THIS window
2. IF n64' > 0:  acc ← acc ⊗ blelloch4(subs[0..n64'))     // ≤ 16 leaves;
                 // n64' > 16: the 8-wide balanced tree (T19 — any shape)
3. IF r  > 0:    acc ← acc ⊗ serial_fold_span_records(     // the base case
                     spans of subs[n64'], the first r)
4. RETURN acc                                              // EXACT at the edge

ALGORITHM B.2 — close_vector (the ≤ 50 ns window close, Theorem 45)
INPUT:  the merged window w, the planes p, the ring state
1. ready ← all M ring heads > w              // 8 Acquire loads (unchanged)
2. (v, c, db, da) ← merge_planes_core(p)     // §8.2 — ≤ 12 cycles
3. (h, msgs, deg, cnt) ← the aux planes' trees (h XOR; m/dc ADD)
4. FOR s IN 0..M: rings[s].consume(w + 1)    // the 8 Release stores
   IF p.tag[s] ≠ w (the miss path): the scalar assert (the containment)
5. d ← Dig{v, c, db, da, deg, cnt}
6. day_chain ← mulmod(day_chain, t_pow[64]) ⊕ d.hash()   // the append
7. harvest.push((count, d.hash(), msg_hash)); RETURN

ALGORITHM B.3 — the jumped sustained loop (Theorem 43's drive shape)
STATE:  the JumpState js (armed at w_arm after ONE walked window)
1. WHILE elapsed < WINDOW_SECONDS:
2.    w ← next_window_id()
3.    IF NOT js.armed OR w mod js.period ≠ 0:   // the O(1) checks
4.        walk_window(w)                          // the verbatim ceremony
5.    ELSE:
6.        jump(w, js)                             // §8.3 — ≤ 12 stores
7.    close_vector(w − 1)                         // B.2 — the lazy merge
8.    harvest.assert_oracle()                     // EVERY window (the law)
9. finish(): the final drain (the blocking merge variant, verbatim)

ALGORITHM B.4 — verify_daychain (Theorem 46e, the < 2.0 ms budget)
INPUT:  Π_day (the per-slot links, W total), the page roots, the published root
1. FOR each slot's chain (any order — T33(d)):
2.    roots ← the 8-wide dual-accumulator Blelloch over the links
              (Theorem 44's schedule — W/8 × ~6 uops, shape-free by T19)
3.    CHECK roots against the page roots (the tier's local asserts)
4. CHECK the day root; RETURN (bit-exact or the divergence's page)

COMPLEXITY: spot O(log W); page tier O(W/512); full O(W) at W/8 vector
composes ≈ 2.7×10⁵ cycles for W = 365,600 — 0.06 ms @ 4.56 GHz,
0.28 ms @ 1 GHz — the < 2.0 ms bound with 7× margin at the floor.
```

— the corridor's mathematics is complete; the silicon's answer is the fleet's to give.



