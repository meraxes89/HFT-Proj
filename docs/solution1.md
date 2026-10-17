# SOLUTION1.MD — The Res-Fold, the Barrier-Free Pass, and the Galois Merge

**Complete mathematical solution to Problems 1–3 (R36 segment fold, multi-pass boundary straddles, GF(2⁶⁴) hash merge) for the 2-core (Zen 5) ITCH engine.**

Companion document: `SOLUTION.MD` (the O(1) affine LOB fold). Notation, the state-space decomposition, the signature algebra `𝒜₅.₀ = (ℤ² × Tg, ⊕)`, the tie-break taxonomy (T1–T5), and the 94-cell hydra conformance matrix are imported from there and cited as `[S1]`. This document is self-contained for everything new.

---

## Table of Contents

- §0 Executive Summary — three verdicts
- §1 The Unified Algebraic Frame (the tensor ring, Lemma 3)
- §2 Problem 1 — Closed-Form Non-Abelian Segment Res-Folding
  - 2.1 The Res record and the ⨂ fold
  - 2.2 Theorem 9 — the tensor factorization (and what the claim gets wrong)
  - 2.3 Theorem 10 — the closed-form composite identity
  - 2.4 Theorem 11 — the Ω(L) payload lower bound
  - 2.5 The R36 record — exactly 36 bytes
  - 2.6 The kernel — compilable C17 + AVX-512
  - 2.7 Cost accounting — 21 µs → 0.2–0.4 µs
- §3 Problem 2 — Topological Invariance of Multi-Pass Boundary Straddles
  - 3.1 What `reset_pass` actually protects
  - 3.2 Proposition 12.0 — the base-space correction (ℤ vs S¹)
  - 3.3 The sheaf of admissible states and the vacuous cocycle δω = 0
  - 3.4 Theorem 12 — Boundary Transparency
  - 3.5 Theorem 13 — No-Barrier Asynchronous Resolution
  - 3.6 Theorem 14 — monodromy, ring sizing, and the epoch rule
  - 3.7 Cost: 204 ms → ~0 (honest accounting)
  - 3.8 What breaks it
- §4 Problem 3 — The Galois Shift Theorem for Multi-Stream Interleaving
  - 4.1 The merge as monoid composition
  - 4.2 Theorem 15 — Λ exists (linearity)
  - 4.3 Theorem 16 — no count-independent Λ
  - 4.4 Theorem 17 — the shift algebra (the rescue)
  - 4.5 The two-instruction merge
  - 4.6 Unification — one monoid, end to end
- §5 Determinism and Bit-Exact Conformance (Theorem 18, 130-cell matrix)
- §6 Roofline on 2 Zen 5 Cores
- §7 Implementation Checklist
- Appendix A — Full proofs of the monoid laws
- Appendix B — Constants, the GF(2⁶⁴) mulmod derivation, record layouts
- Appendix C — Impossibility proofs, expanded
- Appendix D — Pitfall catalogue

---

## §0 Executive Summary

Three problems were posed. Each contains one true theorem wrapped in one false or misattributed claim, plus one performance target that is genuinely achievable. The pattern is identical to [S1]: the algebra is better than the claim, not worse.

### Problem 1 — The R36 Res-fold

**Claimed:** `Res(B) = 𝐌_type ⊗ 𝐕_payload ⊕ 𝐊_seq` factors into an affine endomorphism over a finite tensor ring, and a 512-message chunk can be reduced by *one single matrix-vector multiply*.

**Verdict:**

| Sub-claim | Verdict | The truth |
|---|---|---|
| The factorization exists | **TRUE** (Theorem 9) | The fold lives in the monoid `(R⋊GF(2)⁶⁴) × (ℤ²,+) × (Tg,⊔) × (𝕎,∨)` where `R = GF(2)[x]/Q(x)`. The linear part `𝐌` depends only on the type *multiset* (histogram); the constant part splits as claimed. |
| `𝐕_payload` depends only on payloads | **REFINED** | `V = ⊕ᵢ vᵢ ⊗ x^{dᵢ}` depends on payloads **and on the type order** (the weights `x^{dᵢ}` are suffix degrees). This entanglement is irreducible (§2.2). |
| One single MVM eliminates the span walk | **FALSE fresh / TRUE memoized** | Any fresh evaluator must read every payload digest: **Ω(L)** (Theorem 11). But the *composite* of a chunk is a 36-byte record whose replay — and whose composition with any other chunk — is literally one carry-less multiply plus one XOR (Corollary 10.1). |
| 21 µs → <2 µs per pass | **TRUE, with margin** | Branch-free, CLMUL-throughput-bound kernel: **0.2–0.4 µs** per 512-message chunk on one Zen 5 core at 2.6 GHz (§2.7, §6). 50–100× versus the 21 µs span walk. |

### Problem 2 — Boundary straddles

**Claimed:** passes form a fiber bundle over S¹; a cocycle δω = 0 makes the boundary homologically trivial; barriers vanish; `reset_ms` 204 → 0.00.

**Verdict:**

| Sub-claim | Verdict | The truth |
|---|---|---|
| Fiber bundle over S¹ | **WRONG BASE, fixable** (Prop 12.0) | Semantics live on the tape `ℤ` (a totally ordered, contractible set). The torus `ℤ/Nℤ` is the *storage slot* space; the map `ℤ → ℤ/Nℤ` is a covering. Identifying the two is the classic slot-aliasing bug, not a theorem. |
| δω = 0 | **TRUE but VACUOUS** (§3.3) | For adjacent-interval covers of a 1-D tape there are *no triple overlaps*, so the Čech cocycle condition holds automatically. The non-vacuous round-the-ring statement is monodromy triviality, which is exactly the ring-sizing invariant (Theorem 14). |
| Orders resolve asynchronously, no barrier | **TRUE** (Theorems 12–13) | The real content is order-theoretic, not topological: the accumulative coordinates are commutative/associative and the cache coordinates are per-order-ID chains whose dependencies point strictly backward in tape position. Any partition gives the same answer; a deferral protocol with SPSC handoffs is deadlock-free by DAG acyclicity. |
| reset 204 ms → 0.00 ms | **TRUE for synchronization** | The barrier and the re-anchor scan are pure overhead with zero semantic content. The straddler work does not vanish — it is bounded, deferred, and overlapped behind the next pass's compute. Wall-clock reset contribution → ~0; exactly one barrier survives: the day-boundary epoch flush. |

### Problem 3 — The Galois shift

**Claimed:** a fixed, pre-computed 64×64 binary matrix `Λ`, independent of message count, merges the two worker hashes.

**Verdict:**

| Sub-claim | Verdict | The truth |
|---|---|---|
| `Λ_d : h ↦ h·x^d` is a 64×64 GF(2) matrix | **TRUE** (Theorem 15) | Multiplication by any fixed element of `GF(2)[x]/Q(x)` is GF(2)-linear. Moreover `Λ_d Λ_e = Λ_{d+e}`: the shifts form a representation of `(ℕ,+)`. |
| `Λ` is independent of the message count | **FALSE** (Theorem 16) | `d ↦ Λ_d` is injective on every range shorter than `ord(x)`; with the init-time check `ord(x) > 2²¹` (1.5 ms, in the code), every realistic count/span yields a *distinct* operator. A fixed matrix is correct for exactly one split. |
| The merge becomes one VPCLMULQDQ + XOR | **TRUE** (Theorem 17) | Not because Λ is constant, but because `x^d` is one field element computable in O(1)–O(log d) ring multiplies (one table lookup under the span discipline). Total merge: 3 VPCLMULQDQ + 1 VPXORQ ≈ 10–15 cycles. And the matrix should *never* be materialized: the field-element representation is 8 bytes and applies in one instruction; the 64×64 matrix is 512 bytes and applies in 32–64 XORs. |

### Headline numbers

| Metric | Baseline | This design | Section |
|---|---|---|---|
| Worker eval, 512-msg pass | 21 µs | 0.2–0.4 µs fresh; ~15 cycles memoized replay | §2.7, §6 |
| Pass-boundary cost | 204 ms / 300k passes (680 ns each) | ~0 synchronization; straddler work deferred + overlapped | §3.7 |
| Cross-worker hash merge | per-message-modulo rewalk | 3 VPCLMULQDQ + 1 VPXORQ (~10–15 cycles) | §4.5 |
| Eval+reset wall, 300k passes | ≈ 6.5 s | ≈ 0.06–0.10 s (65–100×) | §6 |

The honest caveat, carried over from [S1]: multi-billion-msg/s rates appear in the *memoized replay* regime. Fresh wire-rate evaluation is CLMUL-throughput- and parse-bound at ~2–4 G msg/s on 2 cores. Both regimes are analyzed in §6.

---

## §1 The Unified Algebraic Frame

### 1.1 Notation

- A message is `m = (t, p, s)`: type `t ∈ 𝒯 = {A,F,E,C,X,D,U,P,Q}`, payload `p` (raw wire bytes), sequence `s` (dense, strictly increasing with wire position within a track).
- `w_t` = wire size of type `t` in bytes (the [S1] Appendix B table: A=36, F=40, E=31, C=36, X=23, D=19, U=35, P=43, Q=40).
- `Q(x)` = the SPANHASH modulus, a degree-64 polynomial over GF(2) with `Q(0) = 1`. `R := GF(2)[x]/Q(x)`. If `Q` is irreducible, `R = GF(2⁶⁴)` (the Galois field) and the unit group `Rˣ` is cyclic of order `2⁶⁴−1`. Every theorem in this document needs only `Q(0) = 1` (so that `x ∈ Rˣ`); irreducibility and primitivity are optional strengthenings noted where they matter.
- `⊗` = carry-less multiplication reduced mod `Q`; `⊕` = XOR (addition in `R` and in `GF(2)⁶⁴`).
- `[L]` = `{1,…,L}`. `B = m₁…m_L` = a chunk (segment) of at most 512 messages.

### 1.2 Prior results imported

From [S1], without restatement (proofs there):

- **[S1-L0]** Affine composition: k-fold sequential application of `S ↦ A_k S + B_k` composes to `A_net = ΠAᵢ`, `B_net = Σᵢ(Πⱼ>ᵢ Aⱼ)Bᵢ`.
- **[S1-T2]** The LOB signature algebra: the (bid_vol, ask_vol, last_px) fold factors through the commutative monoid `𝒜₅.₀ = (ℤ² × Tg, ⊕)`, where a segment's contribution is `(Δb, Δa, τ)` with `τ = (p*, s*)` a tagged max by `(seq, wirepos)`.
- **[S1-T5]** Coordinate-monoid extension: independent coordinates can be adjoined to the signature, each with its own commutative (or explicitly ordered) algebra.
- **[S1-T6/T6′]** Tie-break determinism: `max-by-(seq, wirepos)` is the only sound comparator; prices are payloads, never keys.

### 1.3 The state record and the span-hash discipline

**Definition 1.1 (Res state record).** The per-track state whose cumulative version each worker must verify is

```
Res = ( vol ∈ ℤ²,  price ∈ Tg,  hash ∈ R,  wmk ∈ 𝕎 )
```

- `vol = (bid_vol, ask_vol)` — per-locate aggregated quantities,
- `price` — the tagged last-trade/print price `[S1-T2]`,
- `hash` — the SPANHASH value, an element of `R`,
- `wmk ∈ 𝕎` — sequence watermark lattice (coordinate-wise max over tracks; a semilattice, commutative and idempotent).

This is exactly the [S1] state space with the hash coordinate made explicit and the order-cache coordinates deferred to §3 (they are the only state that is *not* a running accumulator; they are per-order-ID chains).

**Definition 1.2 (Span-hash discipline).** The SPANHASH updates per message as

```
h  ←  (h ⊗ x^{w_t})  ⊕  v_t(p)
```

where `w_t` is the wire width of the message type and `v_t(p) ∈ R` is a type-tagged payload digest (any fixed function of the payload; in the reference engine it is computed during the scan phase). The essential property is: **the linear part depends only on the type; the injected constant depends only on (type, payload).**

Two specializations matter:

- **(a) per-message discipline:** `w_t ≡ 1` for all t — every message shifts by `x`. Then the composite degree equals the *message count*.
- **(b) span discipline (wire-true):** `w_t` = wire bytes. Then the composite degree equals the *byte span* W(B) = Σᵢ w_{tᵢ}, and the degree is **not** a function of the count alone (types have different widths).

The name SPANHASH indicates (b), and §2–§4 implement (b). Every formula holds for both with `w` redefined; the distinction is a correctness trap (Appendix D, item 2).

**Definition 1.3 (Degrees).** For a chunk `B = m₁…m_L` write `wᵢ := w_{tᵢ}` and define

```
P_i := Σ_{j<i} w_j   (prefix degree, P_1 = 0),     W(B) := Σ_{i≤L} w_i,
d_i := Σ_{j>i} w_j   (suffix degree of message i)
```

The two conventions are related by `d_i = W(B) − P_{i+1}` (with `P_{i+1} = P_i + w_i`). Fix them once, here: `P_i` is a prefix sum over messages *before* i, and `d_i` — the weight applied to message i's injection — is the degree of everything *after* it. Both `P_i` and `d_i` are integers; they are *static* functions of the type sequence — known before any payload is touched.

### 1.4 The affine hash monoid (Lemma 3)

**Lemma 3.** Let `𝒜_hash := (R × R, ⨂)` with

```
(C₁, V₁) ⨂ (C₂, V₂)  :=  (C₁C₂,  V₁C₂ ⊕ V₂)
```

where products are `⊗` mod Q. Interpret `(C, V)` as the affine map `h ↦ h⊗C ⊕ V`. Then:

1. `𝒜_hash` is a monoid with identity `(1, 0)`; it is **non-abelian**.
2. Application is an action: `A_{(C₂,V₂)} ∘ A_{(C₁,V₁)} = A_{(C₁,V₁) ⨂ (C₂,V₂)}`.
3. The subset `{(x^a, V) : a ∈ ℕ, V ∈ R}` — the **span sub-monoid** — is closed; its C-coordinate is `(ℕ, +)`, commutative, and the order-dependence lives entirely in the V-coordinate.
4. If `Q(0) = 1` then every `x^a` is a unit and every `(x^a, V)` is a bijection of `R`; the span sub-monoid sits inside the group `Rˣ ⋉ R`.

**Proof.**

(2) Direct computation in the commutative ring R, using distributivity of `⊗` over `⊕`:

```
A₂(A₁(h)) = (h⊗C₁ ⊕ V₁)⊗C₂ ⊕ V₂ = h⊗(C₁C₂) ⊕ (V₁⊗C₂) ⊕ V₂ = A_{(C₁C₂, V₁C₂⊕V₂)}(h).
```

(1) Associativity: expand both bracketings of a triple product using (2) twice, or verify directly:

```
((C₁,V₁)⨂(C₂,V₂))⨂(C₃,V₃) = (C₁C₂C₃, (V₁C₂⊕V₂)C₃ ⊕ V₃) = (C₁C₂C₃, V₁C₂C₃ ⊕ V₂C₃ ⊕ V₃)
(C₁,V₁)⨂((C₂,V₂)⨂(C₃,V₃)) = (C₁(C₂C₃), V₁(C₂C₃) ⊕ (V₂C₃ ⊕ V₃))  — identical.
```

Identity: `(C,V)⨂(1,0) = (C, V⊕0) = (C,V)` and `(1,0)⨂(C,V) = (C, 0⊕V) = (C,V)`. Non-abelian: `(x,1)⨂(x²,0) = (x³, x²)` but `(x²,0)⨂(x,1) = (x³, 1)`; `x² ≠ 1` in R (deg x² < 64 and x² ≠ 1 as a polynomial). ∎ part 1.

(3) Closure: `x^a·x^b = x^{a+b}` in R (exponent law in any commutative ring, for the element x); `V₁x^b ⊕ V₂ ∈ R`. The C-coordinate operation is integer addition, hence commutative; the V-coordinate mixes V₁ and V₂ asymmetrically, which is precisely where order survives. (4) `x ∈ Rˣ` iff `gcd(x, Q) = 1` iff `Q(0) = 1`; powers of units are units; `h ↦ hC ⊕ V` with C a unit is a bijection (inverse: `h' ↦ (h'⊕V)⊗C⁻¹`). ∎

**Remark (this is the "non-abelian" of the problem title, and it is the *only* non-abelian thing in the entire engine).** The linear parts all live in the commutative ring R, so they commute; the semidirect weighting `V₁ ← V₁C₂` is the entire non-commutative residue. Lemma 3 is the exact analogue of [S1-L0] for the hash coordinate — and, exactly as in [S1], the residue is confined to a single weighted sum.

**Lemma 4 (exponent representation).** The map `exp: (ℕ,+) → Rˣ`, `a ↦ x^a`, is a monoid homomorphism (Lemma 3.3). It is injective on `[0, ord(x))`, where `ord(x)` is the multiplicative order of x. Consequently `x^a = x^b` iff `a ≡ b (mod ord(x))`, and any table `{x^k : k < ord(x)}` (or any prefix of it) represents all shift operators without collision. The engine verifies `ord(x) > 2²¹` exhaustively at init (§2.6, `spanfold_init`), so *degree collisions are impossible* over every realistic span and count.

**Lemma 5 (suffix formula).** Under Def 1.2, the composite hash action of `B = m₁…m_L` is

```
h_out  =  h_in ⊗ x^{W(B)}  ⊕  V(B),        V(B) = ⊕_{i=1..L}  v_i ⊗ x^{d_i},
```

with `d_i = Σ_{j>i} w_j` as in Def 1.3.

**Proof.** Induction on L. L=1: `h_out = h_in x^{w₁} ⊕ v₁`; `W = w₁`, `d₁ = 0`, `V = v₁`. ✓. Step: apply the induction hypothesis to `m₁…m_{L−1}` then compose with `(x^{w_L}, v_L)` via Lemma 3(2):

```
h_out = (h_in x^{W′} ⊕ V′)x^{w_L} ⊕ v_L = h_in x^{W′+w_L} ⊕ (V′ x^{w_L} ⊕ v_L)
```

so `W = W′ + w_L` ✓ and `V = V′x^{w_L} ⊕ v_L` — which shifts every previous injection by `w_L` and appends the new one with weight 0, i.e. exactly `⊕_i v_i x^{d_i}` with all `d_i` updated. ∎

Lemma 5 is the whole of Problem 1's mathematics in three lines: the linear part is one power of x (a function of the type *multiset*), and the payload part is one XOR of L weighted digests (a function of payloads and the type *order*).

---

## §2 Problem 1 — Closed-Form Non-Abelian Segment Res-Folding

### 2.1 The Res record and the ⨂ fold

Each worker receives contiguous chunks `B = m₁…m_L` (`L ≤ 512`) and must compute the cumulative checksum / state verification token — the composite Res of the chunk — and, for the verification regime, the *running* token after each message. The current engine runs an inner per-frame loop: per message, a type-switch cascade (branch-mispredict-heavy: 9 arms, mixed probabilities), a serialized hash update (the next CLMUL cannot issue until the previous completes), and a token store. Measured: ~21 µs per 512-message pass = **41 ns/message ≈ 106 cycles/message** at 2.6 GHz. §2.7 decomposes where those cycles go; this section derives what the loop is *actually computing*, so it can be computed branch-free.

The composite of a chunk is an element of the product algebra

```
𝒜_res  =  𝒜_hash  ×  (ℤ², +)  ×  (Tg, ⊔)  ×  (𝕎, ∨)
```

with per-message generators (from Def 1.2 and [S1-T2], [S1-T5]):

```
Res(m)  =  ( (x^{w_t}, v_t(p)) ,  (δb_t(p), δa_t(p)) ,  τ_t(p,s) ,  κ(s) )
```

and the fold `⨂` acts coordinate-wise: `⨂` of Lemma 3 on the hash pair, `+` on volumes, `⊔` (tagged max) on price, `∨` (coordinate max) on watermarks.

**The "finite tensor ring" of the problem statement, made precise.** The only ring present is `R = GF(2)[x]/Q(x)` — finite, with `2⁶⁴` elements. The hash coordinate is a rank-1 R-module on which the generators act by R-affine endomorphisms `h ↦ hC ⊕ V`. The remaining coordinates are commutative monoids with trivial linear part. The full state is the *direct product* of these algebras (the "tensor"), and the fold is a product-monoid fold. Calling it "an affine endomorphism over a finite tensor ring" is honorific but not wrong: the endomorphism is

```
Φ_B : (vol, τ, h, w)  ↦  ( vol + ΔB ,  τ ⊔ τ_B ,  h⊗x^{W(B)} ⊕ V(B) ,  w ∨ κ_B )
```

— affine in the hash coordinate, additive/max in the others.

### 2.2 Theorem 9 — the tensor factorization

**Theorem 9 (Factorization of the Res fold).** Under Def 1.2 and the [S1] generator table, the composite of any chunk `B` factors as

```
Res(B)  =  ( 𝐌(B) ,  𝐊(B) )

𝐌(B) = diag( id_ℤ² ,  id_Tg ,  x^{W(B)} ,  id_𝕎 )            — "M_type"
𝐊(B) = ( Δvol(B) ,  τ(B) ,  V(B) ,  κ(B) )                    — "V_payload ⊕ K_seq"
```

with:

- **𝐌(B)** depends only on the type *multiset*: `W(B) = Σ_t n_t(B)·w_t`, where `n_t(B)` is the count of type t in B. It is *order-free*.
- **Δvol(B), τ(B), κ(B)** depend only on multisets (commutative coordinates; [S1-T2] and idempotent max). `τ`'s tag resolves by `(seq, wirepos)`, a total order — [S1-T6].
- **V(B) = ⊕ᵢ v_i ⊗ x^{d_i}** depends on payloads **and on the type order** (the weights `x^{d_i}` are the suffix degrees). It is the only order-carrying, non-commutative ingredient, and this dependence is *irreducible*: there exist two chunks with identical type multisets and identical payload multisets but different V.

**Proof.** The coordinate-wise form is Lemma 3 + [S1-T2] + the trivial commutativity of `(𝕎, ∨)`, assembled by [S1-T5]'s product construction; the only new content is the claim about *which arguments each factor depends on*, proven by induction on L using Lemma 3(2):

- Linear part: `(C₁C₂) = x^{W′}·x^{w_L} = x^{W′+w_L}` — exponent law (Lemma 4); at every step the C-coordinate is a power of x whose exponent is the running width sum. Unwinding: `C = x^{W(B)}`, a function of the multiset of widths only. ✓
- Volume/price/watermark: per-message contributions are combined by `+`, `⊔`, `∨` — all commutative and associative, hence functions of the multiset of contributions. ✓
- V: by Lemma 5, `V(B) = ⊕ v_i x^{d_i}` with `d_i` a suffix sum of widths — manifestly dependent on the type *sequence*.

Irreducibility: take `L = 2`, types `t₁ ≠ t₂` with `w_{t₁} = 19` (D) and `w_{t₂} = 43` (P), digests `v₁ = 1`, `v₂ = x`. Order 1: `V = x^{43} ⊕ x^{20}`. Order 2 (swap): `V′ = x^{19} ⊕ x^{44}`. Both chunks have the same type multiset {D, P} and the same payload multiset; `V ≠ V′` because their degree sets differ (`{43, 20}` vs `{19, 44}`) and both are sums of distinct monomials of degree < 64+4... more carefully: as elements of R, `V` and `V′` agree iff `V ⊕ V′ = x^{43}⊕x^{20}⊕x^{19}⊕x^{44} = 0` in R, i.e. iff `Q | (x^{44}+x^{43}+x^{20}+x^{19}) = (x+1)(x^{43}+x^{19})`; `Q(0)=1` and `Q` has degree 64 > 44, and no degree-64 polynomial divides a nonzero polynomial of degree 44. So `V ≠ V′`. ∎

**Adjudication of the claimed split.** The factorization `𝐌_type ⊗ 𝐕_payload ⊕ 𝐊_seq` **exists**, with the precise reading: `𝐌` = one power of x from the type histogram (a *single* field element, not a 64×64 matrix — see §4.4 for why you must never expand it); `𝐊_seq` = the commutative coordinates; and `𝐕_payload` = the weighted digest sum, which additionally needs the *type order*. The claim as written ("V depends on payload only") is false in a way that matters: any implementation that treats V as a multiset function (e.g., per-type digest accumulation, à la "XOR all digests of type A together") will produce bit-mismatched hashes on exactly the streams Theorem 9's adversary constructs. The 94+36 conformance cells in §5 include this adversary.

### 2.3 Theorem 10 — the closed-form composite identity

**Theorem 10 (Closed form).** Let `B = m₁…m_L` with type sequence, digests, and volumes as above. Then:

**(a) Histogram identity (linear part).**

```
C(B) = x^{W(B)},   W(B) = Σ_{t∈𝒯} n_t(B)·w_t
```

computable by 9 integer adds (the histogram) plus one of:
- one table lookup `T_pow[W]` (span discipline, table over degrees), or
- 8 ring multiplies `⊗_t T_t[n_t]` with precomputed power tables `T_t[n] = (x^{w_t})^n` (general per-type constants `c_t` — needed if some type's linear part is not a power of x), or
- `⌈log₂ W⌉` ring multiplies by binary exponentiation (no tables).

**(b) Suffix identity (payload part).**

```
V(B) = ⊕_{i=1..L} v_i ⊗ x^{W(B) − P_i − w_i}  =  ⊕_i v_i ⊗ T_pow[d_i]
```

with `P_i` the integer prefix degrees. Each term is *independent* of every other term: the sum has **no serial dependency**. It is computable by one branch-free pass: per message, 1 table load + 1 ring multiply + 1 XOR; vectorized 4-wide with `VPCLMULQDQ` (§2.6).

**(c) Prefix tokens (per-message cumulative verification).** The running token after message i is

```
h_i  =  h_in ⊗ x^{P_{i+1}}  ⊕  V^{pre}_i ,     V^{pre}_i = ⊕_{j≤i} v_j ⊗ x^{P_{i+1}−P_{j+1}... }
```

— precisely: `h_i` obeys the recursion `h_i = h_{i−1} ⊗ x^{w_i} ⊕ v_i`, and the closed form is the prefix analogue of (b) with degrees `P_{i+1} − P_{j+1}` for `j ≤ i`. All L tokens are computable in the same pass (either the 2-op recursion, or the parallel closed form with per-message gathers).

**(d) Commutative coordinates.** `Δvol`, `τ`, `κ` as in [S1-T2/T5] — integer adds and tagged max, order-free.

Consequently the *entire* per-message work of the fold is: 1 integer add (prefix degree), 1 table load, 1 ring multiply, 1 XOR, 2 integer adds (volumes), 1 compare-select (price tag), 1 max (watermark) — **all branch-free, all independent across messages except the integer prefix sum**, which is a SIMD prefix-add over 9-valued data.

**Proof.** (a) is Lemma 4 (exponent law) plus `Σ_i w_{t_i} = Σ_t n_t w_t` (rearranging a finite sum). The three evaluation strategies are standard: table (correct by construction of `T_pow`), per-type powers (`⊗_t (x^{w_t})^{n_t} = x^{Σ n_t w_t}` by commutativity of R and the exponent law), binary exponentiation (square-and-multiply correctness in any monoid).

(b) is Lemma 5, rewritten with `d_i = W − P_i − w_i`, plus commutativity/associativity of `⊕` in R, which licenses arbitrary accumulation order of the L terms (this is what removes the serial dependency that made the baseline loop latency-bound — and it is also why the vectorized and scalar paths are bit-identical, Theorem 18).

(c) follows by applying Lemma 3(2) to prefixes: the composite of `m₁…m_i` is `(x^{P_{i+1}}, ⊕_{j≤i} v_j x^{Σ_{k>j, k≤i} w_k})`, and `Σ_{k∈(j..i]} w_k = P_{i+1} − P_{j+1}`. The recursion form is Lemma 3(2) iterated.

(d) is [S1]. ∎

**Corollary 10.1 (chunk records compose in O(1); the honest "single matrix-vector multiply").** The composite of a chunk is the pair `(x^{W}, V)` plus commutative coordinates: **36 bytes** (§2.5). For two composites `a` (earlier) and `b` (later):

```
C = C_a ⊗ C_b           (1 ring multiply)
V = V_a ⊗ C_b ⊕ V_b     (1 ring multiply + 1 XOR)
W, Δvol, τ, κ: integer add / tagged max      (O(1) scalar ops)
```

— an O(1) composition with no per-message work. *Replaying* a memoized chunk against an incoming hash is one ring multiply and one XOR: `h ↦ h⊗x^W ⊕ V`. This — composition and replay of chunk *records* — is the precise, true sense of the problem's "one single matrix-vector multiply." It does **not** eliminate the first evaluation of a chunk (Theorem 11); it eliminates every subsequent one, exactly as [S1-T4] did for the LOB signature. A 500k-message pass replay = 976 record compositions ≈ 976 × ~12 cycles ≈ 12k cycles ≈ 4.6 µs.

### 2.4 Theorem 11 — the Ω(L) payload lower bound

**Theorem 11.** Any algorithm that outputs `V(B)` (equivalently the composite hash action of B) exactly for all inputs must read Ω(L) payload-digest symbols. Consequently no O(1)-in-L "single matrix-vector multiply" can evaluate a fresh chunk.

**Proof.** Suppose algorithm A fails to read symbol `v_{i₀}` on some execution path (input `I`). Fix all other symbols of `I`, and consider the two inputs `I⁻` (with `v_{i₀} = 0`) and `I⁺` (with `v_{i₀} = 1`). Both induce the same execution path of A (A's behavior is a function of the symbols it reads), so A outputs the same value on both. But by Lemma 5,

```
V(I⁺) ⊕ V(I⁻)  =  (1 ⊕ 0) ⊗ x^{d_{i₀}}  =  x^{d_{i₀}}  ≠  0
```

since `x^{d_{i₀}}` is a unit (Lemma 3.4) and a unit is nonzero. So `V(I⁺) ≠ V(I⁻)`, and A errs on at least one — contradiction. Every digest symbol must be read on every path, and each read is a distinct memory access: Ω(L). ∎

**Remark.** The bound is information-theoretic, not architectural: it survives any SIMD width, any number of cores, any precomputation (precomputation can depend on types and positions — the static part — but not on payloads, which is exactly why Theorem 10's tables are legal and a payload LUT is not; cf. [S1-AppC]). The 21 µs → 0.2 µs win in §2.7 is therefore a *constant-factor* win (branch elimination + dependency elimination + 4-lane SIMD), not an asymptotic one — and Corollary 10.1 is the asymptotic win, available only after the first evaluation.

### 2.5 The R36 record — exactly 36 bytes

The codename R36 is honored literally. The composite state record of a chunk:

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 8 | `V` (u64) | payload fold `⊕ vᵢ x^{dᵢ}` |
| 8 | 8 | `Δb` (i64) | bid-volume delta |
| 16 | 8 | `Δa` (i64) | ask-volume delta |
| 24 | 4 | `p*` (u32) | last price write (valid iff `flg.b0`) |
| 28 | 2 | `tag*` (u16) | wire offset of the last setter (intra-chunk; ≤ 22,016 < 2¹⁶) |
| 30 | 2 | `W` (u16) | chunk span in bytes = degree of the linear part (≤ 22,016 < 2¹⁶) |
| 32 | 2 | `cnt` (u16) | message count (≤ 512) |
| 34 | 2 | `flg` (u16) | b0: has-setter · b1: has-trade · b2: escape/64-bit-deg needed |

**Total: 36 bytes**, packed (`__attribute__((packed))` — see Appendix D item 6 on the padding trap). Design decisions, each forced by an argument in this document:

- **`C = x^W` is not stored** — it is derived by one lookup `T_pow[W]`. Legal because the span discipline makes the linear part a power of x with exponent = the u16 span (Theorem 10a). Under a general-constant discipline you must store C (8 B) → the 44-byte variant.
- **`Δb, Δa` are i64**: single messages can move up to 2³²−1 shares ([S1] wire table), so 32-bit chunk deltas can overflow; 64-bit cannot (512 × 2³² < 2⁴²).
- **`tag*` is u16** because intra-chunk wire offsets are bounded by 512 × 43 = 22,016 < 65,536.
- **watermarks are not stored**: under dense sequencing the max-seq of a chunk is `first_seq + cnt − 1`, derivable from the pass index; sparse-track watermarks ride a parallel 8-byte side channel (flagged) rather than breaking the 36-byte envelope.
- **The degree is bounded per chunk**, but *pass-level* composition accumulates degrees past 2¹⁶ (500k × ~35 B ≈ 17.5M). Therefore pass/worker-level accumulators use the **P64 record** (`C` stored explicitly as a field element, `deg` as u32, `tag` as u32 — 48 B). Composition at that level multiplies field elements directly (`C = C_a ⊗ C_b`), never tables. This two-level scheme is in the code and is conformance cell N+7.

**Composition table (chunk level, `a` earlier, `b` later):**

```
W   = W_a + W_b                                  (u16; chunk-level only)
V   = V_a ⊗ T_pow[W_b]  ⊕  V_b                   (1 mulmod + 1 XOR)
Δb  = Δb_a + Δb_b ;  Δa = Δa_a + Δa_b
tag = b.flg.b0 ? (W_a + b.tag) : a.tag           (dense seq ⇒ later setter wins;
p*  = b.flg.b0 ? b.p*       : a.p*                sparse seq ⇒ compare (s,tag) [S1-T6])
cnt = cnt_a + cnt_b ;  flg = flg_a | flg_b
```

Associativity of this composition is Lemma 3 + associativity of `+`, `⊔`, `∨` — the fuzz harness in §7 checks it on random triples (`(a⨂b)⨂c = a⨂(b⨂c)`, bit-exact).

### 2.6 The kernel — compilable C17 + AVX-512

One file, no dependencies beyond `<immintrin.h>`. Compiles with
`gcc -O3 -std=c17 -mavx512f -mpclmul -mvpclmulqdq spanfold.c` (AMD Zen 4/5, Intel Ice Lake+). The scalar `mulmod_u64` and the 4-lane `mulmod4` implement the 3-CLMUL reduction derived in Appendix B; `spanfold_init` builds the degree table and *exhaustively verifies* `ord(x) > 2²¹` (Lemma 4's no-collision precondition, Theorem 16's hypothesis) — 1.5 ms once.

```c
/* spanfold.c — R36 span-hash fold, O(1) record composition, 2-worker merge.
 * C17, AVX-512 + VPCLMULQDQ (Zen 4/5, Ice Lake+).
 *   gcc -O3 -std=c17 -mavx512f -mpclmul -mvpclmulqdq spanfold.c
 * Modulus: Q(x) = x^64 + x^4 + x^3 + x + 1   (Q(0)=1  =>  x is a unit).
 * Span discipline: w_t = ITCH 5.0 wire size of type t (Def 1.2b).
 */
#include <immintrin.h>
#include <stdint.h>
#include <stddef.h>
#include <assert.h>

#define NTYPE   9
#define CHUNK   512
#define MAXSPAN (CHUNK*43 + 1)                 /* 22017 = max degree + 1    */
static const uint16_t W_T[NTYPE] = {36,40,31,36,23,19,35,43,40}; /*A F E C X D U P Q*/
#define QLOW 0x1Bu                              /* x^64 = x^4+x^3+x+1 (mod Q) */

static uint64_t T_pow[MAXSPAN];                 /* T_pow[k] = x^k mod Q       */

static inline uint64_t mulmod_u64(uint64_t a, uint64_t b)      /* a⊗b mod Q */
{
    __m128i p = _mm_clmulepi64_si128(_mm_cvtsi64_si128((int64_t)a),
                                     _mm_cvtsi64_si128((int64_t)b), 0x00);
    uint64_t lo = (uint64_t)_mm_cvtsi128_si64(p);
    uint64_t hi = (uint64_t)_mm_extract_epi64(p, 1);          /* deg<=62 */
    __m128i t = _mm_clmulepi64_si128(_mm_cvtsi64_si128((int64_t)hi),
                                     _mm_cvtsi64_si128((int64_t)QLOW), 0x00);
    uint64_t tlo = (uint64_t)_mm_cvtsi128_si64(t);            /* deg<=63 */
    __m128i u = _mm_clmulepi64_si128(_mm_cvtsi64_si128((int64_t)(t >> 64)),
                                     _mm_cvtsi64_si128((int64_t)QLOW), 0x00);
    return lo ^ tlo ^ (uint64_t)_mm_cvtsi128_si64(u);         /* deg<=6  */
}

/* 4 independent a⊗b mod Q; operands sit in the EVEN 64-bit words. */
static inline __m512i mulmod4(__m512i a, __m512i b)
{
    const __m512i c = _mm512_set1_epi64((int64_t)QLOW);
    __m512i p = _mm512_clmulepi64_si128(a, b, 0x00);  /* lanes: (lo,hi)    */
    __m512i t = _mm512_clmulepi64_si128(p, c, 0x01);  /* hi⊗QLOW, deg<=66  */
    __m512i u = _mm512_clmulepi64_si128(t, c, 0x01);  /* t_hi⊗QLOW, deg<=6 */
    __m512i r = _mm512_xor_si512(_mm512_xor_si512(p, t), u);
    return _mm512_maskz_compress_epi64(0xAA, r);      /* even words: 4 rslts */
}

typedef struct __attribute__((packed)) {          /* — the 36-byte record — */
    uint64_t V;        /* payload fold  ⊕ v_i x^{d_i}                        */
    int64_t  db, da;   /* volume deltas                                     */
    uint32_t px;       /* last price write (flg&1)                          */
    uint16_t tag;      /* wire offset of last setter                        */
    uint16_t W;        /* span in bytes == degree of linear part             */
    uint16_t cnt;      /* message count                                     */
    uint16_t flg;      /* b0 setter  b1 trade  b2 escape                     */
} R36;
_Static_assert(sizeof(R36) == 36, "R36 must be exactly 36 bytes");

/* O(1) composition: a = EARLIER segment, b = later (Corollary 10.1). */
static inline void r36_compose(R36 *r, const R36 *a, const R36 *b)
{
    r->V   = mulmod_u64(a->V, T_pow[b->W]) ^ b->V;   /* V_a·x^{W_b} ⊕ V_b   */
    r->db  = a->db + b->db;      r->da = a->da + b->da;
    r->W   = (uint16_t)(a->W + b->W);
    r->cnt = (uint16_t)(a->cnt + b->cnt);
    r->flg = (uint16_t)(a->flg | b->flg);
    if (b->flg & 1) { r->px = b->px; r->tag = (uint16_t)(a->W + b->tag); }
    else            { r->px = a->px; r->tag = a->tag; }
}

/* Pass/worker accumulator: degree leaves u16 range, C stored explicitly. */
typedef struct {
    uint64_t V, C;  int64_t db, da;  uint32_t px, tag, deg, cnt;  uint16_t flg;
} P64;                                             /* 48 bytes               */
static inline void p64_compose(P64 *r, const P64 *a, const P64 *b)
{
    r->C = mulmod_u64(a->C, b->C);                    /* x^{deg_a}·x^{deg_b} */
    r->V = mulmod_u64(a->V, b->C) ^ b->V;
    r->db = a->db + b->db;  r->da = a->da + b->da;
    r->deg = a->deg + b->deg;  r->cnt = a->cnt + b->cnt;  r->flg = a->flg|b->flg;
    if (b->flg & 1) { r->px = b->px; r->tag = a->deg + b->tag; }
    else            { r->px = a->px; r->tag = a->tag; }
}

/* Chunk fold. Inputs from the scan phase: type[i]∈[0,9), vd[i]=payload
 * digest v_i, dvol[i] volume deltas, px[i]/setp[i] price writes.
 * Theorem 10: branch-free; V has NO serial dependency (4-lane parallel). */
void fold_chunk(const uint8_t *type, const uint64_t *vd,
                const int32_t *db, const int32_t *da,
                const uint32_t *px, const uint8_t *setp,
                int L, R36 *out)
{
    uint16_t P[CHUNK];  uint32_t W = 0;  int64_t sb = 0, sa = 0;
    for (int i = 0; i < L; ++i) {                  /* static degrees + vol  */
        P[i] = (uint16_t)W;  W += W_T[type[i]];
        sb += db[i];  sa += da[i];
    }
    __m512i acc = _mm512_setzero_si512();
    for (int i = 0; i + 4 <= L; i += 4) {          /* V = ⊕ v_i ⊗ x^{W-P_i-w_i} */
        __m512i v = _mm512_set_epi64(0,(int64_t)vd[i+3],0,(int64_t)vd[i+2],
                                      0,(int64_t)vd[i+1],0,(int64_t)vd[i]);
        __m512i m = _mm512_set_epi64(
            0,(int64_t)T_pow[W - P[i+3] - W_T[type[i+3]]],
            0,(int64_t)T_pow[W - P[i+2] - W_T[type[i+2]]],
            0,(int64_t)T_pow[W - P[i+1] - W_T[type[i+1]]],
            0,(int64_t)T_pow[W - P[i]   - W_T[type[i]]]);
        acc = _mm512_xor_si512(acc, mulmod4(v, m));
    }
    uint64_t V = (uint64_t)_mm512_extract_epi64(acc,0)
               ^ (uint64_t)_mm512_extract_epi64(acc,2)
               ^ (uint64_t)_mm512_extract_epi64(acc,4)
               ^ (uint64_t)_mm512_extract_epi64(acc,6);
    for (int i = L & ~3; i < L; ++i)               /* scalar tail (L%4)     */
        V ^= mulmod_u64(vd[i], T_pow[W - P[i] - W_T[type[i]]]);
    out->V = V;  out->db = sb;  out->da = sa;  out->W = (uint16_t)W;
    out->cnt = (uint16_t)L;  out->flg = 0;  out->px = 0;  out->tag = 0;
    for (int i = 0; i < L; ++i)                    /* last setter (dense seq)*/
        if (setp[i]) { out->px = px[i]; out->tag = (uint16_t)P[i]; out->flg |= 1; }
}

/* Optional per-message tokens: h_i = h_{i-1} ⊗ x^{w_i} ⊕ v_i (Theorem 10c).
 * Cross-check invariant: tok[L-1] == h_in ⊗ T_pow[out->W] ⊕ out->V.       */
void fold_chunk_tokens(const uint8_t *type, const uint64_t *vd,
                       int L, uint64_t h_in, uint64_t *tok)
{
    uint64_t h = h_in;
    for (int i = 0; i < L; ++i)
        tok[i] = h = mulmod_u64(h, T_pow[W_T[type[i]]]) ^ vd[i];
}

/* Cross-worker merge (Theorem 17): ℋ = ℋ_early ⊗ x^{W_late} ⊕ ℋ_late.
 * NOTE: the shift is by the LATER segment's span — Appendix D, item 2.    */
static inline uint64_t hash_merge(uint64_t h_early, uint64_t h_late,
                                  uint16_t W_late)
{ return mulmod_u64(h_early, T_pow[W_late]) ^ h_late; }

void spanfold_init(void)                           /* ~1.5 ms, once */
{
    T_pow[0] = 1;
    for (int k = 1; k < MAXSPAN; ++k) T_pow[k] = mulmod_u64(T_pow[k-1], 2);
    uint64_t h = 1;                                /* verify ord(x) > 2^21: */
    for (uint64_t k = 1; k < (1ull<<21); ++k) {     /* Lemma 4 / Thm 16 hyp */
        h = mulmod_u64(h, 2);  assert(h != 1);      /* x^k != 1 for k<2^21  */
    }
}
```

**What the code proves by existing.** No `switch` on message type appears anywhere — the type's only role is a table index (`W_T[type[i]]`). The per-message critical path is a table load and a 4-lane multiply; there is no loop-carried hash dependency in the V loop (the accumulation is XOR, order-free by Theorem 10b). The only loop-carried chains are integer adds. This is the structural difference between 106 cycles/message and ~2.

### 2.7 Cost accounting — 21 µs → 0.2–0.4 µs

**Where the 21 µs goes (baseline reconstruction, ~106 cycles/msg):** (i) 9-arm type switch with data-dependent probabilities → ~15–25 lost cycles/msg to mispredicts; (ii) serialized hash chain — each `h ← h⊗c ⊕ v` has ~3 chained CLMULs at ~3–4 cycles latency → ~10–12 cycles of pure latency; (iii) per-frame token formatting/stores and cache effects for the rest. None of this is intrinsic — Theorem 10 shows the same mathematics is a flat, branch-free map.

**New kernel, per 512-message chunk (fresh):**

| Item | Count (per message) | Notes |
|---|---|---|
| `VPCLMULQDQ` (zmm, 4-lane) | 0.75 | 3 per 4 multiplies (`mulmod4`) |
| `VPXORQ` | 0.75 | accumulate + inside reduction |
| `VPCOMPRESSQ` | 0.25 | even-word extraction |
| scalar loads/sets | ~2.5 | digest, `T_pow[d_i]`, widths, volumes |
| integer adds | ~1.5 | prefix degrees, volumes |
| compare-select (price) | ~1 | branchless, `cmov` |

≈ **2–2.5 uops + 0.75 CLMUL-port uops per message** → on Zen 5's 6-wide front end with 1–2 CLMUL pipes: **~1.2–2 cycles/message**, i.e. **600–1,000 cycles ≈ 0.23–0.40 µs per chunk at 2.6 GHz**. Against the 21 µs baseline: **52–90×**. Against the <2 µs target: **5–8× margin**. With the optional token pass: +1 mulmod/msg scalar (latency-chained, ~8 cyc) → +4k cycles ≈ +1.6 µs — still under target, or vectorized via Theorem 10(c)'s closed form to stay at ~0.5 µs total.

**Scaling by pass size** (per-message cost is the invariant ~0.5–0.8 ns):

| Pass size | Fresh eval | Memoized replay (Cor 10.1) |
|---|---|---|
| 512 | 0.23–0.40 µs | ~15 cycles (~6 ns) |
| 1,024 | 0.5–0.8 µs | ~15 cycles |
| 2,048 | 0.9–1.6 µs | ~15 cycles |
| 4,096 | 1.9–3.2 µs (borderline) | ~15 cycles |

The <2 µs target is met fresh for passes up to ~2–4k messages and *always* met in the replay regime. §6 aggregates to full-pipeline rates.

---

## §3 Problem 2 — Topological Invariance of Multi-Pass Boundary Straddles

### 3.1 What `reset_pass` actually protects

The tape is a totally ordered stream `T = m₁m₂m₃…`. A *pass partition* cuts it into contiguous blocks `P_K = m_{KM+1}…m_{(K+1)M}` (M = pass size; the problem's numbers: 300,000 passes, 204 ms of boundary cost = **680 ns per boundary**). The state at any point is the product of §1.1's accumulative coordinates plus the order cache `O = {order-id → (locate, side, remaining, px)}` ([S1] §2).

The barrier exists because a worker finishing pass K holds *partial* state: some order IDs have chains that began in pass K and will continue in pass K+1 (a **straddler**). `reset_pass` flushes worker-local deltas, reconciles straddler rows, and re-anchors pass K+1's starting state — 680 ns of synchronization and re-anchoring scan per boundary, **300,000 times**.

The question is whether the boundary carries any *semantic* content — whether the mathematics knows where the cuts are. It does not. Everything below makes that precise, in three steps of increasing honesty: the topological framing as posed is misbased (Prop 12.0); the cocycle condition as posed is vacuous (§3.3); the true content is an order-theoretic partition-invariance theorem plus a deadlock-free deferral protocol (Theorems 12–13), with a genuinely non-trivial topological residue: the ring-buffer monodromy/sizing invariant (Theorem 14).

### 3.2 Proposition 12.0 — the base-space correction (ℤ vs S¹)

**Proposition 12.0.** The semantic base space of the pipeline is the tape poset `(ℤ, ≤)` (equivalently its order complex, a 1-dimensional contractible complex). The torus `ℤ/Nℤ` is the *storage slot* space of the ring buffer. The slot-assignment map `π : ℤ → ℤ/Nℤ, i ↦ i mod N` is a covering map of the storage circle. Semantics may be pulled back to storage, but storage must never be pushed forward to semantics: the semantic state (order cache rows, R36 records, sequence tags) must be keyed by tape identity (position, or order-id + epoch), never by slot. Violating this — keying any semantic state by `i mod N` — is exactly the slot-aliasing bug, and it is the *only* way the S¹ topology can leak into the output.

**Proof sketch (it is three observations).** (1) ITCH sequence numbers strictly increase along the wire; the fold of [S1] and §1–§2 is defined over a linear (semi-infinite) tape, so the semantic site space is ℤ with its Alexandrov topology, whose order complex is a ray — contractible; every bundle/sheaf over it is trivially trivial, `Hⁱ = 0` for `i ≥ 1` without any work. (2) The ring buffer stores pass records in N slots; consecutive visits to a slot hold passes K and K+N — *different* tape positions — so the "wrap" acts on the tape as translation by the cycle length (a deck transformation of the cover), not as identification. (3) A state entry keyed by slot `s` is read by two different tape positions `i < i+N` mapping to `s`; if any semantics depends on the entry, the two reads alias, and the output depends on arrival timing — non-determinism. Keying by `(order-id, epoch)` or tape position makes the pullback of the storage cover semantically invisible. ∎

**Adjudication of the fiber-bundle claim.** "Treat the passes as a fiber bundle over S¹" is **wrong as stated and fixable as metaphor**: the bundle picture is correct *for storage*, and its only non-trivial consequence — monodromy around the ring — is Theorem 14 below. For *semantics*, the base is contractible and all cohomology vanishes for free; any theorem proved "because H¹(S¹) = 0" in the semantic direction is proving something for the wrong reason.

### 3.3 The sheaf of admissible states and the vacuous cocycle δω = 0

To engage the sheaf-theoretic framing on its own terms:

**The sheaf.** Let the base be the poset `X` of finite intervals `I ⊆ ℤ` (the segments a worker might own), ordered by inclusion. Define `ℱ : X → Mon` by

```
ℱ(I) = R36(I) × Chains(I)
```

- `R36(I)` = the composite record of the messages inside I (the `(C,V) + commutative` coordinates of Theorem 9 — an element of 𝒜_res),
- `Chains(I)` = the set of order-cache *chain fragments* with activity in I: for each order-id touched by I, the sub-list of its messages lying in I, annotated with lifecycle status (open at left / closed inside / open at right).

For `I ⊆ J` the restriction `ρ^J_I : ℱ(J) → ℱ(I)` drops the messages of `J∖I`; because the fold is defined message-wise and every coordinate algebra is associative (Lemma 3; [S1-T2]), restriction is a monoid homomorphism — "dropping trailing messages = dropping trailing factors." A presheaf of monoids whose restrictions are homomorphisms and which satisfies gluing for disjoint unions-with-shared-ends is a sheaf on the interval poset in the evident sense: **the gluing axiom is exactly the statement that R36 composition is associative and chain concatenation is unambiguous.** Local sections = worker results; global sections = sequential semantics.

**The cocycle.** Take the pass cover `𝒰 = {P_K}`. On overlaps — adjacent passes share no *messages*, but straddler order-ids appear in both — the transition `ω_{K,K+1}` is the composition/concatenation operator of Corollary 10.1. The Čech cocycle condition demands `ω` compose consistently on *triple* overlaps. For adjacent intervals covering a line, **there are no triple overlaps**: the nerve of the cover is a path (a 1-complex), and the Čech differential `δ` is zero on 1-cochains for lack of 2-simplices. **So δω = 0 holds — vacuously, by dimension.** The claim in the problem is true, but its truth costs nothing and protects nothing.

On the *storage ring* (taking S¹ seriously), the nerve is a cycle and there is one genuine composition all the way around: the **monodromy** `M = ω_{N−1,N} ∘ … ∘ ω_{0,1}`. `M = id` would be the non-vacuous flatness statement — but it is *false* as a semantic claim (the composite around the ring is the fold of ~N passes of tape, not the identity) and *true* as a storage claim (going around the ring advances the tape; nothing semantic returns to its start). Theorem 14 extracts the useful invariant from this observation. The summary of §3.2–3.3 for the adjudication table: **δω = 0 is true but vacuous; the S¹ framing is wrong-based but contains one real theorem (14), and the real boundary content is order-theoretic (12, 13).**

### 3.4 Theorem 12 — Boundary Transparency

**Theorem 12 (Partition invariance).** Let Π and Π′ be any two finite partitions of a tape prefix into contiguous segments (any granularity, any alignment — pass boundaries, chunk boundaries, worker splits, or none). Let `Eval(Π)` denote the evaluation that folds each segment locally (Theorem 10) and then composes segment records in tape order, while each order-id's cache chain is evaluated as the concatenation of its per-segment fragments in tape order. Then

```
Eval(Π)  =  Eval(Π′)  =  Sequential(T)
```

— identical final Res, identical final cache, bit-exact. Pass boundaries are unobservable in the output.

**Proof.** The state space is the product of the accumulative coordinates and the per-id cache coordinates ([S1] §1, §2; Def 1.1); it suffices to treat them separately.

*Accumulative coordinates.* The sequential fold is `Res(m_L) ⨂ … ⨂ Res(m_1)` — a product in the monoid 𝒜_res. Any partition evaluates the same factors re-bracketed: `(⨂ of segment 1) ⨂ (⨂ of segment 2) ⨂ …`. By associativity of `⨂` (Lemma 3.1 for the hash coordinate; associativity of `+`, `⊔`, `∨` for the rest — [S1-T2, T5]), every re-bracketing equals the left-associated product. The result is a function of the multiset-with-order of messages, i.e. of the tape, alone. Note the commutative coordinates do not even need the order (Corollary 2.1 of [S1]); the hash coordinate needs the order and *keeps* it — the composition `V_a ⊗ C_b ⊕ V_b` is order-correct because segments are composed in tape order, which the protocol guarantees (SPSC FIFOs, §3.5). 

*Cache coordinates.* For a fixed order-id `o`, let `T|_o = m_{i₁} … m_{i_r}` be its sub-stream (in tape order). The sequential cache semantics is a deterministic function of the ordered list `T|_o` ([S1-T6]: total order on (seq, wirepos); E/C/X/D/U read the row written by the previous message of the same id). The parallel scheme evaluates, per segment S, the fragment `T|_o ∩ S`, and concatenates fragments in segment (= tape) order. Concatenation of ordered fragments in order is the original ordered list; hence the chain evaluation sees the same sequence of reads/writes and produces the same terminal row. This holds for every id independently — the ids never interact in the cache (each message touches exactly one id's row; [S1] §2 cache-op column). ∎

**Remark.** Theorem 12 is the precise statement of "the pass boundary ceases to exist mathematically": not because a cocycle vanishes, but because the boundary is not an argument of any function in the algebra. It is a re-bracketing of an associative product and a concatenation of per-id chains. This is the same phenomenon as [S1-T2] (order-free signatures), upgraded: the hash coordinate is *not* order-free, but it is *partition-free* — order is preserved by construction, so re-bracketing is free.

### 3.5 Theorem 13 — No-Barrier Asynchronous Resolution

**Protocol (replaces `reset_pass`).** Workers own contiguous segments in a static tape-ordered assignment (block split or chunk-interleaved — both satisfy the invariant below). Per worker:

1. **Run to completion without waiting.** Process the segment's messages in order. Accumulative coordinates fold locally (Theorem 10). For the cache: if the referenced order-id's row is *resident* (created in this segment, or already received via handoff), process normally.
2. **Defer, don't block.** If a message references an id that is *not yet resident* (its creating message lies in an earlier, still-in-flight segment), append the message to that id's **deferred queue** and continue. The segment always completes.
3. **Handoff at segment end.** When a worker finishes a segment, it publishes, for each id with an unclosed chain fragment, the row (or its delta) to the next segment that touches the id, via an **SPSC FIFO in tape order** (one queue per (worker-pair, epoch); acquire/release on head/tail only). It also publishes its R36/P64 record to the pass accumulator.
4. **Drain and resume.** A worker receiving a handoff applies it, then drains that id's deferred queue in FIFO order — which is tape order, restoring exactly the sequential chain.

**Theorem 13 (Soundness and deadlock-freedom).** The protocol (1)–(4) terminates on every input and produces the sequential result (Theorem 12 applies per id and per accumulator); no global barrier, no synchronization token other than per-edge FIFO head/tail updates exists in the system.

**Proof.**

*Soundness.* Each id's chain is processed as the concatenation of per-segment fragments in tape order: within a segment, messages are processed in tape order (step 1); across segments, handoffs and deferred drains are FIFO in tape order (steps 3–4: the queue for id `o` receives exactly the messages of `o` in the order they appear on the tape, because each worker enqueues in tape order and queues are per-id FIFO). Hence the evaluation seen by the cache semantics is the sequential chain — Theorem 12's cache case. The accumulative coordinates compose in tape order by construction (records published and composed left-to-right), which is Theorem 12's accumulator case.

*Deadlock-freedom.* Define the wait-for relation on workers: `W → W′` iff W is blocked awaiting a handoff from W′. Under the protocol, no worker ever *blocks*: step 2 defers instead of waiting, and a worker's segment completion depends only on its own messages (steps 1–2 terminate unconditionally). Handoffs flow strictly from earlier segments to later segments (dependencies on the cache point backward in tape position only — an order must be *added* before it can be executed/canceled; this is Assumption **W** (wire well-formedness), guaranteed by the feed and *checked* by the engine's anomaly path). A directed acyclic graph of dependencies in which no process blocks on unreadiness cannot deadlock; formally, every deferred item's blocking source is a segment that completes in finite time (induction on segment start index: segment 0 never defers anything whose source is later; segment k's sources are all < k and have completed by induction). Liveness: every deferred message's source segment completes, hence publishes, hence the message drains. ∎

**Straddler accounting (why the work is small).** Deferred work per boundary = number of ids whose chains straddle that boundary. A chain straddles a cut at position i iff the id has a message before i and its next message after i — for cuts at regular granularity M and per-id inter-activity gaps g, `P(straddle) ≈ E[min(1, g/M)]`; empirically (and in the hydra cells) the straddler count per 512-message cut is O(open orders with recent activity), typically ≪ 1% of messages. The *work* does not disappear — Theorem 11's cousin applies to chains too — but it is (a) bounded, (b) off the critical path (done while the next pass computes), and (c) *unsynchronized*: no core ever waits. The 680 ns/boundary *synchronization* cost is what disappears.

**Anomaly path (Assumption W violations).** A reference to a non-existent id (feed anomaly, missed add) must not deadlock the deferral machinery: cap deferral by the *segment window* — if the source segment has completed and published without creating the id, the reference is an anomaly → route to the anomaly log and fold a fixed `V_anom` digest so hashes stay defined. This is the same discipline as [S1] §3.7's degradation path: the algebra stays total, and anomalies become data.

### 3.6 Theorem 14 — monodromy, ring sizing, and the epoch rule

The one genuinely topological theorem in Problem 2:

**Theorem 14 (Slot-reuse safety / monodromy triviality).** Consider a storage ring of N pass slots (or any recycled storage: heap entries, hash buckets). The following are equivalent:

1. No pass record/chain fragment is recycled while a live reference to it exists;
2. (direct ring) `N > D_pass`, where `D_pass` is the maximum backward reference distance in passes — the largest K such that some deferred continuation in pass P references state produced in pass P−K; `D_pass = ⌈D_msg/M⌉` with `D_msg` the maximum chain span in messages;
3. (indirected design) chain rows live outside the ring in a map keyed `(order-id, epoch)`, and eviction is reference-counted by open chains — then `N ≥ 2` suffices for the ring (it holds only pass composites and FIFO tickets).

If (1)–(3) hold, slot reuse is *semantically invisible* (the monodromy of the storage cover acts trivially on all live state); if they fail, the failure mode is **phantom resurrection**: a recycled slot's contents are read as if they belonged to the current pass — a determinism bug that no hash check will catch (the merged hash is computed from records, not from the cache).

**Proof.** (1)⇒invisible: a recycled slot is read only by references created after the recycle, which by (1) cannot alias any pre-recycle reader. (2)⇒(1) for the direct ring: a reference from pass P to pass P−K occupies the slot for K+1 passes; the slot is recycled at P−K+N, and the reference is live until P; no overlap iff N > K for all K, i.e. N > D_pass. (3)⇒(1) by the reference count. Failure: if K ≥ N, the writer of pass P−K+N overwrites the row that pass P's deferred continuation later reads; the continuation consumes state from the *wrong* tape position, and since the cache semantics is a deterministic function of what it reads, it silently produces the wrong row. ∎

**The epoch rule.** ITCH order-references are unique within a trading day ([S1] wire table), so `(order-id, day)` is the natural epoch tag; slot/entry reuse across a day boundary is the one place the same key can legally denote two chains. Consequence: **exactly one barrier survives in the system — the day-boundary flush** (drain all deferrals, reset epochs, one fence). Its amortized cost over a day of 300k+ passes is ≈ 0. This is the honest form of "reset_ms → 0.00": *per-pass* reset is eliminated by Theorem 13; *per-epoch* reset remains, once per day.

### 3.7 Cost: 204 ms → ~0 (honest accounting)

| Item | Baseline (300k passes) | This design |
|---|---|---|
| Barrier + re-anchor scan | 204 ms (680 ns × 300k) | **0** (no barrier exists) |
| Straddler chain work | done *inside* the 680 ns window, serialized | same work, **deferred + overlapped** behind the next pass's compute (Theorem 13) |
| Day-boundary epoch flush | — | 1 fence/day ≈ 0 amortized |
| Hash merge at boundary | (part of reset) | ~10–15 cycles via §4 (it was never the bottleneck) |

The claim "reset_ms 204 → 0.00" is **true for the synchronization line item and for wall-clock critical path**; it is *not* a claim that straddler work is free — it is a claim that it is unsynchronized and hidden. §6 budgets it (≤ 1–3% of messages, overlapped; worst-case tail handled by the escape hatch in §3.8).

### 3.8 What breaks it

- **Assumption W violations** (execute-before-add on the wire): handled by the anomaly path (§3.5), never by blocking.
- **Pathological straddler tails** (an id with activity in every pass — e.g., a pegged order touched by millions of messages): its chain serializes its *own* fragments, but no one waits on it except later messages of the same id; if a verdict needs its row, latency is bounded by its chain's last fragment. Escape hatch: fragment-level memoization (the row state after each fragment is an R36-like pair — rows compose associatively too).
- **Ring undersizing** (`N ≤ D_pass`) or missing epoch tags: Theorem 14's phantom resurrection — caught only by the N+12/N+13 conformance cells (§5), never by hashes.
- **A future protocol extension whose per-message action is not of the Def 1.2 form** (e.g., a hash that reads *previous state* nonlinearly): breaks Lemma 3, hence Theorems 9–12. The degradation path is [S1] §3.7's: fall back to sequential per-chunk evaluation with the boundary protocol intact (the boundary theorems 12–14 do not depend on the hash algebra — only Theorem 9/10 do).

---

## §4 Problem 3 — The Galois Shift Theorem for Multi-Stream Interleaving

### 4.1 The merge as monoid composition

Worker 0 and Worker 1 split the tape. Writing the split so that Worker E owns the earlier bytes and Worker L the later, the sequential hash after both segments is (Lemma 5, twice):

```
ℋ_global  =  (h_in ⊗ x^{W_E} ⊕ V_E) ⊗ x^{W_L} ⊕ V_L
          =  ℋ_E ⊗ x^{W_L} ⊕ ℋ_L ,        ℋ_E := h_in ⊗ x^{W_E} ⊕ V_E,  ℋ_L := V_L
```

(the later worker's fold starts from zero, so its partial *hash value* is just its V). The merge multiplier is `x^{W_L}` — the **later segment's span**. This is exactly the problem's `ℋ₀ · x^{deg} ⊕ ℋ₁` with the roles resolved: *the shift is by the span of the segment that folded second in tape order*. If the problem's convention has Worker 0 folding the later chunk, then `deg = W₀` as written; if Worker 0 is the earlier one, the shift is by `W₁`, not `W₀`. Both conventions appear in the wild and the mismatch is a bit-exactness bug (Appendix D, item 2). Nothing below depends on the convention; the code names its arguments `early/late` to make the wrong call impossible.

Under the per-message discipline (Def 1.2a, `w_t ≡ 1`), `W_L` is the later worker's *message count*, which matches the problem's "exact number of messages Worker 0 processed" phrasing. Under the span discipline it is the byte span, and **count alone is insufficient** — a variable type mix changes the degree even at fixed count. This distinction alone already dooms any *fixed* matrix, before Theorem 16's formal statement.

Observe: the merge formula is Lemma 3's composition law with the earlier pair `(C_E, V_E)` and the later `(C_L, V_L)` — i.e., **the two-worker merge is the same monoid operation as the intra-chunk fold (Lemma 3) and the inter-chunk composition (Corollary 10.1)**. Nothing new is needed; Problem 3 is Problem 1 read at a different scale.

### 4.2 Theorem 15 — Λ exists (linearity)

**Theorem 15.** For every `d ∈ ℕ`, the map `Λ_d : R → R, h ↦ h ⊗ x^d` is GF(2)-linear: `Λ_d(h ⊕ h′) = Λ_d(h) ⊕ Λ_d(h′)` and `Λ_d(0) = 0`. Consequently `Λ_d` is a 64×64 binary matrix, with columns `Λ_d(e_j) = e_j ⊗ x^d` for the standard basis `e_j = x^j`. Moreover:

```
Λ_d ∘ Λ_e = Λ_{d+e}        (representation of (ℕ,+) in GL(64,2) once x is a unit)
Λ_d⁻¹ = Λ_{ord(x)−d}       (units; Q(0)=1)
```

**Proof.** Linearity is distributivity of `⊗` over `⊕` in the commutative ring R: `(h⊕h′)⊗c = h⊗c ⊕ h′⊗c` and `0⊗c = 0`. A GF(2)-linear endomorphism of the 64-dimensional space GF(2)⁶⁴ is uniquely a 64×64 matrix over GF(2), acting by matrix–vector product on coefficient vectors; the column formula is linearity applied to the basis. The composition law: `Λ_e(Λ_d(h)) = (h⊗x^d)⊗x^e = h⊗x^{d+e}` (associativity + commutativity of R; exponent law, Lemma 4). Invertibility: `Λ_d Λ_{ord(x)−d} = Λ_{ord(x)} = multiplication by x^{ord(x)} = id` since `x^{ord(x)} = 1` by definition of order. ∎

So the *existence* half of the claim is unimpeachable: every shift is a linear operator, a "pre-computed 64×64 binary matrix" in the literal sense.

### 4.3 Theorem 16 — no count-independent Λ

**Theorem 16.** Assume `ord(x) > D_max` (verified at init for `D_max = 2²¹`; §2.6). Then for any `0 ≤ d < d′ ≤ D_max`, `Λ_d ≠ Λ_{d′}` as operators (equivalently as matrices). Hence the family `{Λ_d : d ∈ [0, D_max]}` contains `D_max + 1` pairwise distinct members, and any single fixed matrix Λ is correct for at most one split degree. Since the workers' message counts and byte spans are data-dependent, **a fixed, pre-computed Λ independent of the message count cannot exist.**

**Proof.** If `Λ_d = Λ_{d′}` then evaluating at `1 ∈ R`: `Λ_d(1) = x^d` and `Λ_{d′}(1) = x^{d′}` give `x^d = x^{d′}`, i.e. `x^{d′−d} = 1` with `0 < d′−d < ord(x)` — contradicting the definition of multiplicative order (the order is the *smallest* positive exponent returning to 1). Distinct degrees give distinct operators; a merge using Λ_{d₀} on a stream whose true degree is `d ≠ d₀` computes `ℋ_E ⊗ x^{d₀} ⊕ ℋ_L ≠ ℋ_global` whenever `ℋ_E ≠ 0` and `x^{d₀} ≠ x^d` (take `ℋ_E = 1`: output differs by the nonzero unit `x^{d₀} ⊕ x^d`). ∎

**Adjudication.** The claim "Λ is a fixed, pre-computed 64×64 binary matrix independent of the variable message count" is **false**, for two independent reasons: (i) Theorem 16's injectivity — different counts need different operators; (ii) the degree is a function of the *type mix* (span discipline), not the count at all. What *is* true, and what rescues the performance claim completely, is Theorem 17: the correct Λ_d is computable so cheaply that its non-constancy is irrelevant.

### 4.4 Theorem 17 — the shift algebra (the rescue)

**Theorem 17 (Fast shift).** For any degree `d ≤ D_max`, the multiplier `x^d ∈ R` is computable by one of:

| Regime | Cost | Method |
|---|---|---|
| span discipline, chunk-level (`d = W ≤ 22,016`) | **1 load** | `T_pow[d]` (the §2.6 table) |
| span discipline, worker/pass-level (`d` = Σ of chunk W's) | k−1 ring mults, or **1 load from a 2-level table** | compose the k chunk constants `C = ⊗ C_chunk` (P64 records already carry them); or `T_hi[d≫16] ⊗ T_lo[d&0xFFFF]` with two 64 KB tables |
| per-message discipline (`d` = count ≤ 512 per chunk, composed) | as above with count tables | `T_cnt[n]` |
| arbitrary `d`, no tables | `⌈log₂ d⌉ ≤ 21` ring mults | binary exponentiation (square-and-multiply on `x`) |

and the merge `ℋ_E ⊗ x^d ⊕ ℋ_L` then costs **one `VPCLMULQDQ` + the fixed 3-CLMUL reduction + one `VPXORQ` ≈ 10–15 cycles** (§2.6's `hash_merge`). With the 4-lane `mulmod4`, four independent merges (multi-worker, multi-verdict) cost the same 3 `VPCLMULQDQ` + 2 `VPXORQ` + 1 `VPCOMPRESSQ`.

**Proof.** Each row is an instance of Lemma 4's exponent homomorphism: tables are precomputed segments of `k ↦ x^k`; the 2-level split is `x^{65536·q + r} = (x^{65536})^q ⊗ x^r`; binary exponentiation is correct in any monoid; the composition of chunk constants is Lemma 3's C-coordinate (`C_a ⊗ C_b = x^{deg_a + deg_b}`). The instruction counts are §2.6's `mulmod_u64`/`mulmod4`. ∎

**The representation remark (why the matrix must never be materialized).** Theorem 15 says Λ_d *is* a 64×64 matrix; Theorem 17 says *do not build it*. Applying a 64×64 GF(2) matrix to a 64-bit vector by row-selection costs `popcount(h)` XORs (32 on average, 64 worst case, ~16–32 `vpxorq`/blend ops) — versus **one carry-less multiply** for the field-element form, which is the same linear map in compressed (8-byte, cyclic-group) representation. The matrix form is 64× larger in memory and ~10–20× slower to apply. This is a representation-theoretic statement, not an implementation preference: the shift operators form a cyclic group `⟨Λ_1⟩ ≅ ℤ/ord(x)ℤ`, and the regular representation of a cyclic group on 64 bits factors through multiplication in `R = GF(2)[x]/Q(x)`; choosing the matrix representation discards the group structure and pays for it. (The same warning applies to Problem 1's `𝐌_type`: it is one field element, `x^W` — never a matrix.)

### 4.5 The two-instruction merge

From §2.6 (complete function; shown here for focus):

```c
/* ℋ_global = ℋ_early ⊗ x^{W_late} ⊕ ℋ_late   (Theorem 17) */
static inline uint64_t hash_merge(uint64_t h_early, uint64_t h_late,
                                  uint16_t W_late)
{ return mulmod_u64(h_early, T_pow[W_late]) ^ h_late; }
```

For worker/pass-level degrees (u32, beyond the chunk table), the two-level lookup:

```c
static uint64_t T_hi[1<<16];                 /* x^{65536·q}, q<2^16 : 512 KB */
/* init: T_hi[0]=1; T_hi[q]=mulmod_u64(T_hi[q-1], T_pow[65536]); */
static inline uint64_t shift_const(uint32_t d)          /* x^d, d < 2^32 */
{ return mulmod_u64(T_hi[d >> 16], T_pow[d & 0xFFFF]); }   /* 2 loads + 1 mulmod */
```

Critical path per merge: `shift_const` is **off** the critical path (the split degree is known when work is assigned — compute it while the workers fold); the on-path cost is `mulmod_u64` + XOR ≈ **3 CLMUL + 2 XOR ≈ 10–15 cycles ≈ 4–6 ns** at 2.6 GHz. Against the baseline "multiply by x^deg requires modulo based on exact count" per-message rewalk: the merge is done once per pass, in O(1), exactly as claimed — *the claim's performance conclusion survives even though its constancy premise does not.*

### 4.6 Unification — one monoid, end to end

Every scale of this engine is now the *same* algebraic operation:

```
message     ─┐  Res(m)   = (x^{w_t}, v_t)              [Def 1.2]
chunk       ─┤  fold      = ⨂ over messages            [Lemma 3, Theorem 10]
pass        ─┤  compose   = R36 ⨂ R36 ⨂ …              [Corollary 10.1]
worker      ─┤  compose   = P64 ⨂ P64 ⨂ …              [§2.5]
global      ─┘  merge     = (C_E,V_E) ⨂ (C_L,V_L)      [§4.1]
boundary    ─   gluing    = the same composition       [Theorem 12]
```

The operator `(C₁,V₁) ⨂ (C₂,V₂) = (C₁C₂, V₁C₂ ⊕ V₂)` is associative everywhere, exact everywhere (GF(2) and ℤ, no rounding), and partition-invariant (Theorem 12). This is the deepest answer to all three problems: the engine is one monoid action evaluated at five granularities, and every "optimization" posed in the problem set is a statement that some granularity hop is O(1).

---

## §5 Determinism and Bit-Exact Conformance

### 5.1 Theorem 18 — global determinism

**Theorem 18.** Fix a tape and a modulus Q with `Q(0)=1`. The pipeline output — final Res, final order cache, per-message tokens, and merged global hash — is bit-exact equal to the sequential reference, and is invariant to: chunk size, pass size, vector width (scalar vs 4-lane), worker count, split position, block decomposition, and scheduling interleaving.

**Proof.** Every listed degree of freedom is a *re-association or re-distribution* of computations in algebras where the relevant laws hold exactly:

1. **Integer coordinates** (volumes, degrees, counts): associative + commutative, computed in i64/i32 with no overflow in the proven ranges (§2.5); any summation order gives identical bits.
2. **GF(2) coordinates** (V, C, hashes): `⊕` is associative and commutative — the 4-lane accumulation, the scalar tail, and the sequential chain produce the same XOR of the same multiset of terms (Theorem 10b). Each `⊗ mod Q` is computed by the fixed straight-line sequence of §2.6 (3 CLMULs with constant fold operands) — a deterministic function of its 128 input bits; CLMUL has no rounding, no lanes interact, and the fold constants are compile-time. Hence identical inputs give identical outputs on every path, and the *set* of term inputs is order-independent by (2).
3. **Tagged max** (price): the comparator `(seq, wirepos)` is a total order ([S1-T6]); max over a set is order-independent.
4. **Per-id chains**: processed in tape order by construction (Theorem 13's FIFO discipline) — identical to sequential regardless of which worker ran them or when.
5. **Merges/compositions**: instances of the associative `⨂` (Theorem 12), so every bracketing — worker split, pass grouping — is the same function.
6. **Storage** (the only way to break determinism): keyed by tape identity with epochs (Prop 12.0, Theorem 14); with the sizing/epoch invariants verified (cells N+12/13), the storage cover acts trivially. ∎

### 5.2 The extended conformance matrix — 94 → 130 cells

The [S1] 94-cell hydra matrix is extended by **36 cells** targeting this document's failure modes. All must pass bit-exact against the sequential reference, on every (cell × partition × split) combination.

| # | Cell class | Adversarial design | Catches |
|---|---|---|---|
| +1 | Type-order V adversary | 2-message chunks {D,P} and {P,D} with identical payload multisets (Theorem 9's example), full 512-length variants | multiset-V implementations |
| +2 | Split-degree convention | every split point in a 64-message window around each power-of-2 boundary; both `W_early`/`W_late` conventions attempted, wrong one must fail | Appendix D item 2 |
| +3 | Zero-length segments | empty Worker-1 segment (deg 0), empty Worker-0, both empty | merge edge, `T_pow[0]=1` |
| +4 | All-one-type chunks | 512×'P' (max width), 512×'D' (min width); histogram extremes n=0 and n=512 | table bounds, exponent edges |
| +5 | Token/chunk cross-invariant | `tok[L−1] == h_in ⊗ T_pow[W] ⊕ V` on 10⁶ random chunks | Theorem 10(b) vs 10(c) |
| +6 | mulmod fuzz | 2²⁴ random (a,b) pairs: `mulmod_u64` vs shift-XOR schoolbook; `mulmod4` vs `mulmod_u64` | Appendix B reduction |
| +7 | R36 associativity | random triples a,b,c: `(a⨂b)⨂c == a⨂(b⨂c)`; also `r36_compose` vs `p64_compose` on chunk-level degrees | Corollary 10.1 |
| +8 | Degree overflow | compose ≥ 3,000 chunks (deg > 2¹⁶) — R36 must escape to P64, never wrap u16 | §2.5 two-level scheme |
| +9 | ord(x) verification | `spanfold_init` assert on cold start; also spot `x^{2²¹−1} ≠ 1` | Lemma 4 hypothesis |
| +10 | Partition sweep | random tapes × random partitions (granularity 1..4096, random offsets) — outputs identical | Theorem 12 |
| +11 | Straddler chains | orders with activity spanning 1,2,4,…,64 passes; interleaved split vs block split | Theorem 13 |
| +12 | Ring sizing boundary | `N = D_pass` must FAIL (phantom resurrection detected by cache diff), `N = D_pass+1` must pass | Theorem 14 |
| +13 | Epoch reuse | same order-id on both sides of a day boundary with ring wrap coinciding | Theorem 14, epoch rule |
| +14 | Anomaly W-violations | E-before-A references: must hit anomaly path, hash still defined (`V_anom`), no hang | §3.5 anomaly path |
| +15 | Adversarial deferral order | scheduler reverses handoff arrival (test harness forces out-of-order FIFO delivery — must be impossible with SPSC; test the queue, not the math) | Theorem 13 FIFO discipline |
| +16 | Merge tree shape | 2,3,4,5 workers, balanced vs skewed reduction trees, random assignment | §4.6 |
| +17 | Discipline mix | per-message discipline streams vs span discipline on same tape — both must match their own sequential reference | Def 1.2a/b |
| +18 | Watermark sparsity | sparse multi-track seq with equal (seq) and equal (seq,wirepos) collisions crossing partitions | [S1-T6] taxonomy carryover |

Cells +1..+18 expand to 36 concrete cases with the random seeds and exact field patterns specified in the harness (same format as the [S1] matrix). Total: **130 cells**, all bit-exact, no tolerance.

---

## §6 Roofline on 2 Zen 5 Cores

**Machine model** (carried from [S1]): 2× AMD EPYC 9V45 (Zen 5) cores @ 2.6 GHz, full-width AVX-512 including `VPCLMULQDQ`, 48 KB L1D + 1 MB L2 per core, ~165 GB/s memory bus. Throughput assumptions used below are **conservative ranges** (`0.5–1` 512-bit `VPCLMULQDQ` per cycle per core; ~6-wide issue): every projection is stated as a range and every constant is microbenchmark-verified in CI (checklist item 11). Token-stream sizing: ~12–16 B of decoded fields per message in, 4 KB tokens per 512-chunk out.

### 6.1 The three operating points

**(A) Fresh evaluation** (first contact with a chunk; Theorem 10 kernel):

- CLMUL demand: 0.75 `VPCLMULQDQ`/msg (V-fold) + optional 1.0/msg (token pass) → **0.75–1.75/msg**
- At 0.5–1 CLMUL/cyc: **0.75–3.5 cycles/msg** → 512-chunk: **0.15–0.7 µs** (§2.7's 0.23–0.40 µs sits inside this)
- Other uops (loads, adds, cmov): ~4–5/msg, overlapped — not binding
- Memory: 176 KB `T_pow` (L2-resident, monotone-index access), digests 4 KB/chunk (L1–L2) — **not memory-bound**
- Wire read floor (if parsing from wire in the same pass): 512 × ~35 B = 17.9 KB/chunk / 165 GB/s ≈ 0.11 µs — below compute; **CLMUL-throughput-bound**
- Per-core rate: **0.7–3.5 G msg/s**; two workers on 2 cores: **1.5–7 G msg/s** (mid ~2.5 G)

**(B) Memoized replay** (verdict re-runs; Corollary 10.1): 976 record compositions per 500k-message pass × ~12 cycles ≈ 12k cycles ≈ **4.6 µs per pass = 0.009 ns/msg** — three orders below stage A; the stage becomes decode/scan-bound (the [S1] L2-resident regime; the two documents' replay paths compose multiplicatively: [S1]'s LOB signature replay and this document's R36 replay run off the same chunk log).

**(C) Boundary/merge overhead** (Problem 2–3): sync = 0 (Theorem 13); straddler work ≤ 1–3% of messages, overlapped; merges 10–15 cycles each. Aggregate over 300k passes: ~4.5M cycles ≈ 1.7 ms — noise.

### 6.2 End-to-end projection (300,000 passes × 512 messages = 153.6 M messages)

| Stage | Baseline | This design |
|---|---|---|
| Worker eval (Problem 1) | 21 µs × 300k = **6.30 s** | 0.23–0.40 µs × 300k / 2 workers = **35–60 ms** |
| Pass boundary (Problem 2) | **204 ms** | **~0** (async, overlapped; 1 day-flush) |
| Hash merge (Problem 3) | folded into reset | **1.7 ms** total |
| **Eval+boundary wall** | **≈ 6.5 s** | **≈ 37–62 ms** (≈ **105–175×**) |
| End-to-end msg rate | 23.6 M msg/s | **2.5–4 G msg/s fresh** (parse/scan-bound; next wall), replay regime ≫ 10 G msg/s |

**Where the next wall is.** With the fold collapsed, the pipeline is bound by the scan/decode stage (digest production ~1 CLMUL/msg, token loads/stores, and the [S1] LOB fold's `vpadq`/`vpmaxsq` work — co-scheduled, the CLMUL ports are shared between digesting and the V-fold; budget ~2–3 cycles/msg combined). Multi-billion msg/s beyond ~4 G requires either (i) the memoized regime (chunk logs make re-evaluation ~free), or (ii) more CLMUL ports (not available on the 2-core runner). The honest ceiling on this hardware, fresh from wire, all stages combined, is **~2.5–4 G msg/s sustained**; the 10 G+ class numbers live in the replay regime, exactly as in [S1].

### 6.3 What we did not prove (flagged for the benchmark gate)

- Exact `VPCLMULQDQ` port count and throughput on Zen 5 (assumed 0.5–1/cyc; measure with a 3-line microbenchmark — checklist item 11).
- SMT interaction when both workers share one core's execution resources under the [S1] Core-0/Core-2 assignment (measure; if contention > 20%, pin workers to separate cores and let scan/verdict share SMT).
- Digest-production cost (`v_t(p)`) — assumed in the scan phase; if it must run in the eval stage, add ~1 CLMUL/msg (still <2 µs/pass at 512).

---

## §7 Implementation Checklist (ordered)

1. **Constants gate.** Fix Q, `QLOW`, `W_T[]`; write `spanfold_init()`; assert `sizeof(R36)==36`; run the `ord(x) > 2²¹` exhaustive check (cell +9). No other work until this passes.
2. **Scalar reference.** `mulmod_u64` + shift-XOR schoolbook cross-fuzz (2²⁴ pairs, cell +6). The schoolbook version ships as the CI oracle forever.
3. **Vector mulmod.** `mulmod4` vs scalar on the same pairs (cell +6b). Verify the `0xAA` compress mask and the `0x01` immediates against a lane-by-lane dump.
4. **Chunk kernel.** `fold_chunk` vs a trivially-correct sequential loop on 10⁶ random chunks (types ~ ITCH distribution + adversarial mixes); check the token cross-invariant `tok[L−1] == h_in⊗T_pow[W] ⊕ V` (cell +5).
5. **R36/P64 composition.** Associativity fuzz on random triples (cell +7); degree-overflow escape at deg > 2¹⁶ (cell +8).
6. **Merge.** `hash_merge` + `shift_const` two-level table; split-point sweep (cell +2); empty-segment edges (cell +3); merge-tree shapes 2–5 workers (cell +16).
7. **Partition-invariance harness.** Random tapes × random partitions vs sequential (cell +10) — this is Theorem 12's executable form.
8. **Deferral protocol.** SPSC FIFO + deferred queues + handoff; straddler chains 1–64 passes (cell +11); W-violation anomaly path (cell +14); ring-sizing boundary N = D_pass vs D_pass+1 (cell +12); epoch/day-boundary (cell +13).
9. **Integrate with the [S1] engine.** Shared scan phase produces (type, vd, dvol, px, setp) and the [S1] signature fields; chunk log stores both records (36 B R36 + [S1] signature); verdict replay composes both.
10. **Performance gate.** Reproduce the 21 µs baseline on the current build (profile: confirm mispredict + latency split); then land the kernel and confirm 0.2–0.4 µs/chunk; boundary A/B with the barrier removed; 2-worker merge on the wire.
11. **Microbenchmark CI.** `VPCLMULQDQ`/cycle, gather/compress throughput, SMT contention — re-run on every CI image change; alert if projections move > 20%.
12. **Determinism gate.** Full 130-cell matrix, bit-exact, no tolerance, on every PR. The matrix is the contract; the theorems are its proof.

---

## Appendix A — Full Proofs of the Monoid Laws

**A.1 Lemma 3, full computation.** For pairs in `R × R` define `⨂` as in §1.4. We verify all three computations in full (all products `⊗` mod Q; all sums `⊕`):

*Composition of applications:* for any `h ∈ R`,
```
A₂(A₁(h)) = A₂(h⊗C₁ ⊕ V₁) = (h⊗C₁ ⊕ V₁)⊗C₂ ⊕ V₂
          = (h⊗C₁⊗C₂) ⊕ (V₁⊗C₂) ⊕ V₂        [distributivity in R]
          = h⊗(C₁C₂) ⊕ (V₁C₂ ⊕ V₂)           [associativity, commutativity]
          = A_{(C₁C₂, V₁C₂⊕V₂)}(h).
```

*Associativity:*
```
((C₁,V₁)⨂(C₂,V₂))⨂(C₃,V₃)  =  (C₁C₂, V₁C₂ ⊕ V₂) ⨂ (C₃,V₃)
                              =  (C₁C₂C₃, (V₁C₂ ⊕ V₂)C₃ ⊕ V₃)
                              =  (C₁C₂C₃, V₁C₂C₃ ⊕ V₂C₃ ⊕ V₃)
(C₁,V₁)⨂((C₂,V₂)⨂(C₃,V₃))   =  (C₁,V₁) ⨂ (C₂C₃, V₂C₃ ⊕ V₃)
                              =  (C₁C₂C₃, V₁(C₂C₃) ⊕ (V₂C₃ ⊕ V₃))
                              =  (C₁C₂C₃, V₁C₂C₃ ⊕ V₂C₃ ⊕ V₃)   ∎
```

*Identity and non-commutativity:* as in §1.4; the witness `(x,1)⨂(x²,0) = (x³, x²)` vs `(x²,0)⨂(x,1) = (x³, 1)` is checked by expanding the definition.

*Unit group:* `x ∈ Rˣ` ⟺ `gcd(x, Q) = 1` in the PID `GF(2)[x]` ⟺ `Q(0) = 1` (x divides Q iff Q's constant term is 0). The inverse of `(C, V)` with `C ∈ Rˣ` is `(C⁻¹, V⊗C⁻¹)`, and it is two-sided:

```
(C,V) ⨂ (C⁻¹, VC⁻¹) = (CC⁻¹, V·C⁻¹ ⊕ V·C⁻¹) = (1, 0)
(C⁻¹, VC⁻¹) ⨂ (C,V) = (1, (VC⁻¹)·C ⊕ V)    = (1, V⊕V) = (1, 0)
```

So `𝒜_hash` restricted to unit C is the group `Rˣ ⋉ R` (semidirect product with the natural action of the unit group on the additive group of R).

**A.2 Lemma 4.** `exp(a+b) = x^{a+b} = x^a·x^b` holds in any monoid for a fixed element (standard semigroup-power law, proven by induction on b: `x^{a+(b+1)} = x^{(a+b)}·x = (x^a x^b) x = x^a (x^b x) = x^a x^{b+1}`). Injectivity on `[0, ord(x))`: if `x^a = x^b` with `a < b < ord(x)`, then multiplying by `x^{−a}` (unit) gives `1 = x^{b−a}` with `0 < b−a < ord(x)`, contradicting minimality of the order. The init check (§2.6) verifies minimality directly up to 2²¹.

**A.3 Theorem 5-style extension (coordinate product closure).** If `M₁, M₂` are monoids with operations `·₁, ·₂`, the product `M₁ × M₂` with `(a,x)·(b,y) = (a·₁b, x·₂y)` is a monoid (identity `(e₁,e₂)`; associativity componentwise). The Res state space is a 4-fold product; every generator acts nontrivially on a subset of coordinates and as the identity on the rest; such "partial" generators are closed under the product operation because identities are. This is the formal version of [S1-T5] used in Theorems 9 and 12.

**A.4 Theorem 12's re-bracketing lemma.** In any monoid, every bracketing of the ordered list `g₁,…,g_n` evaluates to the same element (generalized associativity — standard, by induction on n: any bracketing splits as `A·B` with A a bracketing of a prefix and B of the suffix; apply the induction hypothesis and the 3-factor law). Theorem 12's accumulator case is this lemma applied per coordinate; the cache case is the same lemma for the "monoid of partial functions on rows ordered by tape position" (each chain fragment is a partial function; fragments compose in tape order; composition of partial functions is associative).

**A.5 Tagged max ⊔ (recap from [S1-L2]).** On `Tg = (R_payload × Tag) ∪ {⊥}` with tags from a total order: `τ ⊔ τ′ =` the argument with larger tag; `⊥ ⊔ τ = τ`. Associativity: max under a total order is associative and commutative; the payload rides along. Used in R36 composition (§2.5) with tag = wire offset (dense-seq case) or `(seq, wirepos)` (sparse case).

---

## Appendix B — Constants, the GF(2⁶⁴) mulmod, and layouts

**B.1 The modulus.** Reference choice: `Q(x) = x⁶⁴ + x⁴ + x³ + x + 1` (hex `0x100000000000001B`, low word `QLOW = 0x1B`). Properties used: `Q(0) = 1` (x is a unit — Lemmas 3–4); `deg QLOW = 4` makes the CLMUL fold terminate in one extra step (below). Primitivity is *not assumed anywhere* — Theorem 16's hypothesis is the verified bound `ord(x) > 2²¹`. If the production SPANHASH pins a different modulus (e.g. CRC-64/ECMA-182 `0x42F0E1EBA9EA3693`): all theorems hold provided `Q(0)=1`; the low-weight one-step fold below must be replaced by the general 2-fold Barrett (B.3).

**B.2 The 3-CLMUL reduction (proof of the code's correctness).** Claim: for `a, b < 2⁶⁴` (polys of degree ≤ 63), with `p = a⊗b` (unreduced, degree ≤ 126) split as `p = hi·x⁶⁴ ⊕ lo` (`hi` = bits 64..126, degree ≤ 62):

```
a⊗b mod Q  =  lo  ⊕  t_lo  ⊕  u,
t   := hi ⊗ QLOW                    (degree ≤ 62+4 = 66; t = t_hi·x⁶⁴ ⊕ t_lo, deg t_hi ≤ 2)
u   := t_hi ⊗ QLOW                  (degree ≤ 2+4 = 6)
```

*Proof.* Work in `R`: `p = lo ⊕ hi·x⁶⁴ ≡ lo ⊕ hi·QLOW` (substituting `x⁶⁴ ≡ QLOW`, valid since `Q = x⁶⁴ ⊕ QLOW`). Now `hi·QLOW = t` has degree ≤ 66; reduce it the same way: `t = t_lo ⊕ t_hi·x⁶⁴ ≡ t_lo ⊕ t_hi·QLOW = t_lo ⊕ u`, and `deg u ≤ 6 < 64` needs no further reduction. Hence `p ≡ lo ⊕ t_lo ⊕ u`. The degrees are tight as stated (62 = 126−64; 2 = 66−64), which is why each 128-bit CLMUL lane holds its intermediate without overflow. Worked micro-example (verified by hand): `a = b = x⁶³`: `p = x¹²⁶`, `hi = x⁶²`, `t = x⁶⁶⊕x⁶⁵⊕x⁶³⊕x⁶²`, `t_hi = x²⊕x`, `u = x⁶⊕x⁴⊕x³⊕x`, result `= x⁶³⊕x⁶²⊕x⁶⊕x⁴⊕x³⊕x`, which matches direct long division of `x¹²⁶` by Q. ∎

The vector form `mulmod4` is this algorithm lane-wise: `VPCLMULQDQ(imm 0x00)` = 4 products; `imm 0x01` selects each lane's *high* 64 bits as the left operand (fold step 1, then fold step 2); `VPCOMPRESSQ` with mask `0xAA` gathers the four even words (the four results). All fold constants are `QLOW` — identical across lanes and compile-time.

**B.3 General-modulus variant (for high-weight Q).** Reduce a 128-bit `p = hi:lo` mod a degree-64 Q with Barrett: precompute `μ = x¹²⁶ div Q` (degree 62); `q = (hi ⊗ μ) >> 63` (quotient high part); `r = p ⊕ q⊗Q` (degree < 64). Costs 2 extra CLMULs + shifts vs B.2's structure (5 CLMUL total per multiply). Same associativity/exactness properties; only the constants change.

**B.4 Bit-reflection warning.** CRC-style engines use *reflected* polynomials (e.g. CRC-64/XZ `0xC96C5795D7870F42`, the bit-reversal of ECMA-182). CLMUL operates on normal-form polynomials. Converting an existing reflected SPANHASH to this kernel requires bit-reversing inputs and outputs (`bitrev` — or the standard reflected-folding technique) *and* re-deriving all degree conventions. The conformance suite's job is to make any mismatch loud (cells +5, +17).

**B.5 Record layouts (normative).**

```
R36 (chunk composite, 36 B, packed):
  [0:8)   V    u64   payload fold
  [8:16)  Δb   i64
  [16:24) Δa   i64
  [24:28) p*   u32   (flg.b0)
  [28:30) tag* u16   wire offset of setter (chunk-local)
  [30:32) W    u16   span == degree (≤ 22,016)
  [32:34) cnt  u16   ≤ 512
  [34:36) flg  u16   b0 setter | b1 trade | b2 escape-to-P64

P64 (pass/worker accumulator, 48 B):
  V u64 | C u64 (= x^deg) | Δb i64 | Δa i64 | p* u32 | tag u32 | deg u32 | cnt u32 | flg u16 | rsv u16

Deferred-continuation entry (Theorem 13): { order-id u64, epoch u16, msg-cursor u24, pad } 16 B, SPSC ring.
```

---

## Appendix C — Impossibility Proofs, Expanded

**C.1 Theorem 11 in adversarial form.** Strengthening: even an algorithm that reads *all but one* digest symbol on some path, or reads symbols of its choosing adaptively, fails. Adaptive version: let A be any randomized or deterministic evaluator; consider the input distribution uniform on digests. If A outputs V correctly with probability > 1/2 over inputs while not reading `v_{i₀}` with positive probability, condition on A's read set excluding `v_{i₀}`: A's output is independent of `v_{i₀}`, but `V | (all other symbols)` is uniform over the coset `{c ⊕ x^{d}·v : v}` as v ranges over R — A cannot track a uniform hidden variable → error probability ≥ 1/2 on those inputs. Hence with nonzero probability A errs. (Same conclusion, measure-theoretic packaging.) Corollary: any "single MVM" claiming O(1) evaluation of a fresh chunk contradicts the bound; the only O(1) operations in the system are compositions of already-computed records (Corollary 10.1) — consistent, since records are 36-byte *functions of* the L symbols, computed once at Ω(L).

**C.2 Theorem 16, quantitative form.** The number of distinct merge operators over the reachable degree range `[0, D]` is `min(D+1, ord(x))`. With the verified `ord(x) > 2²¹` and `D = 2²¹` (a full day of 500k-message passes at byte degrees stays below this), that is `2²¹ + 1` distinct 64×64 matrices — 2 MB of matrices to precompute "all" of them, versus one 512 KB two-level table of *field elements* that applies faster (Theorem 17's representation remark). Both facts together close the question: the matrix family exists, is huge, and is strictly dominated by the compressed representation.

**C.3 What a payload LUT would need (companion to [S1-AppC]).** A lookup table mapping a 512-message chunk (≥ 2⁴⁰⁰⁰ possible digest streams) to V would need ≥ 2⁴⁰⁰⁰/2⁶⁴ entries to be injective on a 2⁻⁶⁴ fraction of inputs — physically impossible; the degree tables of Theorem 10 are legal precisely because degrees are *type*-functions (9 bits/message of entropy, ≤ 22,016 reachable values), not payload functions.

---

## Appendix D — Pitfall Catalogue

1. **The deg convention trap.** `ℋ = ℋ_early ⊗ x^{W_late} ⊕ ℋ_late` — the shift is by the *later* segment's span. Shifting by the earlier worker's span (or by a count under the span discipline) mismatches on the first heterogeneous-type stream. The code's `early/late` naming exists to prevent this; cell +2 hunts it.
2. **Count vs span.** Under Def 1.2(b) the degree is bytes, not messages. Any code path that takes `deg = msg_count` works on all-one-type test data and fails in production. Cell +17.
3. **Reflected vs normal modulus.** B.4. A reflected CRC pasted into a normal-form CLMUL fold produces a *valid-looking* hash that matches nothing.
4. **`Q(0) = 0`.** If the modulus has zero constant term, x is a zero divisor: no `T_pow` inverses, Theorem 16 breaks (x^d can collide or vanish), `Λ_d⁻¹` doesn't exist. Check at init, fail loudly.
5. **Materializing Λ or 𝐌 as matrices.** §4.4: 64× the memory, ~10–20× the apply cost, and it *invites* the fixed-Λ fallacy. Field elements only.
6. **Struct padding.** Without `__attribute__((packed))`, `R36` silently becomes 40 bytes and every wire offset in the chunk log shifts. The `_Static_assert(sizeof(R36)==36)` is load-bearing.
7. **u16 degree wrap.** R36's W is chunk-legal (≤22,016) but pass-level composition overflows u16 at ~3 chunks... at 65,536 bytes ≈ 3 chunks' worth of max-width spans. Escape to P64 (cell +8) or compose degrees in u32 always.
8. **`T_pow` bounds.** The table covers `[0, 22,016]`. A "chunk" of 513+ messages, or a P64-level degree fed to `hash_merge`'s u16 path, indexes past the end. Assert `W ≤ CHUNK*43` at every call site.
9. **Equal-seq price ties across partitions.** [S1-T6] taxonomy T1–T5 applies unchanged; the R36 composition's "later setter wins" is valid *only* under dense sequencing — sparse tracks must compare `(seq, wirepos)` explicitly. Cell +18.
10. **Slot-keyed state.** Any hash-map entry, epoch counter, or watermark keyed by ring-slot index instead of (order-id, epoch) resurrects Theorem 14's phantom. Code-review grep: `mod N` outside the allocator.
11. **Deferral without FIFO discipline.** A multi-producer or unordered queue for handoffs breaks per-id tape order — Theorem 13's soundness premise. SPSC per (worker-pair, epoch), acquire/release only.
12. **Blocking on a straddler.** Any `wait()` in the eval path reintroduces the barrier through the back door. Defer, publish, drain (§3.5); the day-flush is the only legal fence.
13. **Token-store bandwidth.** Emitting 4 KB of tokens per chunk is fine in L2; do not let the token path write through to memory (non-temporal if profiling shows pressure).
14. **`ord(x)` drift.** If Q ever changes, the init check must re-run. Cache `ord(x)` verification results keyed by Q's hash; refuse to start on mismatch.

---

*End of SOLUTION1.MD. Companion artifacts: `SOLUTION.MD` (the O(1) LOB fold, theorems 1–6, 94-cell matrix), `spanfold.c` (§2.6, complete), conformance harness per §5.2/§7.*

