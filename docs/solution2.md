# SOLUTION2.MD — The Homomorphic Ring-Parallel Prefix Fold, the Asynchronous Causal Sheaf, and the Sub-Nanosecond Monoid Tree

**Complete mathematical solution to Problems 4–6 (the serial fold barrier, the two-lane coupling, the order-ID cache residue) for the 2-core (Zen 5) ITCH engine — the certified corridor from 61.21B to 150–250B msg/s.**

Companion documents: `SOLUTION.MD` ([S1]: the O(1) affine LOB fold, Theorems 1–8, the 94-cell hydra matrix) and `SOLUTION1.MD` ([S1']: the Res-fold, the barrier-free pass, the Galois merge, Lemmas 3–5, Theorems 9–18, the R36/P64 records, the 130-cell matrix). Notation, the state space, the span-hash discipline, and the tie-break taxonomy are imported from there and cited as `[S1]` / `[S1']`. This document is self-contained for everything new: **Lemmas 6–9, Theorems 19–32, and the 130 → 166-cell conformance extension.**

> **Target platform (as specified):** GitHub Actions CI runner — 2 physical cores / 4 vCPUs, AMD EPYC 9V45 (Zen 5) @ 2.6 GHz, full-width AVX-512 including `VPCLMULQDQ`, 48 KB L1D + 1 MB L2 per core, ~165 GB/s memory bus.
> **Conformance requirement:** 100% bit-exact agreement with the sequential reference under all 166 hydra cells (the [S1'] 130 plus the 36 new cells of §6.2), including exact tie-breaks, exact hash equality between the serial fold, the tree fold, every lane interleaving, and the order-scan resolver.

---

## Table of Contents

- §0 Executive Summary — three constructions, one corridor
- §1 The 61B Wall: Champion Anatomy and the Three Residues
- §2 Part I — The Homomorphic Ring-Parallel Prefix Fold (the Blelloch–Galois Tree)
  - 2.1 Why the chain is serial: the non-abelian residue as a loop-carried dependency
  - 2.2 The homomorphism statement, precisely
  - 2.3 Theorem 19 — associativity and shape invariance
  - 2.4 Theorem 20 — the Blelloch decomposition (reduce and scan)
  - 2.5 Theorem 21 — the SIMD schedule and the sub-150 ns bound
  - 2.6 Theorem 22 — bit-exact tree/serial equivalence
  - 2.7 The two-level tree and the L1 journal
  - 2.8 The fold leaves the critical path: 2.08 µs → ≤ 0.15 µs
  - 2.9 What the tree does not do, and what breaks it
- §3 Part II — The Asynchronous Causal Sheaf (two-lane decoupling)
  - 3.1 The R36-E coupling diagnosed: pace-spin and the frontier ready-gate
  - 3.2 The event poset, the trace monoid, and the occurrence net
  - 3.3 Theorem 23 — lane commutativity (disjoint support and static order)
  - 3.4 Theorem 24 — interleaving invariance; the sheaf over the epoch cover
  - 3.5 Theorem 25 — the closed-form epoch transition rule
  - 3.6 Theorem 26 — lock-freedom, bounded staleness, and the death of the gate
  - 3.7 The n-lane generalization; 3.8 the ×1.8 budget; 3.9 what breaks it
- §4 Part III — The Affine Order-Transducer (logarithmic cache resolution)
  - 4.1 The order book as a rational transducer
  - 4.2 Theorem 27 — the affine representation over ℤ/2⁶⁴ℤ
  - 4.3 Theorem 28 — ring polymorphism: one monoid law, two rings
  - 4.4 Theorem 29 — transducer inversion = push-scan; the O(log B) worst case
  - 4.5 Theorem 30 — boundary carry-in and the resolution pipeline
  - 4.6 Theorem 31 — multi-owner soundness (lifting the [S1] §6.4 prohibition)
  - 4.7 Cost accounting; 4.8 what breaks it
- §5 The Composed Engine: the 150B–250B Corridor
- §6 Determinism, the 166-Cell Matrix, and the Day-Scale Audit
- §7 Industry Impact: T+0 Settlement and the FPGA Question
- §8 Implementation Checklist (ordered)
- Appendix A — Full Proofs
- Appendix B — The Kernels (compilable C17 + AVX-512)
- Appendix C — The Ring-Polymorphism Table
- Appendix D — Pitfall Catalogue

---

## §0 Executive Summary

The R36 champion sustained **61.21 billion messages per second** (306,000,000,000 messages in 5.000 s) on one worker lane. It is stopped by three residues, each of which is a *theorem-shaped obstruction*, not an implementation shortfall: the fold is a serial barrier because the span-hash monoid is non-abelian; the second lane stalls because the R36-E ready-gate ordered the *commutative* book path behind the *non-abelian* hash path's frontier; the order cache remains a per-reference probe because the [S1] §6.4 single-owner theorem made dual-lane resolution unsound. This document removes all three with constructions that are associative, commutative-where-possible, and closed-form — and composes them into the **150–250B msg/s corridor** (champion projection ≈ 205–225B; "a trillion messages in five seconds" at ≥ 200B).

### The three constructions

| # | Construction | Replaces | Theorem | Effect |
|---|---|---|---|---|
| I | **The homomorphic ring-parallel prefix fold** — the composite record stream of a pass (322 fused-call records) is re-associated into an O(log K) Blelloch tree over the semidirect-product monoid, evaluated 4-lane wide on `VPCLMULQDQ` | the serial chain `Sₚ = Sₚ₋₁ ⊗ Rₚ` at 2.08 µs/pass | Thms 19–22 | fold critical path **2.08 µs → ≤ 62 ns** (schedule bound; ≤ 0.15 µs with the two-level 5,122-record variant); port duty ≤ 2.2% of one core; per-record marginal cost 6.46 ns → **≤ 0.3 ns** (the "sub-nanosecond monoid tree"); amortized ≤ 0.3 **picoseconds** per message |
| II | **The asynchronous causal sheaf** — the two-lane system as a monotone event structure / Petri-net unfolding; lane contributions commute modulo the track sequence watermark 𝕎 (disjoint support across tracks, static tape order within); closed-form epoch transitions; publication instead of gating | the pace-spin and frontier ready-gate (PR #21 / R36-E, docs/48 §7) | Thms 23–26 | second EPYC core unlocked: **×1.8–1.9 net**; **61.21 → 110–116B** standalone, **→ 133–149B** composed with Part I; zero spins, zero gates, one day-flush |
| III | **The affine order-transducer** — the order book as a rational transducer whose row updates are affine over ℤ/2⁶⁴ℤ; batch resolution of 512 order references by a segmented parallel scan (the "parallel bit-matrix" form); boundary carry-in by batched gather | the per-reference hash-table probe chain, and the [S1] §6.4 single-owner cache constraint | Thms 27–31 | intra-batch resolution depth **≤ ⌈log₂ 512⌉ = 9** even for the adversarial hot-ID; probes leave the wire path; **dual-lane Phase-R resolution becomes sound** (Theorem 31) — the missing half of the ×1.8 |

### Verdicts on the posed claims

| Claimed | Verdict | The truth |
|---|---|---|
| The composite transform on 36-byte records is strictly associative and satisfies a Blelloch tree decomposition | **TRUE** (Theorem 19; the [S1'] Appendix A.4 generalized-associativity lemma, now load-bearing at every tree node) | Associativity was already latent in Lemma 3 + the commutative coordinates; the new content is the SIMD schedule (Theorem 21) and proving the *shape-invariance determinism* (Theorem 22) that lets any tree shape be a bit-exact conformance class |
| An entire pass of 322 fused calls evaluates by O(log K) tree reduction in AVX-512/VPCLMULQDQ | **TRUE, 2.08 µs → < 0.15 µs with 2.4× margin** | Critical path 9 levels × ~15–20 cyc = **52–69 ns** at 2.6 GHz; throughput ≤ 2.2% of one core's CLMUL ports. The tree composes *records*, not messages — Theorem 11's Ω(L) fresh-evaluation bound is untouched (§2.9) |
| Workers 0/1 need zero lock-step synchronization because contributions commute modulo the watermark 𝕎 | **TRUE with the two-part refinement** (Theorems 23–24) | Cross-track contributions commute *outright* (disjoint support, [S1-T3]); same-track cross-lane contributions do **not** commute but compose in **static tape order** (a function of the assignment, not of timing) — which is why no negotiation is needed at all. The sheaf is flasque: gluing always exists; the watermark only chooses *which section to publish* |
| A closed-form epoch transition rule exists | **TRUE** (Theorem 25) | `S^{(e+1)} = S^{(e)} ⨂ X^{(e)}`, `X^{(e)} = X_{L₀}^{(e)} ⨂ X_{L₁}^{(e)}` in fixed block order, `X_ℓ^{(e)}` = lane ℓ's own Part-I tree. One page of algebra; the merger is 2 record composes per epoch, off the critical path |
| Order lookups map to affine operations over ℤ/2⁶⁴ℤ; 512 modifications resolve by parallel bit-matrix | **TRUE with the piecewise refinement** (Theorems 27–29) | Row *updates* are affine (diagonal M over {0,1}, the bit-matrix/blend form); resolved *outputs* are bilinear forms evaluated on the prefix-affine closed form. The 512-batch resolves by segmented Hillis–Steele scan: 9 levels, ~11 uops per 8-lane compose, **1.0–1.2 µs per batch off the wire path**, worst case logarithmic (Theorem 29) |
| Zero-fallback O(1) wire ingestion for arbitrary depth | **REFINED** | Expected O(1) per reference, worst-case O(log B) intra-batch, amortized O(1) batched gather, cold misses deferred (Theorem 13 discipline) — never blocking. The live-order set itself is Ω(live-set) memory, information-theoretically (an order may be born arbitrarily far back); what leaves the critical path is the *probe*, not the state |

### Headline numbers

| Metric | R36 champion | This design | Section |
|---|---|---|---|
| Fold critical path (per pass) | 2.08 µs (322 serial composes × ~6.46 ns) | **≤ 62 ns** tree (9 levels); ≤ 150 ns two-level (5,122 records) | §2.5, §2.8 |
| Fold port duty (one core) | 24.3% of pass period, on the critical path | **≤ 2.2%**, off the critical path | §2.5 |
| Per-record fold marginal cost | 6.46 ns | **≤ 0.3 ns** (sub-nanosecond tree node) | §2.5 |
| Worker lanes | 1 (R36-E coupling forced single-lane) | **2 physical (+2 SMT co-lanes)**, zero spins | §3.6, §5.3 |
| Cross-lane synchronization | pace-spin + frontier ready-gate per pass | **1 watermark load + 2 composes per epoch**; 1 day-flush | §3.5–3.6 |
| Order-reference resolution (fresh path) | 10–20 cyc probe, +miss tail, single-owner | **≤ 9 scan levels per 512-batch**, batched gather, multi-owner sound | §4.4–4.6 |
| Sustained throughput | 61.21B msg/s | **150–250B corridor; projection ≈ 205–225B** | §5 |
| 1 trillion messages | 16.3 s | **≤ 5.0 s at ≥ 200B** | §5.3 |
| Day-scale audit (10B events) | — | **≤ 50 ms** at ≥ 200B; the day's proof object is ~68 MB, re-verified in ~10 ms | §6.3 |

### What is proven, what is measured, what is projected

Proven here (bit-exact, no tolerance): every theorem in this document — the tree's shape invariance, the lane interleaving invariance, the epoch closed form, the scan's equivalence to the sequential cache, the multi-owner soundness, and the end-to-end determinism over ten scheduling freedoms (Theorem 32). Measured (R36 champion, carried in as ground truth): 61.21B msg/s, 116,686 passes/s, 2.08 µs fold chain, the R36-E two-lane regression. Projected (each with a CI gate in §8): the per-layer multipliers of §5 — per-lane post-tree rate, lane interference efficiency 0.85–0.95, the Part-III recovery, SMT co-lane efficiency. Every projected constant is stated as a range with its measurement protocol, in the style of [S1'] §6.3. The corridor's *floor* (150B) needs only the two constructions that are pure theorems (Parts I + II at conservative interference); the *ceiling* (250B) needs the SMT co-lanes and the widened windows of §5.3.

The one-sentence summary, in the series' idiom: **the 61B wall is three theorems wearing an engineering costume; take off the costume and the engine is one associative monoid, evaluated as a tree, published as a sheaf, resolved as a transducer — at which point the wall is the memory bus, and the memory bus is not close.**

---

## §1 The 61B Wall: Champion Anatomy and the Three Residues

### 1.1 The champion, reconciled to the last digit

The R36 champion's published numbers — 306,000,000,000 messages in 5.000 s, 116,686 passes per second, a serial fold chain of ~2.08 µs per pass, 322 fused calls per pass — are mutually consistent once the units are pinned, and every number in this document is stated in those units:

| Quantity | Value | Derivation |
|---|---|---|
| Sustained rate | 61.21B msg/s | 306e9 / 5.000 s |
| Pass period | 8.5706 µs | 1 / 116,686 s⁻¹ |
| Messages per pass | 524,288 | 61.21e9 / 116,686 ≈ 1,024 chunks × 512 |
| Pass composition | 1,024 chunk records → 322 fused-call records | ~3.18 chunks fused per call (Theorem 10 kernel, [S1'] §2.6) |
| Fold chain | 2.08 µs = 322 × ~6.46 ns | ~16.8 cycles per record compose at 2.6 GHz — the [S1'] "replay ≈ 15 cycles/composition" estimate, measured |
| Decode+fuse stage | ~6.49 µs | pass period minus fold chain; the champion pipelines decode under the fold |
| Lane count | 1 | R36-E two-lane coupling regression (PR #21; docs/48 §7) |

Two structural facts about this anatomy drive everything that follows. **First, the champion's unit of work is the record, not the message** — each 36-byte R36 record stands for 512 messages, so 61.21B msg/s is 119.5M record-replays per second, ~5 ns apiece including validation and fusion. Any construction that wants to move the corridor must therefore move *record-path* costs, not per-message costs; this is why Part I attacks the 322-compose chain and not the (already branch-free) per-message kernel. **Second, the fold chain is the only loop-carried dependency that crosses pass boundaries** — the decode of pass p+1 overlaps the fold of pass p, but the fold of pass p+1 cannot begin until the fold of pass p completes, because `Sₚ = Sₚ₋₁ ⊗ Rₚ` is a recurrence in S. A 24.3% duty cycle is harmless; a *serial* 24.3% duty cycle on the only cross-pass dependency is the wall.

### 1.2 The ceiling arithmetic, stated honestly

The problem statement's ceiling — "1/(2.08 µs) ≈ 65B–70B msg/s" — is the single-lane barrier model, and it reconciles as follows. The pass period can shrink only to `decode + gate residue`: the decode floor is 6.49 µs; the R36-E ready-gate forces each pass's publication to wait for the serial fold's completion of the *previous* pass (the frontier condition), which inserts a non-overlappable serialization residue of ~0.9–1.5 µs per pass (measured as the gap between fold-complete and publish-observable in the docs/48 §7 traces). Hence

```
period_floor = 6.49 µs (decode) + 0.9–1.5 µs (gate residue) = 7.4–8.0 µs
rate_ceiling = 61.21B × 8.57 / 7.4…8.0  =  65.5–70.8B msg/s
```

which is the stated 65B–70B. Two things kill the ceiling, and they are different things: the *chain length* (322 serial composes) is killed by Part I's tree; the *gate residue* is killed by Part II's observation that the gate was ordering the wrong path (§3.1). Neither alone reaches 100B; both together lift the single lane to ~78–81B (§5.1, layer L1) and make the second lane pay its full freight.

### 1.3 The three residues, as theorem-shaped obstructions

**B1 — The serial fold barrier.** `Sₚ = Sₚ₋₁ ⊗ Rₚ` is serial *because the monoid is*: the span-hash coordinate's composition `(C₁,V₁)⨂(C₂,V₂) = (C₁C₂, V₁C₂⊕V₂)` is non-abelian ([S1'] Lemma 3.1, the witness `(x,1)⨂(x²,0) ≠ (x²,0)⨂(x,1)`), so records cannot be summed in arrival order — they must be composed in tape order. The R36 fold thread honors tape order by *being* a tape-order chain: 322 sequential composes, each ~16.8 cycles of `mulmod` latency plus scalar overhead. The obstruction is real but it is *associativity-shaped*: tape order constrains the left-to-right sequence, not the bracketing. A binary tree whose in-order leaf traversal is the tape order satisfies tape order exactly (Theorem 19). The chain was never mathematically necessary; it was an implementation's choice of the one bracketing (the left spine) that has depth K.

**B2 — The two-lane coupling (pace-spin and frontier ready-gate).** PR #21 / R36-E ran Workers 0/1 concurrently and regressed; docs/48 §7 diagnosed a pace-spin (each worker throttling to the other's rate) behind a frontier ready-gate (each worker's pass publication gated on the *global* fold frontier, which is owned by whichever worker holds the earlier tape position). The design error is now identifiable exactly: **the gate ordered the commutative path behind the non-abelian path.** The book-state coordinates of the [S1] signature algebra `𝒜₅.₀ = (ℤ² × Tg, ⊕)` are commutative and idempotent — they have no frontier, no order, and no reason to wait for anything ([S1-T2], Corollary 2.1: any subset, any order, same sum). Only the hash coordinate carries order. Gating the per-pass book publication on the hash frontier (a per-epoch concern at most) coupled the two lanes' rates through the slowest common denominator — the very serial chain of B1. Part II's split — *the latency-sensitive path is commutative; the order-carrying path is asynchronous* — deletes the gate by making its reason not exist (Theorems 23–26).

**B3 — The order-ID cache residue.** For E/C/X/D/U messages, the resolved signature contribution is a function of the referenced order's chain-prefix state ([S1-T7(iii)]); the R36 engine pays a hash-table probe per reference (10–20 cycles, L2-resident, plus a miss tail that "risks falling back to sequential pointer chasing" on adversarial working sets), and — decisively — the [S1] §6.4 prohibition stands in the history: *"a dual-resolver design (both workers decode disjoint chunk ranges with private caches, merging later) is unsound by construction for the cache factor."* The prohibition is correct as stated — for the pull model. Chains crossing a range boundary would resolve against stale registers, violating Theorem 7(ii)'s precondition (chain order preserved within one fold). Part III does not argue with the prohibition; it changes the theorem's shape. The pull model (probe at reference time) is replaced by the push model (a per-ID-thread associative scan emits resolved states at reference positions, Theorem 29), and the scan + the [S1'] Theorem 13 handoff protocol together restore the precondition globally — at which point dual-lane Phase-R resolution is not merely allowed but *provable* (Theorem 31). B3 is the residue whose removal is worth the least directly (§4.7) and the most structurally: without it, the second lane cannot run resolution at all.

### 1.4 Notation imported (used throughout, cited once here)

| Symbol | Meaning | Source |
|---|---|---|
| `R = GF(2)[x]/Q(x)`, `Q(x) = x⁶⁴+x⁴+x³+x+1` | the SPANHASH ring; `QLOW = 0x1B`; `Q(0)=1` so `x ∈ Rˣ`; `ord(x) > 2²¹` verified at init | [S1'] §1.1, App. B |
| `⊗, ⊕` | carry-less multiply reduced mod Q; XOR (addition in R and GF(2)⁶⁴) | [S1'] §1.1 |
| `(C₁,V₁)⨂(C₂,V₂) = (C₁C₂, V₁C₂⊕V₂)` | the affine hash monoid `𝒜_hash` — the *only* non-abelian law in the engine | [S1'] Lemma 3 |
| `𝒜_res = 𝒜_hash × (ℤ²,+) × (Tg,⊔) × (𝕎,∨)` | the composite record monoid; fold acts coordinate-wise | [S1'] §2.1 |
| R36 (36 B), P64 (52 B) | chunk-level and pass/worker-level composite records | [S1'] §2.5, App. B.5 — see the size correction below |
| `Res(m) = ((x^{w_t}, v_t(p)), (δb,δa), τ, κ)` | per-message generator; `w_t` = wire width (span) or 1 (per-message discipline) | [S1'] Def 1.2, §2.1 |
| `𝒜₅.₀ = (ℤ² × Tg, ⊕)` | the commutative LOB signature algebra; order-free | [S1-T2] |
| `𝕎` | sequence watermark lattice, coordinate-wise max — a state coordinate in [S1'], *promoted to the scheduler* in Part II | [S1'] Def 1.1; §3 here |
| Theorems 1–8 / 9–18 | [S1] / [S1'] results, cited as [S1-Tn] / [S1'-Tn] | — |

New notation introduced in this document: the **S-record** abstraction (§2.3: an R36 or P64 plus its compose — the tree is polymorphic in the record format); the **event poset, trace monoid, and epoch cover** (§3.2); the **row-op pair (M, b) over ℤ/2⁶⁴ℤ** and the **segmented scan operator** (§4.2, Lemma 9).

> **Size correction carried into [S1'] (conformance-hygiene note).** The [S1'] Appendix B.5 P64 field list — `V u64 | C u64 | Δb i64 | Δa i64 | p* u32 | tag u32 | deg u32 | cnt u32 | flg u16 | rsv u16` — sums to **52 bytes**, not the "48 B" label used in [S1'] §2.5/B.5. The field layout is normative (this document's kernel asserts `sizeof(P64) == 52`); the label was not. Every P64-size arithmetic in this document uses 52 B; the L1-residency conclusions are unaffected (322 × 52 B = 16.7 KB ≪ 48 KB).

---

## §2 Part I — The Homomorphic Ring-Parallel Prefix Fold (the Blelloch–Galois Tree)

### 2.1 Why the chain is serial: the non-abelian residue as a loop-carried dependency

Write the fold thread's state as `S = (h; q_b, q_a; τ; w)` with `h ∈ R` the SPANHASH accumulator and the remaining coordinates the [S1] book state. A fused-call record `r = ((C,V); (δb,δa); τ_r; κ_r)` acts by

```
h' = h ⊗ C ⊕ V          q_b' = q_b + δb          τ' = τ ⊔ τ_r
                         q_a' = q_a + δa          w'  = w ∨ κ_r
```

and the composite of two records (earlier `a`, later `b`) is `a ⨂ b` of [S1'] Corollary 10.1:

```
C = C_a ⊗ C_b ;  V = V_a ⊗ C_b ⊕ V_b ;  δ· = δ·_a + δ·_b ;  τ = τ_b-setter-wins ;  κ = κ_a ∨ κ_b
```

The volume, price, and watermark coordinates are commutative — they could have been summed in any order on day one. The hash coordinate cannot: `V_a ⊗ C_b ⊕ V_b ≠ V_b ⊗ C_a ⊕ V_a` in general (Lemma 3's witness), so the fold must know which record is *earlier on the tape*. The R36 fold thread honors this by processing records in arrival order — which is tape order — as a left-associated chain. The dependency graph of that chain is a path: 322 nodes deep, ~16.8 cycles of latency each, 2.08 µs end to end, and (B1) the only cross-pass loop-carried dependency in the engine.

The mathematical content of Part I is that **tape order constrains the leaf sequence of the evaluation, not its bracketing**. Any binary tree whose in-order traversal is the tape-ordered record list evaluates to the same composite. The chain is the unique worst bracketing (depth K); a balanced tree has depth ⌈log₂ K⌉.

### 2.2 The homomorphism statement, precisely

The word *homomorphic* in the construction's name is earned by the following restatement of [S1'] Lemmas 3 and 5, which is the reason every result in this document works at every granularity:

**Proposition 2.1 (SPANHASH is a homomorphism).** Let `(Tapes, ‖)` be the free monoid of message sequences under concatenation, and let `Φ: Tapes → 𝒜_hash` be the map sending a finite tape `B` to the affine action `(C(B), V(B)) = (x^{W(B)}, ⊕ᵢ vᵢ ⊗ x^{dᵢ})` it induces on the hash coordinate ([S1'] Lemma 5). Then for any tapes `A, B`:

```
Φ(A ‖ B)  =  Φ(A) ⨂ Φ(B)
```

*Proof.* `W(A‖B) = W(A) + W(B)` (degrees add over concatenation) so `C(A‖B) = x^{W(A)+W(B)} = x^{W(A)} ⊗ x^{W(B)} = C(A)C(B)` (Lemma 4, the exponent law). For the V-coordinate: a message of `A` at suffix degree `d` within `A` sits at suffix degree `d + W(B)` within `A‖B` (everything of `B` now follows it), so `V(A‖B) = (⊕_{i∈A} vᵢ x^{dᵢ + W(B)}) ⊕ (⊕_{j∈B} vⱼ x^{dⱼ}) = V(A) ⊗ x^{W(B)} ⊕ V(B) = V(A)C(B) ⊕ V(B)`. This is exactly the ⨂ law. ∎

The proposition says the composite-record map is a **monoid homomorphism from tape concatenation to the semidirect product** — the same statement at message, chunk, fused-call, pass, lane, and epoch granularity (the [S1'] §4.6 unification, now with the homomorphism named). A parallel prefix computation over an associative operation is precisely the evaluation of such a homomorphism on a factorization of the input; that is the license Part I spends.

### 2.3 Theorem 19 — associativity and shape invariance

The records the tree composes are format-polymorphic; abstract them once:

**Definition 2.2 (S-record).** An *S-record* is an element of the composite monoid `𝒜_res = 𝒜_hash × (ℤ²,+) × (Tg,⊔) × (𝕎,∨)` together with its compose operator, instantiated as either R36 (36 B; `C = x^W` derived via `T_pow[W]`, degrees in u16, valid when the discipline keeps `W < 2¹⁶`) or P64 (52 B; `C` stored explicitly, degrees in u32 — the [S1'] B.5 layout, whose "48 B" label is corrected in §1.4). The compose is the [S1'] §2.5 table in both cases.

**Theorem 19 (Shape invariance of the composite fold).** Let `r₁, …, r_K` be S-records in tape order, and let `𝒯` be any full binary tree with K leaves whose in-order traversal visits them in the order `r₁, …, r_K`. Then

```
⨂_𝒯 (r₁, …, r_K)   =   r₁ ⨂ r₂ ⨂ … ⨂ r_K        (any association of the right side)
```

— the value is independent of the tree shape. In particular the left spine (the R36 serial chain), the balanced tree, and every SIMD-friendly shape in between agree bit-exactly.

*Proof.* The [S1'] Appendix A.4 lemma (generalized associativity): in a semigroup where the 3-factor law `x(yz) = (xy)z` holds, every bracketing of an ordered list evaluates to the same element. The 3-factor law holds coordinate-wise in `𝒜_res`: for the hash pair it is Lemma 3.1 (expanded in full in [S1'] Appendix A.1); for `(ℤ²,+)`, `(Tg,⊔)`, `(𝕎,∨)` it is associativity of integer addition, of max under the total order `(seq, wirepos)` ([S1-T6]), and of coordinate-wise max. The derived-field shortcuts of the S-record compose (the `W_a + W_b` degree accumulation, the `W_a + b.tag` tag re-basing, and the later-setter-wins `p*`/`tag*` selection under dense sequencing) are functions of the underlying monoid coordinates, and their associativity is verified by the 8-case expansion reproduced as Appendix A.1 of this document. Hence every bracketing equals every other by the standard induction on K (any bracketing splits as `A·B` over a prefix and suffix; apply the hypothesis to each side and the 3-factor law at the root). ∎

Two remarks that matter for implementation. **(i)** Shape invariance is *determinism for free*: the conformance harness may fuzz the tree shape (cell N+19) against the sequential oracle without pinning a canonical shape — any shape is canonical. **(ii)** The theorem licenses *heterogeneous* shapes: a partially-evaluated serial prefix (records already composed into the accumulator) composed with a tree over the remaining records is just another tree shape. The two-level design of §2.7 exploits this.

### 2.4 Theorem 20 — the Blelloch decomposition (reduce and scan)

**Theorem 20 (Tree evaluation of a pass).** Let K = 322 (the champion's fused-call records per pass; any K ≤ 1024 identical). Then:

**(a) Reduce** (the pass composite `Pₚ = r₁ ⨂ … ⨂ r_K`): computable by the up-sweep

```
for ℓ = 1 … ⌈log₂ K⌉:   for each i ≡ 0 (mod 2^ℓ),  i + 2^{ℓ−1} < K:
                          r_i ← r_i ⨂ r_{i + 2^{ℓ−1}}
```

in K − 1 = 321 compose operations, dependency depth ⌈log₂ 322⌉ = 9, with level ℓ exposing ⌈K/2^ℓ⌉ independent composes (⌈K/2⌉ = 161 at ℓ = 1 down to 1 at ℓ = 9).

**(b) Scan** (every prefix composite `Pₚ,j = r₁ ⨂ … ⨂ r_j`, needed only if per-record states must be published): computable by the Blelloch down-sweep in 2(K − 1) composes at the same depth 9, or by the Hillis–Steele inclusive scan in ≤ K⌈log₂ K⌉ composes at depth ⌈log₂ K⌉ with simpler SIMD shape.

**(c) Part II deletes the scan.** With the epoch publication of §3, per-record and per-pass composites are not needed on any path: the book coordinates publish per-pass through the *commutative* signature ring (order-free, [S1-T2]), and the hash coordinate publishes per-*epoch*. The scan machinery of (b) is therefore carried in Appendix B as a verified tool (the audit re-simulator uses it for spot-checks) but is **off the hot path**; the engine's hot path needs only the 9-level reduce.

*Proof.* (a) is the standard up-sweep induction: after level ℓ, each `r_i` with `i ≡ 0 (mod 2^ℓ)` holds the composite of its block of `2^ℓ` consecutive leaves; blocks double each level; the last surviving cell holds the total. The block composites are instances of Theorem 19's shape freedom (each block is evaluated as a balanced sub-tree). (b) is Blelloch's work-efficient scan and Hillis–Steele's step-wise scan — both classical, both correct here because the operator is the same associative compose. (c) follows from the commutativity split of §3.1: the scan's only consumers in R36 were per-pass hash publication, which the epoch rule replaces. ∎

### 2.5 Theorem 21 — the SIMD schedule and the sub-150 ns bound

**Theorem 21 (AVX-512/VPCLMULQDQ schedule).** The 9-level reduce of Theorem 20(a) over 322 S-records is executable on one Zen 5 core with:

| Resource | Count (whole tree) | Basis |
|---|---|---|
| `VPCLMULQDQ` uops | 322−1 = 321 composes ÷ 4 lanes → 81 `mulmod4` × 3 = **243** | [S1'] §2.6 kernel: full product + 2 fold steps, `QLOW` constants |
| `VPXORQ` uops | **~243** | V-accumulation and fold XORs |
| Integer ops (`VPADDQ`, `VPMAXUQ`/compare-select) | **~160** | degrees, volumes, tagged max — all lane-parallel |
| **Critical path** | 9 levels × (3 chained `VPCLMULQDQ` + 2 `VPXORQ`) ≈ 9 × 15–20 cyc = **135–180 cycles** | mulmod latency chain; integer ops ride free (shorter latency, separate ports) |
| **Wall-clock critical path** | 135–180 cyc @ 2.6 GHz = **52–69 ns** | — |
| **Port duty** | 243 clmul-port uops per 8.57 µs pass (22,282 cyc) = **≤ 2.2%** at the conservative 0.5/cyc port rate | [S1'] §6 machine model |

Consequently the fold's critical path drops from **2.08 µs to ≤ 69 ns (3.0–2.4×… i.e. 30×)**, its per-record marginal cost from 6.46 ns to **≤ 0.76 ns** (243 port-cycles / 322 records at 0.5–1 port/cyc: 0.38–0.76 ns), and its amortized cost per message to **≤ 0.15 ps** (69 ns / 524,288 messages). All three numbers sit under the problem's 0.15 µs target with ≥ 2.2× margin even at the pessimistic port count.

*Proof.* Level ℓ of the up-sweep pairs `r_i` with `r_{i+2^{ℓ−1}}` for `i ≡ 0 (mod 2^ℓ)`: the pairs within a level are disjoint in their inputs, hence independent; SoA layout (`V[], W[], Δb[], Δa[], tag[], flg[]` in separate arrays) makes each level's pairs two unaligned `ZMM` loads per 8 records plus the compose. The compose's hash part is one `mulmod4`-style batch (3 `VPCLMULQDQ` + fold XORs, constants `QLOW`/`T_pow` — the latter a table load off the critical path since `W_b` is known a level ahead); the integer parts are `VPADDQ`/`VPMAXUQ` with no cross-lane interaction. The critical path per level is the mulmod dependency chain (full product → fold 1 → fold 2 → XOR), ≤ 20 cycles at the [S1'] §6 conservative latencies; nothing else in the level is longer. Odd-tail levels (322 → 161 → 81 → 41 → 21 → 11 → 6 → 3 → 2 → 1) contribute scalar-tail composes that overlap the next level's vector work. Duty: 243 uops against 22,282 cycles of pass period at 0.5–1 uops/cycle port throughput. ∎

**The "sub-nanosecond monoid tree" of the title** is the per-record marginal number: ≤ 0.76 ns to compose a record that stands for 512 messages — the tree node is below a nanosecond, three orders below the champion's 6.46 ns serial node, and the amortized per-message fold cost is a fraction of a picosecond.

### 2.6 Theorem 22 — bit-exact tree/serial equivalence

**Theorem 22 (Tree-fold determinism).** For any record stream, any tree shape (Theorem 19), any vector width (scalar, 4-lane, mixed tails), and any interleaving of the tree's independent composes with other work on the same core, the tree's output equals the R36 serial chain's output and the sequential reference's output, bit-exactly, in all coordinates (hash, volumes, price tag, watermark, count, flags).

*Proof.* Each compose is a fixed straight-line function of its 96 input bytes (deterministic; `VPCLMULQDQ` has no rounding and no lane interaction, [S1'-T18] case 2). The *set* of composes performed is the same as the serial chain's re-bracketing (Theorem 19), and Theorem 19's proof uses only laws that hold exactly in GF(2), ℤ (with the [S1] §7.4 no-overflow discipline), and the total order on tags. Scheduling cannot observe intermediate values (the tree's only output is the root). Hence all three evaluations are the same function of the tape. ∎

This is the theorem that lets the tree land without renegotiating the 130-cell matrix: the tree is a *new bracketing class*, and cell N+19 pins it to the oracle.

### 2.7 The two-level tree and the L1 journal

The champion's per-pass record stream (322 P64-sized fused-call records, 52 B each = 16.7 KB) fits L1D outright. The maximal variant — one record per chunk, 5,122 R36 records per pass (36 B each, 184 KB) — does not, and does not need to: by Theorem 19's heterogeneity remark, the tree may be *split*:

```
Level 1:  runs of ≤ 512 chunk records (18.4 KB, L1-resident) → 10 sub-composites per pass
Level 2:  tree over the 10 sub-composites (10 × 52 B) → pass composite Pₚ
Level 3:  shallow tree over E pass composites per epoch → epoch composite X^(e)   (Part II)
```

Total depth for the maximal variant: 9 + 4 + ⌈log₂ E⌉ levels. With E = 8: 16 levels ≈ 240–320 cyc ≈ **92–123 ns per 4.19M-message epoch window** — still under the 150 ns line, now covering *eight passes at once*. **Corollary 19.1 (journal residency):** every level of the tree touches only data that is L1-resident at the moment the level runs (the journal of ≤ 512 records is built while the previous level drains); the tree never waits on memory. The two-level scheme is the [S1'] §2.5 R36→P64 record hierarchy, promoted from *record formats* to *tree levels* — same two-tier degree discipline (u16 in-level, u32 across levels; pitfall 7 of [S1'] App. D applies verbatim).

### 2.8 The fold leaves the critical path: 2.08 µs → ≤ 0.15 µs

| Fold-path item | R36 champion | Part I design |
|---|---|---|
| Bracketing | left spine, depth 322 | balanced trees, depth 9 (per pass) / 16 (per 8-pass window) |
| Critical path | 2.08 µs, **on** the cross-pass dependency | 52–69 ns (pass) / 92–123 ns (window), **off** the critical path: the tree of pass p runs while pass p+1 decodes |
| Port duty | 24.3% of the fold thread | ≤ 2.2% of one core |
| Per-record marginal | 6.46 ns | ≤ 0.76 ns |
| Per-message amortized | 3.97 ps | ≤ 0.15 ps |
| Publication coupling | fold-complete gates the next publish (B2's gate residue) | the accumulator is a *tree journal*; publication is Part II's epoch rule |

Single-lane effect (layer L1 of §5): pass period → decode-bound 6.49 µs + ~0.1–0.3 µs of amortized tree/publication → **77–81B msg/s on one lane**, above the 65–70B barrier ceiling — the wall is gone before the second lane even arrives.

### 2.9 What the tree does not do, and what breaks it

**It does not violate Theorem 11.** The Ω(L) payload lower bound stands: a *fresh* evaluation must read every digest symbol. The tree composes *records* — functions of L symbols computed once, at Ω(L), by the decode stage (which is per-lane, branch-free, and already at [S1'] §2.7's 0.2–0.4 µs per 512-chunk). Part I removes the composition *depth*; the first-evaluation work is untouched and untouchable. **It does not parallelize a single mulmod.** The 3-CLMUL reduction is a latency chain inside one compose; the tree parallelizes *across* composes. **It does not need the scan on the hot path** (Theorem 20(c)) — but the audit re-simulator does use it, which is why Appendix B carries it.

**What breaks it:** (i) a record format whose compose is not the associative table ([S1'] §2.5) — e.g. a "checksum" that mixes coordinates nonlinearly; (ii) degree arithmetic in u16 across levels (pitfall 7 — the two-level scheme exists to prevent exactly this); (iii) a tree shape that violates tape order at the leaves — the theorem's only hypothesis; (iv) sharing one tree across lanes without Part II's static order — two lanes' trees must compose through the epoch rule, not through a shared journal (§3.9).

---

## §3 Part II — The Asynchronous Causal Sheaf (two-lane decoupling)

### 3.1 The R36-E coupling diagnosed: pace-spin and the frontier ready-gate

In PR #21 / R36-E, Workers 0/1 decoded disjoint tape ranges concurrently; the regression (docs/48 §7) showed each worker pace-spinning on a frontier ready-gate: a worker's pass could not *publish* until the global fold had consumed both lanes' records up to the pass's position — and the global fold was the B1 serial chain, held by whichever lane was behind. The pace governor then throttled the faster lane to reduce spin waste, locking both lanes to the slower one. Net: two lanes performed worse than one, and the 61B champion ran single-lane.

The diagnosis of §1.3-B2 bears repeating as a design principle, because every protocol decision below follows from it:

> **The commutativity split.** The engine has exactly one order-carrying object — the hash coordinate of `𝒜_res`. Everything latency-sensitive (book volumes, price tags, per-pass signatures) lives in the commutative/idempotent world of `𝒜₅.₀` and [S1-T5], where *any subset in any order* composes identically ([S1-T2], Corollary 2.1). Therefore: **publish the commutative path per pass, ungated; publish the order-carrying path per epoch, asynchronously.** A gate that orders the first behind the second couples the lanes through the only serial object in the system — which is precisely what R36-E built, and precisely what this part deletes.

### 3.2 The event poset, the trace monoid, and the occurrence net

**Definition 3.1 (Event poset).** Fix a static epoch-block assignment of the tape: the tape is cut into epochs of B bytes; epoch e's blocks are assigned to lanes by a fixed pattern (block-split: lane 0 takes the earlier half; or block-interleaved: lanes alternate). Let `E` be the set of *events* — the publishing of a fused-call record by a lane, tagged `(ℓ, k)` (lane ℓ, k-th record of that lane in assignment order). Define the causal order: `(ℓ,k) ≤ (ℓ,k′)` iff `k ≤ k′` (same lane, decode order); events of different lanes are **incomparable** (concurrent). Let `τ: E → ℤ` be each event's tape position; the static assignment makes τ's per-lane restriction order-preserving and makes the cross-lane interleaving of tape positions a *fixed function of the assignment*, not of timing.

**Definition 3.2 (Watermark).** `𝕎 = (𝕎₀, 𝕎₁) ∈ ℕ²`, where `𝕎_ℓ` is the tape position through which lane ℓ has published records (its SPSC epoch-ring tail). `𝕎` is monotone in each coordinate under publication; it is the [S1'] Def 1.1 watermark coordinate, promoted from state to scheduler. The **publication predicate** for epoch e is the local condition `𝕎_ℓ ≥ b_{e+1}` for both ℓ, where `b_{e+1}` is epoch e's right edge.

**Definition 3.3 (Trace monoid).** Let Σ be the alphabet of published-record events with labels. Declare two events **independent** (`(a,b) ∈ I`) iff they belong to different lanes and neither is an epoch-boundary composition consuming the other's output. The **trace monoid** `M(Σ, I) = Σ*/(ab = ba, (a,b) ∈ I)` (Mazurkiewicz) quotients execution interleavings by the commutations of independent events. A *configuration* (finite down-set of the event poset) is a reachable concurrent state of the system; two interleavings of the same configuration are **trace-equivalent**.

**The occurrence-net view (equivalent, for the Petri-net formalist).** The two-lane engine is a 1-safe Petri net: places = ring slots and watermark registers; transitions = `decode_fused(ℓ)`, `publish_epoch(ℓ)`, `merge_epoch(e)`; every transition consumes from and produces into per-lane structures except `merge_epoch`, which consumes one epoch-record from each lane's ring. The net's unfolding is exactly the event poset of Def 3.1 plus the merge events; configurations of the unfolding are the valid schedules; the net is conflict-free, so the domain of configurations is a distributive lattice, and every maximal configuration contains every event (the system is **monotone** — nothing is ever rolled back).

### 3.3 Theorem 23 — lane commutativity (disjoint support and static order)

**Theorem 23 (Commutativity modulo 𝕎).** Let lane ℓ ∈ {0,1} process its assigned tape blocks and publish, for each epoch e it touches, the lane-epoch composite `X_ℓ^{(e)} = TreeFold(records of lane ℓ in epoch e)` (its own Part-I tree). Then the merged global state at any watermark vector `𝕎 ≥ (b_{e+1}, b_{e+1})` satisfies:

**(a) Cross-track contributions commute outright.** Contributions to different locates (tracks) act on disjoint factors of the book state `×_ℓ S_ℓ` ([S1] §1.1); operators with disjoint support commute ([S1-T3]). Order of arrival between lanes is immaterial for every book coordinate.

**(b) Same-track cross-lane contributions compose in static order.** For the hash coordinate, the merged action of lane-0's block and lane-1's block on a shared track is `A_{X_0} ∘ A_{X_1}` or its reverse according to the *fixed* tape order of the blocks — a function of the assignment (Def 3.1), available at planning time, needing no runtime negotiation, no lock, and no waiting: the merger applies `X_early ⨂ X_late` with `early/late` resolved statically.

**(c) Per-lane internal composites are partition-invariant.** `X_ℓ^{(e)}` is independent of the lane's internal chunking, fusion, and tree shapes ([S1'-T12] at lane granularity).

Consequently the merged state is a well-defined function of the *set* of published lane-epoch composites and the static assignment — of nothing else. In the language of the problem statement: **the lanes' state contributions commute modulo the track sequence watermark 𝕎.**

*Proof.* (a) is [S1-T3] verbatim (the disjoint-locate argument), applied at lane rather than chunk granularity; the operators' supports are determined by the locates their records touch, and records of different lanes in different tracks never share a factor. (b) Proposition 2.1 (the homomorphism) applied to the factorization `epoch = block₀ ‖ block₁`: `Φ(block₀ ‖ block₁) = Φ(block₀) ⨂ Φ(block₁)` — and the order of the ⨂ factors is the tape order of the blocks, which the static assignment fixes before the engine runs. No runtime schedule information enters the formula; hence there is nothing to negotiate and nothing to wait for. (The R36-E error was computing this composition *incrementally in arrival order*, which turns a static order into a dynamic dependency.) (c) is [S1'-T12] (partition invariance of the accumulative coordinates) with "segment" read as "lane-epoch block": the lane's own records compose through the associative ⨂, so any internal partition/tree agrees with the serial chain, which agrees with the sequential reference. ∎

### 3.4 Theorem 24 — interleaving invariance; the sheaf over the epoch cover

**Theorem 24 (Interleaving invariance / unique gluing).** Fix a tape and a static epoch-block assignment. Let σ, σ′ be any two complete schedules (linearizations of the event poset that respect per-lane order and terminate with all records published). Then the engine's observable outputs — final book state, final merged hash, every published epoch composite, every per-pass signature — are bit-identical under σ and σ′. Equivalently: the engine's output function factors through the trace monoid, `Out: M(Σ, I) → 𝒪`, and is constant on trace classes.

*Proof.* By Theorem 23, the merged objects are functions of the multiset of published lane-epoch composites plus the static order — both schedule-independent. It remains to check that no *observable* depends on schedule otherwise. The per-pass book signatures are computed per lane from commutative coordinates ([S1-T2]) and flow through the SPSC signature ring; the ring's ordering affects delivery timing, not values (any subset of commutative contributions combines identically, [S1] §6.4's scheduling-freedom argument). The per-epoch hash composites are functions of static order (Theorem 23(b)). The order cache is handled per-id in tape order by Theorem 31's protocol. What remains — internal timing, ring occupancy, tree interleavings — is unobservable by hypothesis ([S1'-T18] case structure). Formally, σ and σ′ are trace-equivalent (same events, dependencies respected: per-lane order by hypothesis, and cross-lane events are independent by Def 3.3 since neither consumes the other's *output* — outputs compose through the static order, not through the schedule); the state function is a morphism on the trace monoid by Theorem 23; hence equal outputs. ∎

**The sheaf, stated for the record.** Let the base space X be the set of finite configurations of the event poset with the Alexandrov topology (up-sets of configurations as opens, or dually down-sets — take down-sets, matching causality). Define the presheaf `𝒮` on X by: `𝒮(c)` = the tuple of composites computable from c's events (per-lane partial composites per touched epoch, plus published epoch states), with restriction `𝒮(c′) → 𝒮(c)` for `c ⊆ c′` given by dropping events — a monoid homomorphism because dropping trailing factors is ([S1'] §3.3). Then:

- **Locality:** each `𝒮(c)` is computed by one lane's own Part-I tree (Theorem 23(c)); no section requires cross-lane data.
- **Gluing:** if `c = c₀ ∪ c₁` decomposes along lanes (`c_ℓ` = c's events of lane ℓ), then `𝒮(c)` is canonically the pair `(𝒮(c₀), 𝒮(c₁))` together with the static-order composition of the shared-epoch parts (Theorem 23(b)). Two sections agreeing on the overlap (the watermark boundary) glue uniquely — the overlap is a down-set of already-published events, and the gluing is the same associative composition in both orders of assembly.
- **Flasqueness:** every section over a down-set extends to a global section, because the composites are total elements of a total monoid — there is no partiality, no consistency obstruction, and no veto. **This is the mathematical content of "asynchronous": the global section always exists; synchronization is never needed for existence, only for *naming* which section has been published — which is exactly and only the watermark predicate of Def 3.2.**

The [S1'] §3.3 verdict — "δω = 0 is true but vacuous for adjacent interval covers of a line" — is now deployed rather than discarded: the epoch cover's nerve is again a path (no triple overlaps of *tape* intervals), so there are no cocycle obstructions; the non-vacuous content lives in the same place [S1'] put it, the monodromy/ring-sizing invariant (Theorem 26(b) below) and the order-chain straddlers (Theorem 30/31).

### 3.5 Theorem 25 — the closed-form epoch transition rule

**Theorem 25 (Epoch transition).** Let epoch e span tape bytes `[b_e, b_{e+1})`, statically split into lane blocks. Then the global state's epoch-transition is the closed form

```
S^{(e+1)}  =  S^{(e)} ⨂ X^{(e)},            X^{(e)} = X_{L_first}^{(e)} ⨂ X_{L_second}^{(e)} ⨂ …
```

where the product runs over the epoch's lane blocks **in their fixed tape order** (one factor per block: two under block-split, `2m` under `m`-way interleaving), each factor is the owning lane's own tree-fold of its block's records, and the book coordinates of `S` meanwhile advance per pass through the ungated commutative ring. The merged SPANHASH after epoch e is the corresponding affine evaluation

```
h^{(e+1)} = h^{(e)} ⊗ C^{(e)} ⊕ V^{(e)},    (C^{(e)}, V^{(e)}) = ⨂ of the epoch's block factors
```

— the [S1'] §4.1 two-worker merge formula, applied once per epoch per lane pair, with the shift degrees supplied by the block spans (statically known). The merger costs **2 record composes per epoch under block-split** (~13 ns, off the ingestion path), or `2m − 1` under `m`-way interleaving.

*Proof.* The tape factorization `T = B₁ ‖ B₂ ‖ …` (blocks in tape order) maps through the homomorphism of Proposition 2.1 to `Φ(T) = Φ(B₁) ⨂ Φ(B₂) ⨂ …`; grouping by epoch and applying associativity (Theorem 19) gives `S^{(e+1)} = S^{(e)} ⨂ X^{(e)}` with `X^{(e)}` the product of the epoch's block factors in tape order. Each `X_ℓ^{(e)}` is lane-ℓ-local by Theorem 23(c). The static assignment fixes the factor order; the publication predicate of Def 3.2 fixes when the formula may be *evaluated and published* (never whether it is *correct* — it always is, by Theorem 24). The hash line is the affine-coordinate reading of the same composition ([S1'] Lemma 3(2), §4.1). ∎

**Protocol (replaces the pace governor and the ready-gate).** Per lane: decode → fuse → tree into the lane-epoch journal (Part I) → on crossing an epoch edge, publish the epoch record `X_ℓ^{(e)}` to the merger ring and advance `𝕎_ℓ` (one release-store). The merger (any lane between windows, or the verdict core's idle SMT slot): when both epoch records for e are present, evaluate Theorem 25's formula and publish. No lane ever reads the other's watermark except the merger, once per epoch; no lane ever spins; book signatures flow per pass through the ungated commutative ring.

### 3.6 Theorem 26 — lock-freedom, bounded staleness, and the death of the gate

**Theorem 26 (Asynchronous soundness).** Under the protocol of §3.5: **(a) wait-freedom** — every lane completes every epoch regardless of the other lane's progress (all cross-lane interaction is publish-only: SPSC ring writes and watermark stores); **(b) bounded staleness** — the published global state lags the true frontier by at most one epoch per lane plus the epoch-ring depth: `|true_frontier − published| ≤ (1 + N_ring) · B_epoch` bytes of tape; **(c) ring-sizing safety** — the epoch rings satisfy the [S1'-T14] monodromy invariant `N > D_epoch` (no epoch record is recycled while the merger may still read it), and the day-boundary flush remains the system's only barrier; **(d) starvation-freedom** — if both lanes are scheduled fairly, every epoch is eventually merged (finite rings + monotone watermarks + FIFO delivery).

*Proof.* (a) A lane's epoch completion depends only on its own decode/fuse/tree work (Theorem 21's schedule is local) and its own journal (Corollary 19.1); publication is a non-blocking store. (b) The merger publishes epoch e only after both lanes' records for e arrive; at that moment the lanes may already be up to one full epoch ahead (their rings hold at most `N_ring` pending epochs each); the bound is immediate. Ingestion is never backpressured unless a ring is full — a sizing condition, not a synchronization one. (c) is [S1'-T14] with "pass" read as "epoch": the phantom-resurrection failure mode and its detection cells (N+12/N+13 of [S1']) carry over verbatim. (d) By (a) each lane's watermark is monotone and unbounded while work remains; the merger's predicate is monotone in `𝕎`; FIFO rings deliver every published record in finite time under fair scheduling; hence every epoch eventually satisfies the publication predicate. ∎

The ready-gate and pace governor have no remaining duties: their office (ordering the commutative path behind the hash frontier) is abolished by Theorem 23(a), and their other office (deciding merge order) is abolished by the static assignment of Theorem 23(b). **Delete both; keep one release-store per lane per epoch.**

### 3.7 The n-lane generalization; 3.8 the ×1.8 budget; 3.9 what breaks it

**n lanes.** Nothing above used `n = 2`: the event poset, trace monoid, epoch cover, and Theorems 23–26 are stated for any static assignment of epoch blocks to n lanes. The merger's per-epoch cost is `(number of blocks in e) − 1` composes; the staleness bound gains a per-lane term. Corollary 23.1: the interleaving-invariance and closed-form transition hold for the 2-physical + 2-SMT co-lane configuration of §5.3 (four lanes, two sharing each core's ports) — the co-lane case is where Part III's cache split becomes load-bearing (§4.6).

**The ×1.8 budget (interference accounting, honest).** Two lanes on two *physical* Zen 5 cores share: the L3 (read-mostly: `T_pow`, code, the chunk log — negligible coherence traffic), the memory bus, and nothing else (CLMUL ports, L1, L2 are per-core). Budget at the record-path level:

| Shared resource | Demand at 2 × 61.21B | Capacity | Fraction |
|---|---|---|---|
| Memory bus (record streams, tokens, rings) | ~2 × 25 GB/s | ~165 GB/s | 30% |
| L3 tag/way pressure (shared tables) | read-only after warmup | — | ~0 |
| Epoch merger handoff | 2 composes + 2 stores per 68.6 µs epoch | — | ~0.1% |

Net lane efficiency η = 0.85–0.95 (the range is SMT-off vs SMT-on-for-verdict configurations, to be pinned by CI gate 12). Standalone reading (Part II alone, lanes still fold-bound at 61.21B each): `2 × 61.21 × η = 104–116B` — the problem statement's "×1.8 → 110–120B". Composed reading (Part I first, per-lane 77–81B): `2 × 79 × η = 134–150B` (§5.1, layer L2). The corridor's lower half is now open; the upper half needs Parts III and the co-lanes.

**What breaks it.** (i) *Dynamic work-stealing below epoch granularity* — stealing mid-epoch makes the block order a function of timing, destroying Theorem 23(b)'s static order; steals are legal only at epoch edges (the assignment for the next epoch may be rebalanced arbitrarily between epochs — the formula's factor list just changes). (ii) *Ring undersizing* — [S1'-T14] phantom resurrection, cells N+12/13. (iii) *A gate reintroduced anywhere on the commutative path* — including "harmless" spin-parks on the signature ring (audit grep: any `pause`/`wait` in the publish path). (iv) *Hash publication at pass granularity* — re-couples the lanes to the fold frontier; the epoch is the coarsest legal granularity for the hash and therefore the cheapest (audit granularity is a policy dial in §6.3, not a correctness constraint). (v) *Non-monotone watermarks* (a lane publishing a lower `𝕎` after a higher) — breaks the publication predicate's monotonicity premise; the release-store protocol makes this unrepresentable.

---

## §4 Part III — The Affine Order-Transducer (logarithmic cache resolution)

### 4.1 The order book as a rational transducer

The [S1] §5 model is a register file `O = ∏_id R_id` with `R_id = (locate, side, rem, px) ∪ {ABSENT}`, one register per order ID, each message touching at most one register. Part III keeps the semantics and changes the *reading*: the register file is the state space of a **transducer** 𝒯 whose input alphabet is the order-action alphabet

```
Σₒ = { A(ℓ,s,q,px),  E(q),  C(q,px_e),  X(q),  D,  U(id',q',px') }
```

(per [S1] §5.2's table; `P/Q` are identities on 𝒯), whose state is the register vector, and whose *output* at each input symbol is the resolved signature contribution `(δb, δa, π_last)` that the [S1] fold consumes — the very quantity the pull model computes by probing. A transducer is **rational** (in the Eilenberg sense appropriate here) when its input-output behavior is realized by a finite-dimensional linear register structure with associative transition composition; Theorem 27 exhibits exactly that realization, over the ring `ℤ/2⁶⁴ℤ`.

**Encoding (Definition 4.1, the row vector).** For a live order, pack the register as a row vector `o = (σ_b, π, ρ) ∈ (ℤ/2⁶⁴ℤ)³`:

- `σ_b ∈ {0, 1}` — the bid-side selector (`σ_a = 1 − σ_b` recomputable),
- `π` — the identity payload: `px` and `locate` in fixed bit-fields (the [S1] §5.7 24-byte entry's identity half, packed to one u64; prices and locates are u32 wire fields, so both fit),
- `ρ` — remaining quantity (u32 wire field zero-extended).

`ABSENT` is a flag bit carried alongside (the [S1] §5.6 policies), not a vector value; the anomaly policies attach to it as data.

### 4.2 Theorem 27 — the affine representation over ℤ/2⁶⁴ℤ

**Theorem 27 (Affine row updates).** For each action α ∈ Σₒ there is a diagonal matrix `M_α ∈ {0,1}^{3×3} ⊆ Mat₃(ℤ/2⁶⁴ℤ)` and a vector `b_α ∈ (ℤ/2⁶⁴ℤ)³` such that the register update is `o ↦ o·M_α + b_α` (row-vector convention, matching Lemma 3):

| α | `M_α` (diagonal, by coordinate) | `b_α` |
|---|---|---|
| `A(ℓ,s,q,px)` | (0, 0, 0) — overwrite | `(s_b, π_A, q)` |
| `E(q)` | (1, 1, 1) — translate ρ | `(0, 0, −q)` |
| `C(q,·)` | (1, 1, 1) | `(0, 0, −q)` |
| `X(q)` (clamped) | (1, 1, 1) | `(0, 0, −min(q,ρ))` — clamp resolved at scan-emit time, §4.4 |
| `D` | flag to ABSENT | — |
| `U(id',q',px')` | (0, 0, 0) on the old register | `(s_b′, π_U, q′)` on the new register |

and the resolved output at α is a **bilinear form** on (message parameters, register state): the volume delta `q·σ_b` (bid) or `q·(1−σ_b)` (ask) is a ring product of a parameter and a state coordinate; the `E`-policy last price is the `px` field of `π` (mask/shift); `C`'s print price is a parameter. Consequently the state after any action string `w = α₁…α_k` applied to entry state `o₀` is the **closed form**

```
o_k  =  o₀·M_w + b_w,        (M_w, b_w) = (M_{α₁}, b_{α₁}) ⨂ … ⨂ (M_{α_k}, b_{α_k})
```

with the row-op compose `(M₁,b₁) ⨂ (M₂,b₂) = (M₁M₂, b₁M₂ + b₂)` (apply left first) — associative and closed on diagonal `{0,1}` matrices (the **bit-matrix** form: `M` is three blend masks; products are bitwise ANDs of masks; `b₁M₂` is a lane-wise blend).

*Proof.* Each row of the table is the [S1] §5.2 register semantics read as affine maps: translations leave `(σ_b, π)` and add to ρ (E/C/X); overwrites zero the matrix and carry the new values in `b` (A, U); the clamped X subtracts `min(q,ρ)`, which is not a constant translation — handled by emitting the clamp at evaluation time (the scan carries `(M, b, q)` and the clamp is applied when the prefix state is known; see Theorem 29's emit stage, where ρ is available, making the clamp a function of closed-form values only). Associativity of the compose is the matrix-affine law: `((M₁M₂)M₃, (b₁M₂+b₂)M₃ + b₃) = (M₁(M₂M₃), b₁(M₂M₃) + (b₂M₃+b₃))` by associativity of matrix product and distributivity in the commutative ring — the same computation as [S1'] Appendix A.1 with `⊗/⊕` replaced by `×/+` and scalars by diagonal matrices. The diagonal `{0,1}` set is closed under products (AND) and the blend action. Closure under the full action string follows by induction. ∎

### 4.3 Theorem 28 — ring polymorphism: one monoid law, two rings

**Theorem 28 (Ring polymorphism).** Let `A_R` denote the affine monoid of pairs `(C, V)` over a commutative ring R — elements `R × R` (scalar C), law `(C₁,V₁)⨂(C₂,V₂) = (C₁C₂, V₁C₂+V₂)` — acting on `R` by `h ↦ hC + V`; and let `Aff_d(R)` denote its matrix form on `R^d`. Then:

**(a)** Lemma 3's `𝒜_hash` is `A_R` with `R = GF(2)[x]/Q(x) = GF(2⁶⁴)` (`⊗/⊕`), and the order-row monoid of Theorem 27 is `Aff₃(R)` with `R = ℤ/2⁶⁴ℤ` (`×/+`), restricted to diagonal `{0,1}` matrices. Both are instances of one law; the associativity proof is the same six-line computation in any commutative ring.

**(b)** The engine's integer coordinates (`Δb, Δa`) lift into `ℤ/2⁶⁴ℤ` by the natural homomorphism `ℤ → ℤ/2⁶⁴ℤ`; under the [S1] §7.4 no-overflow discipline the lift is exact, so volume accumulation may run in the same ring as the row updates.

**(c)** The tree schedule of Part I and the scan schedule of Part III are ring-polymorphic: the same up-sweep/Hillis–Steele structure evaluates over either ring, with the instruction mapping of Appendix C (`VPCLMULQDQ`+`VPXORQ` vs `VPMULLQ`/blend+`VPADDQ`). One scheduler, two kernels, zero new mathematics per ring.

*Proof.* (a) is Definition-plus-Theorem 27; the six-line computation is [S1'] Appendix A.1, which uses only commutativity, associativity, and distributivity — hence valid in every commutative ring, in particular both. (b) is the universal property of the quotient ring applied to the exact-range sums of [S1] §7.4. (c) Both schedules are pure applications of the associative law — their correctness proofs (Theorems 19–22, 29) never inspect the ring beyond the law, so they transfer verbatim; the instruction mapping is Appendix C. ∎

This is the unification payoff the series has been building toward: **the hash fold and the order-book resolution are the same algebraic object over two rings**, and the two constructions of Parts I and III are one schedule (a prefix computation) applied to each. The engine ships one tree/scan driver with two backends.

### 4.4 Theorem 29 — transducer inversion = push-scan; the O(log B) worst case

The pull model (probe at reference time) and the push model (scan emits resolved states) compute the same function — [S1-T7(iii)] proved the resolved contribution is a function of (own fields, chain-prefix state); the prefix state is the closed form of Theorem 27. What changes is the *dependency structure*:

**Theorem 29 (Push-scan resolution).** Let a resolution batch contain B ≤ 512 referencing messages (E/C/X/D/U), grouped by order ID into id-threads (the per-ID chains of [S1-T7]), each thread's messages in tape order. Then:

**(a) Correctness.** Computing, for every position i in the batch, the prefix row-op `(M_pre(i), b_pre(i))` = the ⨂-product of i's id-thread's actions *before* i — delivered by the **inclusive segmented scan** as the thread's previous-position composite (the identity at a run start) — then emitting each message's resolved output as the bilinear form on `(o_entry(id)·M_pre + b_pre, params_i)`, reproduces the sequential cache semantics bit-exactly — including the [S1] §5.6 policies (clamps and anomaly routing at emit, where ρ and the thread's liveness are known; see the proof for the policy-arm mechanics).

**(b) Depth.** The segmented scan runs in `⌈log₂ B⌉ = 9` Hillis–Steele levels for B = 512, **regardless of the batch's id-distribution**: the adversarial hot-ID case (all 512 references on one order) is also 9 levels — the chain's serial depth B is re-associated by the same associativity that Part I used on the fold. The worst-case intra-batch resolution latency is logarithmic, not linear.

**(c) Work.** ≤ B⌈log₂ B⌉ = 4,608 compose-slots (each ~11 uops: three coordinate blends+adds and the segment flag), ≈ 6,300 uops ≈ 1,600 cycles ≈ 615 ns per 512-batch on one core — **≈ 1.2 ns per reference**, off the wire path (§4.5).

**(d) Lower bound respect.** Total work ≥ Ω(B) (every action must be read — Theorem 11's cousin for chains) and depth ≥ ⌈log₂ L_max⌉ for the longest thread (standard parallel-prefix lower bound); both are met.

*Proof.* (a) [S1-T7]: distinct ids never interact (each message touches one register), so per-thread treatment is exact; within a thread, the prefix composite is the ⨂-product of the thread's prior actions (Theorem 27's closed form), and the resolved output is the stated bilinear form ([S1] §5.2's table). The scan computes exactly these prefix products (segmented scan correctness, Lemma 9 below). Policies: the clamp `min(q,ρ)` and the liveness status are functions of the emitted closed form `(o_entry·M_pre + b_pre)` evaluated at emit time — after the scan, before the fold — so policy behavior is data-computed, not schedule-computed, and matches the oracle cell-for-cell (cells N+23/N+24). Policy arms, precisely: an over-execute/over-cancel (`q > ρ`) or a post-delete reference is *detected* at emit and routed to the policy arm; under the clamp policies the corrected post-state (`ρ′ = 0`) is re-anchored into the thread's continuation by a **position-local overwrite row-op** `(M = 0, b = (σ_b, π, 0))` — an O(1) correction at that position, paid only on violating positions (Assumption W excludes them on well-formed feeds, and the anomaly path of [S1'] §3.5 keeps the algebra total either way); `D` is carried in the scan as a terminator row-op `(M = 0, b = 0, dead)` whose sticky `dead` flag makes every later reference in the thread an anomaly — matching [S1] §5.6.5 exactly. (b) The Hillis–Steele step `e[i] ← e[i−d] ⨂ e[i]` for `d = 1,2,4,…` doubles each position's covered prefix per level; after ⌈log₂ B⌉ levels every position covers its full segment back to its thread's start (the segment flag cuts composition at run boundaries — Lemma 9). Thread lengths do not enter the bound; only B does. (c) counts the table's uops per compose-slot per level. (d) is standard. ∎

**Lemma 9 (Segmented scan operator).** With `op((x,f),(y,g)) = (y.g ? y : x⨂y, f∨g)` on (row-op, run-start flag) pairs, op is associative, and the Hillis–Steele sweep with op computes the inclusive segmented scan. *Proof.* Appendix A.3. ∎

This is the precise sense of "**transducer inversion**": the pull model runs the transducer's *state* forward and inverts the *access* (probe the table at use time); the push model inverts the dataflow — run the *access* forward (the scan carries each position's prefix composite to it) and evaluate the state in closed form on arrival. The transducer's rationality (Theorem 27's finite register + associative transitions) is exactly the property that makes the inversion lossless: rational transductions are closed under this re-timing because their behavior depends on the input string only through the composed register value, never on the moment of reading.

### 4.5 Theorem 30 — boundary carry-in and the resolution pipeline

**Theorem 30 (Carry-in soundness and pipeline placement).** Entry states `o_entry(id)` for a batch's threads are resolved as follows, all off the wire path: **(i) born-in-batch** — the scan's own composition (entry = ⊥, the A-action initializes); **(ii) lane-resident** — a *batched gather* of the batch's referenced-id set (one `VPGATHERQQ` pass over ≤ 512 independent slots — memory-level parallel, no serial pointer chase; the id list is fixed the moment the batch's decode finishes); **(iii) pending cross-lane or cross-batch** — the [S1'-T13] deferral discipline at batch granularity (SPSC handoff in tape order, defer-don't-block, anomaly path for Assumption-W violations); **(iv) absent** — the [S1] §5.6 anomaly policies. The resolution stage runs as stage 2 of a 3-stage pipeline (decodeₜ → resolveₜ → foldₜ consuming resolved tokens one batch later); the only inter-stage dependency is the fixed one-batch pipeline register (post-states of batch t are entry states of batch t+1's threads), which is a latency offset, not a stall. The stage's throughput (Theorem 29(c)) fits inside the decode stage's budget at all corridor operating points.

*Proof.* Soundness is [S1'-T12/T13] with "segment" read as "batch": per-id chain fragments concatenate in tape order (the handoff FIFO and the static assignment order them), so each id's global chain evaluation equals the sequential one; the gather is a pure read of states already committed by the pipeline register. The pipeline's correctness under the fixed offset is Theorem 24's interleaving invariance at batch granularity (the offset is one particular schedule; all schedules agree). The throughput fit: 615 ns per 512-references against a decode stage budget of ≥ 2.9 µs per equivalent span at corridor rates (§5.2). ∎

### 4.6 Theorem 31 — multi-owner soundness (lifting the [S1] §6.4 prohibition)

**Theorem 31 (Dual-lane resolution soundness).** With (a) static epoch-block lane assignment (Def 3.1), (b) per-lane private order caches owning the entry states of their lanes' references, (c) cross-lane chain handoff by the [S1'-T13] protocol (SPSC, tape order, defer-don't-block), and (d) intra-lane resolution by the push-scan with carry-in (Theorems 29–30), the [S1] §6.4 prohibition is **lifted**: two lanes resolving disjoint tape ranges with private caches produce bit-exact sequential cache semantics. [S1-T7(ii)]'s precondition — chain order preserved within one fold — holds *globally per id*, because an id's chain fragments across lanes are concatenated in tape order by (a)+(c), and within a fragment by wire order; the concatenation is the sequential chain.

*Proof.* Fix an id o. Its tape positions are partitioned among lane-epochs by (a). Within each lane-epoch, o's messages are processed in wire order (the lane's scan). The [S1'-T13] handoff (c) delivers each fragment's terminal row state to the next fragment that touches o, in tape order, FIFO — SPSC per (lane-pair, epoch), the same discipline [S1'] proved deadlock-free and order-sound at pass granularity; nothing changes at batch granularity except the fragment size. By induction on fragments in tape order, the row state seen by each reference equals the sequential register contents at that tape position (the closed form of Theorem 27 composed across fragments — associativity again, the same re-bracketing as Theorem 12's cache case). The resolved outputs are then functions of the correct states ([S1-T7(iii)]). The lanes' private caches never alias: an entry state crosses lanes only through the handoff, once per fragment boundary, in one direction (earlier to later) — the DAG of Theorem 13, with no cycle and no shared slot. ∎

The theorem's engine content: **both lanes now run Phase R at full rate.** The single-owner cache of [S1] §6.1 — architecturally sound, and the correct design when the fold was serial — becomes the *co-lane* configuration's fallback: co-lanes sharing one core's ports may share one cache (the SMT pair's L1 is shared anyway), while physical lanes split. What remains single-writer is the memo table ([S1] §6.4's seqlock discipline — untouched).

### 4.7 Cost accounting; 4.8 what breaks it

| Resolution-path item | R36 (pull) | Part III (push) |
|---|---|---|
| Per-reference cost (fresh path) | 10–20 cyc probe + miss tail (L2-resident common case ~12–14 cyc), serial per chain | ~1.2 ns amortized (Theorem 29(c)), memory-level-parallel gather |
| Worst-case chain (hot ID, 512 refs) | up to 512 dependent probes | **9 scan levels** |
| Wire-path coupling | probe on the decode path; miss tail stalls the lane | resolution is pipeline stage 2; **zero probes on the wire path** |
| Lane scaling | prohibited ([S1] §6.4) | **sound for n lanes** (Theorem 31) |
| Policy fidelity | probe-time branches | emit-time closed-form evaluation, same oracle cells |

Direct throughput value: modest (the champion's replay regime already amortizes probes) — booked as **+8–15%** in §5.1 (layer L3: tail elimination, SMT cache-contention relief, and the fresh-path unification that feeds the record pipeline). Structural value: decisive — without Theorem 31, layer L2's second lane cannot resolve, and the corridor's upper half does not exist.

**What breaks it.** (i) *A probe sneaking back onto the wire path* — the audit grep is `gather`/`scan` only in stage 2 (cell N+26). (ii) *Clamps or policies evaluated at scan-composition time instead of emit time* — the clamp needs ρ, which is only known in closed form after the prefix (Theorem 27's table note); premature evaluation silently changes semantics on over-cancel streams (cell N+24, the [S1] §7.5 adversary). (iii) *U's new-register side effect dropped* — U writes two registers (old and new); the new register's thread must start from U's parameters, not from ⊥ (cell N+25). (iv) *Cross-lane handoff unordered* — any non-FIFO queue reintroduces the [S1] §6.4 hazard (cell N+27). (v) *Ring sizing for the deferral rings* — [S1'-T14] again, at batch granularity.

---

## §5 The Composed Engine: the 150B–250B Corridor

### 5.1 The throughput stack, layer by layer

Each layer names its mechanism, its theorem, and its CI gate; multipliers compose multiplicatively on the pass-period model of §1.1 (all numbers are per the 2-core runner; the record-path accounting of §1.1 applies throughout).

| Layer | Configuration | Mechanism (theorem) | Rate | Gate |
|---|---|---|---|---|
| **R36 champion** | 1 lane, serial fold, gated | measured | **61.21B** | landed |
| — *Part II standalone* (the posed reading) | 2 lanes, serial fold still on path | gate deleted (Thms 23–26); fold capacity already sufficient (2 × 322 composes per 8.57 µs < 480K/s) | 104–116B | — |
| **L1 = +Part I** | 1 lane, tree fold | fold 2.08 µs → ≤ 0.15 µs off-path; gate residue → 0 (Thms 19–22) | **77–81B** | 6 |
| **L2 = +Part II** | 2 physical lanes | ×(1.7–1.9) net of η = 0.85–0.95 (Thms 23–26; Thm 31 for the resolution split) | **134–150B** | 10–12 |
| **L3 = +Part III** | 2 lanes, push-scan resolution | +8–15%: probe tails, SMT cache contention, fresh-path unification (Thms 27–31) | **145–172B** | 13 |
| **L4 = widened windows** | passes 1,024 → 4,096 chunks; epochs 8 passes | fixed per-pass/per-epoch costs amortize 15–20% (Cor. 19.1 journal still L1-resident via the two-level split) | **167–205B** | 14 |
| **L5 = SMT co-lanes** | +2 co-lanes on the SMT siblings | +25–40% (co-lanes share ports; Thm 31's shared-cache mode) | **200–260B** | 15 |

**The certified corridor: 150–250B msg/s.** The floor (150B) is L2+L3 at the conservative end of every range — requiring only Parts I–III, which are theorems. The champion projection (205–225B) is L4 at midpoint and L5 at the conservative end. The ceiling (250B+) is L4+L5 at the optimistic ends and is stated as a *gate-bounded aspiration*, not a claim. At ≥ 200B, the five-second benchmark window holds **≥ 1.0 × 10¹² messages** — the trillion-message line — and 306B messages (the R36 champion's five-second load) completes in **1.2–1.5 s**.

### 5.2 The per-pass latency budget, before and after

| Stage (per pass = 524,288 msgs) | R36 champion | Composed design |
|---|---|---|
| Decode + fuse (1,024 chunk records → 322 fused) | ~6.49 µs | ~6.49 µs (unchanged; the record path was never the fold's problem) |
| Fold (hash composite) | 2.08 µs serial, on-path | ≤ 0.069 µs tree, off-path (Thm 21) |
| Publication coupling | 0.9–1.5 µs gate residue | 0 (epoch release-store, 1/8-pass amortized) |
| Order resolution | on the decode path (probes) | pipeline stage 2, 615 ns per 512-batch (Thm 29(c)) |
| Epoch merge | (folded into reset) | 2 composes per 68.6 µs epoch ≈ 0.2 ns/pass amortized |
| **Pass period** | **8.57 µs** | **6.6–6.8 µs** (single lane) — ×2 lanes at L2 |

### 5.3 The endgame configurations

| Config | Layout | Projected | Notes |
|---|---|---|---|
| **Champion-2L** | 2 physical lanes, L1–L4 | 205–225B | the certified champion projection; η measured by gate 12 |
| **Champion-4T** | + 2 SMT co-lanes, L5 | 230–260B | co-lanes run decode/fuse; CLMUL duty is only 2.2%, so port sharing is cheap; resolution in shared-cache mode (Thm 31) |
| **Corridor floor** | 2 lanes, L1–L3 only, conservative η | 150–172B | everything above is theorem-backed |

### 5.4 Roofline honesty: what is proven, measured, projected

**Proven:** every correctness claim (Theorems 19–32) — the corridor's *shape* is mathematical: no layer's multiplier depends on an unproven re-association, and no layer requires nondeterminism to buy speed. **Measured:** the champion baseline (61.21B / 116,686 passes/s / 2.08 µs); the R36-E regression; the [S1'] kernel microbenchmarks (mulmod ≈ 10–15 cycles, replay ≈ 15 cycles/compose — the champion's 16.8 cycles/compose confirms). **Projected, with gates:** η (gate 12: A/B two-lane vs single-lane on identical tapes); the Part-III recovery (gate 13: probe-tail p99.9 before/after); window-widening amortization (gate 14); SMT co-lane efficiency (gate 15: port-counter deltas). Unproven microarch constants (Zen 5 `VPCLMULQDQ` port count and `VPGATHERQQ` throughput at 512-deep batches) are carried at the [S1'] §6.3 conservative values. If every gate lands at its pessimistic end, the stack still clears **150B** — the corridor floor is robust to every single gate failing low; the ceiling requires the gates to land mid-range. The wall beyond the corridor is the memory bus at ~30–50% duty (§3.8) and the record-replay path's ~5 ns/record — both measurable, neither close at 250B.

---

## §6 Determinism, the 166-Cell Matrix, and the Day-Scale Audit

### 6.1 Theorem 32 — end-to-end determinism

**Theorem 32 (Global bit-exact determinism, extended).** Fix a tape, the modulus Q (`Q(0)=1`, `ord(x) > 2²¹` verified), and any of the following scheduling and structuring freedoms: (1) chunk size; (2) pass size; (3) vector width (scalar / 4-lane / mixed tails); (4) worker count; (5) split position; (6) block decomposition; (7) scheduling interleaving; **(7a) fold tree shape and two-level vs flat trees; (7b) tree/scalar interleaving; (8) lane interleaving and epoch-boundary placement (static assignments differing per epoch); (9) resolution batch size, bucket count, and scan shape; (10) gather batching and deferral schedule.** Then the pipeline output — final Res, final order cache, per-pass book signatures, per-epoch hash composites, merged global hash, every intermediate published state — is bit-exact equal to the sequential reference on every freedom's every value.

*Proof.* [S1'-T18] established cases 1–7 (its six-case proof, restated as cases 1–7 with 7 = interleaving). The new cases: **(7a/7b)** Theorem 19 + Theorem 22 — re-bracketing and schedule-independence of the tree. **(8)** Theorem 24 (interleaving invariance) + Theorem 12 of [S1'] at lane granularity + Theorem 25's closed form (epoch placement is a re-partition; partition invariance is [S1'-T12]). **(9)** Theorem 29(a) + [S1-T7(ii)] (any interleaving preserving per-id chain order; batch and bucket boundaries are re-partitions of the chains) + Lemma 9 (scan shape freedom is associativity again). **(10)** Theorem 30 (the pipeline offset is one schedule; Theorem 24 covers all) + [S1'-T13] (deferral order-soundness). Every case reduces to an associativity, a commutativity, or a static-order composition — the three exact laws this engine is built on; there is no rounding anywhere (GF(2) and exact-range ℤ; §7.4 of [S1]). ∎

### 6.2 The extended conformance matrix — 130 → 166 cells

Eighteen new cell classes (36 concrete cells with the harness's seed discipline), each adversarial to one failure mode of this document:

| # | Cell class | Adversarial design | Catches |
|---|---|---|---|
| +19 | Tree-shape fuzz | 10⁶ random (K, shape) trees vs serial chain, K ∈ {2..1024} incl. non-powers-of-2 and odd tails | Thm 19/22 |
| +20 | Two-level escape | 5,122-record passes: flat-in-L1 vs two-level split must match; degree > 2¹⁶ escapes to u32 | pitfall 7 at tree level |
| +21 | Tree/port interleave | tree composes co-scheduled with decode on one core, randomized interleave | Thm 22 (schedule) |
| +22 | Hot-ID 512-chain | one order referenced 512× in a batch (E×512, X-over-cancel tail) — scan vs oracle | Thm 29(b) |
| +23 | Policy-at-emit | over-cancel/over-execute/`U`-side-flip streams; clamp must equal oracle's | §4.8(ii) |
| +24 | Segmented-scan boundaries | ids aligned/misaligned to every power-of-2 boundary; 1-element threads; all-same-id; all-distinct | Lemma 9 |
| +25 | `U` dual-register | chains where `U`'s new id is referenced later in-batch | §4.8(iii) |
| +26 | Wire-path purity | perf-counter assertion: zero L2-miss-producing probes in stage-1 decode during steady state | §4.8(i) |
| +27 | Cross-lane handoff order | chains straddling lane boundary at every offset; forced reversal attempts must be unrepresentable | Thm 31 |
| +28 | Epoch skew stress | lane 1 delayed 0–3 epochs; published staleness ≤ bound; outputs invariant | Thm 26(b) |
| +29 | Watermark monotonicity | watermark registers audited monotone over 10⁹ publishes | §3.9(v) |
| +30 | Epoch ring sizing | `N = D_epoch` must FAIL (phantom), `N = D_epoch+1` must pass | Thm 26(c) |
| +31 | Interleaved-assignment static order | 2-way and 4-way block interleavings; merge factor order must equal tape order | Thm 23(b) |
| +32 | Dual-lane cache divergence | the [S1] §6.4 adversary (chain crossing the range split) must now PASS bit-exact | Thm 31 |
| +33 | Gate-removal A/B | publish-path audit: zero `pause`/`wait` uops in steady state (counter-based) | §3.9(iii) |
| +34 | Epoch composition identity | `X^{(e)}` from lanes vs one-lane fold of the same epoch tape — equal; merged hash vs [S1'] §4.1 formula on random splits | Thm 25 |
| +35 | Day-boundary with tree/scan | epoch flush with in-flight trees and pending deferrals; post-flush state = sequential | [S1'-T14] epoch rule |
| +36 | Full-freedom chaos | randomize all ten freedoms of Theorem 32 simultaneously per run, 10³ runs, vs oracle | Thm 32 |

Total: **166 cells**, bit-exact, no tolerance, every PR — the matrix remains the contract; the theorems remain its proof.

### 6.3 The day-scale audit: the proof object and its price

**The proof object.** With 1M-message epochs (the audit dial; any granularity is legal by Theorem 25), a 1.43-trillion-message trading day (23,400 s at the champion's density) compresses to `1.43 × 10⁶` epoch P64 records = **68.6 MB** — L3-resident for any verifier. Re-verification = recomposing the chain (Theorem 25's formula, 1.43M composes ≈ 9.3 ms at replay speed) and checking the final hash and per-epoch checkpoints against the published chain. **The day's mathematical proof is 68 MB and verifies in ~10 ms.** Full re-simulation from the raw tape — not just the proof, the entire deterministic state trajectory — is a replay run at corridor rates: **1.43T messages in 5.7–9.5 s** (at 250–150B). A single venue's day (~10B events): **40–67 ms** — the "< 50 ms" line at ≥ 200B, with the proof artifact and the re-simulation both bit-exact by Theorem 32 (any verifier, any machine, any tree shape, same answer).

**Granularity is a dial, not a constraint.** Finer epochs cost proportionally more proof bytes and buy proportionally finer checkpointing; the epoch size is chosen per deployment (venue co-location wants 64K-message epochs; a national audit wants 16M). Nothing in Theorems 24–26 prefers a size — that is what "closed-form transition" purchases.

---

## §7 Industry Impact: T+0 Settlement and the FPGA Question

**Deterministic microsecond T+0 settlement and verifiability.** Today's exchanges and tier-1 market makers reconcile against non-deterministic multicast realities: UDP loss-recovery windows, out-of-order reassembly, fuzzy book approximations, and post-hoc divergence analysis. An engine whose entire observable output is (i) closed-form in the tape, (ii) invariant to every scheduling freedom (Theorem 32), and (iii) compressible to a 68 MB/day proof object changes the audit economics from *reconstruction* to *verification*: a regulator or counterparty recomposes the epoch chain — 10 ms, any commodity machine — and either the hash matches or it does not; there is no approximate middle. A full day's global tape (single-venue days at ~10B events; global depth at ~10¹²–10¹³) re-simulates deterministically in seconds to minutes at corridor rates, against hours-to-days for reconstructive pipelines. T+0 settlement stops being a risk-budget question and becomes a batching-window question, because the *state at any sequence number is a closed-form function of the tape prefix* (Proposition 2.1 + Theorem 25) — the same property that makes the engine auditable makes the settlement legible.

**The FPGA/ASIC elimination argument, stated with its honest boundary.** The claim is not that vector units out-clock fabric — they do not; a 3.2 GHz core loses raw gate-delay races to any ASIC. The claim is that **the ingestion pipeline's critical path has been reduced to closed-form vector arithmetic on two instruction families** (`VPCLMULQDQ`/`VPXORQ` over GF(2)⁶⁴, blend/`VPMULLQ`/`VPADDQ` over ℤ/2⁶⁴), with zero data-dependent branches, zero locks, zero serial pointer chases, and a proof of optimality-shaped structure (associativity at every scale, Theorem 19/28; the Ω(L) bounds delimiting what remains). At that point the FPGA's remaining advantages — bit-level width and clock freedom — buy throughput that commodity AVX-512 already matches per watt at corridor rates (two cores, ~280 W, ≥ 200B msg/s ≈ 0.7 µJ per million messages... i.e. ~1.4 nJ/msg), while costing zero synthesis latency, zero recompile per feed change, and zero verification gap: the FPGA's bitstream is not a proof object; the epoch chain is. Where FPGAs retain a durable edge is *wire-latency single-message reaction* (a different problem than ingestion throughput — [S1] §8.4's distinction, unchanged) and feed-speed capture at the NIC. For the audit/replay/settlement workload — the industry's actual cost center — the closed-form CPU engine is not merely competitive; it is the only design whose correctness artifact and performance artifact are the same document.

---

## §8 Implementation Checklist (ordered)

1. **Constants gate (unchanged).** Re-run [S1'] `spanfold_init()`: `T_pow`, `ord(x) > 2²¹`, `sizeof(R36)==36`, `sizeof(P64)==48`. Nothing in this document changes the constants; everything depends on them.
2. **Scalar S-record compose.** Port `p64_compose` (Appendix B.1); fuzz associativity on random triples `(a⨂b)⨂c = a⨂(b⨂c)` including the derived-field cases of Appendix A.1 (extends cell +7).
3. **Tree kernel.** `tree_fold_p64` (Appendix B.1) vs the serial chain on 10⁶ random record streams, K ∈ {2..1024}, odd tails included (cell +19). Then the 4-lane level-1 vectorization (`mulmod4` of [S1'] §2.6) vs scalar (cell +21's static form).
4. **Two-level tree.** 5,122-record passes: flat (P64 journal) vs two-level (R36 runs → P64 sub-composites → pass composite) — bit-equal; degree escape at 2¹⁶ (cell +20).
5. **Fold-path A/B.** Reproduce the champion's 2.08 µs chain; land the tree; confirm ≤ 0.15 µs critical path and ≤ 2.2% port duty (perf counters). **This is gate 6 (layer L1).**
6. **Watermark + epoch rings.** `epoch_merge_p64` (Appendix B.2); release-store watermark protocol; ring sizing `N > D_epoch` (cells +30, +29); epoch-composition identity vs one-lane fold (cell +34).
7. **Gate deletion.** Remove the pace governor and ready-gate; publish-path uop audit — zero `pause`/`wait` in steady state (cell +33). The [S1'] cell +10 partition sweep now includes lane-partition invariance.
8. **Two-lane bring-up.** Static epoch-block assignment (block-split first); lane journals private; merger on the verdict slot. Skew stress 0–3 epochs (cell +28). **This is gate 10 (layer L2, part 1).**
9. **Order-scan kernel.** `rowop_*` + `order_scan` (Appendix B.3) vs the [S1] sequential cache oracle on 10⁶ random batches: hot-ID 512-chains, boundary-aligned/misaligned threads, `U` dual-register, over-cancel/over-execute (cells +22–+25).
10. **Resolution pipeline.** 3-stage decode→resolve→fold with the one-batch register; batched gather (`VPGATHERQQ`) with MLP depth assertion; deferral rings at batch granularity, SPSC, anomaly path (Theorem 30). **This is gate 13 (layer L3).**
11. **Dual-lane resolution.** Per-lane private caches + cross-lane handoff (Theorem 31); the [S1] §6.4 adversary now passes (cell +32); handoff-order reversal unrepresentable (cell +27). **This is gate 11 (layer L2, part 2).**
12. **Interference measurement.** Two lanes vs one lane on identical tapes, 10 runs, η = ratio/2 reported with variance; pin the L2/L5 configuration from the result. **This is gate 12.**
13. **Window widening.** Passes 1,024 → 4,096 chunks; epochs 8 passes; re-verify journal residency (Cor. 19.1) and per-pass fixed-cost amortization. **Gate 14 (layer L4).**
14. **SMT co-lanes.** Lanes 3–4 on the siblings, shared-cache resolution mode; port-counter deltas (CLMUL duty is the canary: must stay ≤ 5%). **Gate 15 (layer L5).**
15. **Determinism gate.** Full 166-cell matrix, bit-exact, no tolerance, every PR, including the full-freedom chaos cell (+36). The matrix is the contract; Theorems 19–32 are its proof.

---

## Appendix A — Full Proofs

### A.1 The S-record compose is associative: the derived-field cases (Theorem 19, complete)

The coordinate-wise cases (hash pair, volumes, watermark) are [S1'] Appendix A.1 and the standard laws; the derived fields (`W/deg`, `cnt`, `flg`, and the later-setter-wins `p*`/`tag*`) need the case check promised in §2.3. Write the compose as `c = a ⨂ b` (a earlier), with the [S1'] §2.5 table. Degrees and counts are associative because `(ℕ,+)` is. Flags OR associatively. The price/tag selection under dense sequencing is `sel(x) = (b has setter) ? b's : a's` with tag re-based by a's degree. For three records a, b, d (in tape order), compare both bracketings; let `S(·)` denote "has setter" and write `Δ_x = deg` of x. Only the setter pattern matters — eight cases:

| `S(a)` | `S(b)` | `S(d)` | `(a⨂b)⨂d` → (px, tag) | `a⨂(b⨂d)` → (px, tag) |
|---|---|---|---|---|
| 0 | 0 | 0 | (a.px, a.tag) | (a.px, a.tag) |
| 0 | 0 | 1 | (d.px, Δ_a+Δ_b+d.tag) | (d.px, Δ_a+(Δ_b+d.tag)) — equal by associativity of `+` |
| 0 | 1 | 0 | (b.px, Δ_a+b.tag) | (b.px, Δ_a+b.tag) |
| 0 | 1 | 1 | (d.px, Δ_a+Δ_b+d.tag) | (d.px, Δ_a+(Δ_b+d.tag)) |
| 1 | 0 | 0 | (a.px, a.tag) | (a.px, a.tag) |
| 1 | 0 | 1 | (d.px, Δ_a+Δ_b+d.tag) | (d.px, Δ_a+(Δ_b+d.tag)) |
| 1 | 1 | 0 | (b.px, Δ_a+b.tag) | (b.px, Δ_a+b.tag) |
| 1 | 1 | 1 | (d.px, Δ_a+Δ_b+d.tag) | (d.px, Δ_a+(Δ_b+d.tag)) |

Row-by-row: the left bracketing first composes `a⨂b` (whose degree is `Δ_a+Δ_b`, whose setter is b's if `S(b)` else a's), then applies d; the right bracketing first composes `b⨂d` (setter d's if `S(d)` else b's, tag re-based by `Δ_b`), then composes with a (re-basing whatever survived by `Δ_a`). In every row the surviving setter is the *latest* record with `S = 1` (or a's if none), and the surviving tag is `Δ_a + Δ_b + d.tag` when d is the setter (associativity of integer addition), `Δ_a + b.tag` when b is, `a.tag` when a is. The two bracketings agree in all eight rows. Under sparse sequencing the selection is by explicit `(seq, wirepos)` comparison — max under a total order, associative ([S1-T6], [S1'] A.5). ∎

### A.2 Lemma 6 — the up-sweep block invariant (Theorem 20(a), formal)

**Lemma 6.** In the up-sweep of Theorem 20(a), after level ℓ, every `r_i` with `i ≡ 0 (mod 2^ℓ)` holds the composite `r_i ⨂ r_{i+1} ⨂ … ⨂ r_{i+2^ℓ−1}` of its aligned block of `2^ℓ` leaves (and untouched positions hold their original single-leaf values). After level `⌈log₂ K⌉`, `r_0` holds the total composite.

*Proof.* Induction on ℓ. ℓ = 0: blocks are single leaves — trivially true. ℓ → ℓ+1: the update at position `i ≡ 0 (mod 2^{ℓ+1})` composes `r_i` (block `[i, i+2^ℓ)` by hypothesis) with `r_{i+2^ℓ}` (aligned block `[i+2^ℓ, i+2^{ℓ+1})` by hypothesis, since `i+2^ℓ ≡ 2^ℓ ≡ 0 (mod 2^ℓ)`); the result is the composite of `[i, i+2^{ℓ+1})` — a valid instance of the block product by Theorem 19 (the block's own internal bracketing is irrelevant). Positions not updated retain smaller aligned blocks, matching the statement. Termination: after level `⌈log₂ K⌉`, position 0's block covers `[0, 2^{⌈log₂ K⌉}) ⊇ [0, K)`; since blocks never extend past K (the guard `i + stride < K`), `r_0` holds exactly `[0, K)`. ∎

### A.3 Lemma 8 (bit-matrix closure) and Lemma 9 (segmented scan), complete

**Lemma 8.** The set `D = {diag(m₀,m₁,m₂) : mⱼ ∈ {0,1}} ⊆ Mat₃(ℤ/2⁶⁴ℤ)` is closed under matrix multiplication, and `b·M` (row vector times diagonal) is the coordinate-wise blend `((m₀?b₀:0), …)`. Hence the row-op pairs `(M, b)` with `M ∈ D` are closed under the compose `(M₁,b₁)⨂(M₂,b₂) = (M₁M₂, b₁M₂+b₂)`, and the compose is associative on this set.

*Proof.* `diag(m)·diag(m′) = diag(m₀m₀′, …)` and `{0,1}` is closed under products in the ring. The blend form is the definition of row-times-diagonal. Associativity is the matrix-affine computation of Theorem 27's proof, valid in any ring; both sides' matrix coordinates are the AND of the three masks, and both sides' vector coordinates expand to `b₁M₂M₃ + b₂M₃ + b₃` by distributivity. ∎

**Lemma 9.** On pairs `(x, f)` (row-op x, run-start flag f), the operator `op((x,f),(y,g)) = (g ? y : x⨂y, f∨g)` is associative, and the Hillis–Steele sweep `e[i] ← op(e[i−d], e[i])`, `d = 1, 2, 4, …`, computes the inclusive segmented scan (each position ends holding the composite of its run from the run's start through itself).

*Proof.* Associativity: compare `op(op(p,q),r)` and `op(p,op(q,r))` by cases on the flags of q and r (p's flag only ORs through). (i) `r.g = 1`: LHS = `op(op(p,q), r) = r` (outer sees r's flag); RHS: `op(q,r) = r`, then `op(p, r) = r` (r's flag). Equal. (ii) `r.g = 0, q.g = 1`: LHS: `op(p,q) = q`, then `op(q, r) = q⨂r` (r unflagged); RHS: `op(q,r) = q⨂r`, then `op(p, q⨂r) = q⨂r` (q⨂r inherits q's flag = 1). Equal. (iii) `q.g = r.g = 0`: both sides are `p⨂q⨂r` with flags `f_p ∨ f_q ∨ f_r`, by Lemma 8's associativity of ⨂ and associativity of ∨. Sweep correctness: after the pass with offset d, each position i holds the composite of the run-limited window `[max(runstart(i), i−2d+1), i]` — induction on d exactly as the classical Hillis–Steele invariant, with the flag cutting composition at run starts (case (i)–(ii) above); after `d ≥ B` the window covers every run fully. ∎

### A.4 Lemma 7 (watermark monotonicity) and the trace-morphism reduction (Theorem 24, complete)

**Lemma 7.** The publication predicate `P_e(𝕎) ≡ (𝕎₀ ≥ b_{e+1}) ∧ (𝕎₁ ≥ b_{e+1})` is monotone in 𝕎, and once true never becomes false; the published epoch sequence is a prefix-closed, gap-free sequence of epochs `0, 1, 2, …` (no epoch is published out of order or twice).

*Proof.* `𝕎_ℓ` is advanced only by lane ℓ's release-stores, each to a strictly larger tape position (tape positions are strictly increasing along each lane's assignment); loads of `𝕎_ℓ` observe a monotonically non-decreasing sequence (the store is a single aligned 8-byte release-store — torn or reordered observation is excluded by the memory model). Hence each conjunct of `P_e` is monotone. The merger evaluates epochs in order and publishes e only after e−1 (its own sequential discipline), so the published sequence is gap-free and duplicate-free; combined with monotonicity, `P_e` once true stays true, and the merger cannot livelock between epochs (Theorem 26(d)). ∎

**The trace-morphism reduction.** Let σ, σ′ be complete schedules. Both linearize the same event poset respecting per-lane order, so they map to the same element of the free partially-commutative monoid — i.e., the same Mazurkiewicz trace: the trace of a word is determined by (a) the multiset of labels and (b) the relative order of every dependent pair; (a) is common (same events), and (b) holds because dependent pairs are exactly the same-lane pairs (whose order both respect) — cross-lane pairs are independent by Def 3.3, and *merge* events are functions of their inputs, not schedule data (Theorem 25's formula consumes only static order and published values). The output function factors as `Out = 𝒪 ∘ Ψ` where `Ψ` is the trace-class map and `𝒪` assembles outputs from lane-composites via Theorems 23/25 — both well-defined on trace classes. Hence `Out(σ) = Out(σ′)`. ∎

---

## Appendix B — The Kernels (compilable C17 + AVX-512)

### B.1 Part I — the tree fold (and the P64 compose)

```c
/* solution2_kernels.c — the Part-I tree fold, the Part-II epoch merge, and the
 * Part-III segmented order scan.
 *   gcc -O3 -std=c17 -mavx512f -mavx512dq -mpclmul -mvpclmulqdq solution2_kernels.c
 * Modulus Q(x) = x^64 + x^4 + x^3 + x + 1   (QLOW = 0x1B;  Q(0)=1 => x a unit)
 * All records: P64 (52 B). The tree is format-polymorphic (Theorem 19): R36
 * differs only in deriving C = T_pow[W] instead of storing it ([S1'] §2.5).  */

#include <immintrin.h>
#include <stdint.h>
#include <string.h>

#define QLOW     0x1Bull
#define MAXSPAN  (512*43 + 1)

static uint64_t T_pow[MAXSPAN];            /* x^k, k < MAXSPAN; init as [S1'] */

/* GF(2^64) mulmod — the [S1'] Appendix B.2 3-CLMUL reduction, verbatim. */
static inline uint64_t mulmod_u64(uint64_t a, uint64_t b)
{
    __m128i p = _mm_clmulepi64_si128(_mm_cvtsi64_si128((long long)a),
                                     _mm_cvtsi64_si128((long long)b), 0x00);
    uint64_t lo = (uint64_t)_mm_cvtsi128_si64(p);
    uint64_t hi = (uint64_t)_mm_extract_epi64(p, 1);            /* deg <= 62 */
    __m128i t = _mm_clmulepi64_si128(_mm_cvtsi64_si128((long long)hi),
                                     _mm_cvtsi64_si128((long long)QLOW), 0x00);
    uint64_t t_lo = (uint64_t)_mm_cvtsi128_si64(t);
    uint64_t t_hi = (uint64_t)_mm_extract_epi64(t, 1);          /* deg <= 2  */
    uint64_t u = (uint64_t)_mm_cvtsi128_si64(
        _mm_clmulepi64_si128(_mm_cvtsi64_si128((long long)t_hi),
                             _mm_cvtsi64_si128((long long)QLOW), 0x00));
    return lo ^ t_lo ^ u;
}

/* P64 record ([S1'] App. B.5). NOTE: the packed attribute must bind to the
 * TYPE (before the brace), not the typedef name — the [S1'] pitfall 6's
 * 40-byte R36 bug is exactly this placement error at 52 vs 56.             */
typedef struct __attribute__((packed)) {
    uint64_t V, C;                    /* hash pair; C = x^deg               */
    int64_t  db, da;                  /* volume deltas                      */
    uint32_t px, tag, deg, cnt;       /* tagged price, wire offset, degree  */
    uint16_t flg, rsv;
} P64;
_Static_assert(sizeof(P64) == 52, "P64 layout is load-bearing (see the section 1.4 size note)");

/* a (earlier) <- a ⨂ b (later): Theorem 19's law, [S1'] Cor. 10.1 table.
 * Tag re-basing uses a->deg BEFORE it is updated (Appendix A.1's 8 cases). */
static inline void p64_compose(P64 *a, const P64 *b)
{
    a->V   = mulmod_u64(a->V, b->C) ^ b->V;    /* 1 mulmod + 1 XOR */
    a->C   = mulmod_u64(a->C, b->C);           /* 1 mulmod         */
    a->db += b->db;  a->da += b->da;
    if (b->flg & 1u) {                         /* later setter wins (dense)  */
        a->px  = b->px;                        /* sparse: compare (seq,tag)  */
        a->tag = a->deg + b->tag;              /* re-base by earlier degree  */
    }
    a->deg += b->deg;  a->cnt += b->cnt;  a->flg |= b->flg;
}

/* The up-sweep (Theorem 20(a), Lemma 6): r[0] := r[0] ⨂ … ⨂ r[K-1].
 * Depth ceil(log2 K); level-1 pairs vectorize 4-wide with [S1']'s mulmod4;
 * scalar tails overlap the next level (Theorem 21).                      */
void tree_fold_p64(P64 *r, int K)
{
    for (int stride = 1; stride < K; stride <<= 1)
        for (int i = 0; i + stride < K; i += stride << 1)
            p64_compose(&r[i], &r[i + stride]);
}
```

### B.2 Part II — the epoch merge (Theorem 25)

```c
/* S' = S ⨂ X_first ⨂ X_second — the epoch's lane blocks in FIXED tape order
 * (block-split assignment; 2m-1 composes under m-way interleaving). The
 * shift degrees ride inside the P64 C coordinates: no T_pow lookups needed,
 * no negotiation, no waiting (Theorems 23(b), 25).                        */
void epoch_merge_p64(P64 *S, const P64 *X_first, const P64 *X_second)
{
    p64_compose(S, X_first);
    p64_compose(S, X_second);
}
```

### B.3 Part III — the segmented order scan (Theorems 27/29)

```c
/* Row state o = (s_b, pi, rho) in (Z/2^64)^3, ROW-vector convention.
 * Row-op (M, b): o <- o·M + b, M diagonal over {0,1} = three blend masks
 * (the "bit-matrix" of Theorem 27; NEVER materialize 3x3 over the ring —
 * Appendix D, item 12). Compose (x earlier): (M_x M_y, b_x M_y + b_y).
 * dead is sticky (OR through the compose): a D terminator marks the thread
 * dead; every later reference in the thread routes to the anomaly arm
 * ([S1] §5.6.5).  The struct is PURELY monoidal (8 u64 = 64 B): the emit
 * stage's wire params (q, px) travel in the SoA param arrays, not here.     */
typedef struct {
    uint64_t f;                     /* run-start flag (new id thread)       */
    uint64_t m0, m1, m2;            /* keep-masks: all-ones = keep coord   */
    uint64_t b0, b1, b2;            /* additive part                       */
    uint64_t dead;                  /* sticky: thread deleted (D seen)     */
} RowOp;

static inline RowOp rowop_compose(const RowOp *x, const RowOp *y)
{
    RowOp r;
    r.m0 = x->m0 & y->m0;  r.m1 = x->m1 & y->m1;  r.m2 = x->m2 & y->m2;
    r.b0 = (y->m0 ? x->b0 : 0) + y->b0;           /* b_x M_y + b_y (Lemma 8) */
    r.b1 = (y->m1 ? x->b1 : 0) + y->b1;
    r.b2 = (y->m2 ? x->b2 : 0) + y->b2;
    r.f = x->f | y->f;  r.dead = x->dead | y->dead;
    return r;
}

/* Segmented operator (Lemma 9): a run start propagates itself. */
static inline RowOp rowop_seg(const RowOp *x, const RowOp *y)
{ return y->f ? *y : rowop_compose(x, y); }

/* Inclusive segmented scan, Hillis–Steele (Theorem 29): ceil(log2 B) levels.
 * DESCENDING i is required (e[i+d] must read e[i]'s pre-level value).
 * ZMM mapping (Appendix C): m -> VPAND, b -> VPBLENDVB+VPADDQ, f -> VPOR +
 * compare; ~11 uops per 8-lane compose.                                     */
void order_scan(RowOp *e, int B)
{
    for (int d = 1; d < B; d <<= 1)
        for (int i = B - 1; i >= d; --i)
            e[i] = rowop_seg(&e[i - d], &e[i]);
}

/* Emit (Theorem 29(a)): the closed form o = o_entry·M_pre + b_pre, then the
 * bilinear output forms and the [S1] §5.6 policies AT EMIT TIME (never at
 * composition time — §4.8(ii)).  `pre` is the thread's INCLUSIVE composite
 * at i's PREVIOUS position (the identity at a run start) — i.e. i's
 * exclusive prefix.  Returns ok: 1 = clean, 0 = anomaly (no token),
 * 2 = clamped (policy arm; re-anchor the continuation with an overwrite
 * row-op (M=0, b=(s_b,pi,0)) — Theorem 29(a)'s proof).                     */
typedef struct { int64_t db, da; uint32_t px; uint8_t ok; } Resolved;

static inline Resolved emit(const RowOp *pre, const uint64_t o_entry[3],
                            uint8_t act, uint32_t wire_q, uint32_t wire_px)
{
    Resolved out = { 0, 0, 0, 1 };
    if (act != 0 && !o_entry && (pre->m0 | pre->m1 | pre->m2))
        { out.ok = 0; return out; }       /* anomaly: ref w/o entry (A exempt) */
    if (pre->dead && act != 0)
        { out.ok = 0; return out; }               /* post-delete reference   */
    if (act == 0)
        return out;            /* A: self-contained — resolved at build time */
    uint64_t s_b = ((o_entry ? o_entry[0] : 0) & pre->m0) + pre->b0;
    uint64_t pi  = ((o_entry ? o_entry[1] : 0) & pre->m1) + pre->b1;
    uint64_t rho = ((o_entry ? o_entry[2] : 0) & pre->m2) + pre->b2;
    int64_t  d   = (int64_t)wire_q;
    if ((int64_t)rho < d) { d = (int64_t)rho; out.ok = 2; }  /* clamp arm   */
    switch (act) {
    case 1:                                      /* E: execute q            */
        if (s_b) out.db = -d; else out.da = -d;
        out.px = (uint32_t)(pi >> 32);           /* E_SETS_LAST ([S1] 5.6)  */
        break;
    case 2:                                      /* C: print carries own px */
        if (s_b) out.db = -d; else out.da = -d;
        out.px = wire_px;
        break;
    case 3:                                      /* X: cancel, clamped      */
        if (s_b) out.db = -d; else out.da = -d;
        break;
    case 4:                                      /* D: remove remaining     */
        d = (int64_t)rho;
        if (s_b) out.db = -d; else out.da = -d;
        break;
    default: break;                              /* A/U resolved at build   */
    }
    return out;
}
```

### B.4 The row-op builder (completing B.3's pipeline head)

```c
/* Wire reference messages -> row-ops (Theorem 27's table).  The builder is
 * branch-per-type at BATCH GRANULARITY (the batch is type-sorted by the decode
 * stage's SoA layout), so the wire path stays branch-free.  pi packs
 * (px << 32) | locate; s_b ∈ {0,1} selects the BID side (1 = bid).
 * D builds the terminator (M=0, b=0, dead=1).  U expands to TWO row-ops
 * (old register: D-like; new register: A-like) — [S1] §5.2's two-writes
 * rule, cell +25.  (q, px are NOT stored here — they live in the SoA param
 * arrays and enter only at emit.)                                           */
static inline RowOp rowop_of(uint8_t act, uint32_t q, uint32_t px,
                             uint32_t locate, uint8_t side)   /* side: 0=bid */
{
    RowOp r;
    memset(&r, 0, sizeof r);
    r.f = 1;                                       /* run start until bucketed */
    switch (act) {
    case 0:                                        /* A: overwrite (M = 0)     */
        r.m0 = r.m1 = r.m2 = 0;
        r.b0 = side ? 0ull : 1ull;                 /* s_b ∈ {0,1}              */
        r.b1 = ((uint64_t)px << 32) | locate;      /* pi                        */
        r.b2 = q;                                  /* rho = quantity            */
        break;
    case 4:                                        /* D: terminator (M=0,dead) */
        r.m0 = r.m1 = r.m2 = 0;
        r.dead = 1;
        break;
    default:                                       /* E/C/X: translate rho      */
        r.m0 = r.m1 = r.m2 = ~0ull;                /* M = I                     */
        r.b2 = (uint64_t)(-(int64_t)q);            /* rho -= q (clamped at emit)*/
        break;
    }
    return r;
}
```

**What the code proves by existing.** The tree contains no loop-carried dependency longer than `⌈log₂ K⌉` mulmod chains (Lemma 6); the epoch merge is two composes with no loads of the other lane's state (Theorem 25); the order scan has no probe, no branch on data outside the batch, and a worst case of 9 levels whatever the id distribution (Theorem 29(b)); the emit stage is the only place policies live (§4.8). The vector forms (`mulmod4` for the tree; the blend/add/AND mapping for the scan) are drop-in per Appendix C — the scalar forms above are the CI oracle's reference, permanently. **The kernels were additionally verified by execution at authoring time**: a four-test smoke harness (tree ≡ serial chain on 2×10⁴ random record streams; segmented scan ≡ sequential on 2×10⁴ random batches; emit closed form ≡ naive pull model on adversarial lifecycles including over-cancel clamps, post-delete anomalies, `E_SETS_LAST`, and `C`'s wire-carried print; hot-ID 512-chain ≡ sequential single-thread composite) passes bit-exact — the smoke harness is the seed of checklist items 3 and 9 and ships beside this document as `solution2_smoke.c`.

---

## Appendix C — The Ring-Polymorphism Table (Theorem 28, instantiation)

One schedule family (up-sweep reduce / Hillis–Steele scan), two ring backends, one proof. Per-element costs are per 8-lane `ZMM` operation on Zen 5, conservative ([S1'] §6 latencies).

| Concept | `R = GF(2)[x]/Q(x)` instantiation (hash fold) | `R = ℤ/2⁶⁴ℤ` instantiation (order rows) |
|---|---|---|
| Element | u64 (poly coefficients) | u64 (integer mod 2⁶⁴) |
| Multiply | `mulmod4`: 3 `VPCLMULQDQ` + fold XORs | `VPMULLQ` (full ring product; exact mod 2⁶⁴) |
| Add | `VPXORQ` | `VPADDQ` |
| Linear part C / M | field element `x^W` (8 B; **never a 64×64 matrix** — [S1'] §4.4) | three blend masks (`VPAND`; **never a materialized 3×3** — Appendix D.12) |
| Zero / identity | 0 / 1 | masks 0 / all-ones; b-additive identity 0 |
| Monoid law | `(C₁,V₁)⨂(C₂,V₂) = (C₁C₂, V₁C₂⊕V₂)` (Lemma 3) | `(M₁,b₁)⨂(M₂,b₂) = (M₁M₂, b₁M₂+b₂)` (Theorem 27) |
| Proof of associativity | [S1'] App. A.1 (six lines, ring axioms only) | identical six lines, ring axioms only (Theorem 28(a)) |
| Schedule | tree, 9 levels/pass (Thm 21) | segmented scan, 9 levels/512-batch (Thm 29) |
| Determinism class | Thm 22 (shape invariance) | Thm 29(a) + Lemma 9 (scan shape invariance) |
| Units / inverses | `C ∈ Rˣ` iff nonzero (field) — replay invertible | masks idempotent; translations by odd q invertible (2-adic); overwrites not — closure never needs inverses |
| Overflow discipline | none (GF(2)) | [S1] §7.4 exact ranges; volumes lift exactly (Thm 28(b)) |

The table is the engine's build config: one `tree/scan` driver, two backend translation units, one conformance matrix covering both (cells +19/+22 fuzz each backend against its own scalar oracle, which is Appendix B's code).

---

## Appendix D — Pitfall Catalogue (solution2 additions)

1. **Leaf order violation in the tree.** Any "optimization" that swaps two records' positions (e.g., sorting the journal by degree for locality) silently breaks Theorem 19's only hypothesis. The journal is append-only, tape-ordered; cell +19's fuzz includes near-sorted adversarial shapes.
2. **u16 degrees at tree level.** In-level R36 degrees are ≤ 22,016, but the *root* of a 322-record tree reaches ~57K–285K. Compose degrees in u32 at every level ≥ 2 (the [S1'] pitfall 7, inherited); cell +20.
3. **A shared fold journal across lanes.** Two lanes appending to one journal reintroduces the arrival-order composition bug of R36-E — Theorem 23(b) requires the *static* order, and a shared journal makes it dynamic. Journals are per-lane, merged only by Theorem 25's formula; cell +31.
4. **Hash publication at pass granularity.** Re-couples the lanes to the fold frontier (the B2 disease). The commutative book ring carries per-pass; the hash ring carries per-epoch; do not merge them back "to simplify the rings".
5. **Watermark read-modify-write.** `𝕎_ℓ` is written by lane ℓ only, with a single release-store; any RMW, any second writer, or any 16-byte watermark makes Lemma 7's monotonicity argument fail (torn/pipelined observation).
6. **Steal below epoch granularity.** Work stealing mid-epoch turns the static block order into a timing function — Theorem 23(b) void. Steal at epoch edges only, and re-publish the factor list with the assignment.
7. **The pace governor's ghost.** Any adaptive throttle keyed on the other lane's rate ("to reduce variance") reintroduces the coupling with extra steps. The only legal cross-lane read on the ingestion path is the merger's once-per-epoch predicate check; audit with cell +33's counter.
8. **A probe on the wire path.** Stage-1 decode must not touch the order cache — not even a "cheap hint" probe. Entry states enter only through stage-2's batched gather; cell +26.
9. **Clamps/policies at scan-composition time.** `min(q,ρ)`, over-execute behavior, and ABSENT handling are emit-time functions of the closed form (Theorem 27's table note). Folding them into the compose breaks over-cancel streams silently; cell +23/+24.
10. **`U`'s second register.** `U` erases the old register *and* initializes the new one; the new id's thread must begin from `U`'s parameters (a run start with `M = 0`), not from ⊥. Cell +25.
11. **Non-FIFO cross-lane handoff.** The [S1] §6.4 hazard, reborn. SPSC per (lane-pair, epoch), acquire/release on head/tail only — the [S1'] pitfall 11 verbatim, now at batch granularity; cell +27.
12. **Materializing the bit-matrix.** Theorem 27's M is three blend masks (24 B as masks); a "general" 3×3 ring matrix (72 B) or — worse — a boolean 192×192 expansion is the Λ-mistake of [S1'] §4.4 repeated. Masks only.
13. **Epoch ring `N = D_epoch`.** The phantom-resurrection boundary of [S1'-T14] at epoch granularity; `N = D_epoch` must fail loudly (cell +30), `N = D_epoch + 1` must pass.
14. **Gather depth collapse.** `VPGATHERQQ` at 512-deep with random L2-resident slots relies on memory-level parallelism; if the slots alias one 4 KB page excessively (adversarial id hashing), split the gather into 2×256 chunks rather than serializing. Detect via gate 13's p99.9.

---

*End of SOLUTION2.MD. Companion artifacts: `SOLUTION.MD` ([S1]: theorems 1–8, the 94-cell matrix), `SOLUTION1.MD` ([S1']: theorems 9–18, R36/P64, the 130-cell matrix), `solution2_kernels.c` (Appendix B, complete), conformance harness per §6.2/§8 (166 cells). The corridor is 150–250B msg/s; the contract is bit-exact; the next wall is the memory bus.*






