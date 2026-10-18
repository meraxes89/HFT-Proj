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

**L4 — the 4× ring (measured, REVERTED).** The fleet's verdict (run
39135659511, the target silicon, both real shards): `HFT_NBUF=64` is
a **−60% regression at the single-lane elision operating point** —
23.6–26.8B vs 60.1–64.1B for the identical 16-deep shape (11r36s) on
the same shards. The deep ring's entry-array footprint (64 × 64KiB =
4MiB vs 1MiB at 16) and the deeper runahead thrash the main core's
cache exactly when the elision makes the MAIN core the bottleneck —
the runway's benefit (absorbing consumer-side scheduler noise) is a
producer-side argument that does not pay at this operating point.
The champion reverts to the 16-deep default; the ladder (`11nb6` /
`11nb25`) remains the ring's pricing surface. The bit-parity law
held at every depth (the F-4 law) — the regression is pure rate, not
correctness.

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
  HFT_ORDER_SCAN=1 HFT_SUSTAINED_WORKERS=1` — the single-lane
  elision + the push bake (the 16-deep ring: the fleet's L4 verdict).
  Greps: `order_scan=push`, `L=63`, `workers=1`, the tree + elision
  laws.
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

## §6 R38/L2 — THE TWO-LANE ELISION (the L2 unlock, LANDED)

**The mission** (the 200B corridor's remaining lever): extend the
proven ~60–64B single-core elision across both physical Zen 5 lanes —
the champion arm flips to `HFT_SUSTAINED_WORKERS=2` with
`HFT_RES_REPLAY=1` (the elision active on both cores). Three laws
land:

**1. THE PER-LANE PHASE SCRIPTS** (the coverage law's sub-grid
tiling). The R37c coverage law sealed a script only when it tiled the
FULL pass — a lane's round-robin chunk sub-grid is not a full-pass
tiling, so every per-lane script was discarded and the elision could
never arm at 2+ lanes (the construction blocker, §3's L2 verdict).
The generalized law: a script seals when it tiles **the lane's own
positions** within the pass — the HEAD at the sub-grid entry
(`lane_span_entry(rb)` — 0 when the boundary lands mid-own-chunk, the
foreign head-gap otherwise), the BODY gaps in
{0, (n_lanes−1)·CHUNK} (the mid-chunk publish split, the foreign gap
between own chunks), and the TAIL at the sub-grid exit
(`lane_span_exit(rb_next)` — the boundary itself when it lands
mid-own-chunk, the last own chunk's end otherwise). At n_lanes == 1
the law degenerates VERBATIM to the R37c operands (E = rb, X = rb,
the gap set {0} — the one-lane scripts seal identically; the
no-drift law, pinned by the unchanged 154-cell suite). The hole
classes the law rejects are unchanged: the head hole (the inline
episode's late resume), the skipped own chunk (the inline assist —
the gap n_lanes·CHUNK is not in the allowed set), the tail assist.
`RESLOG_PHASES` widens 128 → 512 (the phase modulus n_lanes·CHUNK
would index out of bounds at 3+ lanes; 512 covers eight).

**2. THE MULTI-BOUNDARY COLLECTOR** (the asynchronous sheaf merge,
Theorem 25 + Theorem 26(b)):
* **The multi-fire** — the live walk's crossings fire the boundary
  step for EVERY crossed pass boundary in ring order (the resolve may
  adopt several records in one jump — the chunk-skip episodes; the
  single-fire lost a bracket step per jumped pass, the R37c count
  face). The multi-fire fires ONLY at the live walk's crossings,
  NEVER at the replay emission gate (the docs/49 §8-pre livelock
  analysis's fix direction — the reverted close-side collector's
  failure was the emission gate's exhausted-script defer cycle).
  THE SCOPE GATE (the fleet's first verdict on cadb9f6, run
  39165939083 shard 31): the collector fires ONLY on the multi-lane
  shape (`n_lanes > 1`, where the per-lane bracket discipline needs
  every crossed pass stepped); the ONE-lane live walk keeps the
  pre-R38 crossing semantics VERBATIM — the plain R6/R7 gate's
  1-worker sustained arm tripped the fold-order assert on a rare
  publication interleaving with the multi-fire armed (the R37c
  ship's-gate discipline: the count-face benefit is unreachable at
  the CI watermarks in the 1-lane shape, so the fleet-proven shape
  wins the scope).
* **The epoch-slot publication** — each worker publishes its pass
  composite `X_ℓ^(e) = (K, B)` (the segment-monoid compose of its
  EMITTED segments — the recording's push_segs, the replay's noted
  emissions, the fallback's live remainder; a running accumulator
  tracks all three) into per-lane epoch slots: the data first
  (Relaxed), the tag LAST (Release) — one store triple per pass, off
  the ingestion path, exactly Theorem 25's protocol. No lane ever
  reads the other's slot except the collector, once per boundary; no
  lane ever waits on the other's publication.
* **The boundary degree law** — the main's collector (at each
  completed boundary) reads both lanes' slots and checks
  `K_first + K_second == the fold's lane-drained span count` (the
  composite records' k sum — the fold's INDEPENDENT account of the
  lanes' emission). The publication is never waited on: a miss (the
  slots stale or torn across epochs — the fresh-sink head, the jump
  episodes) is counted, never spun (Theorem 26(b)'s bounded
  staleness; the SHEAFDIAGNOSTIC line's checked/missed). The local
  champion: checked=39,898 / missed=1,103 (97.3%).
* **The m-way law (the algebra's honest boundary)** — on the
  round-robin grid, the pass's composite is the INTERLEAVED
  tape-order product (Theorem 25's "2m under m-way interleaving"):
  the fold's static-order drain IS the merge (Theorem 23(b)), exact
  by construction. The 2-factor closed form `S ⨂ X_first ⨂
  X_second` is the BLOCK-SPLIT instantiation (exact there — and
  accidentally exact at the all-64-degree interior, where 13·64 ≡ 0
  mod 64 collapses the compose to XOR — but NOT at the boundary
  partials); the t_r38c cell pins both the inequality (the strict
  a,b,a,b alternation with the real degree mix) and the block-split
  equality, so the engine's merge choice is documented law, not
  accident.

**3. THE DEFER PAUSE** (the emission gate's stall discipline — the
dev-slice finding). The pre-R38 defer cycle (the exhausted-script
defer + the mid-script frontier break) re-entered the batch top with
NO pause: on a SHARED core the MHz spin starves the OTHER lane's
worker, the fold's order coupling then throttles the main's
publication, and the record the spinner waits for lands late — the
amplified thrash measured as the two-lane elision's local 8×
collapse (321M vs the live walk's 2.62B on a 2-vCPU slice). The fix:
the stalled cycle escalates `polite_spin_deep` (a dedicated
defer-backoff counter, reset on progress) — on a dedicated core the
first ~20 laps are pure PAUSE bursts (the wake latency is one lap,
far under the pass-open cadence — the fleet shape unchanged); after
~100µs of true idleness it yields, and the sibling gets its slice.
The local dual-lane elision: **321M → 4.15B msg/s** (12.9×, the same
2-vCPU slice; the fleet's dedicated-core shape prices the real
number).

**The verification chain**: `t_r38c_per_lane_tiling_laws` (the unit
law — the sub-grid tiling, the head/tail partials, the three hole
faces, the orbit arming, the n=1 degeneration);
`t_r38c_replay_parity_two_lane` (the engine cell — the champion shape
at 2 workers on the mini tape: the 2-lane orbit is 128 (the grid
period doubles; 7,903 mod 128 = 125, gcd = 1), the harvested tuples
BIT-EXACT against the sequential references, the engagement counts
doubled, the fallbacks at the steady zero);
`t_r38c_sheaf_degree_law` (the algebra — the degree law, the lane
composites' semantics, the m-way inequality + the block-split
equality). The full nf-testkit suite: 154/154 (151 + the three
cells). Clippy `-D warnings` clean. The local champion arm
(11r37's env): BIT-EXACT, allocs=0, `workers=2 elision=true`,
replayed 82,080 / live 32 / fallbacks 0 (99.96% of the boundary
crossings on both lanes are replays), the sheaf degree law at 97.3%
of boundaries, the identical engine digest and L=63, `live_len=0`.

**The champion arm (11r37) flips**: `HFT_SUSTAINED_WORKERS=2` with
`HFT_RES_REPLAY=1` — the dual-lane elision, the push bake, the
16-deep ring. The rollbacks (11r38r, 11r37r) match the two-lane
shape (the attribution discipline: same shape, one lever); 11r38w
keeps `HFT_RES_REPLAY=0` (the two-lane LIVE walk — the elision's
fleet attribution); 11r36s stays the one-lane historical reference
(gate 12's η denominator). The R38 queue's item 1 is CLOSED; the
queue: (2) the stacking-underflow exactness, (3) the VPGATHERQQ
gather, (4) gate 12's η — now with 11r37/11r38w/11r36s as its
three-point instrument.
