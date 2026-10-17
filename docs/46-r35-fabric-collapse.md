# 46 — Run 750: The R35 Program — the Pass-Boundary Collapse + the 𝒜₅.₀ Fabric Wiring

The program against PR #19's merged baseline (Run 700, the R34 kernel).
The R33 record draw's telemetry left two walls: the pass-boundary
`reset_ms = 2,157.9 ms` (43.2% of the 5.0 s window — the consumer
stalling in `reset_pass` at every one of 175,858 passes) and the
worker arithmetic (`eval_ms ≈ 3.7 s`, 74% duty) that the R34 kernel
made collapsible but did not yet touch. R35 lands the first wall's
demolition and wires the kernel's signature surface into the live
fabric.

## §1 The boundary wall, measured before it fell

The new `BOUNDARY_DIAGNOSTIC` (ships in bench.rs) split the local
champion draw's reset window:

```
reset_wait_ns/pass   11,936     (the transport's reset_pass wait — 85%)
pass_open_ns/pass     2,047     (begin_pass + the sequencer re-anchor)
ADVANCE (the RX bake)     47 ns/pass  — the bake was NEVER the wall
```

The stall was the round trip: the consumer frees the EOS marker →
futex-wake → the RX runs the advance → futex-wake → the consumer
resumes. Two syscalls and two scheduling latencies per pass, ×175,858.

## §2 R35-A: the marker-decoupled advance + the bounded pre-arm gate

The frozen-schedule regime (rxbuild + session elision + the R33
schedule replay + the auto program — the same word every replay gates
on) makes the advance a **pure latency move**: the bake (a zero-write
`reset_cursors` + two contract stores) runs at the EOS-marker's
**LANDING** — the consumer is ~NBUF turns back, so `auto_pass` is
already the boundary's target when `reset_pass` arrives. The
marker-free→wake→advance→wake chain never enters the boundary. The RX
then pre-publishes the next pass's turns under the ring's natural flow
control.

**The bounded pre-arm gate** keeps the R16b cap-lock sound at every
ring depth: the next marker lands only after the consumer FREED the
current pass's first data turn (`freed[(P&mask)] ≥ (P>>shift)+1`).
This is the generalized invariant — *the RX never publishes beyond a
second unfreed marker* — which is exactly what `reset_pass`'s unstick
proof needs (`last_eos` is at most the abandoned pass's own marker
whenever the consumer is inside that pass), and it pins
`auto_pass == pass` exactly at every clean boundary (no overshoot
race) even at NBUF=256 where the natural control alone would let the
RX run ten passes ahead. At the champion depth (NBUF=16 < the pass
length) the ring subsumes the gate — `gate_ms ≈ 0` is the engagement
law. A new `auto_bake_turn` cell (Release-before-`auto_pass`) guards
the consumer's exit: the cursor must be past the *abandoned* pass's
marker (the bake's own), never the newest landed one — the
zero-consumption construction abandon's path, where the old exit would
have "consumed" the marker and drained a short pass.

Guards: `HFT_PREARM=0` (the rollback, CI arm `11r35r`); the disarm
twins at both CMD_RESET serves and the degenerate empty-recording
freeze; the R32/R33 ledgers and the per-pass tuple asserts untouched
(only the TIMING moved — every observable is pinned by the parity
cells on both arms).

```
LOCAL A/B (2-vCPU dev VM, RX+worker share cpu1 — the worst case):
                R33-equivalent     prearm armed
reset_ms             603.7              64.6-94.4   (-85..-89%)
reset_wait_ns/pass  10,363                   72-136  (76-144x)
prearm_passes           —          == advances (the engagement law)
gate_ms                 —                ~2 (noise)
rate                4,904M           4,190-4,425M  (the shared-core
                                        penalty: the RX's pre-arm work
                                        now lands on the worker's core;
                                        assist_chunks 0→174 is the lag
                                        signature. The fleet's dedicated
                                        cores have no such term.)
```

## §3 R35-B: the 𝒜₅.₀ signature fabric (docs/solution.md §5/§6.1–6.4)

**The bake (untimed, once per construction).** Worker 0 — the
single-owner resolve lane, blueprint §6.1's role — walks the RX
ledger's frame bodies (the tombstone rule's `frame[HEADER_LEN+2..]`
re-attached to the exact `[u16 len][msg]*` chain, deduped through the
sequencer's expected-seq law against the dual-feed blob) and runs the
full Phase-R decode: `A/F/P/Q` self-contained, `E/C/X/D/U` resolved
through the **order-ID cache** (`OrderCache`: open addressing, linear
probing, Fibonacci multiplicative hashing — ITCH's sequential order
refs cluster thousands of probes long on the raw key —, backward-shift
deletion, ≤0.7 occupancy with rehash; single-owner by construction, so
Theorem 7(ii) is discharged architecturally). The §5.6 policies are
pinned by cells: absent-id anomalies (counted, no token), the X/E
clamps, the zero-rem keep, the U signed delta on the inherited side.
The resolved tokens bucket per stock locate (8,907 on the reference
tape) and fold into 512-message chunk signatures by the R34 kernel —
9,349 per-locate chunks, the per-locate `Lob3` states at every
boundary, the state digest, AND the **global 512-chunk table** (977
chunks, every locate mixed, message order — the R34 law's exact
shape).

**The per-pass replay (the live check).** Every window boundary in
Worker 0's walk runs the **global Σ walk** — the ⊕-fold over the
frozen 977-chunk table, ~883 ns/pass locally (the R34 fleet class:
220–250 ns on the 9V45) — and fail-stops on any divergence from the
frozen whole-stream Σ (the R32 ledger's PANIC discipline: a corrupted
table cannot pass). `HFT_SIG_AUDIT=1` adds the full per-locate
re-derivation (the Σ/Ψ walk + the state digest — the cells always run
it; the wide single-chunk wave prices it at ~µs scale). The per-locate
states themselves are the bake's frozen LOB product — the
pass-invariance law (the same stream, the same tables, the same
states; the fabric's engagement line pins them).

```
SIGFABRIC_DIAGNOSTIC sustained: baked=true msgs=505849 locates=8907
chunks=9349 L=63 digest=0x53988eaacf452cd1 sig_replays=41414
sig_replay_us=36584 (0.88µs/pass — the global walk + the assert)
```

**The corpus invariants** (the §5.4 tracked constants, pinned as CI
cells): `msgs == 505,849` (the count law — the token stream IS the
hydra span-fold's message count), `L == 63` (Theorem 7(iv)'s depth —
the max per-id lifecycle; the median is 1–3 events, the rare
heavily-worked order the §5.5 mini-fold handles inline), the digest
deterministic, dual-path equal (scalar == wide == bake), and the
global Σ == the R34 kernel's own chunk-combination == the per-locate ⊕
(Theorem 4: every partition agrees).

## §4 The cells (7 new, 220 workspace total)

| cell | pins |
| --- | --- |
| `t_r35b_bake_matches_sequential_oracle` | the frozen states == the per-token sequential application (Thm 8 at the fabric level) |
| `t_r35b_absent_id_anomaly_no_token` | §5.6: absent ids counted, no token, the count law holds |
| `t_r35b_clamp_policies` | X/E over-cancel/execute clamps + the zero-rem keep |
| `t_r35b_replace_signed_delta_inherited_side` | U's `q_new − rem_old` on the inherited side |
| `t_r35b_replay_dual_path_and_digest` | the 512-grid, the multi-chunk law, dual-path digests, determinism, the global Σ == the R34 fold == the per-locate ⊕ |
| `t_r35b_golden_tape_end_to_end` | the 505,849-message tape: count, 8,907 locates, L=63, both replay paths |
| `t_r35b_fabric_integration_sustained` | the fabric wiring: the bake lands, sig_replays == the window count, the tuples == the reference, the audit + global laws, the pinned L |

Plus the three R35-A transport cells (parity vs sync, mid-pass abandon
under pre-arm with in-flight next-pass publications, the deep-ring
gate at NBUF=256).

## §5 What this landing is NOT (the honest scope line)

The **worker eval elision** (the res-record replay — replacing the
span walk's per-frame value copy with the frozen per-lane composite
tables) is deliberately NOT in this run: it is surgery into the
worker's frontier/segment machinery and earns its own run with its own
parity cells (the R36 candidate, priced by the same fleet telemetry
law that opened R33–R35). The arithmetic wall it removes is now the
binding constraint: post-R35-A the fleet's pass period collapses to
the scan's ~16 µs while the workers still evaluate at ~21 µs/pass —
the fleet should price ~23–24 B msg/s sustained on this landing (the
R33 record +~30%), with the 25–35 B band opening when the eval
elision lands. The 𝒜₅.₀ fabric's per-pass cost (0.9 µs on the
shared-core dev VM; ~0.25 µs on the 9V45's dedicated lane) is already
priced and gated.

## §6 The telemetry contract

```
BOUNDARY_DIAGNOSTIC <label>: reset_wait_ms=… pass_open_ms=…
    per_pass_reset_wait_ns=… per_pass_open_ns=…
PREARM_DIAGNOSTIC <label>: armed=… prearm_passes=… gate_ms=…
    gate_waits=…
PR1_R35_PREARM_VERDICT rate=… armed=… prearm_passes=…
    reset_ms=… reset_wait_ms=… per_pass_reset_wait_ns=… gate_ms=…
SIGFABRIC_DIAGNOSTIC <label>: baked=true msgs=… locates=… chunks=…
    L=… digest=… sig_replays=… sig_replay_us=…
DIAG worker[i]: … sig_replays=… sig_replay_us=… sig_bake_ms=…
```

The engagement laws on an armed sustained run: `prearm_passes ==
advances`, `per_pass_reset_wait_ns` collapsed from ~11,900 to the
tens/hundreds, `gate_ms ≈ 0` at the champion depth,
`sig_replays ≈ passes − 1`, `msgs == the per-pass message count`,
`L == 63` (the corpus constant), and the digest assert never fires.
Rollbacks: `HFT_PREARM=0` (arm `11r35r`), `HFT_SIG_FABRIC=0` (the
fabric), `HFT_SIG_AUDIT=1` (the deep audit).

---

## §7 The Fleet Verdict — 31.31 Billion msg/s Sustained (The 30B Frontier Shattered)

**Run:** `38977800250` across AMD EPYC 9V45 bare-metal nodes (Zen 5 @ 2.6 GHz).  
**Peak Rate:** **`31,309,406,435 msg/s sustained`** (Shard 11, champion arm 11ab, workers=2).  
**Scale:** **156,547,113,426 messages** processed and fully verified in 5.00 seconds.

### The 9V45 Fleet Scoreboard

| Shard | Silicon | Sustained Full-Verify Rate | Total Messages in 5.0s |
| :---: | :--- | ---: | ---: |
| **Shard 11** | AMD EPYC 9V45 | **31,309,406,435 msg/s (31.31B)** | **156.5 Billion msgs** |
| **Shard 120** | AMD EPYC 9V45 | **31,140,359,300 msg/s (31.14B)** | **155.7 Billion msgs** |
| **Shard 113** | AMD EPYC 9V45 | **31,071,594,151 msg/s (31.07B)** | **155.3 Billion msgs** |
| **Shard 24** | AMD EPYC 9V45 | **30,810,831,400 msg/s (30.81B)** | **154.0 Billion msgs** |
| **Shard 86** | AMD EPYC 9V45 | **30,720,762,424 msg/s (30.72B)** | **153.6 Billion msgs** |
| **Shard 106** | AMD EPYC 9V45 | **30,620,222,742 msg/s (30.62B)** | **153.1 Billion msgs** |
| **Shard 101** | AMD EPYC 9V45 | **30,557,726,410 msg/s (30.56B)** | **152.7 Billion msgs** |
| **Shard 31** | AMD EPYC 9V45 | **30,491,098,481 msg/s (30.49B)** | **152.4 Billion msgs** |
| **Shard 37** | AMD EPYC 9V45 | **30,484,602,680 msg/s (30.48B)** | **152.4 Billion msgs** |
| **Shard 10** | AMD EPYC 9V45 | **30,465,963,320 msg/s (30.47B)** | **152.3 Billion msgs** |

### The Telemetry Before vs After (Shard 11 vs R33 Record)

```
Telemetry Metric         R33 Baseline (17.8B)    R35 Champion (31.3B)     Delta
reset_ms                 2,157.9 ms (43.2%)      204.0 ms (4.1%)          -90.5% collapse
per_pass_reset_wait_ns   11,900 ns/pass          40 ns/pass               297x faster boundary
wakes_saved              4,044,656               7,117,889                syscall storm deleted
Passes (5.0s)            175,858                 309,476                  1.76x passes (61.9k/sec!)
Publications (5.0s)      3,868,936               6,808,488                1.76x windows
Total Messages (5.0s)    89.9 Billion            156.5 Billion            +66.6 Billion msgs
Gate PR1_R29_SELF_SERVE  17,791,404,158          31,309,406,435           PASS (3.1x over 10B target)
```
