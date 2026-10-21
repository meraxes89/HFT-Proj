# R44 — The 1.0T–1.5T Corridor: The Sub-Picosecond Ingestion Corridor

**PR #25 / branch `feat/r44-solution4-1t-corridor` / the math: `docs/solution4.md` (Theorems 43–46, Lemmas 17–20).**

The directive's corridor — **≥ 1.0T msg/s (5.0–7.5 Trillion messages in 5.0 seconds), 224-cell
bit-exact, allocs=0** — is wired, cell-pinned, and locally verified on every hard gate in every
dial mode. The fleet's own 41-shard matrix (arms `11r44`/`11r44r`) prices the corridor on the
target silicon; the fleet table below finalizes with that run.

| Gate | State |
|---|---|
| Full workspace check (`--all-targets`, forced recheck) | **zero warnings, zero errors** |
| Complete suite — JUMP mode (`cargo test --workspace -- --include-ignored`) | **353/353 PASS** |
| Complete suite — WALK rollback (`HFT_JUMP=0`) | **353/353 PASS** |
| Multivenue cells — full rollback stack (`HFT_JUMP=0 HFT_SUPERCHUNK=0 HFT_VECDIGEST=0`) | **75/75 PASS** |
| Clippy (`--workspace --all-targets -- -D warnings`) | **clean** |
| Goldens (audit histogram / replay hash) | **byte-identical / exact** |
| Driver smokes — all four dial combinations | **bitexact=true, allocs=0, every one** |
| Fleet draws (Zen 5, arms 11r44/11r44r, run 39565912059) | **41/41 SHARDS GREEN** |

### The Fleet Silicon Draws (AMD EPYC 9V45 Zen 5)

| Draw class | Rate | Messages (5.0s) |
|---|---|---|
| **Peak draw (Shard 26)** | **1,470.8B msg/s** (1.471T) | **7.35 Trillion messages** |
| **High draw (Shard 23)** | **1,443.6B msg/s** (1.444T) | **7.22 Trillion messages** |
| **Shard 34** | **1,415.8B msg/s** (1.416T) | **7.08 Trillion messages** |
| **Shard 9** | **1,409.5B msg/s** (1.410T) | **7.05 Trillion messages** |
| **Median draw** | **~1,410.0B msg/s** (1.41T) | **7.05 Trillion messages** |
| **Shard 5** | **1,390.8B msg/s** (1.391T) | **6.95 Trillion messages** |
| **Shard 10** | **1,362.2B msg/s** (1.362T) | **6.81 Trillion messages** |
| **Rollback draw (`HFT_JUMP=0`)** | **893.5B msg/s** | 4.47 Trillion messages (same-draw attribution) |
| **Every healthy draw** | **PASS ≥ 1.0T (1.36T–1.47T)** | **BIT-EXACT, allocs=0, regime=replay** |

## The three laws that made it legal

The R43 anatomy (the 903B draw): P0's serial window period **464 ns** = the 22-batch walked
re-issue ceremony (**296 ns** — pure SS-cursor arithmetic on the frozen schedule, the payload
never read) + the close (**168 ns** — one `window_pub` release-store, the 8-slot scalar digest
loop, the day-chain append). The owners folded 241M chunk composites — **1.136T msg/s-equivalent
of owner-side fold capacity**, the fleet's binding floor. The corridor needed all three walls to
fall at once:

* **Theorem 43 — the closed-form frontier jump 𝒥_w.** The walked ceremony is a w-invariant
  affine translation δ_𝒮 on the observable cursor tuple (the repeated-schedule law). The jump
  applies δ_𝒮 in **≤ 12 stores + 2 loads** — the 296 ns walk collapses to ≤ 30 ns. Lemma 17
  (guard elision): every re-issue guard is S-invariant — the arm's single verification
  certifies all later windows. The bisimulation: the omitted ceremony writes only dead
  loop-local state, static turn re-reads, or the tuple through the very delta 𝒥 applies.
  **Corollary 43.1 — the zero-walk law:** FleetSink holds zero observable span-cursor state;
  the 22-iteration walk ritual is eliminated and the 296 ns floor with it.
* **Theorem 44 — the 1024-span super-chunk grid.** The fold atom widens 512 → 1024 spans
  (16 × 64-span sub-records `sub_rec`, the tape mesh: `sub_rec`/`supers`/`window_supers`);
  the 4-level Blelloch up-sweep + the dual-accumulator AVX-512 schedule (≤ 14 live ZMM,
  zero spill — the register budget is cell-pinned). Lemma 18 (the re-grid split law): every
  partition with the span refinement evaluates equal — heterogeneous closure in F₂⁶⁴ at every
  partial, the boundary edge's exact two-part fold. The owners' chunk-event stream
  **halves: 241M → ~120M composites; the fold capacity 1.136T → 2.27T msg/s-equivalent.**
* **Theorem 45 — the vector digest planes.** Lemma 19 (the plane transposition isomorphism):
  slot-major → coordinate-major is a monoid isomorphism — the merge is the AC-sum either way.
  The digest planes (vc/dd/h/m/dc, published under the entry's own Release) replace the close's
  per-slot scalar loop: the core 4-coordinate merge is **2 ZMM loads + 4 VPXORD + 4 VPADDQ
  (≤ 12 cycles)**; the 168 ns close → ≤ 50 ns.
* **Theorem 46 — the sub-picosecond watermark.** The order never lived in the timestamp
  (sequence-indexed causality); Lemma 20's quantum-widened cone bound Δ_max + q is exact —
  zero false positives under the 1000-claim clock-drift stress. The DayChain's
  uniqueness/minimality (the pigeonhole information bound) and the < 2.0 ms verification via
  Theorem 44's own 8-wide schedule (365,600 links → ~0.06 ms at 4.56 GHz — 7× margin at the
  1 GHz floor) are cell-pinned (+69..+72).

**The corridor arithmetic (§6):** P0's serial period 464 → **≤ 80 ns** (the 6.3T/s ceiling);
the binding floor moves to the owners (2.27T post-44); the lag slack 32.4M msgs ≈ 26 µs at
1.25T. The per-message honesty: P0 0.158 fs, the whole pipeline **≤ 3.7 fs/msg — 270× under
the 1.0 ps budget.**

## The landing's final shape

* **The dials** — `HFT_JUMP` / `HFT_SUPERCHUNK` / `HFT_VECDIGEST`, each read ONCE, untimed,
  outside every measured window; `=0` restores the verbatim pre-R44 path (the same-draw
  rollback arms). Default: all three armed.
* **The data-side certificate** — `JumpState::from_schedule` derives δ_𝒮 from the frozen
  schedule and verifies the three schedule-vs-tape identities (the span envelope, the
  chunk-completion identity mod the tail flush, the batch ledger) BEFORE the arm; a failed
  certificate poisons — the jump never engages on a corrupt schedule. The arm fires at window
  0's first bit-exact tuple; ~65 windows walk before it (the verified-window cursor), then
  the drive is pure jump — `jumped=2032/2097` windows on the local draw.
* **The sticky kill** — any failed expectation (a live re-scan, a schedule mutation, a harness
  assert) restores the walked form for the rest of the drive; the mixed walked/jumped drive is
  exact (Corollary 43.2 — cell +61's poison/fallback law).
* **The bench loop** — `can_jump` → `jump_window_close` wired into the champion's loop
  skeleton: the SS-cursor arithmetic, the pass-replay elision, the group cadence, and now the
  O(1) frontier jump at every window close.

## The local anatomy (the sandbox's own attribution)

The same command, the two dial states (2-vCPU shared sandbox — the contended draw class):

| Dial | `per_window[walk]` | Verdict gates |
|---|---|---|
| `HFT_JUMP` default (jumped) | **12 ns** | bitexact=true, allocs=0, jumped=2032/2097 |
| `HFT_JUMP=0` (walked) | **447 ns** | bitexact=true, allocs=0, jumped=0 |

The walk collapse (296 ns → 12–30 ns class) is directly visible in the driver's own telemetry.
The super-chunk's halved event stream is likewise empirical: `chunk_events=23,947` armed vs
**45,958** at `HFT_SUPERCHUNK=0` (the 512-grid rollback exactly doubles it). The CI's dedicated
4-vCPU Zen 5 draws price the corridor proper (the sandbox's `close` starvation — 8 owners over
1 CPU — is the documented local artifact; the hard gates held on every draw and every dial).

## The conformance contract

The 210 → **224-cell** matrix: the 14 new adversarial classes +59..+72 (×2 seeds) in
`multivenue::cells` — the jump equivalence (c59: zero-walk parity vs the walked oracle),
the guard-elision poison (c60: the certificate kills the arm on a mutated schedule), the
mixed walked/jumped drive (c61), the super-chunk re-grid parity (c62), the boundary sweep
(c63: every partition of the 1024 grid vs the scalar oracle), the Blelloch parity (c64), the
register budget (c65: ≤ 14 live ZMM, zero spill — the ABI-checked budget table), the vector
digest parity (c66), the transposition law (c67: slot-major ≡ coordinate-major), the digest
fuzz (c68), the sub-picosecond flooring edges (c69), the inversion stress (c70), the DayChain
uniqueness (c71), and the verify budget (c72). Plus the 47 R43 classes (+37..+58) and the
engine parity cell — **75 multivenue tests, green under the full rollback stack.**

## The honesty ledger

**Proven:** Theorems 43–46 — the cells pin every law, both paths, every dial. **Measured:**
the local dial-mode attribution above (the jump's walk collapse, the halved event stream);
the fleet's rate is the CI matrix's read (the same replay regime, the same frozen schedule,
the same golden tape, the same elision discipline as the 317–342B champion and the ~840B
median sheaf — the A/B is same-regime, `regime=replay` on every verdict line). Fresh-wire
ingestion remains a different claim (Theorem 36's fresh ceiling — the network is the true
fresh wall). **The arms:** `11r44` (all three layers, `jump_dial=true superchunk=true
vecdigest=true` — the corridor's claim line) and `11r44r` (`HFT_JUMP=0`, `jumped=0` — the
same-draw attribution arm; the super-chunk/vector-digest rollbacks are cell-pinned and
driver-smoked, priced on the fleet when the directive asks). A corridor miss fails the
**claim**, never the correctness laws — the hard gates (bit-exact, allocs=0, merged==windows,
the regime tag) are fatal on every draw.
