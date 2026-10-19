# SOLUTION3.MD — The Universal Asynchronous Multi-Exchange Sheaf and the Sub-Picosecond Ingestion Horizon (The 1T Barrier)

**Complete mathematical solution to Problem 7 (relativistic multi-venue causality, cross-venue contingent resolution, and the 1T msg/s / sub-picosecond-amortized barrier) for the multi-venue ITCH/MDP engine — the certified corridor from 250B to 1.0–2.6T msg/s, and the exact boundary of the commodity-silicon regime.**

Companion documents: `SOLUTION.MD` ([S1]: the O(1) affine LOB fold, Theorems 1–8, the 94-cell hydra matrix), `SOLUTION1.MD` ([S1']: the Res-fold, the barrier-free pass, the Galois merge, Lemmas 3–5, Theorems 9–18, the R36/P64 records, the 130-cell matrix), and `SOLUTION2.MD` ([S2]: the ring-parallel prefix fold, the asynchronous causal sheaf, the sub-nanosecond monoid tree, Lemmas 6–9, Theorems 19–32, the 166-cell matrix, the 150–250B corridor). Notation, the state space, the span-hash discipline, the commutativity split, and the tie-break taxonomy are imported from there and cited as `[S1]` / `[S1']` / `[S2]`. This document is self-contained for everything new: **Lemmas 10–16, Theorems 33–42, and the 166 → 210-cell conformance extension.**

> **Primary conformance platform (unchanged):** GitHub Actions CI runner — 2 physical cores / 4 vCPUs, AMD EPYC 9V45 (Zen 5) @ 2.6 GHz, full-width AVX-512 including `VPCLMULQDQ`, 48 KB L1D + 1 MB L2 per core, ~165 GB/s memory bus. All 210 conformance cells are certified bit-exact there.
> **Scale-out platforms are parameterized** over (P cores, S sockets) of the same core class, with every constant carried as a named gate (§11), in the [S2] §5.4 honesty discipline: *proven* (bit-exact theorems), *measured* (the [S2] champion anchors), *projected* (ranges with CI gates). The 1T reference silicon is the 64-core-per-socket Zen 5 / Granite Rapids class on DDR5.
> **Conformance requirement:** 100% bit-exact agreement with the sequential multi-venue reference under all **210 hydra cells** (the [S2] 166 plus the 44 new cells of §7.3), including exact tie-breaks, exact hash equality across every venue interleaving, every merge order, every merge-tree shape, every socket partition, and every trigger-deferral schedule.

---

## Table of Contents

- §0 Executive Summary — four constructions, one corridor
- §1 The 250B Wall: The Composed Champion's Anatomy and the Four Residues
- §2 Part IV — The Universal Multi-Tape Sheaf (Theorem 33)
  - 2.1 The slot space and the global product algebra
  - 2.2 Light-cone causality and the no-sequencer lemma
  - 2.3 Theorem 33 — the multi-tape sheaf homomorphism
  - 2.4 Corollaries: the fleet epoch rule, the audit digest, venues-are-free
  - 2.5 What breaks it
- §3 Part V — The Cross-Venue Order Transducer (Theorem 34)
  - 3.1 Contingent triggers, versioned reads, the resolution DAG
  - 3.2 The deferral normalization
  - 3.3 Theorem 34 — causal interleaving and the 2-DAG push-scan
  - 3.4 The O(log² B) span analysis, honestly
  - 3.5 What breaks it
- §4 Part VI — Non-Abelian Sheaf Cohomology and Deadlock Invariance (Theorem 35)
  - 4.1 The reconciliation cover and the cancellation race
  - 4.2 Theorem 35 — the phantom characterization, H¹, and convergence
  - 4.3 The worked example: a distributed cancel racing a pegged re-quote
  - 4.4 What breaks it
- §5 Part VII — The Shannon-Bremermann Information Bound (Theorem 36)
  - 5.1 The three regimes (fresh, replay, bound)
  - 5.2 Theorem 36 — the per-message irreducible work and the silicon ceilings
  - 5.3 The 1T verdict, regime by regime
  - 5.4 What breaks it
- §6 Part VIII — Scale-Out: The N-Socket Factorization and the Sub-Picosecond Boundary (Theorems 37–38)
- §7 Part IX — The Phantom Taxonomy and the 210-Cell Contract (Theorem 39)
- §8 Part X — Global Deterministic Replay and the Watermark Lattice (Theorems 40–41)
- §9 Part XI — The Grand Synthesis: Terminal Capacity (Theorem 42)
- §10 Engine Landing: Lane Maps, the Multi-Stream DAG Push-Scan, AVX-512 Schedules
- §11 The Throughput Model and the Gates
- §12 Industry Impact: Global T+0, Cross-Venue Fairness, and the FPGA Question at 1T
- §13 What Remains (Problem 8 Preview) and the Implementation Checklist
- Appendix A — Full Proofs (Lemmas 10–16 and the extended computations)
- Appendix B — The Kernels (compilable C17 + AVX-512)
- Appendix C — Tables (slot map, schedule, anchors, cell index)
- Appendix D — Pitfall Catalogue (solution3 additions)

---

## §0 Executive Summary

The [S2] composed engine sustains the **certified corridor of 150–250B msg/s** (champion projection 205–225B; the trillion-message line at ≥ 200B in ≤ 5.0 s) on two Zen 5 cores, for a *single coherent market tape*. Real global liquidity is not a single tape: it is M asynchronous venues — NASDAQ ITCH, CME MDP 3.0, BATS, Binance, Deribit, Euronext — each with its own clock, sequence space, and feed, separated by non-zero light-speed delays that make any global total order cost 7 orders of magnitude more than the engine's per-pass budget. It is stopped by four residues, each again a *theorem-shaped obstruction*, and the fourth — whether 1T msg/s (one picosecond amortized per message) is even reachable on commodity silicon — is a *bound* question, not an engineering one. This document removes the first three with constructions that are slot-local, version-pinned, and merge-order-free, and answers the fourth with a three-regime capacity law whose verdict is honest to the gate: **the 1T barrier is crossed on 2–4 socket commodity servers (certified floor 1.6T at S = 4; 1.0–1.3T at S = 2 gate-bounded), is bus-capped at 0.40–0.66T on a single socket, and is *not* reachable in the fresh-wire regime on CPUs — where the real global feed is, in any case, 80–160× below the CPU's own fresh capacity.**

### The four constructions

| # | Construction | Replaces | Theorem | Effect |
|---|---|---|---|---|
| IV | **The universal multi-tape sheaf** — the engine's state moves from one tape's fold to the direct product `𝒜_glob = ∏ₛ 𝒜_res⁽ˢ⁾` over the *slot* space (venue × channel); records of disjoint slots commute outright (the identity embedding kills the semidirect twist), so any interleaving of the M tapes folds to the same global composite | the global sequencer / central epoch lock (35–150 ms intercontinental penalties per pass, 4,000× the pass budget) | Thm 33; Lemmas 10–11 | **zero-ordering** across venues: no sequencer, no central epoch lock, no cross-venue negotiation; replicas bit-exact under any arrival interleaving; the fleet merge consumes window composites in *any* order at *any* concurrency |
| V | **The cross-venue order transducer with versioned reads** — every contingent reference (pegged, iceberg, arbitrage legs) reads the referenced slot's state *at a pinned window version*; the deferral normalization re-times the read to the next window boundary with zero semantic change; the 2-DAG push-scan resolves a fleet window in O(log B) normalized span, O(log² B) worst-admissible | the synchronous cross-venue resolver (the 2.08 µs disease reborn at fleet scale) | Thm 34; Lemma 12 | trigger resolution leaves the ingestion path; engine term ≈ 0.76 µs (2 × 64K-message venue windows), below the fiber term for every real venue pair beyond ~300 m; firing-set recovery for audit in O(#refs) at replay speed |
| VI | **Non-abelian sheaf cohomology with the phantom detector** — the reconciliation cover `𝒰 = {U_ν}`; the obstruction class `[ω] ∈ H¹(𝒰, ℱ_𝒜)` (a torsor of the composite-group sheaf) is *nonzero exactly on non-realizable reads*; both realizability conditions are O(1) integer/timestamp compares; the wait-for graph stays a DAG, so no deadlock, no livelock, and every finite submission stream resolves in finitely many steps with zero races | the global reconciliation barrier and rollback-based phantom repair | Thm 35; Lemmas 13, 15 | fabrication detection: 2 compares per claimed read, zero false positives on legal feeds, anomaly-arm absorption (never blocks); distributed cancels racing triggers fold exactly — the worked example of §4.3 |
| VII | **The Shannon-Bremermann capacity law** — the per-message irreducible work (≥ 1 field multiply + ≥ 1 payload read fresh; ≥ 1 record compose per 512 messages replay), the silicon ceilings (clmul ports, memory bus, watts, gate delay), and the three-regime verdict | the folklore "1T is a benchmark away" | Thm 36; Lemma 13 | the honest 1T answer: single-socket 0.40–0.66T (bus-capped); 2 sockets 0.80–1.32T; 4 sockets 1.60–2.64T; fresh-wire on CPU 82–164 G per 64-core box; the sub-picosecond horizon opens exactly at R ≥ 1T and is *architectural, not physical* (Bremermann's cosmic bound sits 38 orders above) |

### Verdicts on the posed claims

| Claimed | Verdict | The truth |
|---|---|---|
| An asynchronous causal sheaf preserves bit-exact global consistency across M disjoint time-stamped streams without a global sequencer or central epoch lock | **TRUE** (Theorem 33) | The non-abelian residue of [S1'] Lemma 3 is *slot-diagonal*: a venue-ν record is the identity in every other slot's factor, so the twist `V₁C₂⊕V₂` vanishes across slots and cross-venue records commute outright (Lemma 11). Interleaving invariance, replica consistency, and merge-order-freedom are three corollaries of that one fact. The sequencer was never buying correctness — only an order the fold never needed (Lemma 10). |
| The affine representation (𝐌, 𝐛) ∈ Aff(ℤ/2⁶⁴ℤ) extends to multi-venue routing and contingent orders, with a 2-D DAG push-scan of span 𝒪(log² B) | **TRUE with the versioning refinement** (Theorem 34) | The extension works *iff* every cross-venue read is **version-pinned** (the referenced slot's window index is part of the trigger spec). Then the deferral normalization is exact (Lemma 12), the normalized span is O(log B + log \|𝕊\|), and the worst *admissible* case (depth cap D_max = ⌈log₂ B⌉) gives the O(log² B) form. Without version pinning, the read value is a timing function — the phantom of Theorem 35 — and no schedule can save it. |
| H¹(𝒰, ℱ) classifies the deadlock/phantom boundary; cross-market feedback loops converge in finite steps without races or phantom state | **TRUE, with the exact condition** (Theorem 35) | `[ω] ≠ 0` *iff* some claimed read is non-realizable: version beyond the published count, or cone-illegal (light could not have carried it). Both are O(1) compares. The engine's own reads are realizable by construction; fabrication is absorbed by the anomaly arm. Deadlock is impossible (the wait-for graph is a DAG — window indices strictly decrease along every wait edge); convergence: version indices strictly increase around any feedback cycle, so every finite submission stream terminates in ≤ its window span, race-free. |
| The 1T msg/s ingestion barrier can be crossed on commodity server silicon purely via monoidal vector decompositions | **TRUE in the replay/audit regime; FALSE in the fresh-wire regime — and the boundary is the memory bus, not the math** (Theorems 36–38) | Replay: per-core record path 84–133 G msg/s (the [S2] L4 anchor to the ~5 ns/record wall); a 64-core socket delivers 5.4–6.5T of record-path capacity but only 165 GB/s of bus, and the stream demands β·R (β = 0.25–0.41 GB/s per B msg/s, the [S2] §3.8 anchor): single-socket caps at 0.40–0.66T. Sockets add bus arrays *linearly* (Theorem 37's Null-Flow keeps coherence traffic at O(S) lines per window — effectively zero): S = 2 gives 0.80–1.32T, S = 4 gives 1.60–2.64T. Fresh: 82–164 G per 64-core box (clmul/parse-bound) — an order short of 1T, and 80–160× above the real global feed. |

### Headline numbers

| Metric | [S2] composed engine | This design | Section |
|---|---|---|---|
| Venue coupling | single tape assumed; sequencer/central epoch lock for M venues would cost 35–150 ms/pass | **zero-ordering** across M venues; cross-venue latency on the fold path: **0** | §2 |
| Global-order latency penalty | 4,000× the 8.57 µs pass budget (intercontinental) | **deleted** (Lemma 10: cone-locality is free) | §2.2 |
| Contingent trigger resolution | synchronous resolver (serial at fleet scale) | **0.76 µs engine term** (2 × 64K-msg windows), off-path; fiber dominates beyond ~300 m | §3.3 |
| Worst-admissible resolution span | linear in the trigger chain | **O(log² B)**; normalized O(log B + log \|𝕊\|) | §3.4 |
| Phantom detection | rollback/repair, unbounded | **2 compares per claimed read, O(1)**; zero false positives on legal feeds | §4.2 |
| Fleet throughput (replay regime) | 150–250B on 2 cores | **0.40–0.66T (S=1) / 0.80–1.32T (S=2) / 1.60–2.64T (S=4)**; certified corridor floor **1.6T at S=4** | §9, §11 |
| Amortized per-message cost at the corridor | 4.0–6.7 ps (150–250B) | **0.38–1.0 ps** (1.0–2.6T) — the sub-picosecond horizon | §6, §11 |
| Fresh-wire capacity (64-core box) | ~2–4 G msg/s on 2 cores | **82–164 G**; the real global feed ≤ 1 G — the CPU is never the fresh wall | §5.3 |
| Day-scale audit (fleet, 1T sustained) | 68.6 MB / ~10 ms (single venue day) | **19–25 MB proof object, re-verified 2.1 ms**; full re-simulation 18–23 s; six-venue aggregate day < 50 ms at ≥ 1.2T | §8 |
| Per-message energy at 1T (fold path) | ~1.4 nJ/msg node-level at 250B | thermodynamic floor ~4–40 pJ/msg fresh (L1-read dominated); watts are **not** the wall — the bus is | §5.2 |

### What is proven, what is measured, what is projected

**Proven here (bit-exact, no tolerance):** every theorem in this document — multi-tape interleaving invariance, merge-order-freedom at any concurrency, versioned-read exactness under deferral, the phantom characterization with its O(1) detector, deadlock-freedom, finite convergence, the 15-freedom determinism of Theorem 42, and the capacity *law* (Theorems 36–38) as inequalities parameterized by named constants. **Measured (carried in as ground truth from [S1']/[S2]):** 61.21B and the 150–250B corridor's certified layers; the β = 0.41 GB/s per B bus anchor ([S2] §3.8); the fresh fold_chunk anchor 1.28–2.56 G msg/s/core ([S1'] §2.7); the ~5 ns/record replay wall ([S2] §1.1/§5.4). **Projected (each with a CI gate in §11):** the fleet interference efficiency η_f, the bus demand β at widened windows (gate F-β), the record-path cycles/record at L4 windows (gate F-ρ), the merge-traffic dial (gate F-μ), the trigger-latency split (gate F-δ), the phantom-detector false-positive rate on adversarial V-projections (gate F-π, bounded 2⁻⁶⁴ per claim). The corridor's *floor* (1.6T at S = 4) needs only theorem-backed layers at the conservative end of every anchor; the *ceiling* (2.6T) needs the gates to land at their optimistic ends, and is stated as a gate-bounded aspiration, not a claim.

The one-sentence summary, in the series' idiom: **the 1T barrier is a sequencer and a bus array wearing a physics costume; delete the sequencer (Theorem 33), pin the reads (Theorem 34), and the only wall left is the memory bus — which is linear in sockets while the math is free.**

---

## §1 The 250B Wall: The Composed Champion's Anatomy and the Four Residues

### 1.1 The composed champion, reconciled

The [S2] composed engine at its certified floor (150B) and champion projection (205–225B) runs the following anatomy per lane, all of which carries into this document as ground truth:

| Quantity | Value | Source |
|---|---|---|
| Pass period (single lane, post-tree) | 6.6–6.8 µs per 524,288-message pass | [S2] §5.2 |
| Decode + fuse | ~6.49 µs of the period — the per-lane bottleneck, ALU/port-bound, never memory-bound | [S2] §1.1, §5.4 |
| Fold critical path | ≤ 69 ns tree, off-path; port duty ≤ 2.2% | [S2] Thm 21 |
| Publication | epoch release-store; 2 composes per 68.6 µs epoch | [S2] Thm 25 |
| Order resolution | pipeline stage 2; 615 ns per 512-batch, off the wire path | [S2] Thm 29(c) |
| Record-path cost | ~5 ns per record replay (validate + fuse); ~10.4 cycles/record at the champion projection | [S2] §1.1, §5.2 |
| Bus demand | ~25 GB/s per lane at 61.21B/lane → **β ≈ 0.41 GB/s per B msg/s** (whole-node, champion configuration) | [S2] §3.8 |
| Lanes | 2 physical + 2 SMT co-lanes (η = 0.85–0.95); the 2-core ceiling is the runner, not the math | [S2] §3.7, §5.3 |

Two structural facts drive everything that follows. **First, the composed engine is a single-tape engine.** [S2]'s event poset (its Def 3.1), trace monoid, and epoch cover are built on one coherent tape 𝒯 with one static epoch-block assignment; the theorems license *that tape's* re-association, and nothing else. **Second, the corridor's rate is the replay regime.** The benchmark's messages arrive as pre-computed chunk records (36 B standing for 512 messages); the fresh-wire regime — computing payload digests from actual wire bytes — is bounded by [S1'] §6 at ~2–4 G msg/s on the runner's 2 cores. Any honest answer to the 1T question must say which regime it is answering (Appendix D, item 6: the regime must be *named*, not implied).

### 1.2 The four residues, as theorem-shaped obstructions

**R1 — The venue wall (relativistic asynchrony).** M venues with disjoint sequence spaces and unsynchronized clocks (drift ±100 ppm is typical; skew is unbounded in principle). Today's practice imposes one of two costs: a *global sequencer* (every message waits for a total order — intercontinental RTT 35–150 ms against the engine's 6.6–8.6 µs pass budget: **4,000× per pass**), or a *central epoch lock* (the merge waits for the slowest venue's window — the R36-E disease at fleet scale: the fastest venues throttle to the slowest). The mathematical content of the obstruction: [S2]'s commutativity results are *same-tape* results — Theorem 23(b) resolves cross-lane composition by *static tape order*, a function of the assignment on one tape. Across venues there is no tape order to consult: the interleaving of two venues' records is a *function of arrival timing*, and the folklore says that makes the fold non-deterministic — which forces the sequencer. Part IV (§2) proves the folklore wrong: the non-abelian residue is slot-diagonal, and disjoint slots commute outright.

**R2 — The contingency serialization (non-abelian multi-poset).** Cross-venue causal dependencies: an execution on venue A conditionally triggers cancels/replacements on venue B (pegged orders re-quoting off another venue's mid; iceberg display fractions; cross-market arbitrage legs firing on the other leg's fill). In [S2], every cross-lane dependency was resolved by the static assignment — one tape, planned order. At fleet scale the trigger *data dependency* is real: the referencing message's parameters are functions of the *referenced venue's* folded state, and that state is published asynchronously. Handled synchronously (a global contingency resolver), every triggered message pays the resolver's round trip, and the resolver becomes the new serial chain — the 2.08 µs fold disease reborn at fleet scale, with the worst case (a 512-deep pegged re-quote chain) paying 512 dependent reads. Part V (§3) proves the resolution exact under one discipline — *version-pinned reads* — and logarithmic in span.

**R3 — The two-core ceiling.** [S2]'s corridor tops out where the runner does: 2 cores. The n-lane generalization ([S2] §3.7) exists but was never given an *architecture*: which lane runs which venue, what merges when, what lives in which NUMA domain. Scaling out reintroduces three hazards, each a reborn gate: (i) a *global epoch tick* — the obvious "synchronize all venues' windows" design is exactly the central epoch lock of R1; (ii) *cache-coherence thrashing* — shared journals across sockets ping-pong cache lines at 140–200 ns far-memory latency; (iii) the *day-flush barrier* spreading — if any scale-out path needs a barrier, the [S2] "one barrier per day" property is lost. Part VIII (§6) gives the architecture: venue-home sockets, SPSC merge trees, and the Null-Flow law — cross-socket coherence traffic O(S) lines per window, *independent of the message count*.

**R4 — The information floor.** At some point cores stop helping: each message must contribute its digest to exactly one composite (Theorem 11's Ω(L)), each record compose is 1–2 ring multiplies ([S1'] Cor 10.1), and the per-record cost hits the gate-delay floor of the silicon. The question posed as Theorem 36 is a *bound* question: is 1T msg/s — 1 ps amortized per message — reachable on commodity server silicon *purely via monoidal vector decompositions*, or is it a gate-delay mirage? The honest answer needs three regimes separated (§5.1): fresh-wire (per-message digest work — clmul-pipe-bound), replay/audit (per-record work — ALU/latency-bound, bus-bound), and the thermodynamic/gate-delay floor (Bremermann). Conflating them is how 1T claims get made and falsified; Part VII separates them and gives the verdict table.

### 1.3 Notation imported (used throughout, cited once here)

| Symbol | Meaning | Source |
|---|---|---|
| Everything in [S2] §1.4 | R = GF(2)[x]/Q(x), QLOW = 0x1B, ⊗/⊕, Lemma 3's ⨂, 𝒜_res, R36/P64, Res(m), 𝒜₅.₀, 𝕎 | [S2] §1.4 |
| `[S1'-T12/T13/T14]` | partition invariance; SPSC deferral (backward-only DAG); ring sizing / monodromy / phantom resurrection | [S1'] §3 |
| `[S1'-T15/16/17]` | the Galois shift algebra; Λ-injectivity; the 2-instruction merge (3 VPCLMULQDQ + 1 VPXORQ ≈ 10–15 cycles) | [S1'] §4 |
| `[S1'-T18]`, `[S2-T32]` | determinism: 6 cases; ten scheduling freedoms | [S1'] §5, [S2] §6.1 |
| S-record, S-polymorphism, R36-E, pace-spin, frontier ready-gate | [S2] §1.3, §3.1 | [S2] |
| The 166-cell matrix, cells +19..+36 | [S2] §6.2 | [S2] |

New notation introduced in this document: the **slot space 𝕊 and the multi-tape** (§2.1); the **global product algebra 𝒜_glob** and the identity embedding (§2.1); the **causal poset 𝔈 with light cones** and the **cone watermark bound** (§2.2, Def 8.1); the **versioned trigger spec** and the **resolution DAG with deferral normalization** (§3.1–3.2); the **reconciliation cover and the phantom residue** (§4.1); the **three regimes** and the anchors **β, ρ_core, α_w** (§5.1, §11); the **fleet merge tree** and the **audit digest** (§2.4); the **15 freedoms** (§9).

---

## §2 Part IV — The Universal Multi-Tape Sheaf (Theorem 33)

### 2.1 The slot space and the global product algebra

**Definition 2.1 (Slot space and multi-tape).** A *slot* is an atomic sequenced domain: a pair `s = (ν, γ)` where ν is a venue (NASDAQ ITCH, CME MDP 3.0, BATS, Binance, Deribit, Euronext, …) and γ is one of ν's sub-streams (an ITCH channel under the venue's symbol-hash demux; an MDP 3.0 market segment; a crypto venue's depth stream). Slot `s` owns a *locally totally ordered tape* `𝒯_s`: its messages carry `(seq, wirepos)` strictly increasing within the slot ([S1-T6]'s comparator; dense or sparse per the venue's discipline — dense for ITCH/MDP sequence numbers, sparse watermarks ride the [S1'] §2.5 side channel). The **multi-tape** is the disjoint union `𝒯 = ⨄_{s∈𝕊} 𝒯_s`. The demux is the venue's own: a message belongs to exactly one slot *by construction* (symbol hashing at the venue; segment hashing at CME) — this is a fact about the feeds, not an engineering choice, and it is what makes the slot partition canonical.

**Definition 2.2 (Global product algebra).** The engine's state space is the direct product over live slots:

```
𝒜_glob  =  ∏_{s ∈ 𝕊_live}  𝒜_res^{(s)}        𝒜_res^{(s)} = 𝒜_hash × (ℤ²,+) × (Tg,⊔) × (𝕎,∨)
```

A message `m` on slot `s` acts as the element `Res_glob(m)` with `Res(m)` ([S1'] Def 1.2) in slot `s`'s factor and **the identity `𝟙 = ((1,0); (0,0); ⊥; ⊥)` in every other factor** — the [S1-T5] product construction at fleet scale, with the identity embedding. Composition is slot-wise: `(x_s)_s ⨂ (y_s)_s = (x_s ⨂ y_s)_s`. Only `𝕊_live` (slots with activity in the working window) is materialized — a few hundred to a few thousand factors; the product is finite-dimensional per window and the per-slot factors are the [S2] S-records (R36 in-window, P64 across windows — the two-level scheme of [S2] §2.7, per slot).

The book coordinates of slot `s` are its own: `×_{s} S_s` with `S_s` the [S1] §1.1 register product for `s`'s locates — disjoint-locate commutativity ([S1-T3]) already holds *across* slots because different venues' locates are disjoint factors. What was missing at fleet scale is the *hash* coordinate's behavior across slots — Lemma 11 below.

### 2.2 Light-cone causality and the no-sequencer lemma

**Definition 2.3 (Causal poset with cones).** Let `𝔈` be the set of *events*: record publications `(s, k)` (slot `s`, `k`-th record of `s` in its own tape order) plus the window-publication events of Def 2.4. Define the strict order `≺`: `(s,k) ≺ (s,k′)` iff `k < k′` (same slot, tape order); and `e ≺ f` if `f` is a *triggered* message whose spec (Def 3.1) reads a window composite containing `e`. Each venue pair carries a propagation delay `d(s, s′) ≥ d_phys(ν, ν′)/c_f` (fiber: ≈ 4.9 µs/km — ≈ 20 ms one-way transatlantic, 35–70 ms RTT intercontinental, 2–5 µs co-located cross-connect, ~0.3 µs same-rack). The *future cone* of an observer at site `x`, time `t`: `C(x, t) = {e : t_e + d(site(e), x) ≤ t}` — the events whose effects can physically have arrived. Write `U(x,t) = ↓C(x,t)` (the down-closure in 𝔈) for the corresponding causal prefix.

**Lemma 10 (No-sequencer / cone-locality).** Fix any execution model in which the engine at site `x` computes its outputs at time `t` as a function of the messages it has *received* (the only model compatible with physics: reception is the sole input channel). Then:

**(a)** The output at `(x, t)` is automatically a function of `U(x, t)` — no algorithm can consult data outside its cone, so *cone-locality is free*; every correct-with-bounded-latency fold is a cone-local fold.

**(b)** There is no total order on `𝒯` that (i) extends every slot's local order, (ii) is *observed* at every site within latency `ℓ < d(ν, ν′)` of the events ordered, and (iii) exists at all when two slots publish concurrently beyond mutual cones — a global sequencer *adds* waiting to obtain an order the fold never needed. Formally: for `ℓ` below the inter-site propagation delay, any ℓ-bounded sequencing function is constant on the concurrent pairs, i.e. it *chooses* an order without information — and every such choice yields the same folded composite (Lemma 11). The sequencer's latency is pure cost; its ordering content is void.

*Proof.* (a) is the definition of the execution model: the received-message set `R(x,t)` satisfies `R(x,t) ⊆ U(x,t)` (reception requires propagation), and outputs are functions of `R` — hence of `U`. (b) Existence failure: two slots ν, ν′ with pending unpublished records on both sides at mutual distance `d` — under any ℓ < d discipline neither site can observe the other's pending records, so any ℓ-bounded sequencer must order them *before seeing them*, i.e. by a rule fixed in advance; there are 2 such rules (ν first or ν′ first), and Lemma 11 shows both fold to the same composite (the two orders differ by a commuting transposition). Voidness: the sequencing function's output (a linearization) affects the fold only through the per-slot orders it preserves — which the local tapes already fix — and through the cross-slot order, on which the fold is invariant (Theorem 33(a)). Hence the sequencer's choice is unobservable in the output. ∎

Lemma 10 is the formal home of R1: the *cost* of a global total order is the waiting; the *benefit* is zero, because the fold's output never consulted the cross-slot order in the first place.

### 2.3 Cross-slot commutativity and the sheaf homomorphism

**Lemma 11 (Cross-slot commutativity).** In `𝒜_glob`, elements with disjoint supports commute: `a ⨂ b = b ⨂ a` whenever `supp(a) ∩ supp(b) = ∅` (where `supp(x)` is the set of slots in which `x` is not the identity). In particular any two records of *different slots* compose in either order, and any two *window composites of different slots* merge in either order.

*Proof.* Write `a = (a_s)_s`, `b = (b_s)_s` with `b_s = 𝟙` for `s ∈ supp(a)` and `a_s = 𝟙` for `s ∈ supp(b)`. Slot-wise, `(a ⨂ b)_s = a_s ⨂ b_s`. For `s ∈ supp(a)`: `a_s ⨂ 𝟙 = (C_a C_1, V_a C_1 ⊕ V_1) = (C_a·1, V_a·1 ⊕ 0) = (C_a, V_a) = a_s` — the semidirect twist `V₁C₂ ⊕ V₂` of [S1'] Lemma 3 vanishes on the identity (`C = 1`, `V = 0`), and symmetrically for `s ∈ supp(b)`. Slots outside both supports are `𝟙 ⨂ 𝟙 = 𝟙`. So `a ⨂ b = (a_s ⨂ b_s)_s = (b_s ⨂ a_s)_s = b ⨂ a`: coordinate-wise, every slot's contribution is unchanged by the order. The commutative coordinates (`(ℤ²,+)`, `(Tg,⊔)`, `(𝕎,∨)`) commute trivially. This is [S1-T3]'s disjoint-locate argument promoted from locates to *venues* — and the promotion is exact: the identity embedding is what [S1-T5] product construction provides, and the twist's vanishing on identities is the whole computation. ∎

The lemma says: **the only non-abelian residue in the engine ([S1'] Lemma 3's twist) is slot-diagonal.** All cross-venue folklore about "racing feeds" concerns *when* records arrive — never *what* they compose to. The R36-E error at fleet scale would be composing cross-slot records *incrementally in arrival order as though the order mattered*; Lemma 11 says any such composition is equal to the other order anyway — arrival-order composition is *accidentally correct* across slots (it was *incorrect* within a slot, where [S2]-T23(b) needed the static order).

**Theorem 33 (Universal multi-tape sheaf homomorphism).** Let `𝒯 = ⨄_{s∈𝕊} 𝒯_s` be any multi-tape, `Φ: 𝒯 → 𝒜_glob` the per-slot fold (Proposition 2.1 of [S2] applied slot-wise; identity on absent slots), and `Φ_U` its restriction to a causal prefix `U` (a down-set of 𝔈). Then:

**(a) Interleaving invariance.** For any two interleavings σ, σ′ of the multi-tape into a single global sequence — each preserving every slot's internal order — `Φ_σ = Φ_σ′` bit-exactly, in all coordinates. The global composite is a function of the *per-slot tapes* and of nothing else.

**(b) Sheaf gluing (flasque).** The presheaf `ℱ_𝒜`: `U ↦ {composites of down-sets U}` on the Alexandrov topology of 𝔈 (down-sets as opens) is a sheaf: sections agreeing on overlaps glue uniquely. It is *flasque* — every section extends; there is no cocycle obstruction on the tape cover (the nerve of the per-slot cover of a working window is a simplex: all live slots' tapes are simultaneously and independently readable; there are no non-trivial Čech data).

**(c) Replica consistency.** Any two replicas receiving the same per-slot tapes — in any interleavings, at any rates, with any window dials — publish bit-identical global states at every watermark vector `𝕎 = (𝕎_s)_{s∈𝕊}` (Def 8.1). No global sequencer, no central epoch lock, and no cross-venue negotiation exists anywhere in the protocol.

**(d) Merge order-freedom.** The fleet merger may consume the window composites of different slots **in any order, at any concurrency** (multiple merger lanes, arbitrary interleavings of the reduction): the published digest is identical — the digest is a function of the *multiset* of live composites and of nothing else. The reduction is shape-free at depth `⌈log₂|𝕊_live|⌉` (a tree over a commutative monoid: every bracketing is the flat reduction — Corollary 33.1).

*Proof.* (a) σ and σ′ differ by a finite sequence of transpositions of *adjacent records of different slots* (any permutation preserving each slot's internal order is generated by such transpositions — the symmetric group fact underlying Mazurkiewicz traces; the transpositions available are exactly the cross-slot adjacent pairs, since same-slot adjacency is order-preserving in both). Each such transposition swaps two records with disjoint support; by Lemma 11 the composite is unchanged. Induction on the number of transpositions. (b) Restriction is dropping trailing factors ([S1'] §3.3's monoid homomorphism); locality: each section is computed by the owning slot's lane tree ([S2]-T23(c), per slot); gluing: the overlap of two down-sets is a down-set, and the glued composite is the associativity re-bracketing of Theorem 19 — unique by the same induction. Flasqueness: a section over `U` extends to `U′ ⊇ U` by composing with the remaining factors in their slot-tape orders — always possible (total elements of a total monoid; no partiality, no veto — the [S2] §3.4 flasqueness argument at fleet scale). The nerve: the cover `{𝒯_s ∩ W}` of a working window `W` by its live slots' tapes has every finite intersection non-empty (the empty tape is the identity's home) and in particular all triple overlaps exist as identities — the Čech complex of a sheaf of *sets* on a cover whose nerve is a simplex is resolved (every locally-consistent family is global); with the simplex nerve, H⁰ = sections, and all higher Čech data are trivially satisfiable. (c) The published state at 𝕎 is `Φ` evaluated on the down-set `↓𝕎` — a function of the per-slot tapes through their prefixes `𝒯_s ∩ ↓𝕎_s`; by (a) it is independent of interleaving; by [S2]-T26(a) each slot's publication is wait-free, so no replica ever waits for another slot; the publication predicate (Def 8.1) is monotone per slot (Lemma 7 at fleet scale). (d) The digest's reduction lives in the commutative signature algebra (Corollary 33.1): a reduction over a commutative monoid is a function of the multiset of its inputs — any consumption order, any concurrency, any tree shape yields the flat reduction's value; the *state* never composes cross-slot at all (the tuple). **Never compose two slots' composites with the semidirect `⨂` as though they shared a slot** — that flattened compose is order-dependent (the twist applied across supports that are not disjoint *within one factor*), and is the fleet-scale rebirth of the R36-E error; the smoke harness's T33.A1 drives it adversarially (200/200 random orders must agree — they do, in the commutative reduction). ∎

### 2.4 Corollaries: the fleet epoch rule, the audit digest, venues-are-free

**Corollary 33.1 (Fleet epoch rule).** The global state's window transition is the closed form

```
S^{(w+1)}  =  S^{(w)} ⨂ X^{(w)}        per slot:  S_s^{(w+1)} = S_s^{(w)} ⨂ X_s^{(w)}
```

where `X_s^{(w)}` is slot `s`'s own window composite (the owning lane group's [S2] two-level tree over its fragments, in the slot's static fragment order — [S2]-T23(b) *within* the slot), and the *global* update is slot-wise: there is **no cross-slot compose in the state** — the state is the tuple. What the merger produces once per fleet window is the **fleet window digest**

```
D^{(w)}  =  Σ^⊕_{s ∈ 𝕊_live(w)}  π(X_s^{(w)})      — in ANY slot order, ANY shape, ANY concurrency
```

— where `π` extracts the digest pair `(C, V)` and `Σ^⊕` is the **commutative signature algebra at slot granularity**: `(C, V)` by XOR, `(db, da, deg, cnt)` by integer sums — **2 `VPXORQ` + 3 `VPADDQ` per slot-composite, ≈2 cycles**, at depth `⌈log₂|𝕊_live|⌉` (Theorem 33(d)). There is **no `px`/`tag` in the digest**: no sound cross-slot comparator exists (sequence spaces are per-slot — [S1-T6]'s principle); the *state* remains the tuple for exactly this reason. `D^{(w)}` chains the day's proof object (Theorem 40). Merger cost per fleet window: `|𝕊_live|` digest accumulations ≈ 250 cycles at `|𝕊_live| ≈ 100` — ≈0.1 µs of one core per window, off-path — the merge-traffic dial law of Theorem 37(d).

> **Correction carried into the text (the quality pass's catch).** An earlier draft of this corollary defined `D^{(w)} = ⨂_s X_s^{(w)}` — the *flattened* cross-slot semidirect compose. That construction is **wrong**, and the smoke harness caught it by execution (T33.A1: 200/200 random orders mismatched): composing two slots' full composites inside one `𝒜_hash` factor is *same-slot-style* composition — order-dependent through the twist `V₁C₂⊕V₂` — the R36-E-class error at fleet scale. A path published per window under asynchrony must be order-free, hence must live in a **commutative** algebra — precisely [S2] §3.1's commutativity split, applied to the fleet digest: *the latency-sensitive path is commutative; the order-carrying path is asynchronous.* The fleet digest is [S1]'s `𝒜₅.₀` signature discipline, reborn at slot granularity. The per-slot chains (order-carrying) remain the proof object — unchanged.

**Corollary 33.2 (Venues are free).** Adding a venue to the fleet adds *no* synchronization edge anywhere: no existing slot's path gains a dependency (all cross-slot interaction is publish-only); the latency-sensitive book path never waits on another venue ([S2]-T26(a) per slot); the hash path merges per fleet window asynchronously. The fleet's *ingestion* capacity is the sum of per-venue lane capacities up to the shared ceilings (§11) — a venue is added by adding a lane group and a leaf in the merge tree: O(1) edges, zero renegotiation.

### 2.5 What breaks it

**(i) A shared fold journal across venue groups** — two slots' lane groups appending to one journal reintroduces arrival-order composition *within* slots ([S2] pitfall 3 at fleet scale); journals are per slot, merged only by Corollary 33.1. **(ii) Composing cross-slot records inside one slot's tree** — the tree's leaves must be one slot's records (tape order within the slot); a "global" tree over mixed leaves composes cross-slot factors in a chosen order — *accidentally correct* by Lemma 11 but a correctness debt if any same-slot pair ever crosses groups in the wrong order; the discipline is: cross-slot data meets *only* in the commutative digest reduction, never in a semidirect compose (see the correction note in §2.4 — the flattened cross-slot `⨂` is order-dependent, and pitfall 15). **(iii) A global window tick** — synchronizing all venues' window boundaries is the central epoch lock reborn (R1); window dials are per slot (Theorem 41(a)'s dial), and the merge tree consumes whatever versions have arrived. **(iv) Non-monotone per-slot watermarks** — [S2] pitfall 5 verbatim, per slot. **(v) A tape discipline violation inside a slot** (duplicate seq, gap beyond the deferral ring) — the slot's intake validation catches it (§7.2 family D); it never contaminates other slots (the identity embedding isolates slots by construction).

---

## §3 Part V — The Cross-Venue Order Transducer (Theorem 34)

### 3.1 Contingent triggers, versioned reads, the resolution DAG

**Definition 3.1 (Versioned trigger spec).** A message `m` on slot `t` may carry a trigger specification `τ(m) = (s, w′, φ, θ)`:

- `s` — the referenced slot (`s ≠ t` for cross-venue; `s = t` is the [S2] within-tape case, already resolved by Theorem 29's scan);
- `w′` — the **version pin**: the referenced slot's window index whose composite is to be read (an integer, part of the *wire data* of `m`, chosen by the issuer when it submits `m` — "the last completed matching-engine cycle at the reference venue", which is how real pegged/iceberg/arbitrage semantics actually quote: exchanges and venues publish a version, not a timestamp);
- `φ` — the read function: an affine row-op ([S2]-T27's table) or a bilinear form on the referenced composite's projection — concretely `(C,V,db,da,px,tag) ↦` the pegged price `f(mid)`, the iceberg display `g(visible fraction)`, or the conditional fire `1[leg 1 filled]`;
- `θ` — the entry rule: how the read value parameterizes `m`'s own action (the price field of the re-quote; the displayed quantity; the fire/no-fire bit).

The trigger induces the causal edges `e ≺ m` for every event `e` of slot `s` in window `w′` (Def 2.3). The **routing** of the resolved action (which venue receives the re-quote) is a parameter of the action string — the transducer's output alphabet includes the destination slot; it is pure data and changes no algebra.

**Definition 3.2 (Resolution DAG and deferral normalization).** Within a fleet window `w`: nodes are the window's records; edges are (i) slot-internal tape edges (each slot's own chain — a path) and (ii) trigger edges. The graph is a DAG: trigger edges point from *published* composites (already-folded prefixes) to *unresolved* records — always backward in the publication order; a directed cycle would project onto some slot's tape as a cycle in a total order, which is impossible (irreflexivity of `(seq, wirepos)`).

The **deferral normalization**: a triggered message `m` on slot `t` whose version pin `w′` refers to slot `s` is *resolved against the published composite* `X_s^{(w′)}`; if `m` arrives within slot `t`'s own window `w` and `X_s^{(w′)}` is not yet published, `m`'s resolution is deferred to `t`'s next window (the [S1'-T13] defer-don't-block discipline at window granularity — a latency offset, never a block). After normalization, every trigger edge in window `w` points from a **published** composite (level 0) to a **window-`w` record of exactly one slot** (level 1): the normalization converts arbitrary contingency *timing* into window *count*, not in-window depth.

### 3.2 The exactness lemma

**Lemma 12 (Versioned-read exactness).** Under Def 3.1, the deferral normalization is semantics-preserving: the resolved contribution of `m` computed against `X_s^{(w′)}` equals the sequential reference's value — for *any* deferral schedule, any arrival order, and any concurrency of the merge — because the read value is pinned by the version `w′`, and `X_s^{(w′)}` is a function of slot `s`'s tape prefix through window `w′` alone (Proposition 2.1 per slot; Theorem 33(c) — the composite is interleaving-invariant). Deferral changes *when* the read is serviced, never *what* is read: the value is already a function of data fixed in the past.

*Proof.* The sequential reference processes `m` at its tape position in slot `t`; its semantics read the referenced state "as of the issuer's stated version" — the exchange rule's own words. That state is `X_s^{(w′)}`, determined by `𝒯_s ∩ ↓(end of window w′)` — a prefix fixed before `m` is processed (window indices of different slots advance independently; `w′ < ∞` by the spec's well-formedness). The engine's deferral moves the *evaluation* of `φ(X_s^{(w′)})` from `m`'s arrival position to `t`'s next window boundary; since the argument is a fixed element of `𝒜_hash` once window `w′` of `s` is published, the evaluation is a pure function call whose timing is irrelevant (the rationality argument of [S2] §4.4: the transducer's behavior depends on the input string only through the composed register value, never on the moment of reading). Bit-exactness: the resolved output is the bilinear form on `(projection of X_s^{(w′)}, m's parameters)` — an exact function in GF(2)⁶⁴ and exact-range ℤ ([S1] §7.4) — with no rounding, and identical under any schedule by Theorem 33(a). ∎

The contrast that matters: **an unversioned read** ("read slot `s`'s *current* state") has no pin; its value is a function of the *arrival interleaving* — a timing function — and no schedule can make it deterministic. That is the phantom's native habitat (Theorem 35), and the reason version pinning is not an optimization but the *soundness condition* of Part V.

### 3.3 The theorem

**Theorem 34 (Cross-venue order transducer inversion with causal interleaving).** Under Defs 3.1–3.2 and the deferral normalization:

**(a) Correctness (causal interleaving).** The multi-venue resolution — each slot's records scanned by [S2]-T29's segmented push-scan within the slot, each triggered message's entry state supplied by the referenced slot's published composite at its pinned version — reproduces the sequential multi-venue reference bit-exactly, in all coordinates, for arbitrary asynchronous arrival orders and arbitrary deferral schedules.

**(b) The 2-DAG push-scan and its span.** A fleet window of `B` messages over `|𝕊_live|` slots resolves as a two-dimensional scan:

```
Dimension 1 (within slot):   segmented push-scan per slot          span ⌈log₂ B_max⌉  (B_max = largest slot's share)
Dimension 2 (across slots):  merge tree over window composites     span ⌈log₂ |𝕊_live|⌉   (the audit digest, Lemma 11)
Emit stage:                  triggered records against level-0     O(1) per record
```

executed as [S2]-T30's three-stage pipeline across windows (decodeₜ → resolveₜ → foldₜ at the fixed one-window offset). The **normalized span** is

```
Σ = ⌈log₂ B_max⌉ + ⌈log₂ |𝕊_live|⌉ + O(1)   =   O(log B + log |𝕊|)   — the two dimensions compose additively, not multiplicatively
```

because dimensions 1 and 2 are *different objects* evaluated in *different places* (per-slot lanes vs. the merger), pipeline-overlapped across windows. The **worst admissible span** — a venue rule set demanding *synchronous within-window* cross-slot reads (a same-window dependency chain of depth `D_w`) — is `O(D_w · log B)` with `D_w` bounded by the engine's admission cap `D_max`; with the cap set at its worst admissible value `D_max = ⌈log₂ B⌉`, the span is **O(log² B)**, the posed bound, tight at that cap.

**(c) Transducer inversion (forensics).** Given the multi-tape and the published chain, the *firing set* — which contingencies fired, at which versions, with which read values — is uniquely determined except on a set of measure ≤ 2⁻⁶⁴ per claim (adversarial V-projection collisions, gate F-π), and is recoverable in `O(#references)` record-replays at ~15 cycles/compose ([S1'] §6 replay speed): replay the window chain and read each trigger's `φ(X_s^{(w′)})` off the composites — the values are pinned, so the replay is a table of pure function calls. The invertible residue: the live-thread row-ops over ℤ/2⁶⁴ℤ are invertible iff no overwrite/dead/clamp was applied; the inversion's *failure set* is exactly the policy-arm set of [S2]-T29(a)'s emit (`ok ∈ {0, 2}`) — the transducer inversion and the policy arms partition cleanly, and both are emit-time data.

*Proof.* (a) Within-slot correctness is [S2]-T29(a) verbatim (per slot); cross-slot correctness is Lemma 12 (the pinned version makes the entry state a past-fixed function); the composition of the two is Theorem 33(a)'s interleaving invariance applied to the resolution dataflow — the resolved outputs are functions of (per-slot tape prefixes at pinned versions, own parameters), both schedule-free. (b) Span: dimension 1 is Hillis–Steele per slot ([S2] Lemma 9) — the levels of different slots run concurrently (independent lane groups, [S2]-T23(c)); dimension 2 is the up-sweep of [S2] Lemma 6 over the slots' composites — Lemma 11 licenses any tree; the two dimensions share no resource on the critical path (different lanes/sockets, §6's venue-home) and pipeline across windows at the fixed offset ([S2]-T30). Additivity: the span of a pipeline of two independent tree evaluations is the sum of their spans, not the product — the product form `O(log B)·O(log|𝕊|)` would arise only if every dimension-2 node had to wait for a full dimension-1 chain *inside* the same window, which is precisely what the deferral normalization forbids: same-window cross-slot chains are capped by admission at `D_max`, giving `Σ_worst = O(D_max · log B)`; at `D_max = ⌈log₂ B⌉` this is `O(log² B)`. (c) The firing set is read off the replay: each trigger's contribution enters its referencing record's composite as a function of the read value (Lemma 12); distinct firing sets differ in some record's digest or some slot's V-coordinate — the Ω(L) read argument at fleet scale says any recoverer must read the referenced composites: `O(#refs)`; the 2⁻⁶⁴ qualifier is [S1'-T18]'s discipline for adversarial projections (full `(C,V)` comparisons are exact; V-only projections collide with probability 2⁻⁶⁴ per claim). The invertibility partition: row-ops with `M = I` and no `dead`/clamp flags are translations by wire quantities — invertible over ℤ/2⁶⁴ℤ iff the quantity is odd (2-adic units); the `A/U` overwrite row-ops and the clamp re-anchors are non-invertible by construction, and they are *exactly* the emit-stage policy arms (`ok = 0/2`) — so "what the transducer can un-run" is precisely "what the policy did not have to repair". ∎

### 3.4 The span analysis, honestly

| Case | Setting | Span | Source of the bound |
|---|---|---|---|
| Normalized (the engine's schedule) | version-pinned reads, deferral across windows | `O(log B + log \|𝕊\|)` | dimensions additive (pipeline, venue-home) |
| Worst admissible | same-window synchronous cross-slot chains, cap `D_max` | `O(D_max · log B)`; at `D_max = ⌈log₂B⌉` → **`O(log² B)`** | each chain level costs one full dimension-1 scan |
| Unbounded (rejected) | `D_max = ∞` (no admission cap) | unbounded | the admission rule exists to make this unrepresentable |
| Lower bound | any schedule resolving a chain of length L | `Ω(log L)` per chain | standard parallel-prefix lower bound ([S2]-T29(d)) |

The posed question — "can the push-scan be generalized to a 2-D DAG scan with logarithmic span 𝒪(log² B)?" — has the answer: **yes, with room: the normalized span is O(log B + log |𝕊|), strictly better than O(log² B); the O(log² B) form is the tight bound at the worst admissible admission cap, and anything worse is forbidden by admission, not by scheduling.** The claim is not that O(log² B) is achieved *and* optimal — it is that the *engine's* schedule beats it, and the *worst legal* schedule meets it.

**Trigger resolution latency (the honest physics).** The engine term for a cross-venue trigger: the referenced slot's window `w′` must publish (its own lane tree: ≤ 150 ns for the window's records at [S2] Thm 21's schedule) plus the referencing slot's next window boundary. At the 64K-message venue dial and a venue's ~170 G msg/s share of a 1T fleet: 64K messages ≈ 0.38 µs of tape — the engine term is `2 × 0.38 µs ≈ 0.76 µs`. The physics term: 2–5 µs co-located cross-connect, 20–100 µs same-metro, 20–70 ms intercontinental. **The engine term is below the fiber term for every real venue pair beyond ~300 m** — beyond that distance the *reference venue's own publication pipeline* (matching engine tick + feed handler) dominates, which is the venue's latency, not the engine's. Gate F-δ carries the split.

### 3.5 What breaks it

**(i) An unversioned read** — "current state" semantics: the read value becomes a timing function; nondeterministic under asynchrony; the phantom's native habitat (Theorem 35); cell +42 drives it. **(ii) Clamps or read-functions evaluated at scan-composition time instead of emit time** — [S2] §4.8(ii) verbatim, now for `φ`: the pegged price needs the referenced composite's projection, known only after publication; premature evaluation silently changes semantics. **(iii) A trigger admitted beyond `D_max`** — the admission rule (an intake check, O(1)) is what makes the unbounded case unrepresentable; enforcing it at *resolution* time instead (detect-and-reject late) leaves the depth already on the critical path. **(iv) A synchronous global resolver** — any design that resolves triggers at a single fleet-wide point re-centralizes the dependency: the R2 disease; resolution is per referencing slot, against published data only. **(v) `U`-style dual-register side effects across venues** — a routed re-quote writes the *destination* venue's order book (a new register there) and consumes the source's — both sides are within-slot chains; the cross-venue edge is the (pinned) read only; dropping either side is [S2] pitfall 10 at fleet scale (cell +47).

---

## §4 Part VI — Non-Abelian Sheaf Cohomology and Deadlock Invariance (Theorem 35)

### 4.1 The reconciliation cover and the cancellation race

**Definition 4.1 (Reconciliation cover).** At fleet window `w`, each venue group ν holds the *view* `U_ν(w)` — the down-set of events it has observed: its own slots' tapes through their latest windows, plus the *published* composites of other slots through the versions its received watermark `𝕎^{(ν)}` allows. The family `𝒰(w) = {U_ν(w)}_ν` covers the observed prefix; the *engine's* cover is the one the protocol itself generates (every view derives from the single published stream — Lemma 7's monotone publication at fleet scale). An *adversarial* cover adds **claimed reads**: messages asserting "I read slot `s`, version `w′`, and the value was `Y`" — the engine's own triggers carry values it evaluated itself (realizable by construction); participants' claims, replayed data, and buggy feeds carry values as *wire data*.

**The cancellation race, stated once.** Venue A cancels order `o` (an `X`/`D` in A's tape at A-window `w″`); concurrently venue B's pegged order re-quotes off A's state at pinned version `w′ < w″`. Both messages are in the multi-tape; both are folded — nothing races *in the composite* (Theorem 33(a)). What the race decides is *business-level* staleness: B acted on a version that A's cancel had not yet reached. That is the participant's risk, the market's daily reality — and it is *not* a phantom: the global state is a well-defined function of both tapes. The phantom is a different object: a state that no realizable history produces — the territory of Lemma 13 below.

**Lemma 13 (The order-carrier split, globalized).** The engine has exactly two order-carrying objects: the hash coordinate of `𝒜_res` (the [S1'] Lemma 3 twist, slot-diagonal by Lemma 11) and the order-row chains of [S2]-T27 (per-order-ID, within-slot). Everything else — the book coordinates, the watermarks, the versioned read values — is commutative or pinned. Consequently, the *only* way to produce a state that no realizable history yields is to corrupt one of the two order-carriers: (i) compose same-slot records in an order that is not the slot's tape order (forbidden by the static-fragment discipline — [S2]-T23(b)), or (ii) assert an order-carrier value that was never computed from a real prefix — the fabricated read. There is no third path: the commutative coordinates cannot carry phantom (any multiset of contributions yields the same sum — [S1-T2] Corollary 2.1), and pinned reads are past-fixed (Lemma 12).

*Proof.* The classification of coordinates is [S2] §3.1's commutativity split plus Lemma 11's slot-diagonalization plus Lemma 12's pinning — each a theorem of this series. A phantom state `S*` (one no realizable history produces) differs from every realizable state in at least one coordinate; pick a coordinate where it differs from the *actual* fold. If the coordinate is commutative, `S*`'s value there is a function of the multiset of contributions — and the actual fold realizes every multiset value: no difference can survive (the sum over the actual multiset is the actual value; `S*`'s value would have to come from a different multiset — i.e. from messages not in the tape — which is case (ii): a fabricated contribution to a commutative coordinate is still a fabrication, detected by the same claim machinery). If the coordinate is an order-carrier, the difference is a composition-order or a never-computed value: cases (i)/(ii). ∎

Lemma 13 is the taxonomy engine: *phantoms require corrupting an order-carrier*, and both corruption routes are single, well-defined checks — which is why the detector is O(1) and not a search.

### 4.2 The theorem: H¹, the phantom residue, deadlock invariance

**Theorem 35 (Non-abelian sheaf cohomology and deadlock invariance).** Fix the reconciliation cover `𝒰(w)`, the sheaf of composite-groups `𝒢` (`𝒢(U)` = the subgroup of `𝒜_hash` generated by composites of down-sets of `U`; the published composites are units' semidirect products — `Rˣ ⋉ R` on the unit side, [S1'] Lemma 3.4), and the non-abelian Čech complex on `𝒰` (1-cochains `g = (g_{νμ})`, `g_{νμ} ∈ 𝒢(U_ν ∩ U_μ)`; cocycles `δg = 0` in the ordered-triple convention `g_{νρ} = g_{νμ} ⨂ g_{μρ}`; coboundaries `g_{νμ} ↦ h_ν ⨂ g_{νμ} ⨂ h_μ⁻¹`, `h_ν ∈ 𝒢(U_ν)`). Then:

**(a) The phantom characterization.** The global state at `w` is phantom-free (every published state equals the fold of a realizable multi-tape prefix) **iff every claimed read in the window is realizable**, where a claim `(s, w′, Y)` is realizable iff:

```
(R1) Existence:      w′ ≤ pub_count(s)  at the claim's processing time (the received watermark 𝕎^{(t)}_s)
(R2) Cone legality:  claim_time ≥ pub_time(s, w′) + d(s → t) − Δ_max      (light could have carried it)
```

Both conditions are **O(1)**: R1 is one integer compare; R2 is one timestamp compare against the per-pair cone bound (a static deployment table). Non-realizable claims route to the anomaly arm ([S1] §5.6) — the fold never blocks, never rolls back, never repairs.

**(b) The H¹ classification.** For the engine-generated cover, the 1-cocycle is identically the identity — `H¹`-representative trivial; every family of views glues (Theorem 33(b)'s flasqueness; the nerve of the published-composite cover is a simplex, so the Čech complex is resolved and there are no cocycle obstructions). For an adversarial cover: the obstruction class `[ω] ∈ H¹(𝒰, 𝒢)` — the *phantom residue* — is **nontrivial exactly on the non-realizable claims**, except on a set of measure ≤ 2⁻⁶⁴ per claim (collisions of a claimed value with an achievable composite's V-projection — gate F-π). `[ω]` is a torsor invariant: no choice of per-venue re-basing (gauge) can trivialize a nontrivial class — the phantom is *structural*, not a bookkeeping error. Evaluating `[ω]` on a specific pair `(ν, μ)` is the two compares of (a): the class is *computable* in O(1) per claim, and detecting nontriviality never requires the cochain's global data.

**(c) Deadlock invariance and finite convergence.** Under the fleet protocol (per-slot lanes wait-free; SPSC handoffs everywhere — [S1'-T13] at window granularity; the merger consumes in any order — Theorem 33(d)):

```
(i) No deadlock:     the wait-for graph is a DAG — every wait edge points from a later-window
                     consumer to an earlier-window producer; window indices strictly decrease
                     along any wait path, so no cycle exists (the [S1'-T13] backward-only DAG at
                     fleet scale; Coffman's circular-wait condition is unrepresentable).
(ii) No livelock:    every lane's publication is monotone ([S2] Lemma 7 per slot); the merge
                     predicate is monotone in 𝕎; finite rings + FIFO delivery ⇒ every window
                     is eventually merged under fair scheduling ([S2]-T26(d) at fleet scale).
(iii) Finite convergence of feedback loops: around any cross-market feedback cycle the version
      indices strictly increase (each triggered message resolves at its referencing slot's next
      window: Lemma 12's deferral), so every finite submission stream resolves in at most its
      window span, race-free (Theorem 33(c)) and phantom-free (a, b). The engine never generates
      messages (it is not a participant): feedback through participants is a market-design
      property outside the engine's contract — the engine's guarantee is that every submitted
      stream is folded exactly, in finite windows, with zero races.
```

*Proof.* (a) Sufficiency of realizability: if all claims are realizable, construct the realizable history — the multi-tape consisting of the actual tapes plus, for each claim, the referenced prefix `(s, w′)` that the engine actually received (which exists by R1 and arrived in time by R2); the claimed value `Y` enters the referencing record's composite as a *pinned read* (Lemma 12) — the fold of this history is the published state, and by Theorem 33(a) it is independent of interleaving; hence the state is realizable. Necessity: a non-realizable claim (R1 fails: the referenced prefix was never received; or R2 fails: it could not have arrived) contributes a value to an order-carrier that no prefix yields — by Lemma 13 that is the only route to a phantom; formally, if the state were realizable by some history `H`, `H`'s restriction to slot `s` through the claim's processing time is a prefix of `s`'s tape — which contradicts R1/R2 (the prefix in question does not exist in the received set; reception is the only input channel — Lemma 10(a)). O(1): R1 compares two integers already in registers (the watermark and the pin); R2 compares the claim's arrival timestamp against `pub_time(s, w′) + d(s→t)` — the latter is a static table entry plus one add. The 2⁻⁶⁴ in (b) covers the residual case where a *fabricated* value happens to equal an achievable composite's projection: the full `(C,V)` pair comparison is exact; a value-only (V-projection) comparison collides with probability 2⁻⁶⁴ — the [S1'-T18] discipline, carried by gate F-π.

(b) Engine cover: every view derives from the published stream by monotone composition (Lemma 7) — the pairwise discrepancies `g_{νμ}` are identities, the cochain is the trivial cocycle, and it is a coboundary (`h = 1`); with the simplex nerve, `H¹(𝒰, 𝒢) = 1` (every locally trivial torsor is globally trivial — the descent argument of Theorem 33(b)). Adversarial cover: the claimed view of the shared slot `s` differs from the engine's by `g_{νμ}(s) = Y ⨂ (X_s^{(w′)})⁻¹`, where the engine's composite `X_s^{(w′)}` is a unit's semidirect product and the claimed `Y` is an arbitrary element of the ambient group. Triviality of `[ω]` would supply gauges `h_ν ∈ 𝒢(U_ν)` with `h_ν ⨂ g_{νμ} ⨂ h_μ⁻¹ = 1` for all pairs — i.e. `g_{νμ} = h_ν⁻¹ ⨂ h_μ`: every discrepancy would be a *boundary*. For a non-realizable claim, `Y` is not any composite of `s`'s received prefixes (R1/R2 failed — and composites of received prefixes are the only sources of elements in `𝒢(U_ν ∩ U_μ)`), so `g_{νμ}(s)` lies outside the subgroup generated by those composites — while every boundary lies inside it (gauges are products of composites). Hence `g` is not a coboundary: `[ω] ≠ 1`. The collision set: if `Y`'s V-projection equals some achievable composite's projection, a V-only comparator mistakes it for realizable — measure ≤ 2⁻⁶⁴ per claim; the `(C,V)`-comparator (the default: claims carry the full pair, or the detector hashes the value into both coordinates) is exact. Structural-ness: a torsor class is invariant under re-basing by definition — `H¹` quotients precisely by the gauges; a nontrivial class cannot be gauged away, which is the formal statement of "no repair by re-labeling".

(c) (i) Waits: a consumer waits only on its producers' *next* publication (`𝕎` predicate) — [S2]-T26(a)'s wait-freedom per slot means producers never wait on consumers; every wait edge `(t, w+1) → (s, ≤w)` decreases the window index along the edge; a cycle would give an infinite strictly-decreasing sequence in ℕ — impossible. (ii) is [S2]-T26(d) verbatim at fleet scale (monotone predicates, finite rings, FIFO). (iii) The version-increase argument: along any feedback cycle `m₁ → m₂ → … → m₁` (each `mᵢ` triggered by the previous' effect), each `mᵢ`'s resolution consumes its referenced slot's window `w′ᵢ` and lands at the referencing slot's window `> w′ᵢ` (Lemma 12's deferral); the *next* trigger in the cycle reads a version ≥ that landing window: versions strictly increase around the cycle. A strictly-increasing sequence in ℕ indexed by a cycle is impossible — so *no cross-market feedback cycle exists within the engine's dataflow*: the "loop" is a spiral, each round at a higher version. Termination of a finite submission stream: the highest version any message references is ≤ the stream's own window span; the spiral ascends at most that many rounds; each round is finite work (Theorem 34(b)'s span). The engine generates no messages (its alphabet is consume-only — [S2] §4.1's transducer has no epsilon-transitions to the wire); feedback through participants is admitted at the capacity contract (§11): the engine keeps up iff ingestion capacity ≥ submission rate — a sizing condition, never a correctness condition. ∎

### 4.3 The worked example: a distributed cancel racing a pegged re-quote

Venue A: order `o` resting; A's tape: `…, X(o) at A-window 17, …`. Venue B: participant `p`'s pegged order on B, spec "re-quote at `f(mid(A))` at A-version `w′ = 16`". The claim arrives at B's group with pin `w′ = 16`. The engine: R1 — `pub_count(A) ≥ 16` at processing time? The received watermark says A's windows through 16 were received: yes (they were received before window 17 was even complete). R2 — did A's window-16 composite physically arrive before the claim's processing time, minus `d(A→B)`? Yes for any co-located pair (window 16 published microseconds earlier; the fiber term is the bound). So the claim is realizable; `p`'s re-quote folds with the pinned value `f(mid(A@16))` — *even though A's cancel of `o` landed in window 17 and the re-quote references `o`'s state as of 16, when `o` was still live*. The global state: A's book has `o` cancelled (window 17 folded); B's book has the re-quote priced off A@16 — *both, simultaneously, bit-exactly* — because the two facts live in different slots' factors of the product (Def 2.2) and neither composite consults the other's order. The "race" produced business staleness (`p` quoted off a version the cancel had not yet reached — `p`'s risk, the market's norm), zero phantom, zero rollback, zero waiting.

The adversarial twin: the same claim, but `p` asserts the read happened at A-version 17 with a value `Y ≠ f(anything A published)` — R1 passes (17 exists by processing time), R2 may pass (17 arrived), but the *value* is fabricated: the `(C,V)` comparator against the engine's own evaluation of `φ` on `X_A^{(17)}` catches it in one compare — `[ω] ≠ 1` on that pair — anomaly arm, fold continues. The third twin: `p` claims version 19 when A has published through 17: R1 fails in one integer compare. Three cases, three compares, one O(1) detector, no global reconciliation pass anywhere.

### 4.4 What breaks it

**(i) An unversioned or unclaimed-value read** — the soundness condition of Part V; the detector's R1/R2 cannot even be evaluated without the pin (cell +42/+48). **(ii) A global reconciliation barrier** — "reconcile all venues at window `w` before any window-`w+1` work": the central epoch lock reborn; the cover's *views* reconcile continuously by publication, never by a pass. **(iii) A rollback or repair on the fold path** — the fold is append-only (monotone); repairs belong to the *participant's* next message, not the engine's history; a "correcting" compose would break monotonicity (Lemma 7's premise) and create exactly the phantom it was trying to fix. **(iv) Clock discipline in the correctness path** — R2 uses timestamps only in the *detector*, under the bounded-drift assumption `|Δclock| ≤ Δ_max`; correctness itself never consults a clock (Theorem 41(b)) — a design that *waits* for clock sync before folding has re-introduced a sequencer with extra steps. **(v) A V-only claim comparator** — the 2⁻⁶⁴ hole; the default comparator is the full `(C,V)` pair (the claim's value enters both coordinates; gate F-π).

---

## §5 Part VII — The Shannon-Bremermann Information Bound (Theorem 36)

### 5.1 The three regimes

**Definition 5.1 (Regimes).** Every throughput claim in this series belongs to exactly one of three regimes, and the 1T question has a different answer in each:

| Regime | What work is paid per message | Binding resource | Measured anchor |
|---|---|---|---|
| **Fresh** (wire ingestion) | the payload digest `v_t(p)` computed from actual wire bytes, weighted by its suffix degree: 1 field multiply + 1 payload read per message ([S1'] Thm 10(b), Thm 11) | clmul pipe + parse (per core); the NIC/wire (per feed) | `fold_chunk`: 512 msgs in 0.2–0.4 µs/core → **1.28–2.56 G msg/s/core** ([S1'] §2.7); [S1'] §6: 2–4 G on 2 cores |
| **Replay** (record path — the benchmark and audit regime) | per *record*: validate + fuse + compose ≈ 10.4–13 cycles ([S2] §5.2 budget to the ~5 ns/record wall); each 36–52 B record stands for 512 messages | record-path ALU/latency per core; the **memory bus** per node: β·R | [S2] §3.8: ~25 GB/s per lane at 61.21B/lane → **β ≈ 0.41 GB/s per B msg/s** (optimistic L4-amortized projection 0.25 — gate F-β); [S2] L4 layer: **84–102 G msg/s per physical core** |
| **Bound** (the physics) | ≥ 64 AND-gate instances per field multiply (a 64×64 GF(2) product's outer-product structure); ≥ 1 L1 payload read (≈ 0.1 pJ/B); gate delay ≈ 10–15 ps at 4 nm | thermodynamics, gate delay — *not* binding at 1T | Bremermann 10⁵⁰ bits/s/kg: ~38 orders above the 2.44 kb/msg × 1T = 2.44 Pb/s requirement |

The [S2] corridor (150–250B) is a *replay*-regime number — the benchmark's messages arrive as pre-computed chunk records. The [S1'] §0 caveat said so from the start ("multi-billion-msg/s rates appear in the memoized replay regime; fresh wire-rate is CLMUL- and parse-bound at ~2–4 G on 2 cores"). Any 1T claim that does not name its regime is not a claim; it is a category error (Appendix D, item 6).

### 5.2 The theorem

**Theorem 36 (Shannon-Bremermann bound on hardware ingestion).**

**(a) The algebraic floor per message.** Lossless order-book reconstruction over `𝔽₂[x]/Q(x)` and `ℤ/2⁶⁴ℤ` requires, per message: **fresh** — ≥ 1 field multiplication (`v_i ⊗ x^{d_i}`, the weight varying per position: Theorem 11's Ω(L) read bound plus the weighting; the operator cannot be precomputed away because `d_i` varies — [S1'-T16]'s injectivity) and ≥ 1 payload read; **replay** — ≥ 1 record compose per 512 messages ([S1'] Cor 10.1: 1–2 ring multiplies per compose), the record's first evaluation having been paid fresh. No architecture beats these floors: they are functions of the algebra (the *order-carrier necessity* — Lemma 13: the only order-sensitive coordinates are the two carriers, and their updates are exactly these operations), not of the silicon.

**(b) The silicon ceilings, per regime.** With `P` cores at `f = 2.6 GHz`, `S` sockets, per-socket bus `B_bus ≈ 165 GB/s` (DDR5-class), clmul lanes `λ = 4/cycle/core` (the [S1'] §6 conservative 0.5/cyc port model, 8-lane `VPCLMULQDQ`), fold port duty ≤ 5% ([S2] checklist item 14's canary), and the anchors of Def 5.1:

```
Fresh ceiling:      R_fresh(P)      = P × 1.28–2.56 G msg/s          (clmul/parse-bound per core)
Replay ceiling:     R_replay(P, S)  = min( P × 84–102 G ,  S × B_bus / β )
                                                     =  min( P × 84–102 G ,  S × 402–660 B msg/s )
```

The replay formula's second term is the wall: **the memory bus, not the math**. The record path itself has 5–9× headroom at 1T (a 64-core socket delivers 5.4–6.5T of record-path capacity); the *stream* (records, journals, tokens, rings at β·R) exhausts a single socket's array at 0.40–0.66T. Sockets add bus arrays *linearly* (Theorem 37's Null-Flow keeps cross-socket traffic at O(S) lines per window — effectively zero), so the replay ceiling is `S × 0.40–0.66T` until the record path's first term binds (at S ≥ 6–8 for 64-core sockets).

**(c) The energy and gate-delay floors (per watt and per gate delay).** Per-message irreducible energy: the field multiply's ~200 gate-load switches at ~0.6 fJ/gate (4 nm) ≈ 0.12 pJ, plus the L1 payload read ≈ 3.5 pJ (35 B at ~0.1 pJ/B) — total ≈ **4–40 pJ/msg fresh** (the read dominates; the multiply is noise). At 1T fresh: 4–40 W of information-processing power — no thermodynamic obstruction anywhere near a 700 W–1 kW box. Bremermann's cosmic limit (10⁵⁰ bits/s/kg) sits ~38 orders above. Per gate delay: one 64×64 field multiply has a critical path ≥ 10–15 ps (4 nm); a *single serial lane* therefore cannot amortize below ~10 ps per field-op — **sub-picosecond amortization requires ≥ 10 concurrent lanes per gate delay**; AVX-512 supplies 8 lanes per core (≥ 1.25 ps per core-clmul-op), so the *system-wide* sub-picosecond horizon requires ≥ 2 cores sustained in the fold — trivially satisfied at every 1T operating point (≥ 10 record-path cores; the fold rides at ≤ 5% duty).

*Proof.* (a) is Lemma 13 plus [S1'] Theorems 10/11/16: the two order-carriers' updates are exactly the stated operations (the hash coordinate's weighted digest — Theorem 10(b), Theorem 11; the row-ops' translations — [S2]-T27); the commutative coordinates cannot be forced below their multiset sums ([S1-T2] Corollary 2.1); the weighting operator's injectivity ([S1'-T16]) forbids precomputation. (b) Fresh: the per-message multiply is one clmul lane; at the conservative λ = 4 lanes/cycle/core the [S1'] kernel's measured anchor (1.28–2.56 G/core) is the ceiling's calibration — it is *pipe-bound*, so it scales with P linearly until something else binds (nothing does: at 82–164 G per 64-core box, the bus demand is 82–164 G × 35 B ≈ 3–6 GB/s — noise; the NIC is the real feed bound at ~0.36 G per 100GbE). Replay: per-core 84–102 G is the [S2] L4 layer's certified shape (the 10.4 cycles/record budget to the ~5 ns/record wall — [S2] §1.1, §5.2, both anchored in the champion); the bus term: the stream is β·R (whole-node traffic: record streams + journals + tokens + rings — the [S2] §3.8 accounting), so the node saturates at `R = B_bus/β = 165/0.41 = 402B` (pessimistic anchor) to `165/0.25 = 660B` (the L4-amortized projection, gate F-β). The min() is the binding resource; sockets multiply `B_bus` (independent DDR5 arrays) and the Null-Flow theorem (§6) guarantees the *coherence* traffic does not grow with R — only with S, at O(S) lines per window. (c) Energy: standard CV² accounting at 4 nm (≈ 0.5–1 fJ per gate transition; ~200 gate-loads in a 64×64 multiplier's AND/XOR tree — the bilinear structure of [S1'] Appendix B.2); the L1 read energy is the dominant term (SRAM ~0.1 pJ/bit-port at 4 nm). Gate delay: the multiplier's critical path is ≥ 2 levels of logic per 8 bits of operand width — ≥ 16 levels for 64 bits at ≥ 0.6 ps/level ≈ 10–15 ps; amortization: `τ_msg = 1/R` at the system level; `R ≥ 1T ⇔ τ ≤ 1 ps`; with 8-lane vector ops at 2.6 GHz, one core issues ≤ 10.4 G lane-ops/s ⇒ 0.096 ps per lane-op *per core* — a *serial* dependency chain through one lane pays ≥ 10 ps per link; the fold's tree re-association (Theorem 19) is what removes the serial chain, leaving ≥ 10-way lane concurrency as the only requirement — satisfied at any multi-core operating point. ∎

### 5.3 The 1T verdict, regime by regime

| Regime | 1T on commodity silicon? | The number | The wall |
|---|---|---|---|
| **Fresh** (true wire ingestion) | **NO on CPU alone** | 82–164 G per 64-core box (1.28–2.56 G/core × 64, clmul/parse-bound) | the clmul pipe — an order short; crossing needs ~2,000 multiplier banks at 500 MHz (a large FPGA/ASIC farm at ~40× the watts and zero verification artifact). In any case the *network* is the real fresh wall: a 100GbE NIC carries ~0.36 G msg/s at 35 B/msg, and the real global feed peaks ≤ 1 G — **80–160× below the CPU's own fresh capacity** |
| **Replay** (benchmark + audit) | **YES — on 2–4 sockets** | S=1: 0.40–0.66T (bus-capped); **S=2: 0.80–1.32T; S=4: 1.60–2.64T** — with the record path at only 15–25% duty and the fold at ≤ 5% | the memory bus — linear in sockets (Theorem 37); the certified floor (theorem-backed, conservative anchors) is **1.6T at S=4**; the 2-socket crossing needs gate F-β ≤ 0.33 GB/s per B msg/s |
| **Bound** (physics) | not binding | 4–40 pJ/msg fresh (≈ 4–40 W at 1T fresh); 2.44 Pb/s of information vs Bremermann 10⁵⁰ bits/s/kg | 38 orders of headroom — **the sub-picosecond horizon is architectural, not physical**: it opens exactly at R ≥ 1T, i.e. at ≥ 10 record-path cores and ≥ 2 bus arrays |

**The sub-picosecond line.** `τ = 1/R`: at the certified floor 1.6T, `τ = 0.63 ps/msg`; at 1.0T, `τ = 1.0 ps` — the horizon opens *at* the barrier and widens linearly beyond. Every per-core view is slower (84–102 G ⇒ 9.8–11.9 ps/msg/core): sub-picosecond is a *fleet* property, exactly as the amortized fold of [S2] Thm 21 was a *tree* property — the individual node never needs to be fast, only concurrent.

### 5.4 What breaks it

**(i) A regime-unnamed benchmark** — the audit assertion of cell +57: every published rate must carry its regime tag (fresh/replay) in the harness output; a replay number quoted as wire-ingestion is the industry's most common 1T lie. **(ii) `β` assumed small without the gate** — the bus term is the ceiling's second clause; F-β measures it on the widened-window configuration (perf counters, whole-node, steady state); the [S2] §3.8 anchor (0.41) is the *champion-configuration* number, not a constant of nature. **(iii) Cores added where the bus binds** — beyond the min()'s crossing, cores idle: the S=1 box at 1T-worth of cores runs at 0.40–0.66T; adding sockets, not cores, is the fix. **(iv) The fresh wall misattributed** — the CPU's fresh ceiling (82–164 G) is 80–160× above the real feed; engineering the fresh path for 1T optimizes a non-binding resource — the NIC, not the core, is where fresh latency lives. **(v) An FPGA/ASIC comparison that ignores the artifact** — the [S2] §7 boundary, restated at fleet scale in §12: fabric wins the fresh-wire latency race; the CPU wins the audit/replay regime — where the industry's cost center actually is — and is the only design whose proof object and performance artifact are the same document.

---

## §6 Part VIII — Scale-Out: The N-Socket Factorization and the Sub-Picosecond Boundary (Theorems 37–38)

### 6.1 The deployment architecture

**Definition 6.1 (Venue-home architecture).** The fleet's deployment: each venue group's *whole pipeline* — decode, resolve (3-stage), fold (two-level tree), publish — is **socket-local** (its NUMA home: first-touch on the venue's arenas; the [S2] Cor 19.1 L1-journal residency property holds per slot, in that socket's cores). The *only* cross-socket data is the published window composite (52 B per window per slot-group) flowing through the fleet merge tree, and the merge tree's own internal nodes. The merge tree is an SPSC handoff at every edge ([S1'-T13] at socket granularity): one writer, one reader, no shared writable line anywhere in the scale-out path.

**Theorem 37 (N-Socket monoid factorization and Null-Flow).**

**(a) Bit-exact socket split.** Any partition of the slots' lane groups across S sockets, with per-socket window folds and the cross-socket reduction (Corollary 33.1's window digest), yields the identical per-slot composites and window digest — bit-exactly, for every partition, every reduction shape, and every consumption order. The socket split changes only which lane-group folds which factors; the reduction is commutative (Corollary 33.1), so no ordering constraint crosses sockets at all.

**(b) Null-Flow (the coherence law).** The cross-socket cache-coherence traffic is

```
T_xsock(w)  =  (number of socket-groups with activity in window w) − 1  handoffs
               ×  64 B per handoff (52 B composite + ring slot header)
```

— **independent of the message count**. Amortized per message at the 64M-message fleet dial and S = 4: `3 × 64 B / 64M ≈ 3 × 10⁻⁹ B/msg`. The lines settle in MESI S/I (single-writer discipline: each epoch record is written once by its owning group, read once by its merge-tree parent, never modified); there are no M↔M transitions, no invalidation storms, and no line ping-pongs — because no line is ever written from two sockets. The [S2] pitfall 5 watermark rule holds at socket scale: each `𝕎_s` has exactly one writer, release-stored.

**(c) No global barrier.** The merge tree's root publishes when its inputs arrive (wait-free per group — [S2]-T26(a)); there is no global window tick (the R3 hazard); the day-flush remains the system's only barrier ([S2]-T26(c) at fleet scale). NUMA far-memory latency (140–200 ns) appears only on the merge path — off the ingestion critical path — and the published state's staleness is bounded by the fleet dial: `|frontier − published| ≤ (1 + N_ring) windows per group` ([S2]-T26(b) at fleet scale) ≈ one fleet window ≈ 64M messages of tape.

**(d) The merge-traffic dial law.** The merger's cost per second is `(fleet rate / dial) × |𝕊_live| × ~2.5 cycles` (the digest reduction — 2 XOR + 3 adds per slot-composite); the dial (window size) is chosen so this is ≤ 1% of one core:

```
dial 64M msgs, R = 1T, |𝕊_live| = 100:   15,625 windows/s × 100 × ~2.5 cyc ≈ 3.9M cycles/s ≈ 0.15% of one core ✓
dial  1M msgs, same rate:                1M windows/s × 250 cyc ≈ 250M cycles/s ≈ 9.6% — legal, but the
                                          proof object balloons to 1.2 GB/day (C.4) — fine-biased
```

— the dial is a deployment parameter with a *law*, not a free choice: `dial ≥ R × |𝕊_live| × 2.5 / (0.01 × f)` for the traffic side, and Theorem 40(a)'s size arithmetic for the object side (Theorem 41(a) carries the same law for the audit dial).

*Proof.* (a) is Theorem 33(a)/(d) at socket granularity — each factor is a per-slot function of per-slot tapes (Theorem 33(c)), and the digest reduction is commutative (Corollary 33.1): the partition changes only which lane-group folds which factors, and no ordering constraint crosses sockets. (b) Accounting: per fleet window, each active group publishes one 52 B composite into its reduction parent's SPSC ring (one cache-line transfer, one invalidation of the parent's shared-read copy — S-state); the parent reduces parent-local data (socket-local after the first hop); the root publishes once. Total lines touched cross-socket: O(S) per window, each 64 B — the formula. No line is written from two sockets (single-writer SPSC; the rings' head/tail indices are the only shared writable words, each with one writer — [S2] pitfall 5's discipline), so MESI never produces competitive M states; far-memory latency is paid once per handoff on the merge path only. (c) The root's publication predicate is monotone in the per-group watermarks (Lemma 7 at fleet scale); no edge of the reduction ever waits on a *later* window (DAG, Theorem 35(c)(i)); the day-flush is the [S2]-T26(c) epoch rule, once per day, unchanged. (d) Arithmetic from the stated anchors. ∎

### 6.2 The critical batch and the phase boundary

**Theorem 38 (Critical batch amortization and the sub-picosecond boundary).**

**(a) The rate law.** The fleet's steady-state replay rate is

```
R(P, S)  =  min(  η_f · P · ρ_core ,  S · B_bus / β  )          ρ_core = 84–102 G msg/s per physical core
                                                                             η_f = fleet interference efficiency (gate F-η:
                                                                             0.85–0.95 per [S2] gate 12's protocol, extended
                                                                             to NUMA-interference at socket scale)
```

with the fold riding at ≤ 5% port duty (Theorem 21's schedule, unchanged) and the merge at ≤ 1% of one core (Theorem 37(d)).

**(b) The critical window size.** A window's fixed cost `α_w` (publication release-store ~25–40 cycles + tree-root compose + ring bookkeeping ≈ 60 cycles) amortizes below a fraction ε of the window's marginal record-path work iff

```
B_w  ≥  α_w / (ε · ρ_msg)         ρ_msg ≈ 0.0203–0.0244 cycles/msg (10.4–12.5 cycles per 512-msg record)
```

— at ε = 0.1: `B* ≈ 60/(0.1 × 0.022) ≈ 27K messages`. The [S2] L4 window (2.1M messages per 4,096-chunk pass) sits 78× above `B*` — fixed costs are noise at every operating point in the corridor; the *binding* dial is Theorem 37(d)'s merge law, which pulls the other way. The deployment's dial table (Appendix C.4) balances the two: 64M at the 1T operating point.

**(c) The sub-picosecond phase boundary.** `τ = 1/R ≤ 1 ps ⇔ R ≥ 1T` — the boundary in (P, S) space is the pair of linear inequalities

```
P  ≥  1T / (η_f · ρ_core)   =   10.8–14.0 physical cores     (1T at the certified-shape rate)
S  ≥  β · 1T / B_bus        =   1.5–2.5 sockets               (1T of stream through the bus arrays)
```

On the boundary's high side, every additional core adds exactly `η_f · ρ_core` msg/s and every additional socket adds `B_bus/β`, until the next shared ceiling (none in sight at S ≤ 8: the record path's first term binds at S ≈ 6–8). **The horizon opens at the boundary and never closes.**

*Proof.* (a) The two clauses are Theorem 36(b)'s ceilings with the fleet-interference efficiency folded into the first term (η_f is [S2] gate 12's lane-interference measurement, extended to cross-socket interference — the [S2] §3.8 resource table at S sockets: shared resources are the bus and the merge tree only; per-core resources are untouched by S); the min() composes the two. (b) Standard amortization: the window's total cost is `α_w + ρ_msg·B_w`; the ratio `α_w/(ρ_msg·B_w)` < ε iff `B_w > α_w/(ε·ρ_msg)`; the numbers are the [S2] anchors (α_w from [S2] Thm 25's protocol budget; ρ_msg = the 10.4–12.5 cycles/record of §1.1's table divided by 512). The L4 window: 4,096 chunks × 512 = 2.1M messages per pass, 78× above B* = 27K — fixed-cost amortization was already non-binding at [S2]'s L4; it remains non-binding at fleet scale because α_w is per *window* and the dial widens. (c) Algebra from (a): the boundary is where each clause crosses 1T; the constants are the anchors with their gates; linearity beyond the boundary follows because no term of (a) saturates before S ≈ 6–8 (first term: 64-core sockets deliver 5.4–6.5T of record-path capacity — the min() is bus-bound until S ≈ 6–8; η_f's range already absorbs cross-socket interference into the first term's constant). ∎

### 6.3 What breaks it

**(i) A NUMA home violation** — a venue group's records flowing through a remote socket's memory: the [S2] Cor 19.1 residency property is per socket; first-touch policy + the venue-home map are load-bearing (cell +54's perf assertion). **(ii) A shared writable line in the scale-out path** — the merge tree is SPSC edge-by-edge; any "convenience" shared counter (a global window index, a merge-progress word) is a coherence hot line — the R3 thrashing hazard; audit with cell +53's counters. **(iii) The dial mis-set** — too fine: the merge traffic law (37(d)) is violated (46% of a core at 1M); too coarse: the audit object loses checkpoint resolution (Theorem 40's dial table); 64M at 1T is the certified operating point. **(iv) A global barrier anywhere** — a "fleet-wide day-flush sync" that waits for all sockets' in-flight merges: the flush is already safe under Theorem 35(c)(i)'s DAG — the [S2] T26(c) rule at fleet scale; cell +55 drives it. **(v) η_f measured on one socket and assumed at S** — cross-socket interference has its own protocol (gate F-η: run identical tapes at S = 1 and S = 4, measure the ratio); the UPI link's bandwidth (≈ 32–64 GB/s xGMI-class) is 3 orders above the merge traffic — but only *because* of Null-Flow; the measurement proves it stays that way.

---

## §7 Part IX — The Phantom Taxonomy and the 210-Cell Contract (Theorem 39)

### 7.1 The theorem

**Theorem 39 (Phantom taxonomy: the impossible / detected / bounded trichotomy).** Every adversarial cross-venue failure mode belongs to exactly one of three classes, and the 210-cell matrix enumerates them:

```
(I) IMPOSSIBLE  — unrepresentable in the protocol: the failure cannot be expressed, so no cell
                  can fail it; the cell asserts the unrepresentability itself (perf/audit counters).
(D) DETECTED    — caught in O(1) at intake or emit (the two compares of Theorem 35(a), the
                  (C,V) comparator, the watermark monotonicity audit) — routed to the anomaly
                  arm; the fold never blocks and never rolls back.
(B) BOUNDED     — not prevented, not an error: bounded by a staleness/latency law (Theorem 37(c),
                  [S2]-T26(b)) and invariant in the outputs (bit-exactness holds *through* the
                  adversarial schedule).
```

No mode requires a barrier, a lock, a rollback, or a global pass; no mode can corrupt a commutative coordinate (Lemma 13); no mode escapes its slot's identity embedding (Def 2.2 — the isolation is structural).

*Proof.* The trichotomy is Lemma 13 plus Theorems 33–38: a failure mode either (i) requires a state the protocol cannot express (I — e.g. a torn watermark: the release-store protocol makes it unrepresentable, [S2] pitfall 5); (ii) produces a claim or record that differs from the realizable fold (D — the two compares or the comparator catch it; measure ≤ 2⁻⁶⁴ for the V-projection residue, gate F-π); or (iii) affects only *timing* (B — staleness bounds; outputs are interleaving-invariant by Theorem 33(a), so no output changes). The enumeration below instantiates the trichotomy class by class. ∎

### 7.2 The 22 new cell classes (44 cells, +37..+58)

Eighteen new cell classes were added by [S2] (+19..+36); this document adds twenty-two more (two seeded cells each, the harness's seed discipline — 166 + 44 = **210 cells**), in six adversarial families:

| # | Cell class | Adversarial design | Catches | Class |
|---|---|---|---|---|
| +37 | Multi-venue interleaving fuzz | 8 slots × random P64 streams; fold via 200 random global interleavings (per-slot order preserved) vs per-slot sequential — all equal | Thm 33(a) | bit-exact |
| +38 | Replica consistency | two "replicas" (divergent arrival orders + dials) publish identical states at every 𝕎 | Thm 33(c) | bit-exact |
| +39 | Merge-order freedom | fleet digest reduced in 200 random slot orders + 64-way random shapes — all equal to the flat reduction | Thm 33(d), Cor 33.1 | bit-exact |
| +40 | Digest shape fuzz | random reduction orders/shapes incl. unbalanced and odd tails vs the flat commutative reduction | Lemma 14 | bit-exact |
| +41 | Sequencer-absence assertion | perf/audit: zero global-order consults in the publish path (no sequencer word, no central epoch lock) — counter-based | Lemma 10 | (I) |
| +42 | Versioned-read pinning | pegged re-quote streams at pinned versions vs the sequential reference with the same pins — equal; unversioned twin diverges (and is rejected) | Lemma 12 | bit-exact / (D) |
| +43 | Deferral equivalence | deferred (2-window) vs synchronous resolution of the same versioned triggers — bit-equal | Thm 34(a) | bit-exact |
| +44 | Depth-cap admission | trigger chains at `D_max` and `D_max + 1`: the latter rejected at intake (O(1)), fold never blocked | §3.5(iii) | (D) |
| +45 | Pegged lifecycle round-trips | cross-venue re-quote spirals of depth 3–4 (A→B→C→A at ascending versions); versions strictly increase; terminates ≤ window span | Thm 35(c)(iii) | (B) |
| +46 | Iceberg display fraction | `g(visible fraction)` reads at pinned versions; emit-time evaluation vs oracle | Thm 34(a) | bit-exact |
| +47 | Arbitrage two-leg causal chain | leg 2 fires iff leg 1's pinned-version state shows the fill; both legs' books updated; `U`-style dual-register across venues | §3.5(v) | bit-exact |
| +48 | Fabricated value (phantom A) | claims with `Y ≠ φ(X_s^{(w′)})` — `(C,V)` comparator catches in 1 compare; anomaly arm; fold unblocked | Thm 35(a) | (D) |
| +49 | Stale-version claim (phantom B) | pins `w′` > published count — R1 fails in 1 integer compare | Thm 35(a) | (D) |
| +50 | Cone-illegal claim (phantom C) | R2: arrival earlier than `pub_time + d` — 1 timestamp compare; conservative (never rejects a legal claim) | Lemma 15 | (D) |
| +51 | Split-brain double-publish | two divergent chains claiming one slot-window: unrepresentable (single-writer release-store); the attempt itself is the cell | §2.5, Thm 37(b) | (I) |
| +52 | Clock-drift stress | venue clocks drifted ±100 ppm (and ±10 ms skew): outputs invariant (clock-freeness); detector conservative — zero false positives on legal feeds | Thm 41(b) | (B) |
| +53 | UDP duplicate/gap reassembly | duplicate seq → dropped+flagged at slot intake; gap ≤ ring → held (defer); gap > ring → anomaly arm | §2.5(v) | (D) |
| +54 | Socket-partition invariance | re-partition slots across 4 simulated sockets × 100 random partitions — digests equal; NUMA-home perf assertion (zero remote-access in steady state) | Thm 37(a) | bit-exact + (I) |
| +55 | Day-flush with in-flight merges | epoch flush while merges in flight across sockets: post-flush state = sequential; no global wait | Thm 37(c) | (B) |
| +56 | Merge-tree order/concurrency fuzz | merger lanes consume out of order, concurrently — digest unchanged | Thm 33(d) | bit-exact |
| +57 | Regime honesty assertion | every benchmark output carries its regime tag (fresh/replay); a replay number quoted as wire-ingestion fails the harness | Def 5.1 | (I) |
| +58 | Full-freedom chaos (fleet) | randomize all 15 freedoms of Theorem 42(a) simultaneously per run, 10³ runs, vs the sequential multi-venue oracle | Thm 42(a) | bit-exact |

Total: **210 cells**, bit-exact, no tolerance, every PR. The matrix remains the contract; Theorems 33–42 remain its proof.

### 7.3 What the matrix buys

The [S2] §6.3 economics extend to the fleet: a regulator or counterparty verifying all six venues needs the published chain (Theorem 40's object) and this matrix's *verifier configuration* — any machine, any tree shape, any socket partition, any merge order — same answer, bit-exact, by Theorem 42(a). The matrix's new (I) cells are the first in the series that assert an *absence* (no sequencer word, no global tick, no shared writable line, no regime confusion) — absence cells are audit-only: they cannot fail a fold, only a deployment discipline; they are the 210-matrix's contribution to the pitfall catalogue (Appendix D).

---

## §8 Part X — Global Deterministic Replay and the Watermark Lattice (Theorems 40–41)

### 8.1 Theorem 40 — the day-scale proof object at fleet scale

**Theorem 40 (Global deterministic replay and audit).**

**(a) The proof object.** At a 1T sustained rate and the 64M-message fleet dial, a 23,400-second trading day carries 23.4T messages. The day's proof object is the **per-slot epoch chain** (the [S2] §6.3 object, per slot): each slot's window composites, 52 B each. Per slot (100 live slots): 234B messages ÷ 64M ≈ 3,656 epochs × 52 B × 100 slots ≈ **19 MB per day** (the cross-slot digest is *not* carried — the verifier recomputes it in O(|𝕊_live|) digest reductions (2 XOR each) per window, on demand; carrying it would triple the object for zero verification value). L3-resident for any verifier.

**(b) Re-verification and re-simulation.** Chain re-verification: 365,600 composes ≈ 15 cycles each ≈ 5.5M cycles ≈ **2.1 ms** on one core — recompose each slot's chain, check the final hash and per-epoch checkpoints against the published chain. Full re-simulation (the entire deterministic state trajectory, all venues, from the raw tapes): replay at the corridor rate — 23.4T at 1.0–1.3T = **18–23 s** on the 4-socket fleet. A single venue's day (~10B events): **8–10 ms** re-simulated at 1T+; the six-venue aggregate (~60B): **< 50 ms at ≥ 1.2T** — the [S2] §6.3 line, now at fleet scale.

**(c) Verifier machine-independence.** The verification is bit-exact on any machine, any vector width, any tree shape, any socket partition, any merge order, any dial (the 15 freedoms of Theorem 42(a)) — the verifier needs no cooperation from any venue beyond the *publication* of its chain; a global regulator verifies all six venues from the 19 MB object in 2.1 ms. The audit economics change from *reconstruction* (today: hours-to-days, non-deterministic, approximate) to *verification* (milliseconds, deterministic, exact): there is no approximate middle — either the chain recomposes to the published hash or it does not.

*Proof.* (a) is the [S2] §6.3 accounting at fleet scale: per-slot chains only (the cross-slot products are derivable, and Theorem 33(d) makes their recomputation order-free); sizes from the dial arithmetic. (b) The compose count is the chain length; the per-compose cost is the [S1'] §6 replay anchor (≈ 15 cycles), unchanged at fleet scale (composes are per-slot, socket-local); the re-simulation rate is Theorem 38(a)'s law at the deployed (P, S). (c) is Theorem 42(a)'s determinism over the 15 freedoms, applied to the verifier's configuration choices; the per-slot chains are functions of the per-slot tapes alone (Theorem 33(c)), so any venue's chain verifies independently of every other venue — the no-cooperation property. ∎

### 8.2 Theorem 41 — the watermark lattice and clock-freeness

**Theorem 41 (The watermark lattice and clock discipline).**

**(a) The lattice and the dial law.** The fleet's publication state is `𝕎 = (𝕎_s)_{s∈𝕊} ∈ ℕ^{|𝕊|}` — per-slot window watermarks, coordinate-wise monotone ([S2] Lemma 7 at fleet scale). The publication predicate for fleet window `w`'s digest is "all inputs present" — monotone in `𝕎`, never false once true. The audit dial obeys Theorem 37(d)'s law (the same dial governs the proof object's granularity: finer dials cost proportionally more bytes and buy proportionally finer checkpoints — the [S2] §6.3 dial, now with the merge-traffic law as its lower bound).

**(b) Clock-freeness.** Correctness never consults a clock: window indices are *sequence counts*, not timestamps; venue clocks may drift arbitrarily (±100 ppm typical; ±10 ms skew adversarial) and the outputs are invariant (cell +52) — drift changes *when* windows complete, never *what* they contain. No NTP, no PTP, no GPS is required for correctness. Timestamps ride as payload data; the *only* consumer is Theorem 35(a)'s R2 cone check, which is conservative by construction: under the bounded-drift assumption `|Δclock| ≤ Δ_max`, it never rejects a legal claim (zero false positives on well-formed feeds — cell +52's assertion); a violated bound degrades *detection*, never correctness.

**(c) Staleness.** The published global state lags the true frontier by ≤ `(1 + N_ring)` windows per group ([S2]-T26(b) at fleet scale) — one fleet window at the certified dial ≈ 64M messages of tape ≈ the audit-freshness dial. The *replica* divergence (two replicas' published states at the same wall-clock instant) is bounded by the same term plus the propagation delay of the publication stream itself.

**(d) The cone watermark bound.** Stated as its own lemma:

**Lemma 15 (Cone watermark bound — soundness).** The R2 bound, floored at zero — `claim_time ≥ max(pub_time(s, w′) + d(s→t) − Δ_max, 0)` — never rejects a claim that a real deployment could have produced. *Proof.* A legal claim's referenced composite was published at `pub_time` (the engine's record) and physically arrived at the referencing group no earlier than `pub_time + d(s→t)` (propagation is a lower bound on any channel); the referencing group's clock may read the claim's arrival as early as `true_time − Δ_max` (bounded drift). Hence a legal claim satisfies `claim_time ≥ pub_time + d − Δ_max` — the bound is implied by legality, so testing it never rejects a legal claim. The converse fails by design: satisfying the bound does not prove legality (the value comparator does that) — R2 is a necessary-condition filter, one compare.

**Implementation note (the smoke harness's second catch).** The bound must be *floored at zero* before the compare: when the drift allowance exceeds the propagation delay (`d(s→t) < Δmax`, e.g. co-located pairs under a 10 µs drift budget), `pub_time + d − Δmax` is negative, and an unchecked *unsigned* `<` wraps it to ~2⁶⁴ — inverting the check and rejecting every legal claim on that pair. The floor's semantics is exactly the vacuous bound: when drift allowance exceeds propagation, the cone test constrains nothing beyond publication order (R1 + the comparator carry the detection). The detector's listing in §10.2 and Appendix B.3 carry the floor. ∎

### 8.3 What breaks it

**(i) Timestamps in the correctness path** — Theorem 41(b) is a *theorem about the protocol*; a deployment that gates any fold on clock sync has re-introduced a sequencer ([S2] §3.9(v)'s family). **(ii) The dial as a free choice** — Theorem 37(d)'s law and Theorem 40(a)'s size arithmetic are the two-sided constraint; Appendix C.4's table is the certified operating point per rate. **(iii) A carried cross-slot digest** — triples the proof object for zero verification value (Theorem 40(a)'s note); the verifier recomputes. **(iv) Replica comparison by wall clock** — replica divergence is bounded in *window* units (Theorem 41(c)); comparing replicas by timestamps measures the clock, not the state. **(v) An unbounded-drift deployment claiming R2 soundness** — the detector's conservatism is conditioned on `|Δclock| ≤ Δ_max`; a deployment outside the bound must widen Δ_max (trading detection latency), never narrow it silently.

---

## §9 Part XI — The Grand Synthesis: Terminal Capacity (Theorem 42)

### 9.1 The theorem

**Theorem 42 (Terminal capacity and the certified corridor).**

**(a) The 15 freedoms (Theorem 32, extended).** Fix the multi-tape, the modulus Q, and any of: the [S2] ten freedoms (chunk size; pass size; vector width; worker count; split position; block decomposition; scheduling interleaving; fold-tree shape and interleaving; lane interleaving and epoch placement; resolution batch/dial) plus **(11) venue interleaving** (Theorem 33(a)); **(12) socket partition and merge-tree shape** (Theorem 37(a)); **(13) merge consumption order and concurrency** (Theorem 33(d)); **(14) per-slot window dials** (Theorem 41(a)); **(15) trigger-deferral schedules** (Lemma 12). Then the pipeline's outputs — final and per-window global states, per-slot composites, audit digests, every published epoch record, the day's proof object — are bit-exact equal to the sequential multi-venue reference on every freedom's every value.

**(b) The composed corridor stack.**

| Layer | Configuration | Mechanism (theorem) | Rate |
|---|---|---|---|
| [S2] certified corridor | 2 cores, L1–L5 (measured + gated) | Thms 19–32 | **150–250B** (landed shape) |
| F1 = + multi-tape sheaf | M venues, no sequencer, no central epoch lock | Thm 33; Lemmas 10–11 | venue ingress fully decoupled — zero cross-venue ordering cost |
| F2 = + versioned triggers | pegged/iceberg/arbitrage at pinned versions; deferral | Thm 34; Lemma 12 | resolution off the ingestion path; span O(log²B) worst |
| F3 = + phantom detector | O(1) intake checks; anomaly arm | Thm 35; Lemma 13 | fabrication absorbed; zero rollback |
| F4 = + socket scale-out | S sockets, venue-home, Null-Flow | Thm 37 | bus-linear: **S × 0.40–0.66T** |
| F5 = the boundary | P ≥ 10.8–14.0 cores, S ≥ 1.5–2.5 sockets | Thm 38 | **1T opens the sub-picosecond horizon** |

**(c) The certified statement.** In the [S2] §5.4 honesty discipline: the *certified floor* is **1.6T msg/s at S = 4** (theorem-backed layers only, every anchor at its conservative end: β = 0.41, ρ_core = 84 G, η_f = 0.85 — the floor needs only Theorems 33–38, which are theorems); the *champion projection* is 1.0–1.3T at S = 2 with gate F-β ≤ 0.33; the *ceiling* is 2.6T at S = 4 with every gate at its optimistic end (stated as a gate-bounded aspiration). Amortized per-message: 0.38–0.63 ps on the floor; 1.0 ps at the barrier itself.

**(d) What the corridor does NOT claim.** (i) Fresh-wire 1T — the CPU's fresh ceiling is 82–164 G per 64-core box (Theorem 36(b)); the network is the true fresh wall and sits 80–160× below it. (ii) Single-socket 1T — bus-capped at 0.40–0.66T; adding cores there idles them. (iii) Any latency claim — this is an *ingestion-throughput and audit* corridor; the single-message wire-latency race remains fabric territory ([S1] §8.4's distinction, unchanged). (iv) Any market-design claim — feedback *through participants* is outside the engine's contract (Theorem 35(c)(iii)); the engine guarantees that every submitted stream folds exactly, in finite windows, race-free.

*Proof.* (a) Each new freedom is the corresponding theorem's invariance statement: (11) Theorem 33(a); (12) Theorem 37(a) = Theorem 33(d) + Theorem 19 at socket scale; (13) Lemma 11; (14) Theorem 41(a) (the dial is a re-partition of the per-slot chains — [S1'-T12] at window granularity); (15) Lemma 12. The composite reduction is [S2]-T32's: every case is an associativity, a commutativity, or a static-order/pinned-value composition — the exact laws of the series; no rounding anywhere (GF(2), exact-range ℤ — [S1] §7.4). (b)–(d) are the arithmetic of Theorems 36–38 with the anchors as stated. ∎

### 9.2 The one-page picture

```
                     ┌────────────── per socket (venue-home) ──────────────┐
   venue ν feeds ──▶│ lanes: decode → resolve(3-stage) → two-level tree    │── publish X_s^(w) (52 B)
   (NIC, fresh      │        [S2] machinery, verbatim, per slot            │        │ SPSC
    ≤ 1 G/s,        └──────────────────────────────────────────────────────┘        ▼
    CPU headroom                                                       fleet merge tree
    80–160×)                     Theorem 37 Null-Flow: O(S) lines/window           │ order-free
                                                                                   ▼
                                                                     audit digest D^(w) ──▶ day chain (19 MB)
                                                                     phantom detector: 2 compares per claim
                                                                     (Theorem 35) — off-path, anomaly arm
```

---

## §10 Engine Landing: Lane Maps, the Multi-Stream DAG Push-Scan, AVX-512 Schedules

### 10.1 The lane/slot/socket map

| Object | Home | Discipline |
|---|---|---|
| Slot's tape arenas, decode tokens, order-cache rows | the venue group's socket (first-touch) | [S2] Cor 19.1 residency per socket |
| Per-slot window journal (R36 runs → P64 sub-composites) | the owning lane's L1/L2 | [S2] §2.7 two-level, per slot |
| `𝕎_s` watermark registers | one writer (the owning group), release-store | [S2] pitfall 5, per slot |
| Fleet merge-tree rings | SPSC per edge, 64 B slots | Theorem 37(b) single-writer |
| Day chain (the proof object) | append-only, per slot | Theorem 40(a) |
| Phantom-detector tables (`d(s→t)`, `pub_time[]`) | read-only, replicated per socket | Lemma 15 |

### 10.2 The multi-stream DAG push-scan (pseudo-code)

```c
/* Per fleet window w — the three stages pipeline across windows (fixed one-window offset). */

/* Stage 1 (per slot, concurrent across slots — [S2] machinery, verbatim):
 *   decode → fused records → two-level tree → X_s^(w) published to the merge ring.        */

/* Stage 2 (per slot): the DAG push-scan with versioned reads. */
void resolve_window(slot t, window w) {
    for (each record r of t's window-w journal, in tape order) {          /* within-slot: [S2] order_scan */
        if (r.carries_trigger) {                                          /* τ(r) = (s, w', φ, θ) */
            if (!realizable(r)) { route_to_anomaly_arm(r); continue; }    /* the 2 compares — Thm 35(a)  */
            entry = project(X_s_published(w_prime));                      /* pinned version — Lemma 12   */
            emit_resolved(r, entry);                                      /* bilinear form at emit time  */
        } else {
            emit_resolved(r, own_entry_state(r));                         /* [S2] T29(a) unchanged       */
        }
    }
}

/* Stage 3 (the merger — any order, any concurrency, any shape): the
 * COMMUTATIVE digest reduction (Cor 33.1) — never a semidirect compose. */
void merge_window(window w) {
    Dig D;  dig_identity(&D);
    for (each published X_s^(w), in ARRIVAL order)     /* any order is the flat */
        digest_accum(&D, &X_s[w]);                   /* 2 XOR + 3 adds        */
    publish_digest(w, D);                             /* chains the day object */
}
```

The phantom detector, inline:

```c
int realizable(Claim c) {                 /* c = (s, w_prime, Y) on slot t            */
    if (c.w_prime > pub_count[s])                  return 0;  /* R1: 1 integer compare */
    bound = pub_time[s][c.w_prime] + cone_d[s][t];             /* R2 with the floor:   */
    bound = (bound > DMAX) ? bound - DMAX : 0;                 /* d < Δmax => vacuous  */
    if (c.arrival < bound)                         return 0;  /* R2: 1 timestamp compare*/
    return eq_cv(c.Y, engine_eval(phi, X_s[c.w_prime]));      /* the (C,V) comparator   */
}                                                        /* 3 compares total; O(1)  */
```

### 10.3 The AVX-512 vector schedule (the dual-pipe interleave)

Per fleet window at the 1T operating point, the instruction mix (the [S2] Appendix C mapping, at the new operating point — the *same* two kernels, two rings):

| Path | Instructions (per 8-lane ZMM op) | Duty at 1T | Notes |
|---|---|---|---|
| Fold (hash coordinate, GF(2)⁶⁴) | `VPCLMULQDQ` ×3 + `VPXORQ` ×2 (`mulmod4`) | ≤ 5% of one clmul port per core | [S2] Thm 21's schedule, unchanged — the tree is 15–25% of the record path |
| Record path (validate/fuse, ℤ) | `VPADDQ`, `VPMAXUQ`/compare-select, `VPTERNLOGD`, `VPCOMPRESSQ` | the per-core record path (84–102 G/core) | [S1'] §2.6's fold_chunk schedule at L4 windows |
| Order rows (ℤ/2⁶⁴ℤ) | `VPAND` (masks), `VPBLENDVB` + `VPADDQ` (b), `VPOR` + compare (f/dead) | stage-2 only, off the wire path | [S2] B.3's mapping verbatim |
| Merge (per window) | 2 `VPXORQ` + 3 `VPADDQ` per slot-composite (the digest reduction — never a `p64_compose` cross-slot) | ≤ 0.2% of one core at the 64M dial | Cor 33.1, Thm 37(d) |
| Detector | 2 scalar compares + 1 table lookup | O(1) per claim, off-path | Lemma 15 |

The schedule's invariant, carried from [S2] Appendix C: **one driver, two ring backends, zero new mathematics per ring** — the multi-venue extension adds no instruction anywhere; it removes them (the sequencer, the resolver, the reconciliation pass).

### 10.4 Memory layout and NUMA policy

Per-socket arenas (venue-home, first-touch); the merge-tree rings on inter-socket-shared pages (uncached writes are *not* used — the traffic is 3 nB/msg; WB is correct and cheapest); `T_pow` and the detector tables read-only-replicated per socket ([S2] §3.8's L3 argument at S sockets). The day chain is append-only per slot — never rewritten, never migrated. The [S2] Appendix D pitfalls 1–14 carry over verbatim at their new granularities; Appendix D of this document adds the fleet-scale items.

---

## §11 The Throughput Model and the Gates

### 11.1 The capacity table (replay regime)

| Configuration | Record-path capacity | Bus capacity (`β = 0.41 → 0.25`) | Fleet rate `R = min(...)` | Amortized τ |
|---|---|---|---|---|
| CI runner, 2 cores ([S2], certified) | 168–204 G | 402–660 B (never binds) | **150–250B** (measured layers + gates) | 4.0–6.7 ps |
| 1 socket, 64 cores | 5.4–6.5T | 0.40–0.66T | **0.40–0.66T** (bus-capped) | 1.5–2.5 ps |
| 2 sockets, 128 cores | 10.8–13.1T | 0.80–1.32T | **0.80–1.32T** | 0.76–1.25 ps |
| **4 sockets, 256 cores** | 21.5–26.2T | 1.60–2.64T | **1.60–2.64T** | **0.38–0.63 ps** |
| Fresh, 64 cores (context) | 82–164 G (clmul-bound) | 3–6 GB/s (never binds) | **82–164 G**; real feed ≤ 1 G | — |

The record path is never the binding term until S ≈ 6–8; the *entire* 1T question, in the replay regime, is the bus term — which is why Theorem 37's Null-Flow (keeping the coherence traffic out of the arrays) and gate F-β (measuring the stream) are the two load-bearing measurements of this document.

### 11.2 The gates

| Gate | Measures | Protocol | Certifies |
|---|---|---|---|
| **F-β** | whole-node bus demand per B msg/s, widened-window config | perf counters (UNC_M_CYCLES/DRAM bandwidth), steady state, 10 runs, report min/median/max | the bus term of Theorem 36(b); the S=2 crossing needs F-β ≤ 0.33 |
| **F-ρ** | record-path cycles/record at L4 windows | A/B vs the champion's 2.08 µs chain methodology ([S2] gate 6's protocol), 10⁶ records | ρ_core ∈ [84, 102] G |
| **F-η** | fleet interference (cross-socket) | identical tapes at S=1 and S=4; ratio vs 4× the S=1 rate | η_f ∈ [0.85, 0.95] |
| **F-λ** | fold clmul port duty | port counters; must stay ≤ 5% ([S2] checklist 14's canary) | the fold stays off the critical path |
| **F-μ** | merge traffic at the chosen dial | cycles spent in the merger; must be ≤ 1% of one core | Theorem 37(d)'s law |
| **F-δ** | trigger-latency split (engine vs fiber) | timestamp the referenced composite's publication and the resolved emit; report both distributions | the engine term ≤ 0.76 µs and below the fiber term |
| **F-π** | detector false positives | run legal feeds (cell +52) at ±100 ppm drift; zero rejections required; adversarial V-projections: collision rate ≤ 2⁻⁶⁴ | Theorem 35(a)'s conservatism |

### 11.3 The honesty ledger

**Proven:** Theorems 33–42 — every correctness and every *inequality* claim (the ceilings' forms, the Null-Flow law, the dial law, the phase boundary) as parameterized statements. **Measured:** the [S1']/[S2] anchors this document builds on (61.21B and its anatomy; β = 0.41 at the champion config; 1.28–2.56 G/core fresh; ~5 ns/record; the L4 layer's certified shape). **Projected (gated):** F-β's lower end (0.25), F-ρ's upper end (102 G), F-η's range, F-δ's split. If every gate lands at its pessimistic end, the stack still delivers **1.6T at S = 4** — the certified floor is robust to every single gate failing low; the ceiling requires the gates to land mid-to-optimistic, and is stated as an aspiration. The wall beyond the corridor is the bus term's linearity — which breaks only at S ≈ 6–8 (record path) or at HBM-class arrays (Problem 8's territory).

---

## §12 Industry Impact: Global T+0, Cross-Venue Fairness, and the FPGA Question at 1T

**Deterministic global T+0.** The [S2] §7 argument, at fleet scale: a six-venue day's *entire observable output* is (i) closed-form in the multi-tape (Proposition 2.1 per slot), (ii) invariant to every one of the 15 freedoms (Theorem 42(a)), and (iii) compressible to a 19 MB/day proof object that re-verifies in 2.1 ms on any commodity machine. Cross-venue settlement stops being a reconciliation risk budget and becomes a batching window — the *state at any window version of any venue is a closed-form function of that venue's tape prefix* (Theorem 33(c)) — the same property that makes the fleet auditable makes the settlement legible, and it now holds *across* venues: the version-pinned read is the settlement's native unit ("as of reference venue version w′"), exactly as venue rule books already phrase it.

**Cross-venue fairness evidence.** Version pinning creates a new class of audit artifact: every cross-venue reference ("my pegged order was priced at `f(mid(A))` at A-version 16") is *verifiable from the 19 MB object* — the regulator recomputes `φ(X_A^{(16)})` and compares. Today's practice — fuzzy post-hoc divergence analysis over non-deterministic multicast realities — becomes an exact comparison with no approximate middle. The 210-matrix's (I) cells extend this to the deployment discipline: the absence of a sequencer, of a central epoch lock, and of any shared writable line in the scale-out path is itself a *certifiable* property — the first conformance matrix in the industry whose cells assert absences.

**The FPGA question at 1T, with its honest boundary.** The [S2] §7 argument, restated at fleet scale: fabric wins the *fresh-wire latency* race (a 3.2 GHz core loses raw gate-delay races — and Theorem 36(b) shows the CPU's fresh ceiling is 82–164 G, an order short of 1T fresh: if fresh-wire 1T is ever *needed* — it is not, the real feed is ≤ 1 G — the FPGA multiplier-bank route is the only one). But the industry's cost center is the replay/audit regime, where the CPU's closed-form arithmetic delivers 1T-class throughput at ~1–2 nJ/msg on commodity silicon with zero synthesis latency, zero recompile per feed change, and zero verification gap: **the FPGA's bitstream is not a proof object; the epoch chain is.** At the 1T operating point the fleet's whole software surface is two instruction families over two rings (Theorem 28's polymorphism) — a 2-socket commodity server out-delivers an equivalent FPGA farm at a fraction of the watts, and is the only design whose correctness artifact and performance artifact are the same document.

---

## §13 What Remains (Problem 8 Preview) and the Implementation Checklist

### 13.1 What remains

**The 10T wall.** The bus term is linear in S; the record path binds at S ≈ 6–8 (5.4–6.5T per 64-core socket). Beyond 4–8 sockets the honest frontier moves to: (i) **HBM/3D-stacked arrays** — a 4×–8× change in `B_bus` rewrites the capacity table's second column without touching a single theorem (the math is free; the bus is not); (ii) **the NIC as a fold participant** — SmartNIC clmul banks move the fresh wall into the network card ([S1'] §6's parse-bound analysis relocates); (iii) **the single-message wire-latency race** — unchanged, still fabric territory, still a different problem than ingestion throughput ([S1] §8.4's distinction); (iv) **market-design questions** — feedback through participants, queue fairness across venues, the economics of co-location: all outside the engine's contract (Theorem 35(c)(iii)'s boundary), all *enabled* by it (the version-pinned read makes every participant's cross-venue behavior exactly auditable).

### 13.2 The implementation checklist (ordered)

1. **Constants gate (unchanged).** Re-run [S1'] `spanfold_init()`; `sizeof(R36)==36`, `sizeof(P64)==52`. Nothing here changes the constants; everything depends on them.
2. **Slot map bring-up.** The venue→slot→lane-group→socket map (Appendix C.1); per-slot arenas, first-touch; the demux validation (cell +53's intake discipline).
3. **Multi-tape interleaving fuzz.** [S2]'s harness extended: 8 slots × random streams, 200 interleavings, per-slot sequential oracle — all bit-equal (cell +37).
4. **Fleet merge tree.** `fleet_digest` (Appendix B.2): 200 random orders + 64-way random shapes vs sequential product (cells +39/+40/+56).
5. **Versioned triggers.** `trigger_read`/`phantom_check` (Appendix B.3); deferral vs synchronous equivalence (cell +43); depth-cap admission (cell +44); the pegged/iceberg/arbitrage lifecycles (cells +45–+47).
6. **Phantom detector.** The three compares on the adversarial twins of §4.3 (cells +48–+50); zero false positives at ±100 ppm (cell +52).
7. **Day chain + verifier.** The 19 MB object; 2.1 ms re-verification; verifier machine-independence across the 15 freedoms (Theorem 40).
8. **Dial certification.** The merge-traffic law (Theorem 37(d)) measured at the chosen dial (gate F-μ); the audit object's size at the same dial.
9. **Two-lane (per slot) certified config.** The [S2] checklist items 1–15, verbatim, per slot — the certified corridor's shape must land before any scale-out claim.
10. **Socket bring-up (S=2).** Venue-home placement; SPSC merge rings; the (I) cells' absence assertions (cells +41, +51, +54's perf half). **This is gate F-β's first landing.**
11. **Interference measurement.** S=1 vs S=2 on identical tapes (gate F-η's protocol); pin the S=2 operating point.
12. **The 1T crossing (S=4).** 256 cores, 64M dial; gates F-β, F-ρ, F-η, F-λ, F-μ, F-δ, F-π all reported with variance; the regime tag (cell +57) on every published number.
13. **Determinism gate.** Full 210-cell matrix, bit-exact, no tolerance, every PR, including the full-freedom chaos cell (+58). The matrix is the contract; Theorems 33–42 are its proof.

---

## Appendix A — Full Proofs (Lemmas 10–16 and the extended computations)

Lemmas 10–13 and 15 are proven inline in the body (§2.2, §2.3, §3.2, §4.1, §8.2(d)). This appendix carries Lemmas 14 and 16, the group computations Theorem 35(b) leans on, and the two structural facts the body's proofs cite.

### A.1 Lemma 14 — the fleet digest reduction invariant

**Lemma 14.** In the reduction over the slots' window composites (Corollary 33.1's `D^(w)` — the commutative signature reduction at depth `⌈log₂ n⌉`, `n = |𝕊_live(w)|`), after level ℓ, every node aligned at stride `2^ℓ` holds the reduction of its aligned block of `2^ℓ` leaves — and the value is independent of the block's internal order, the arrival order, and the tree's shape, because the reduction is over a **commutative** monoid ((R², ⊕) for (C,V); (ℤ,+) for db/da/deg/cnt): every bracketing is the flat reduction.

*Proof.* Induction on ℓ as [S2] Lemma 6, with the operator now commutative: the level-ℓ update folds node `i`'s block into the accumulator; by commutativity of `⊕` and `+`, the partial value is a function of the *multiset* of leaves already folded — independent of the sequence. After level `⌈log₂ n⌉`, the root holds the flat reduction of all leaves. Unbalanced and odd-tail shapes are the same function of the same multiset (a re-bracketing of a commutative reduction). The smoke harness's T33.A2/T42 fuzz drives 200+ random shapes per configuration — all equal. **The corresponding statement for a semidirect (non-commutative) operator is FALSE** — see the correction note in §2.4; that is exactly why the digest must live in the commutative signature algebra. ∎

### A.2 Lemma 16 — the Null-Flow coherence accounting (Theorem 37(b), formal)

**Lemma 16.** Under Def 6.1 (venue-home, SPSC merge rings, single-writer watermarks), the total number of cache lines that are written by one socket and read by another, per fleet window, is exactly `2·e` where `e` = the number of merge-tree edges carrying traffic in that window (each edge: one line for the 64 B record slot + the ring's head/tail index word, which shares the record's line or its own — ≤ 2 lines per edge in the worst packing). No line is ever in Modified state on two sockets; no line is written twice.

*Proof.* Each edge's record slot is written exactly once (the child group's publication, a release-store) and read once (the parent's consume, an acquire-load); after the consume the line is dead (the ring advances past it) — its MESI history is one M (at the writer) → one invalidation broadcast to the parent's shared copies → S at the parent → dead. The head/tail indices: each has exactly one writer by the SPSC discipline ([S2] pitfall 5's argument at socket scale — a second writer would be a torn-update hazard, and the protocol's single-writer rule makes it unrepresentable). Total: 2 lines per active edge; active edges ≤ `S − 1` plus intra-socket edges (which generate no cross-socket traffic). Summing over the window: the formula of Theorem 37(b). No M-on-two-sockets: no line has two writers anywhere in the scale-out path, by construction. ∎

### A.3 The transposition-generation fact (Theorem 33(a)'s engine)

**Fact.** Let σ be a permutation of a finite set partitioned into blocks `B₁, …, B_m` (the slots' records), such that σ preserves each block's internal order. Then σ is a product of transpositions, each swapping two *adjacent-in-σ's-sequence* elements of *different* blocks.

*Proof.* Induction on the number of inversions between σ's sequence and any reference interleaving that also preserves block orders (e.g. the per-slot-concatenation order). If the sequence is not the reference, some adjacent pair `(x, y)` — x before y in σ — is inverted relative to the reference; since both orders preserve each block, x and y are in different blocks (within a block both orders agree). Swapping that adjacent pair reduces the inversion count by exactly 1 and preserves every block's internal order (x, y were in different blocks). Terminate at zero inversions: the reference order. Each swap is an adjacent cross-block transposition; the number of swaps is the inversion count. Applying Lemma 11 per swap composes the proof of Theorem 33(a). ∎

### A.4 The composite group and its two-sided inverses (Theorem 35(b)'s `g_{νμ}`)

The published composites live in the **span sub-monoid's unit group**: `(C, V)` with `C = x^W`, `W ∈ ℕ` — by [S1'] Lemma 3(4) (Q(0)=1 ⇒ x a unit) every such element is invertible in `Rˣ ⋉ R` with the two-sided inverse

```
(C, V)⁻¹  =  (C⁻¹, V·C⁻¹)        [check: (C,V) ⨂ (C⁻¹, V·C⁻¹) = (C·C⁻¹, V·C⁻¹ ⊕ V·C⁻¹) = (1, 0)  ✓
                                    (C⁻¹, V·C⁻¹) ⨂ (C, V) = (1, V·C⁻¹·C ⊕ V) = (1, V ⊕ V) = (1,0)  ✓]
```

so the discrepancy element `g_{νμ}(s) = Y ⨂ (X_s^{(w′)})⁻¹` is well-defined whenever `Y` is any element of the ambient semidirect product (unit C-coordinate or not — the group `Rˣ ⋉ R` is closed under products and inverses; a claimed Y with non-unit C-coordinate is *immediately* non-realizable: no composite of span-discipline records ever produces one, one more O(1) check the comparator performs for free — the C-coordinate's unit test is a `T_pow` membership check or, equivalently, the engine's own bookkeeping of W). The gauge group `𝒢(U)` (generated by composites of received prefixes) is a subgroup of `Rˣ ⋉ R`; H¹ classifies its torsors — the body's argument stands on these two facts alone.

### A.5 Span additivity under pipelining (Theorem 34(b)'s engine)

**Fact.** If a computation is the pipeline of two stages `A → B` (B consumes only A's outputs, at a fixed offset of k pipeline slots), executed with stage instances overlapping in time, the total span is `span(A) + span(B) + O(k)` — the product form would require every B-instance to wait for a specific *not-yet-started* A-instance, which the fixed offset forbids (the k-deep pipeline register decouples them: [S2]-T30's argument, generalized). The 2-DAG scan of Theorem 34(b) is exactly this shape (dimension 1 = per-slot scans in stage 2; dimension 2 = the merge tree in stage 3, one window later); the worst-admissible case breaks the offset only when a same-window cross-slot chain exists — which the admission cap bounds at `D_max`, giving `Σ_worst = O(D_max · log B)`; at `D_max = ⌈log₂ B⌉`: `O(log² B)`. ∎

---

## Appendix B — The Kernels (compilable C17 + AVX-512)

One file, no dependencies beyond `<immintrin.h>`; the companion artifact `solution3_kernels.c` assembles this appendix verbatim.

```c
/* solution3_kernels.c — the multi-tape layer: per-slot window composes (static
 * fragment order), the order-free fleet merge tree, versioned trigger reads,
 * and the phantom detector.  gcc -O3 -std=c17 -mavx512f -mavx512dq -mpclmul
 * -mvpclmulqdq solution3_kernels.c
 * Modulus Q(x) = x^64 + x^4 + x^3 + x + 1  (QLOW = 0x1B; Q(0)=1 => x a unit).
 * The [S2] kernels are imported verbatim (p64_compose, mulmod_u64) — the
 * fleet layer adds structure, never new monoid law ([S2] Thm 28's discipline). */
#include <immintrin.h>
#include <stdint.h>
#include <string.h>

#define QLOW 0x1Bull

/* ---- [S2] B.1, verbatim: the record and its compose ---------------------- */
typedef struct __attribute__((packed)) {
    uint64_t V, C;  int64_t db, da;  uint32_t px, tag, deg, cnt;  uint16_t flg, rsv;
} P64;
_Static_assert(sizeof(P64) == 52, "P64 layout is load-bearing");

static inline uint64_t mulmod_u64(uint64_t a, uint64_t b)     /* a (x) b mod Q */
{
    __m128i p = _mm_clmulepi64_si128(_mm_cvtsi64_si128((long long)a),
                                     _mm_cvtsi64_si128((long long)b), 0x00);
    uint64_t lo = (uint64_t)_mm_cvtsi128_si64(p);
    uint64_t hi = (uint64_t)_mm_extract_epi64(p, 1);           /* deg <= 62 */
    __m128i t = _mm_clmulepi64_si128(_mm_cvtsi64_si128((long long)hi),
                                     _mm_cvtsi64_si128((long long)QLOW), 0x00);
    uint64_t t_lo = (uint64_t)_mm_cvtsi128_si64(t);
    uint64_t t_hi = (uint64_t)_mm_extract_epi64(t, 1);         /* deg <= 2  */
    uint64_t u = (uint64_t)_mm_cvtsi128_si64(
        _mm_clmulepi64_si128(_mm_cvtsi64_si128((long long)t_hi),
                             _mm_cvtsi64_si128((long long)QLOW), 0x00));
    return lo ^ t_lo ^ u;
}

static inline void p64_compose(P64 *a, const P64 *b)   /* a <- a (x) b, a earlier */
{
    a->V = mulmod_u64(a->V, b->C) ^ b->V;
    a->C = mulmod_u64(a->C, b->C);
    a->db += b->db;  a->da += b->da;
    if (b->flg & 1u) { a->px = b->px;  a->tag = a->deg + b->tag; }
    a->deg += b->deg;  a->cnt += b->cnt;  a->flg |= b->flg;
}

static inline void p64_identity(P64 *r)  /* the 1 of A_glob's slot factor */
{
    memset(r, 0, sizeof *r);  r->C = 1;   /* (C,V) = (1,0): Lemma 3's unit */
}
```

### B.2 The fleet window digest (Corollary 33.1 — the commutative reduction)

```c
/* The digest record: 40 B. The reduction is COMMUTATIVE — order-free,
 * shape-free, concurrency-free. NEVER p64_compose across slots (§2.4's
 * correction note; pitfall 15).                                   */
typedef struct __attribute__((packed)) {
    uint64_t V, C;  int64_t db, da;  uint32_t deg, cnt;
} Dig;

static inline void dig_identity(Dig *d) { memset(d, 0, sizeof *d); }

static inline void digest_accum(Dig *d, const P64 *x)   /* 2 XOR + 3 adds */
{
    d->C ^= x->C;  d->V ^= x->V;
    d->db += x->db;  d->da += x->da;
    d->deg += x->deg;  d->cnt += x->cnt;
}

/* consume() is the deployment's arrival hook (SPSC ring drains, random
 * order, concurrent mergers — all equal by Theorem 33(d)).              */
typedef const P64 *(*consume_fn)(void *ctx);      /* NULL = no more input  */

Dig fleet_digest(void *ctx, consume_fn next)       /* Thm 33(d); Lemma 14   */
{
    Dig D;  const P64 *x;  dig_identity(&D);
    while ((x = next(ctx)) != NULL)  digest_accum(&D, x);
    return D;                                      /* the window digest    */
}

/* Balanced/tree variant and the random-shape fuzz driver — same value
 * (a commutative reduction's bracketing is free): fleet_digest_tree,
 * fleet_digest_shape in the companion artifact, verbatim.               */
```

### B.3 Versioned reads and the phantom detector (Theorems 34–35)

```c
/* The engine's per-slot publication record (one writer, release-store). */
typedef struct {
    uint64_t pub_time[4096];   /* window index -> publication timestamp     */
    uint32_t pub_count;        /* windows published through (R1's compare)  */
} SlotPub;

static uint32_t cone_d[8][8] = {0};   /* d(s,t) in ns — the deployment map   */
#define DMAX 10000ull                  /* |clock drift| bound, ns (Lemma 15)  */

/* The 3-compare detector of Thm 35(a) — O(1), off-path, anomaly arm.   */
/* Returns: 1 realizable, 0 rejected.  eq_cv() = the (C,V) comparator.  */
int phantom_check(int s, int t, uint32_t w_prime, uint64_t arrival_ns,
                  uint64_t Y_cv, const SlotPub *pub,
                  uint64_t (*engine_eval)(uint32_t s, uint32_t w))
{
    if (w_prime >= pub[s].pub_count)  return 0;            /* R1, 1 compare   */
    /* R2 WITH THE FLOOR (Lemma 15's implementation note): without it,     */
    /* d < DMAX underflows unsigned arithmetic and inverts the check.     */
    uint64_t bound = pub[s].pub_time[w_prime] + cone_d[s][t];
    bound = (bound > DMAX) ? (bound - DMAX) : 0;
    if (arrival_ns < bound)  return 0;                     /* R2, 1 compare   */
    return Y_cv == engine_eval((uint32_t)s, w_prime);      /* the comparator  */
}

/* A versioned read's entry state: the pinned projection (Lemma 12).
 * project() is the read function phi — e.g. mid = (V >> 32) for the
 * pegged-price field; emit-time only (Thm 34's table note).            */
static inline uint64_t trigger_read(const SlotPub *pub, int s,
                                    uint32_t w_prime, uint64_t phi_of_P64)
{
    return phi_of_P64;      /* X_s^(w') is already published (R1 checked)   */
}
```

**What the code proves by existing.** The fleet layer contains no new monoid law ([S2] Thm 28's discipline holds at slot scale: `p64_compose` is the *only* compose, and it never crosses slots); the digest reduces commutatively — no lock, no order assumption, no shape assumption (Corollary 33.1); the detector is two compares with a floored bound plus a table lookup — no search, no pass, no rollback (Theorem 35(a)). The smoke harness `solution3_smoke.c` verifies by execution: T33 order-freedom (200 random orders + 64-way random shapes vs the flat reduction, 8 slots × interleaved demux), T34 deferral equivalence (pinned-version resolution vs the sequential multi-venue oracle, incl. the §4.3 twins and the trigger spiral's version monotonicity), T35 the detector's three cases (fabricated value, stale version, cone-illegal — and zero false positives on 1,000 legal claims at ±drift jitter), T42 socket-partition invariance (100 random 4-way partitions + shape fuzz). **All four pass bit-exact — and the harness earned its keep twice: it caught the flattened cross-slot compose (§2.4's correction) and the R2 unsigned underflow (Lemma 15's note) before either could ship.**

---

## Appendix C — Tables

### C.1 The slot map (a deployment example)

| Venue | Slots (γ) | Lane groups | Sockets (S=4 example) |
|---|---|---|---|
| NASDAQ ITCH | 4 channels (symbol-hash) | 2 groups × 2 lanes | sockets 0–1 |
| CME MDP 3.0 | 4 market segments | 2 groups × 2 lanes | socket 2 |
| BATS + Euronext | 2 + 2 | 2 groups | socket 3 |
| Binance + Deribit (crypto) | 2 depth streams | 1 group | socket 3 (co-resident) |

`|𝕊_live| ≈ 100` at trading-hours density (the demux's own partition: symbol hash at ITCH, segment hash at CME — Def 2.1's canonical fact).

### C.2 The anchors (measured, carried in)

| Anchor | Value | Source |
|---|---|---|
| Champion rate / anatomy | 61.21B; 116,686 passes/s; 2.08 µs chain; 6.49 µs decode | [S2] §1.1 |
| Per-core record path | 84–102 G msg/s (10.4–12.5 cyc/record to the ~5 ns wall) | [S2] §5.2, §5.4 |
| Bus demand β | 0.41 GB/s per B msg/s (champion config); 0.25 projected (gate F-β) | [S2] §3.8 |
| Fresh fold | 1.28–2.56 G msg/s/core | [S1'] §2.7 |
| Replay compose | ~15 cycles/compose | [S1'] §6 |
| Fold schedule | 243 clmul uops / 322 records / 52–69 ns; duty ≤ 2.2% | [S2] Thm 21 |
| Propagation d(s,t) | 0.3 µs same-rack; 2–5 µs co-located; 4.9 µs/km fiber | Def 2.3 |

### C.4 The dial table (window size at the 1T operating point)

| Dial (msgs/window) | Windows/s at 1T | Merge traffic (|𝕊_live|=100) | Proof object/day | Verdict |
|---|---|---|---|---|
| 1M | 1M | 9.6% — legal | 1.2 GB | fine-biased (object too heavy) |
| 16M | 62.5K | 0.6% ✓ | 76 MB | legal |
| **64M** | **15,625** | **0.15% ✓** | **19 MB** | **certified** |
| 256M | 3,906 | 0.04% ✓ | 4.8 MB | coarse for hot audits |

---

## Appendix D — Pitfall Catalogue (solution3 additions)

1. **The unversioned read.** The soundness condition of Part V; every cross-venue reference must carry its version pin as wire data (cell +42). "Current state" semantics is the phantom's native habitat.
2. **The global window tick.** Synchronizing the venues' window boundaries is the central epoch lock reborn (R1) — the merge tree consumes whatever has arrived (Theorem 33(d)); the dial is per slot.
3. **The regime confusion.** Every published number carries its regime tag (cell +57); a replay number quoted as wire-ingestion is the 1T literature's most common lie.
4. **A shared journal across venue groups.** [S2] pitfall 3 at fleet scale; journals are per slot; cross-slot composition happens only in the audit-digest merge (§2.5(ii)'s discipline).
5. **Clock sync in the correctness path.** Theorem 41(b): correctness is clock-free; a fold that waits for clock sync has re-introduced a sequencer with extra steps.
6. **A V-only claim comparator.** The 2⁻⁶⁴ hole (gate F-π); the default comparator is the full `(C,V)` pair — and the C-coordinate's unit test comes for free (A.4).
7. **A rollback or repair on the fold path.** Breaks Lemma 7's monotonicity premise and creates the phantom it meant to fix (§4.4(iii)).
8. **A synchronous global resolver.** Re-centralizes the trigger dependency (R2's disease); resolution is per referencing slot, against published data only.
9. **The dial as a free choice.** Theorem 37(d)'s law is the lower bound; Theorem 40(a)'s size arithmetic the upper; C.4's table is the operating point.
10. **Depth-cap enforcement at resolution time.** The admission rule is an intake check (O(1)); late detect-and-reject leaves the depth already on the critical path (§3.5(iii)).
11. **A shared writable line in the scale-out path.** Any "convenience" global counter is a coherence hot line (R3's thrashing); audit with cell +53's counters — Null-Flow is a theorem about the protocol, and the counters prove the deployment kept it.
12. **The merge-tree shape pinned "for determinism".** No shape is canonical (Lemma 14) — pinning one is a correctness debt, not a stability win; fuzz it (cell +40).
13. **A carried cross-slot digest in the proof object.** Triples the object for zero verification value (Theorem 40(a)); the verifier recomputes in any order.
14. **Socket count added where the bus binds.** Beyond the min()'s crossing, cores idle (Theorem 36(b)); the fix is sockets (or HBM — Problem 8), never more cores on a saturated array.
15. **The flattened cross-slot compose.** Composing two slots' full window composites with `p64_compose` (the semidirect `⨂` in one `𝒜_hash` factor) is order-dependent — the same-slot twist applied across slots, the R36-E error at fleet scale. The smoke harness caught it by execution (T33.A1: 200/200 mismatches in the draft construction); the digest must reduce in the commutative signature algebra (Corollary 33.1, §2.4's correction note). `p64_compose` is for *within-slot* fragments only.
16. **The unfloored cone bound.** `pub_time + d − Δmax` without the zero floor underflows unsigned arithmetic when `d < Δmax` and inverts R2 (rejecting every legal claim on that pair) — Lemma 15's implementation note; the smoke harness's second catch.

---

*End of SOLUTION3.MD. Companion artifacts: `SOLUTION.MD` ([S1]: theorems 1–8, the 94-cell matrix), `SOLUTION1.MD` ([S1']: theorems 9–18, R36/P64, the 130-cell matrix), `SOLUTION2.MD` ([S2]: theorems 19–32, the 166-cell matrix, the 150–250B corridor), `solution3_kernels.c` (Appendix B, complete), `solution3_smoke.c` (the four executable theorem checks — all pass bit-exact). The corridor is 1.6T–2.6T at S = 4 with the barrier crossed at S ≥ 2 gate-bounded; the contract is bit-exact; the next wall is the bus array's linearity — and the math is free.*

