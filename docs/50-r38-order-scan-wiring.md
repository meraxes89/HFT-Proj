# R38 — The Order-Scan Engine Wiring (Gate 13's L3) + the Corridor's Operating Shape

Run 900 continues on `feat/run900-r37-solution2-corridor`. This run
lands the R38 queue's first item — **the fabric integration of the
Theorem-29 kernel** (`orderscan.rs` → the bake's ACTIVE resolve path) —
plus the corridor's operating shape (L4's 4× RX ring) and the two-lane
lane-shape pricing arm. The verification discipline: a pre-port
differential twin (bit-exact on the full 505,849-message golden tape +
the adversarial fuzz corpus), then the Rust port pinned by the parity
cells at three scales (hand cases, 3k-message fuzz draws, the full
tape), then the engine arms' own fail-stops.

## §1 The L3 wiring — the bake's push resolve

**The change.** `bake_sig_fabric`'s pass 2 (the Phase-R resolve) is now
the `HFT_ORDER_SCAN` arm (default ON): the id-bucketed segmented scan
(`orderscan::order_scan_dispatch` — the AVX-512 twin when present,
`HFT_ORDER_SCAN_VEC=0` forces the scalar SoA) replaces the
`OrderCache` hash-table pull as the ACTIVE engine resolution;
`HFT_ORDER_SCAN=0` restores the pull verbatim (the attribution /
rollback arm, `resolve_pull` — the original loop moved verbatim).
Both paths produce the identical message-order token columns; the
bake's shared epilogue writes the two layouts (the locate-bucketed
SoA + the R36 message-order columns) from the one tuple stream — the
no-drift law, unchanged.

**What it buys.** The bake is untimed, so this is not itself a rate
lever. It is:

* **[S1] §6.4's prohibition lifted at the fabric level** (Theorem
  31's prerequisite): the push model's closed forms carry each
  register's prefix state TO its reference positions — the probe's
  serial dependency chain (depth up to B per thread) re-associated to
  ⌈log₂ B⌉ = 9 levels whatever the id distribution. The dual-lane
  resolution designs (the R38 queue's per-lane phase scripts, the L5
  co-lane configuration) rest on this soundness condition.
* **The B3 wall's dataflow inversion proven in the engine**: the
  resolution is now a pure function of the batch's rows — no register
  file, no probe order, no cache factor. The fabric's frozen tables
  are identical (the parity cells pin it), but their DERIVATION is
  schedule-free.
* **The bake's own cost drops** (the scan is ~5.3× the scalar
  traversal on the AVX-512 hosts — the spanbench regime; `sig_bake_ms`
  in the SIGFABRIC_DIAGNOSTIC prices it per host).

**The port's laws** (`sigpush.rs`, each pinned by the cells):

1. **ROW-BUDGET BATCHING** — ≤ 512 rows per scan batch
   (`SCAN_B_MAX`), a message never splits (a U's two rows — the
   D-like on the old id's thread, the A-like run-start on the new
   id's — must scan together).
2. **ID-BUCKETING** — per-batch first-appearance bucket order
   (deterministic); the segmented operator makes per-thread
   composites bucket-order independent, and the closed forms are
   exact for ANY batch split (Theorem 30's carry-in).
3. **THE RUN-START LAW** — `f = 1` on bucket starts, every `A` row,
   and every u_new row; interior ref/D rows cleared. An A as a run
   start is the pull's insert-overwrite semantics (re-birth); a u_new
   run start is the new register's thread boundary.
4. **THE ENTRY LAW** — the closed form `o = e·M_pre + b_pre` against
   the thread's entry: the carry-in register (continuing threads),
   `(0,0,0)` (A-born threads — the A's zero masks make the entry
   irrelevant), the old thread's state at the U (u_new threads — the
   s_b flows through `rowop_u_new`'s M = (1,0,0)), `None` (absent —
   §5.6's absent-id anomaly, no token).
5. **THE PHANTOM LAW** — a U on an absent/dead register births NO new
   register (the pull's insert fires only on a live probe): the
   u_new entry is `None` and the A-like row's kept m[0] keeps the
   anomaly check armed for the thread's later references.
6. **THE CLAMP RE-ANCHOR** (Theorem 29(a)'s policy arm) — a violating
   E/C/X re-anchors its thread's continuation with the overwrite
   row-op `(M = 0, b = (σ_b, π, 0))`; the loop is monotone (the
   fresh-violation frontier), the telemetry banks only the final
   round.
7. **Theorem 7(iv)'s L is computed** on the push path (the resolved
   lifecycle events per id — `L=63` on the tape, the tracked
   invariant); `probes`/`rehashes`/`max_chain` remain the pull's own
   instruments (the push reports 0 — the attribution law).

**The D-exclusion law** (the port's one hard-won finding): the
kernel's `emit` carries a SIGNED clamp check (`rho as i64 < d`); on
the transient WRAPPED ρ (a pre-re-anchor over-cancel leaves ρ with
the top bit set) a D's check (`d = 0`) reads the wrapped ρ as
negative and fires ok = 2 — and re-anchoring a D would rewrite it
with `dead = 0`, WIPING the thread's terminator and resurrecting the
register (post-D references resolve instead of anomaly). The
kernel's own doc: "U's removal is exact and D's d is the ρ itself —
neither clamps". The wiring's law: **only E/C/X enter the re-anchor
set**; the D's transient token is re-emitted by the next round and
no wrap can survive into the final round (every wrap originates at
an over-cancelling E/C/X, which the loop re-anchors).

## §2 The verification chain

1. **The pre-port twin** (the design oracle): a Python differential
   twin of BOTH resolve paths, run on the full golden tape
   (505,849 messages — 0 mismatches, 0/0/0 telemetry drift) + the
   adversarial fuzz corpus (12 draws × 3k messages, ≈1.9k
   anomalies + clamps each — the anomaly/phantom/straddle/hot-id
   faces the tape never shows). The twin caught the three design
   holes pre-port (the carry-in's born-thread marking, the u_ent
   validity chain, the U-chain σ_b flow).
2. **The parity cells** (`t_r38_bake_push_pull_parity`): 7 hand
   shapes + 6 fuzz draws, every frozen table + the digest + the span
   products + the telemetry compared. `t_r38b_..._tape`: the full
   tape, both paths, every table.
3. **The suite**: 151/151 green with the push path ARMED (all the
   R35-B/R36 cells now bake through the push path — defense in
   depth), and 151/151 with `HFT_ORDER_SCAN=0` (the pull).
4. **The engine arms**: the champion arm's own fail-stops — the
   per-pass digest assert, the SPANHASH/Galois merge asserts, the
   BIT-EXACT gate — all green with `order_scan=push`; the digest is
   IDENTICAL across the push and pull arms
   (`0x53988eaacf452cd1` on all three local runs).

## §3 The corridor's operating shape (the L2/L4 verdicts)

**L4 — the 4× ring (armed).** The champion arms run `HFT_NBUF=64`
(the R33-C ladder's 4× point — the deep-ring runway absorbing
consumer-side scheduler noise; `11nb6` prices the depth, `11r36s`
holds the 16-ring reference). The ring is protocol-transparent (the
F-4 law, `t_r33_nbuf_ladder_parity`) — the bit-parity gate holds.

**L2 — the two-lane measurement (the honest verdict).** The R37b
laws + the R37c coverage law hold; the ship's gate can lift. But the
MEASURED shape on the dev slice: the two-lane LIVE walk runs
**3.2B msg/s vs the single-lane elision's 9.9B** — the eval
dominates (the elision's record emission is ~10× cheaper per span
than the live walk), and the elision CANNOT arm at 2 lanes by
construction: the R37c coverage law seals a phase script only when
it TILES its pass, and a lane's round-robin chunk sub-grid is not a
pass tiling — every per-lane script is discarded at the close, the
replay never arms, and the bench's engagement law (now scoped to
the single-lane shape) would trip. **The two-lane elision — the
per-lane phase scripts + the multi-boundary collector's
emission-gate fix — is the R38 queue's item 3** (the docs/49 §8-pre
livelock analysis stands). Until it lands, the champion keeps the
PROVEN single-lane elision shape (the 9.9B class locally, 61.21B on
the fleet) and **`11r38w` prices the lane shape on the fleet's real
silicon** (2 physical Zen 5 cores — the dev slice's 2-core
contention is the pessimistic bound; the fleet's number is the real
one). If the fleet shows the lane shape paying, the champion flips;
if not, the per-lane elision is the unlock.

## §4 The arms

* **11r37 (the champion)**: `HFT_RXDESC=1 HFT_RES_REPLAY=1
  HFT_ORDER_SCAN=1 HFT_SUSTAINED_WORKERS=1 HFT_NBUF=64` — the
  single-lane elision + the push bake + the 4× ring. Greps:
  `order_scan=push`, `L=63`, `workers=1`, the tree + elision laws.
* **11r38w (NEW — the L2 pricing)**: `HFT_RES_REPLAY=0
  HFT_SUSTAINED_WORKERS=2` — the two-lane live-walk attribution on
  the fleet's silicon. Greps: `workers=2`, `elision=false`, the
  bit-parity floor.
* **11r38r (NEW — the L3 rollback)**: `HFT_ORDER_SCAN=0` — the
  Phase-R pull verbatim; the same-draw delta prices the push scan's
  bake-time contribution. Greps: `order_scan=pull`.
* **The t_r38 cells** before the champion: the parity corpus + the
  full-tape differential.
* **11r37r (the tree rollback)**: the champion's shape with
  `HFT_TREE_FOLD=0` (the tree's own attribution, unchanged law).

## §5 The R38 queue (carried forward)

1. **The two-lane elision** — the per-lane phase scripts (the lane's
   chunk sub-grid as the script's tiling — the static epoch-block
   assignment of Def 3.1) + the multi-boundary collector's
   emission-gate fix (the live-walk-only multi-fire; the docs/49
   §8-pre three-way livelock analysis). THE L2 unlock.
2. The stacking-underflow re-anchor's adversarial exactness
   (multi-round chains).
3. The batched VPGATHERQQ entry gather (Theorem 30(ii)).
4. Gate 12's η measurement (the lane interference efficiency —
   11r38w's fleet number is its first input).
