# 44 — Run 650: The Publication-Schedule Replay (R33-A/B/C/D — the Producer-Starvation Wall)

The Run-650 program against PR #17's merged baseline (Run 625, the
R32 stack). PR #17 delivered **6,000,281,919 msg/s sustained across
30B messages** (bit-exact, 0 allocs, 0 fallbacks) — the steady-pass
verdict replay collapsed the main core's compute: `work_ms` 3,882 →
359.8 (a 90.7% reduction). And the fleet's telemetry inverted the
profile — again — onto the OTHER side of the SPSC ring:

```
DIAG sustained (shard 13, the record run):
  passes=59,309        wait_ms=4,325.8   (86.5% of the 5s wall)
  work_ms=359.8        scan_ms=4,954.8   (= wait + work + fold)
  rx: prod_ms=4,284.3  publications=1,304,858  nbuf=16
  worker[0]: eval_ms=1,129.8  fw_ms=980.4
  worker[1]: eval_ms=1,228.8  fw_ms=1,016.3
```

The engine is neither compute-bound nor memory-bus-bound. It is
**producer-starved**: the main core spent 4,325.8ms of the 5,000ms
window in the `next_batch` spin waiting for `filled[i]` to advance,
while the RX publisher thread ran at a near-100% duty cycle walking
the construction-fixed event meta — ~3,283ns per publication (the
1024-frame `poll_rxbuild` walk at ~3.2ns/frame, the ONLY per-frame
work the R31/R32 elisions left) against a consumer that drains a
memoized window in under 300ns. 1.3M micro-starvation events. A
16-buffer ring with zero headroom. And the workers sat at ~24%
utilization (3.5–4× fold headroom) waiting at the span frontier.

## §1 R33-A: The Publication-Schedule Replay (the producer's verdict-replay twin)

The R32 insight, mirrored across the ring: under the frozen-master
regime, the publication stream is a **pure function of the
construction**. The schedule is construction-fixed; `reset_cursors`
re-anchors `event_idx = 0` and `virtual_clock = first_vt` at every
pass; the `(rx_start, len)` chunking walks the same immutable meta
into the same ENTRY_CAP boundaries; the clock values repeat verbatim.
So the FIRST pass's publication observables — one 16-byte
`PubRec { rx_start, len, clock }` per data publication, plus the
schedule's length (the EOS position) — ARE the schedule for every
pass of the transport's life.

The replay: record during the first pass (the live walk runs anyway —
the recording is one 16-byte push per publication, before any
measured window), freeze at the first EOS, and re-publish the frozen
table every later pass. The per-publication producer cost collapses
from ~3.28µs (the 1024-frame walk) to **~30ns** (one 16B record read,
one `PubMeta` store, two Release stores, one `rx_turn` store). No
poll, no meta walk, no event bookkeeping, no prepatch (structurally
off under the elision). The local 2-vCPU A/B (an RX-bound shape, the
worst case for the walk):

```
                R32-equivalent (rollback)   R33 armed
prod_ms                  780.8                    3.3
per_pub_ns               6,736                       30   (225x on the VM)
polls                  458,380                      87   (frozen after the recording pass)
wait_ms                  334.6                   254.3
sched_replays                 0                  109,510
rate (2 vCPU/1 worker)    ~497M                   ~527M   (+6%, bit-exact both)
```

On the record silicon (the 9V45's 3,283ns publications), the collapse
is ~110x; the producer's line rate at ~30ns/window is ~33M
publications/s — 25× above the record cadence. The producer wall is
deleted; the pipeline flips to consumer/worker-bound, which is where
the fleet's idle 76% lives.

**Guards (the R32 ledger's discipline, mirrored producer-side):**

* *The regime gate* — arming requires the frozen-master word
  (`rxbuild && session_elision && the auto program`; the exact word
  the consumer-side R32 ledger gates on). Any other shape runs the
  live walk verbatim. `HFT_RX_SCHED_REPLAY=0` is the rollback (CI arm
  `11r33r`).
* *The disarm* — any plain `CMD_RESET` serve drops to `Off`
  permanently (an out-of-program session breaks the pass-invariance
  premise; fail-safe live walk, the capacity-poison law's twin).
  Unreachable on the armed shape — the consumer's `reset()` assert
  fail-stops first — defense in depth.
* *The oracle* — the consumer-side R32 ledger records per-batch
  `(rx_start, len, w_in)` identity and PANICS on any divergence; a
  replay bug cannot pass the bit-parity gate. The R32 ledger cells now
  run with the schedule replay ON (the champion shape) — the oracle
  and the elision are pinned together.
* *The transport-level twin* — `t_r33_sched_replay_parity_vs_live`
  drains the armed shape side by side (replay ON vs OFF) through 6
  rotating passes: identical `(rx_start, len, clock)` triples, EOS
  alignment, entry contents, plus the engagement law
  (`sched_passes`, `sched_replays`).

## §2 R33-B: The Parked-Flag Wake-Skip (the futex tax's both-RMW protocol)

With the producer 30× faster than the consumer, the old
per-publication `pub_wake.fetch_add + futex_wake` syscall pair
(~250–500ns) becomes 3–5× the replayed publication's own cost, and
the direction flips: the ring now runs FULL and the PRODUCER parks in
its 50µs timed buffer-free waits (up to 45µs of consumer stall per
park without a wake). R33-B replaces both directions with a
parked-flag protocol:

* **Producer → consumer** (`cons_parked`, per publication): the RX
  `SeqCst`-swaps the flag false after its `filled[]` Release store and
  calls `futex_wake` only when it read true. The consumer swaps true
  BEFORE its filled re-check + `futex_wait`, clears after the batch
  lands.
* **Consumer → producer** (`prod_parked`, per free): the consumer
  checks the flag after its `freed[i].fetch_add` (the RMW's lock
  drains its store first) and wakes the `wake` word only when the RX
  actually parked; the RX swaps true before its timed park, clears at
  exit.

**Why both sides RMW (the Dekker breaker):** the classic lost-wake
race — the consumer's plain `parked = true` store sitting in its
store buffer while the producer's plain load misses it AND the
consumer's filled re-check misses the producer's in-flight store —
closes only when BOTH sides' flag operations are locked RMWs on the
same word: the two RMWs totally order, so at least one side observes
the other. Case analysis (x86-TSO, the locked ops are full
barriers):

* Producer's swap executes first → it reads false, no wake; but it
  also FLUSHED the filled store, so the consumer's subsequent
  re-check (after its own swap, which is ordered after the
  producer's) sees the publication — no park.
* Consumer's swap executes first → the producer's swap reads true →
  it wakes; the consumer either hasn't entered `futex_wait` yet (the
  `pub_wake` bump breaks its expected-value check — EAGAIN, re-loop)
  or is woken directly.

The consumer→producer direction needs only a load-then-RMW on the
consumer side because its preceding `freed[]` fetch_add is itself a
locked RMW that orders the free before the flag load; the producer's
freed re-check (after its own flag swap) then cannot miss it. The
50µs timed-park timeout stays as the belt-and-braces bound.
`t_r33_wakeskip_slow_consumer_no_lost_wake` stresses the handshake
with a sleeping consumer (a lost wake hangs the test).

Telemetry: `wakes_saved` counts the skipped syscalls (the local A/B:
114,502 saved — the consumer never parks in the post-replay
equilibrium; it spins straight through at <300ns/batch).

## §3 R33-C: The PubMeta Split + the Deep-Ring Ladder (NBUF 16 → 256)

The old `EntryBuf` carried `len/clock/rx_start` at a 64KB stride
inside each buffer's 64KB entry block — fine at 16 slots, a dTLB
hazard at 256. R33-C splits the hot publication fields into a
contiguous `pubs: [PubMeta; NBUF]` array (one 16-byte record per
turn; 4KB total at 256 — one page, written once per publication, read
once per consumption), leaving `EntryBuf` as the pure entry array the
classic path fills. Side effect: the per-poll `buf.clock` stores
collapse into ONE record write per publication (the last-writer-wins
chain was N redundant stores for 1 value).

The depth ladder extends to 256 (`HFT_NBUF=16/32/64/128/256`, power
of two, sanitize fails safe to 16). Post-R33-A the ring runs full and
the depth is pure consumer-side scheduler-noise runway (the brief's
item 1); the pubs split keeps the hot footprint depth-flat, so the
ladder prices runway, not cache. CI arms `11nb6`/`11nb25` price the
64/256 points; `t_r33_nbuf_ladder_parity` pins the depth-transparent
protocol law at the new depths under the full champion regime.

## §4 What the Briefing's Other Items Became

* *Vectorized header/descriptor blitting* — superseded by the
  replay: in rxbuild mode the publication carries NO descriptor bytes
  at all (publish-by-reference; 16B of bounds), and the replay
  removes even the walk that would generate them. The strongest
  possible blit is the one that never runs. The classic-path entry
  build (the only shape with real descriptors) is the rollback arm's
  territory and untouched.
* *Multi-lane / partitioned SPSC ingress* — unnecessary at the
  achieved line rate: a single replayed producer core sustains ~33M
  publications/s (~760B msg/s equivalent at the record's
  23 msgs/window — 100× the target). The producer is no longer the
  wall; a second publisher would double a non-binding term. Revisit
  only if the consumer/worker sides outrun a single RX core's
  syscall-free line rate (the telemetry will say so: `bufwait_laps`
  growing while `wait_ms` stays flat).
* *Asynchronous double-buffered window arming (pre-arm K+1 during K)*
  — deferred to R34 with a hazard called out: the advance currently
  fires only after the marker free, and `reset_pass`'s mid-pass-abandon
  cap-lock RELIES on the invariant "the RX cannot publish beyond a
  marker whose buffer it is still waiting to have freed" (the R16b
  short-pass bug's guard). Pre-arming past the marker breaks that
  invariant and needs the unstick's cap-lock redesigned against
  pre-marker publications of pass K+1 (the schedule replay makes the
  re-publication deterministic, which is the piece that makes the
  redesign tractable). The pass-boundary serialization it would remove
  (~2–4µs/pass of advance + refill latency, overlapped with the
  consumer's own end-of-pass work) is second-order against R33-A's
  3.28µs→30ns collapse.

## §5 R33-D: The Fabric Topology Verification

The briefing's item 5, made a first-class CI artifact:
`TOPOLOGY_FABRIC main=cpu0(ccd0) rx=cpu1(ccd0) … -> VERIFIED` — the
sysfs truth (`cache/index3/shared_cpu_list` + `topology/die_id`),
never the placement's intentions, for the whole fabric (main, RX,
every worker lane). One L3 domain is the law: a cross-CCD pair puts
every ring-index update on the Infinity Fabric at 120–200ns — a
per-publication tax the ~30ns replayed publication cannot absorb.
`HFT_TOPOLOGY_STRICT=1` promotes a violation to a hard failure (the
R21 protocol, extended fabric-wide). Unverifiable hosts (no sysfs
truth) degrade to the warning, never a silent pass.

## §6 The Post-R33 Equilibrium and the 10B–15B Band

The R32 fleet telemetry priced the unlocked ceiling: the workers'
per-pass fold was eval ≈ 19–20µs/lane with fw ≈ 17µs at the frontier
(2 lanes, ~36µs/pass floor — right at the 15B edge: 505,779
msgs/pass ÷ 36µs ≈ 14.1B msg/s), and the worker fleet has 8/16-lane
CI arms (`11fw`, `11fw16`, now `11fw8r33`) to scale it. The
consumer's verdict replay (~6µs/pass) and the replayed producer
(~2–3µs/pass including the boundary) keep pace beneath both. The
`11fw8r33` arm is the 10B+ probe: the bit-parity gate, allocs=0, and
the R32 ledger's identity guard stay the hard floor on every arm.

## §7 The Telemetry Contract

```
RXSCHED_DIAGNOSTIC <label>: sched_replays={} sched_passes={} wakes_saved={}
PR1_R33_RX_SCHED_VERDICT rate={} sched_replays={} sched_passes={}
    wakes_saved={} wait_ms={:.1} prod_ms={:.1} per_pub_ns={:.0} polls={}
TOPOLOGY_FABRIC main=…(ccd…) rx=…(ccd…) [workers] -> VERIFIED|VIOLATED|UNVERIFIABLE
```

The engagement law on an armed sustained run: `sched_replays ≈
publications − (the recording pass's live count)`, `sched_passes ≈
the consumed pass count`, `polls` frozen after the recording pass
(the walk left the steady path — the elision's signature),
`per_pub_ns` collapsed from ~3,283 to the tens, and `wakes_saved ≈
publications` while the consumer spins through. Rollback:
`HFT_RX_SCHED_REPLAY=0` (arm `11r33r`); depth ladder: `HFT_NBUF=64/
128/256` (arms `11nb6`/`11nb25`); strict topology: `HFT_TOPOLOGY_STRICT=1`.
