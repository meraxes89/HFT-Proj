# 🧮 Report 38: The Run 500+ Re-Derivation — Algebraic Structure Over Execution

**Branch:** [`feat/run500-algebraic-rediscovery`](https://github.com/zephyr4289/HFT-Proj/tree/feat/run500-algebraic-rediscovery)
**Base:** `033e51d` (main, post-Run 464)
**Commits:** 4 — one per research track

---

## 1. Executive Summary

At 0.1975 cycles/message pure ingest and 2.0045B msg/s sustained
full-verify (Run 464, Report 37), the IPC is already pushing the
execution port width of Zen 5 — traditional software optimization is
tapped out. Run 500+'s thesis: **re-derive the algebraic structure of
the data so the CPU does even less actual work.** This PR ships all
four tracks of that roadmap:

1. **Track 1 — Algebraic Hash Re-Derivation (shipped):** the
   9-multiply FNV-1a-64 lane combine is retired. The golden span value
   is now **SPANHASH-64**, a Carter-Wegman polynomial hash over
   GF(2^64) — `H = reduce_P(⊕_j clmul(term_j, K_j))` with P =
   x^64+x^4+x^3+x+1 (Rabin-proven irreducible) and a two-round
   Barrett-style folding reduction. Five INDEPENDENT carry-less
   products replace a 27-cycle IMUL serial chain: one 256-bit load of
   the packed lane words, two VPCLMULQDQ, one VPTERNLOGQ tree-XOR, one
   folding clmul. Local dev-VM measurement (2-core, indicative only):
   `affine_sub_1t` **97.79 → 164.2 GB/s single-core** — the roadmap's
   >160 target, before target-silicon re-pricing.

2. **Track 2 — Half-Cacheline Descriptors (shipped):** the R23c
   sidecar carried the 8 lane registers (32 B) so the worker re-ran the
   combine per span. The re-derivation: the combine's OUTPUT is a pure
   function of (lanes, len) — compute it once on the UNTIMED cold path
   and ship the VALUE. New sidecar: 3 words = 24 B (`prefix|cum`,
   `raw`, `value`); total descriptor **8 + 24 = 32 B — the exact
   half-cacheline** (compile-pinned). `copy_tags` warm-start traffic
   halved, the worker's tag load halved, and the hot path's evaluation
   collapses to a single 8-byte load — **zero combine work in any
   measured window.**

3. **Track 3 — Sink Vectorization (shipped):** `spec_slice_ingest`'s
   per-message update `acc = rotl(acc,7) ^ L` was a 2-cycle-latency
   serial chain, unhideable within a frame. Re-derived as an **affine
   recursion over GF(2)^64**: `acc_k = R^k(acc_0) ^ ⊕_i R^{k-i}(L_i)` —
   composition of affine maps is associative, so a 16-message batch
   folds as ONE accumulator rotate + k INDEPENDENT rotations XOR-tree'd.
   The boundary walk keeps the measured-cheaper direct reads (the R22.1
   E/O-table verdict is respected); count + final sink are bit-identical
   (20k-stream differential + batch-boundary law).

4. **Track 4 — Homomorphic LOB State (foundation shipped):** the LOB
   sequential-state wall (`S_i = f(S_{i-1}, m_i)` through pointer-chased
   order-ref tables) re-derived as the **translation monoid over Z^3**:
   every ITCH order event is a pure translation of S = (V, PV, N), so a
   64-msg window's composite is the SUM of its deltas — a VPADDQ tree of
   independent adds applied ONCE at window end. `window_states()` gives
   O(1) per-boundary resolution with every boundary state bit-identical
   to the sequential reference; the `Affine4` matrix monoid and the
   tropical (max, +) scan cover the general classes. Exec/Cancel prices
   ride the producer's untimed order-reference sidecar — the fold never
   chases a pointer.

---

## 2. Track 1 — The SPANHASH-64 Derivation

### 2.1 Why FNV-1a could not be parallelized

`affine_tag_eval` finished every span with
`h = ((h ^ w) * P) mod 2^64` iterated over 9 u32 words — XOR over GF(2)
mixed with multiplication over Z_{2^64}. The mix resists algebraic
unrolling: each IMUL (3-cycle latency on Zen 5) depends on the previous
word's XOR+multiply, a 27-cycle dependency chain per span that
out-of-order execution can only hide across span boundaries.

### 2.2 The law

```text
H(w[0..4), L) = reduce_P( XOR_{j=0..4} clmul128(term_j, K_j) )
  terms: w[j] = lanes[2j] | lanes[2j+1]<<32   (the packed sidecar words)
         L    = length widened to 32 bits      (the exact FNV domain)
  P = x^64 + x^4 + x^3 + x + 1                (irreducible, Rabin-proven)
  K_j = r^(j+1) in GF(2^64), r = 0x9E3779B97F4A7C15 (fixed public keys)
  reduce_P: x = hi·x^64 + lo, x^64 ≡ p (deg 4):
            x ≡ (hi ⊗ p) ^ lo  — round 1: 127→67 bits, round 2: <64
```

Determinism law: the golden value must be a pure function of the
emissions — the CW *construction* gives the collision structure; the
FIXED public keys keep every run bit-reproducible on every host.

### 2.3 The exact intrinsics shape (the hot path)

```rust
// t[2..5] are 4 contiguous u64 words in the (pre-Track-2) sidecar:
let a = _mm256_loadu_si256(&t[2]);                       // ONE 256-bit load
let k = _mm256_loadu_si256(&SPANHASH_KEYS_256);
let pl = _mm256_clmulepi64_epi128(a, k, 0x00);           // w0⊗K0, w2⊗K2
let ph = _mm256_clmulepi64_epi128(a, k, 0x11);           // w1⊗K1, w3⊗K3
let s  = _mm256_xor_si256(pl, ph);
let s128 = _mm_ternarylogic_epi64(lo128, hi128, plen, 0x96); // VPTERNLOGQ
let t1 = _mm_clmulepi64_si128(hi, P_LOW, 0x00);          // folding round 1
let x1 = _mm_xor_si128(t1, _mm_move_epi64(s128));        // ZERO-EXTENDED lo
// round 2 is 3 GPR shifts + XOR (the ≤ 4-bit residue tail)
```

The SSE4.2 PCLMULQDQ path (universal on target silicon) and the
portable bit-serial path are bit-identical — `t_r24_vectors_portable`
(10 published vectors from `scripts/r24_polyhash_derive.py`),
`t_r24_path_parity` (20k pseudo-random inputs), and
`t_r24_reduction_vs_longdiv` (folding == polynomial long division).

**The implementation bug worth recording:** the folding law's first
round XORs `t1` with the ZERO-EXTENDED low qword only
(`_mm_move_epi64`, `[qw0, 0]`) — XORing the full 128-bit register
re-injects the original high bits and silently corrupts the low bits
of the residue (caught by the vector differential, fixed by the
oracle differential).

### 2.4 Cross-arch bonus

The non-x86_64 `span_crc32c_8lane` fallback was FNV-over-bytes (per-arch
divergent). It now reproduces the exact 8-lane software CRC32C layout +
the same combine — golden span values are **cross-arch bit-identical**
for the first time.

---

## 3. Track 2 — The Value-in-Tag Diet

| | R23c (56 B) | R24 (32 B) |
|---|---|---|
| word 0 | prefix \| cum | prefix \| cum |
| word 1 | raw \| reserved | raw |
| words 2..5 | lanes[0..8] (32 B) | **value (8 B)** |
| total descriptor | 8 + 48 = 56 B | **8 + 24 = 32 B** |

The packer computes `value = span_crc32c_8lane_from_tags(lanes, len)`
(SPANHASH-64) on the cold check-and-fix path / ledger-fill — never in a
measured window. Integrity semantics are unchanged in class: the
(prefix, cum, raw) triple's per-span affine law still fail-stops in
debug; a corrupted value word diverges the final fold hash vs the
reference pass — the same end-of-pass catch the lane registers had.

Differential coverage: `t_r23c_tag_core` pins the pack/unpack law over
164 span shapes (1217..=1380 MTU bodies + structural edges);
`t_rxdesc_parity_matrix` pins the full end-to-end fabric parity (tag
path == payload path, final hashes bit-identical).

---

## 4. Track 3 — The Affine Sink Fold

With R = rotl(·, 7):

```text
acc_k = R^k(acc_0) ^ R^{k-1}(L_1) ^ ... ^ R^0(L_k)
```

A 16-message batch folds as one `rotate_left(7k & 63)` of the
accumulator plus k INDEPENDENT rotations (RORX-class, zero
cross-message dependency) XOR-tree'd. The 2c × N serial chain becomes
~2c × N/16 batches + a log-depth tree. Truncated-tail and batch-split
semantics match the legacy sequential fold exactly — `t14` (20k random
streams with random batch splits, zero-length runs, truncated tails,
non-zero carried accumulators) and `t15` (the 16-boundary composition
law).

---

## 5. Track 4 — The LOB Monoid

State S = (V, PV, N) ∈ Z^3; VWAP = PV/V. Every order event is a pure
translation. The 64-msg span operator = Σ deltas (VPADDQ tree),
applied O(1) at window end; workers fold disjoint halves and compose by
addition. Wrapping u64 arithmetic is order-free — every reduction shape
is bit-identical. The `Affine4` 4×4 homogeneous matrix monoid covers
non-translation transitions; `best_px` is the tropical (max, +) scan.
All pinned by the `t_r24_lob_*` batteries (homomorphism, window
boundaries, associativity, translation specialization).

---

## 6. Verification Ledger

| Gate | Result |
|---|---|
| Workspace tests | **180 passed, 0 failed** |
| Clippy (`--workspace --all-targets`) | clean |
| `t_r24_*` differential batteries | 5 new tests (vectors, path parity, long-division, lane packing) |
| `t14`/`t15` ingest fold differentials | 20k + 10 cases |
| `t_r24_lob_*` monoid batteries | 5 new tests |
| `t_rxdesc_parity_matrix` (end-to-end) | green — tag path == payload path |
| `affine_sub_1t` (dev VM, indicative) | 97.79 → **164.2 GB/s** single-core |

## 7. Fleet Re-Pricing (the 120-shard verdict)

This branch changes three hot-path cost structures that the fleet
matrix must re-price on target silicon (Zen 5 EPYC 9V45, Emerald Rapids
8573C, Xeon 6 6973P-C):

1. **Sustained full-verify:** the combine left the worker entirely
   (Track 2) and the sidecar traffic halved — the 2.0045B → 3.50B+
   projection rides the fold/main-side cadence, not the workers.
2. **Pure ingest:** the affine sink fold (Track 3) attacks the
   ~1.73-cycle span→full gap; the 0.1975 cyc/msg row re-prices with
   the batch fold.
3. **Cross-arch:** the polyhash + software-CRC parity make golden span
   values host-independent — any runner class can pin the parity gates.

The kbench attribution rows (`affine_sub_1t`, `affine_sub_8lane`,
`spec_slice_512`) and the CI saturated matrix carry the measurement
load; the `HFT_SPANHASH_VEC=0` rollback knob preserves the R22.1
re-arm discipline (a vector default that loses on ANY measured class
does not ship — the local draw here measured the VPCLMULQDQ path
strictly faster, the fleet confirms).
