# 45 — R34 (Run 700): the O(1) Affine Ingestion Fold — the 𝒜₅.₀ collapse ported

**The program:** land docs/solution.md's closed-form batch algebra as a
CI-verified kernel — the math team's Theorems 1–8 turned into
`nf-testkit::lobstate` code, differential cells, and a roofline
instrument. The R33 fleet left the engine worker-arithmetic-bound:
`eval_ms ≈ 3.7 s` of the 5 s window (~74% duty) on the 17.79B draw. The
blueprint removes the fold's dependency chain entirely, not its
constants: 500,000 sequential affine updates become **977 chunk folds +
976 O(1) combines**, order-free and bit-exact.

## 1. The collapse (what was proven, what was ported)

The blueprint's three structural facts (F1: every `Aᵢ ∈ {I₃, D₀}`;
F2: unit volume diagonals; F3: assignment-only price writes) collapse
the chain `S_k = A_k S_{k-1} + B_k` to the **commutative monoid**

```
𝒜₅.₀ = (ℤ², +) × (Tg, ⊔_seq)      ⊕ = 2 wrapping u64 adds + 1 tagged max
Ψ(Δb, Δa, (π,s)) : S ↦ (b+Δb, a+Δa, π)   (D₀ iff s > 0 — the T3 invariant)
```

The K-matrix product of 500,000 generators is **one OR bit** (Theorem 1
— `D₀` is idempotent); the weighted-sum `B_net` is **two integer sums
and the last setter's payload** (Corollary 1.1); and because the
signature algebra is commutative, *any* partition / lane count / merge
order yields the identical map (Theorem 4) — the exact shape SIMD and
multi-core consume with zero correction or synchronization. The port:

| Blueprint artifact | `crates/nf-testkit/src/lobstate.rs` |
|---|---|
| `sig_t` (§6.6) | `Sig { db, da, px, seq }` — 32 B |
| `sig_combine` (⊕) | `sig_combine` — seq-first UNSIGNED tagged max, payload-atomic |
| `sig_apply` (Ψ) | `Lob3::apply` — wrapping adds + the `seq > 0` assignment |
| `sig_tokens_t` (§6.2) | `SigTokens` — SoA `[u64; 512] × 4` (16 KB, L1D-resident) |
| `sig_fold_avx512` | `sig_fold_avx512` — VPADDQ lane-sums + `vpcmpuq`/`_mm512_max_epu64`/`_mm512_mask_mov_epi64` pairwise tagged max |
| `sig_fold_scalar` | `sig_fold_scalar` — the portable twin (the fold-factor oracle) |
| chunk engine (§3.5/§6.3) | `sig_chunk_signatures` / `sig_pass_states` — the 512-boundary Ψ-commit and the memo store shape |
| self-test (§6.6) | the 9 `t_r34_*` cells (below) |

## 2. The port contract (every clause is a cell)

* **K1** volumes: u64 **wrapping** sums — `(ℤ/2⁶⁴, +)` is an abelian
  group, so scalar loop / tree / VPADDQ are bit-identical, and the
  sequential oracle accumulates under the identical convention (H4).
* **K2** price: **last-writer-wins**. The key is `seq` — compared FIRST
  and UNSIGNED (`vpcmpuq`, T4); the price is payload, never key (§4.1:
  a price-first compare silently turns recency into price-priority).
  Payload transfers atomically from the single ⊔-winner (Thm 6′(iii)).
* **Sentinel in seq, never in px** (T3): `s* == 0 ⇔ no setter`; `px` is
  dead payload at `seq == 0`. Positions are 1-based (0 collides with ⊥).
* **Injective keys** (T2/T5): the vector path carries a
  `popcnt == 1`-class debug guard; lane-count-varied repartitionings
  fold identically (the partition-invariance cell).

**The reference-kernel fix:** §6.6's C kernel double-counts the sub-8
tail's `db/da` (`vsum_tree` over all `n`, then full-token tail
combines). The port folds the tail exactly once — the K1 tree covers
`[0, nv)`, the tail folds as full tokens — and the 2000-trial
dual-path fuzz pins it (the blueprint's own self-test protocol would
catch the same bug; ours runs it on every commit).

## 3. The cells (9 new, release + debug, both paths)

| cell | pins | blueprint row |
|---|---|---|
| `t_r34_recency_last_write_wins` | up-print then down-print → 99.50 (NOT max-price) | §4.1 |
| `t_r34_equal_price_distinct_seq` | equal prices: payload from the seq-winner | T1 |
| `t_r34_unsigned_key_domain` | keys ≥ 2⁶³ dominate (forced vec path too) | T4 |
| `t_r34_sentinel_all_bottom` | all-⊥ 512-chunk: `I₃`, price untouched | T3 |
| `t_r34_zero_price_print` | `P(0)` assigns `p = 0` — a real D₀ write | §7.5 row 5 |
| `t_r34_combine_laws` | ⊕ identity / commutativity / associativity / ⊔-idempotence | Thm 2(iii) |
| `t_r34_partition_invariance` | chunk {1,16,64,256,512}: identical Σ, states, forward/reverse/tree merges | Thm 4 / row 7 |
| `t_r34_oracle_bit_exact_fuzz` | 2000 trials: oracle == fold+Ψ; scalar == dispatched | Thm 8 / Cor 2.1 |
| `t_r34_pass_boundaries_and_replay` | 500k pass: 977 chunks, every 512-boundary == sequential; the 976-⊕ replay == whole-stream fold == oracle | Thm 8 at commit points + §3.5 |

## 4. The roofline instrument (`src/bin/sigbench.rs`)

Prices the fold factor per the §8.5 protocol on a deterministic
500k-token corpus (¼ setters, injective keys, full-range wrapping
deltas): the cold chunk-fold stage (best-of-5, TSC-calibrated), the
976-⊕ replay (best-of-1000, checksum-sunk so LLVM cannot collapse the
timed region — the same law for the ⊕ micro-rate, which reads the REAL
signature table round-robin to stay scalar-evolution-proof), and the
sequential oracle + dual-path equality asserted on the runner.
Verdict line (CI arm `11r34` greps it):

```
PR1_R34_AFFINE_FOLD_VERDICT fold_cyc_per_msg=<f> cold_fold_msg_per_s=<r>
    replay_msg_per_s_equiv=<rr> chunks=977 combines=976 avx512=<b>
    bitexact=true dual_path=true
```

Local shape check (shared 3.2 GHz Xeon dev VM, unpinned — CI's 9V45
shards price the real numbers): `fold_cyc_per_msg ≈ 4.0`, cold ≈ 0.8B
msg/s, **replay ≈ 1.6T msg/s-equivalent (311 ns per 976-⊕ pass)**,
`bitexact=true dual_path=true`. The replay number is the regime-3
arithmetic alone: the 500k-message pass's whole fold cost is now ~300 ns
of combines — the fold stage has left the critical path exactly as §0
predicted ("verdict replay becomes a table read").

## 5. What this landing is NOT (the rollout path, blueprint §9)

This is checklist **step 2** — the algebra kernel, bit-exact and
CI-gated. Not yet ported (the ordered steps that turn the kernel into
the engine's worker path): the decoder's token emission (step 4), the
Phase-R order-ID cache with its §5.6 resolution policies (steps 3, 7,
8 — the true sequential residue, expected-O(1) per-ID chains), the
per-locate bucketing + 512-boundary commit into the worker fabric
(steps 5–6), the 3-thread topology (step 9), the memo table keyed by
`(chunk bytes ‖ entry-state fingerprint)` (step 10), and the 94-cell
port to boundary-hash comparison (step 11). The kernel deliberately
contains only the algebra that must be bit-exact — the blueprint's own
scope line.

## 6. The next wall (unchanged from the R33 verdict, now sharper)

The R33 fleet's champion DIAG showed `reset_ms = 2,157.9 ms` (43% of
wall) and worker frontier waits (`fw_ms` 653–797). The fold collapse
attacks the *eval* component of worker time; with the fold at ~0.1 ms
per 500k pass, the pass-level cost re-concentrates on decode + probes +
the pass-boundary reset/re-arm serve — the R35 candidates, priced by
the same fleet telemetry law that opened R33 and R34.
