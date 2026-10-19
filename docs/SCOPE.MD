# SCOPE.MD — The Vulnerability, Bottleneck & Horizon Audit

**Branch:** `feat/run900-r37-solution2-corridor` (PR #22) | **HEAD audited:** `6ce1300`
**Audit surface:** `crates/nf-testkit/src/hydra.rs` (10,686 ln), `crates/nf-testkit/src/sigpush.rs` (697 ln),
`crates/nf-transport/src/pipeline.rs` (4,997 ln), `crates/nf-transport/src/rxdesc.rs` (1,563 ln),
`crates/nf-testkit/src/sigfabric.rs` (1,942 ln) — ~20k lines, first-hand.
**Evidence base:** CI runs 39201557063 (the Shard-28 panic), 39216129485 (the 73.72B record + R39a fleet
verdict), 39219756204 (41/41 green, all-discard census draw); the fleet census (docs/SILLICON_DATA.md,
N=2,000); the R40 blueprint (docs/52); worklog Tasks 18–20.

**Mission:** calibrate the next scientific breakthroughs — (1) close the residual vulnerability
surface the Shard-28 family exposed, (2) price the hardware bottlenecks beyond the 200B corridor,
(3) specify the mathematics that scales single-tape ingestion (200B) to multi-exchange distributed
ingestion (1T), (4) define the live chaos-stream harness that measures true production P99.

---

## §0 Executive verdict

The engine's verification chain is **fail-loud by design and fleet-proven**: the fold-order asserts,
the per-pass tuple oracle, the allocs=0 gate, and the three R39a guard laws (zero fires on 39
target-silicon draws) make the *wrong-answer* surface structurally narrow. The audit's honest
verdict on the residual risk, ranked by the failure class that matters for a verification engine:

| Class | Count | The faces |
|---|---|---|
| **Silent wrong answer** (hash divergence, no crash) | **1 confirmed** | A-2: stale `lane.base` across sinks (hydra.rs:6272) — wrong-memory CRC with *correct* order, the one combination the assert lattice cannot catch |
| **Crash (fail-stop)** — reachable | 3 | A-1 `RESLOG_PHASES` OOB at n_lanes ≥ 9 (hydra.rs:443); F-4 44-bit `base_span` truncation at sustained rates (rxdesc.rs:526); F-6 `attach_ledger` OOB on fabric reuse (rxdesc.rs:1469) |
| **Deadlock / hang** (fail-stop converts to stall) | 4 | F-1 double-empty EOS publication; F-2 marker-wait ignores `CMD_RESET`; F-3 unbounded consumer futex parks; D-2 worker panic → `finish()` spin forever |
| **Release-silent width truncation** (latent at 200B+) | 5 | F-7 `rx_start` u32; F-8 `delta ≤ u32::MAX`; A-3 Desc8 off/len; C-2 `LaneMod` domain; C-4 locate sentinel |
| **Hygiene / trap** | 4 | A-4 `R29DBG` eprintln in the release hot path (hydra.rs:5992); B-3 dead `phase_bc`; B-4 epoch-tag wrap false positive; the dangling `t_r39_lane_mod_exactness` outside `mod tests` (hydra.rs:10657) |

The Shard-28 family is **closed at both ends and verified at scale** — but its *cause class* (the
record-adoption interleavings of the two-lane shape, ~2.5%/shard on the fleet) still walks among us:
the R39a guards are containment (telemetry-counter + bit-identical extra boundary), not elimination.
The 200B corridor's dual-producer machinery will *multiply* the interleaving space (two producers, one
boundary rendezvous, an election word). **The single highest-leverage hardening law for R40: every
fail-stop must fail LOUD and BOUNDED — a panic today is a hang tomorrow** (§2, D-2).

---

## §1 The Shard-28 postmortem, verified — and what remains of its family

### 1.1 The anatomy (run 39201557063, hydra.rs:345, fixed in `099d0d2`)

A FULL `SegAcc` buffer (`len == 64`) at a group push: `push8` slices `buf[64..]` — an EMPTY slice —
and `b[0]` panics (`index out of bounds: the len is 0 but the index is 0`). The scalar `push` face is
the same family with no internal guard at all (`buf[len]` at `len == 64` is a naked OOB). The census
family (docs/49 §8): the chunk cursors desync behind `gid` → the chunk-end flush silently DISABLES
(`gid` never equals `chunk_lo + CHUNK` again) → the accumulator grows past its 64-span law.

### 1.2 The three-layer closure, code-verified

- **LAYER 1 — the capacity law, all three push sites** (hydra.rs:3217 group, :2105/:3365 scalar):
  flush-before-push when `len + 8 > 64`. The soundness argument is the R25a split law — the
  composition is exact for EVERY k, so a pre-push flush is a LEGAL extra segment boundary: the
  overflow becomes an extra record + the `seg_guards` counter, never a wrong answer.
- **LAYER 2 — the cursor containment resync** (hydra.rs:3099–3105): `eval ∈ [chunk_lo, chunk_lo +
  CHUNK]` re-derived via `fallback_grid` (hydra.rs:820) — the lane's own chunk at-or-above the
  cursor, clamped up. This restores the chunk-end flush the desync had disabled: the ROOT-CAUSE
  closure.
- **LAYER 3 — the record-window snap** (hydra.rs:3082–3087, ordered snap-then-contain): a record
  base AHEAD of the cursor would wrap `idx = eval − rec_base` to a huge `usize` and read the arrays
  OOB; the snap lands on the record's own base.

**Fleet verification:** `seg_guards = cursor_resyncs = record_snaps = 0` on every worker DIAG line
across all 39 target-silicon draws of run 39216129485 — the guards are pure no-ops on healthy
streams (two register compares), exactly the design contract.

### 1.3 What the audit adds (the family's residual faces)

1. **The D-exclusion clamp / re-anchor law is sound but its cross-batch leg is a comment, not a
   proof.** sigpush.rs:630 excludes D from the re-anchor set for the right reason: a D's transient
   `ok=2` is the kernel's signed-comparison quirk on the WRAPPED ρ (a pre-re-anchor over-cancel
   leaves the top bit set); re-anchoring a D would rewrite it with `dead=0` — **wiping the thread's
   dead terminator and resurrecting the register** (post-D references would resolve instead of
   anomaly). The termination argument is verified: `rewritten[]` is monotone and gates the violation
   push (sigpush.rs:631), so the re-anchor loop ends in ≤ rows rounds. But the closure of the
   residual — "no wrap can survive into the final round" (sigpush.rs:629) — rests on the *carry-in
   being wrap-free*, an induction over batches that is asserted in prose only. **Directive: pin it
   with a cell** (a synthetic two-batch tape: batch 0 ends mid-over-cancel, batch 1 opens with a
   D-only reference) and one `debug_assert!(carry.rho as i64 >= 0)` at the carry bank
   (sigpush.rs:644–678).
2. **The same capacity-law family lives in rxdesc.rs as PROTOCOL panics** (set_arr / set_affine_tag /
   the +8/+16 array gates, rxdesc.rs:978–1404). Correct for replay (a config error should die);
   **the live-feed direction makes them a remote DoS** (§4.4, F-9). The family's law: capacity
   enforced by invariant, not by code — the Shard-28 lesson generalized.
3. **The walk-entry guards protect the DIET worker's choke point.** The pre-diet worker got the Layer-1
   flush (per `099d0d2`) but its walk has no Layer 2/3 equivalent — it relies on the record protocol's
   own containment. Benign today (it has no ResLog and its emission is order-guarded), but R40's
   per-lane scripts will re-route walk entries: **re-verify the choke-point coverage when the lane
   filter lands (R40a)**.

---

## §2 The deadlock & liveness audit (pipeline.rs — the two HIGH faces)

### F-1 [HIGH] Double empty publication at end-of-pass → orphaned EOS marker → `reset_pass` deadlock

**Where:** pipeline.rs:1611–1614 (the zero-poll), :1722–1738 (publication is unconditional),
:1776–1798 (the marker), :2785–2798 (the consumer's EOS), :2693–2766 (`reset_pass`).

**The geometry:** `poll() == 0` only at true end-of-stream, but the accumulate loop exits on the
CAPACITY condition with the schedule exhausted *exactly on that poll* — a pass whose frame count
lands on the publication-size boundary (e.g. 1024 frames at coalesce 256; ~1-in-769 at coalesce 1,
**deterministic per schedule geometry**). The next turn's first poll returns 0 with `acc == 0` → a
**zero-length data publication** is published at turn T, and the marker at T+1 is a *second* len-0
turn. The consumer treats ANY len-0 as the EOS (pipeline.rs:2785) — the real marker is orphaned:
- non-prearm: the RX blocks forever in the advance wait (needs the marker's own free);
  `reset_pass`'s clean-end sets `cap = turn = T+1` and frees nothing → both sides park forever.
- prearm: the exit guard `bake_turn < self.turn` is `T+1 < T+1` = false forever; the ring fills.
- **recording poison:** the len-0 turn's `PubRec` is recorded mid-table → every replayed pass
  re-publishes it → short passes + the same deadlock, forever.

**Fix law:** treat `acc == 0 && eos` as "no data publication" (let the marker BE the turn's
publication), or publish the marker with a distinct sentinel (`len == u32::MAX`) and make the
consumer's EOS path verify it consumed the *marker*. Regression cell: a 1024-frame pass at coalesce
256. **This lands BEFORE R40b — the dual-producer rendezvous reuses exactly this marker arithmetic,
and two producers double the boundary coincidences.**

### F-2 [HIGH] The EOS-marker buffer wait ignores `CMD_RESET` → mid-pass-abandon `reset()` deadlock

**Where:** pipeline.rs:1781–1798 — the wait checks `shutdown` but NOT `mb.cmd` (contrast the
data-turn wait at :1459–1461, which does). The consumer's `reset()` ack wait (pipeline.rs:2603–2608)
is an **unbounded futex park**. Scenario: the RX at maximum ring lead (the normal post-R33-A state)
enters the marker wait; the consumer abandons mid-pass and calls `reset()`; the RX never observes
the command; the consumer parks without timeout. Permanent two-thread deadlock. **Fix law:** add the
`CMD_RESET` break (mirror :1459) + bound the ack wait with a fail-stop.

### F-3 [MEDIUM] Unbounded consumer parks, no watchdog — any RX panic hangs the consumer

`wait_filled_turn` (pipeline.rs:2968–2971) and the reset ack (2603–2608) park without timeout. RX
panic paths exist (`master_mut().expect`, OOM abort in the ledger fill). A dead RX = a sleeping
consumer = a wedged verification lane = workers stalled on `spans_ready` — **and `Drop` never runs
because the stuck thread owns the transport.** Fix law: timed parks with bounded escalation
(1 s → fail-stop diagnostic) or a liveness heartbeat the consumer checks between parks.

### D-2 [MEDIUM-HIGH] Worker panic converts every fail-stop into a main-thread hang (hydra.rs)

`finish()`'s drain (hydra.rs:6741–6747) spins on `fold_pos < next_span` with **no shutdown check and
no timeout**; `Drop for HydraFabric` joins with `let _ = h.join()` (hydra.rs:3899–3901) — worker
panics are swallowed. Any worker-side release assert (replay order :2906, the A-1 OOB, affine
integrity :1467) leaves its lane's `res_head` frozen → `fold_available` parks → `finish()` spins
forever. Same for the sink-side pace spins (:5420, :5614, :5988) — progress-driven, deadlock-free,
but no timeout and no death-awareness. **Fix law: a fabric-level "worker died" flag (panic hook),
checked in `finish()` and the pace gates. This is the difference between a red CI shard and a
35-minute timeout burning runner minutes.**

---

## §3 The ring head/tail race audit (SMT contention)

### 3.1 The inventory (all cross-thread surfaces, verified)

| Ring | File:lines | Protocol | Verdict |
|---|---|---|---|
| Mailbox `filled[]` / `freed[]` | pipeline.rs:440/:444 | SPSC, per-slot Release/Acquire, exact-turn match, pow-2 mask enforced | **Sound**; `Pad` (align 64) isolates cursors |
| `rx_turn` cursor | pipeline.rs:495 | monotone u64, Rel/Acq | Sound |
| rxdesc `records[64]` | rxdesc.rs:610, :861–863 → :927 | Rel/Acq, `next_slot % 64`, sink-side watermark reuse gate | Sound |
| rxdesc `ready` cursor | rxdesc.rs:613, :936 → :942 | gen«48 \| spans⁴⁸, Rel/Acq | Sound — **but see F-4 (width)** |
| `chunk_state[2^15]` | rxdesc.rs:632, :1247–1261 | value = chunk_id+1 (ABA-proof by value), ordered by the ready chain | Sound **only under the unenforced live-lead bound** (~1.4k chunks) |
| Epoch slots (`bc_*`) | hydra.rs:1015–1028 | data-Relaxed / tag-Release + tag-Acquire | Sound (the fleet-proven Theorem-25 surface) |
| Affine ledger | rxdesc.rs:652–657, :1507 → :1518 | write-once + one Release (`ledger_len`) | Sound **except attach-replacement (F-6)** |
| Dekker parked-flag pair (`cons_parked`/`prod_parked`) | pipeline.rs:891–916 | total SeqCst RMW | **Verified sound under both interleavings** — no lost-wake window |

No torn cursors exist (all `AtomicU64`, cacheline-padded). The R37b/R38 ordering fixes (stale-ready
refresh, shutdown escape, the cap-lock load/store order at pipeline.rs:1897–1898 vs :2743–2744) are
present and correct. **The SPSC cores are not where the risk lives.**

### 3.2 The four races that remain

1. **F-5 [MEDIUM] `master_entries()` reads the master `UnsafeCell` unsynchronized**
   (pipeline.rs:3047–3057) — the RX patches it concurrently (master_patch_range, :731–769); a
   mid-pass caller gets torn `FrameEntry` reads (UB by the Rust memory model). Fix: gate behind the
   pass boundary or snapshot under `master_version`.
2. **F-6 [MEDIUM] `attach_ledger` replaces the ledger Box while `ledger_len` keeps a stale value**
   (rxdesc.rs:1469–1488): on fabric reuse, `ledger_lookup`'s walk (1550) indexes the NEW shorter
   array past its end → panic, and the Box replacement frees the old allocation under a potentially
   live raw reference (latent UAF). Fix: `ledger_len.store(0, Release)` BEFORE the replacement.
3. **F-11 [LOW] `last_slot` Relaxed** (rxdesc.rs:618, :864): safe only under the sequential
   single-live-sink contract — which is documented, not enforced. Two live sinks derive the same
   record slot and interleave writes. Fix: Acquire/Release, or a debug token.
4. **F-12 [LOW] `chunk_state` live-lead > 2^15 unenforced** (rxdesc.rs:92–94): past the lead bound,
   `mark_inline(c')` aliases in-flight `c ≡ c'` → double-eval → fold divergence. Fix: a lead
   watermark assert at `mark_inline`.

### 3.3 The SMT-contention geometry under Champion-4T (what R40b must respect)

The audit confirms the blueprint's coherence dividend (each producer's ring writes become
L1-local on its own SMT pair) and adds three contention laws the split must carry:

1. **The wake budget**: the mailbox's `pub_wake`/`wake` futex pair is single-waker
   (`futex_wake(1)`). Two producers must NOT share one wake word — wake-one between two parked
   producers drops wakes. Per-producer wake words, or a wake-all budget.
2. **The futex ABA face** (pipeline.rs:599–609, low-32 compare): theoretical at one producer
   (~2³² bumps); two producers halve the calendar. Note it in the R40b review; no action.
3. **False sharing on `pubs[]`** (pipeline.rs:417 — 16 B stride, four per line): the RX writes
   `pubs[i+1]` while the consumer reads `pubs[i]`. Perf only — but it sits on THE hottest handoff
   surface; pad to 32/64 B when the ring deepens under dual producers.

---

## §4 The malformed-packet & hash-divergence audit

### 4.1 The parsing boundary (where the attacker actually is)

pipeline.rs and rxdesc.rs consume **construction-validated indices**: the raw corpus walk
(render.rs:1672–1687) bounds-checks every message length/count at bake time and drops malformed
events. **No attacker-reachable length/count parsing exists in the audited transport files** — the
sequence-number arithmetic is the sequencer's (out of audit scope), and the session-mismatch guards
fail-stop by contract assert. The reachable surfaces are: capacity (F-9), width truncation (F-4/F-7/
F-8/A-3), and protocol reuse (A-2/F-6). **The threat model changes the day `--random-stream` (§7)
or a live socket feeds the engine — then the construction-time validation wall moves to runtime, and
every one of §4.2's silent faces becomes an exploit path.**

### 4.2 The silent-wrong-answer faces (ranked)

1. **A-2 [HIGH] — stale `lane.base` across sinks** (hydra.rs:6272–6275): write-once-per-lane,
   never reset (`reset()` at :4601 doesn't touch it). A second `HydraSpanSink` over a reused fabric
   with a transport blob at a different address → workers CRC **garbage or freed memory** while span
   ids stay correct → the fold-order assert PASSES → **silently wrong SPANHASH** (or UB if the old
   blob dropped). The rxdesc path re-captures per generation (:5302); the ring path has no
   equivalent. This is the ONLY silent face found in the fold core. **Fix: store `blob_base` per
   generation, or `debug_assert_eq!(lane.base, blob_base)` at chunk-open.**
2. **F-4 [MEDIUM] — 44-bit `base_span` silent truncation** (rxdesc.rs:486–536): debug-only assert;
   release masks silently. At 73.72B msg/s ≈ 1 span/msg, `base_span` exceeds 2⁴⁴ after **~239 s
   sustained**; at the 200B corridor, **~88 s**. A truncated base aliases an older window's record →
   wrong-window descriptors → hash divergence between lanes (the exact class the mission briefs).
   The in-code claim "decades of headroom" is wrong arithmetic at the champion's own rate. **Fix:
   runtime assert at pack time, or widen to 48 bits (pass-bit moves into the gen half).**
3. **F-8 [MEDIUM] — `check_arr8/16_packed`'s `delta ≤ u32::MAX` is debug-only**
   (rxdesc.rs:1097, :1166): a >4 GB blob (or multi-feed span sharing one `blob_base`) lets the
   expected-word transform carry into the len field → the check-and-fix "fixes" HEALTHY descriptors
   with wrong values → verification divergence masquerading as corruption. Fix: runtime assert at
   the pack/check boundary; document the 4 GB blob contract at `capture_blob_base`.
4. **F-7 [MEDIUM] — `rx_start` u32 truncation** (pipeline.rs:1546 debug-only, :1725 cast): a
   >4G-frame pass-local index wraps in release → wrong master slice → bounds panic *after* the
   ledger identity guard may have matched a wrong record. Fix: runtime assert at :1546 and :3019.
5. **A-3 [LOW] — Desc8 offset/len truncation debug-only** (hydra.rs:5338–5344 + 5 more sites): a
   >4 GiB blob or >64 KiB span body silently truncates → workers CRC the wrong bytes. Today's ~15 MB
   margin holds; the assert doesn't.
6. **F-10 [LOW] — `select_group_batch(j)` silent no-op for j ≥ group_n** (pipeline.rs:2894–2903):
   release leaves `cur` at the last member → double-submit → divergence, caught only by the
   end-of-pass tuple assert. Fix: explicit error / clamp-and-count.
7. **B-4 [LOW] — the sheaf epoch-tag wrap** (hydra.rs:4886 with `bc_epoch` init `u64::MAX`,
   :1004): an unset lane masquerades as "epoch −1" → a false-positive degree-law pass; the
   intended "nothing published yet" branch is dead code. Telemetry-only today — a trap if upgraded
   to an assert. Fix: treat `e == u64::MAX` as unset before the match.
8. **C-2/C-4 [LOW] — `LaneMod`'s G–M domain `u < 2³²` debug-only** (hydra.rs:178) and sigfabric's
   locate-sentinel saturation (sigfabric.rs:648–673): six orders of margin at 200B; pin with
   release asserts or an `u % n` fallback past 2³² grid positions.

### 4.3 The crash faces (fail-stop, but see D-2 — a panic is a hang)

- **A-1 [HIGH] `RESLOG_PHASES` OOB at n_lanes ≥ 9** (hydra.rs:443 vs :544/:701/:731): the phase
  key `rb % (n_lanes × CHUNK)` indexes a 512-slot array — the comment *documents* the law ("512
  covers eight lanes") but `HFT_SUSTAINED_WORKERS=9..16` + `HFT_RES_REPLAY=1` panics on the first
  pass boundary. The codebase's own CI arms reference 8/16-lane shapes, and `t_r39_lane_mod_exactness`
  tests 9/16/32 lanes — for `LaneMod`, never for `ResLog`. **Fix: `assert!(n_lanes * CHUNK <=
  RESLOG_PHASES)` in `ResLog::new` (or clamp+poison → permanent live-walk), plus a cell at 9/16.**
- **F-9 [MEDIUM] the rxdesc capacity fail-stops** (rxdesc.rs:978–1404): keep for replay; add an
  overflow-counter + truncation policy for any live ingestion (§7).

### 4.4 The D-exclusion residual (sigpush.rs, the clamp's last leg)

Verified sound: the exclusion set (:630), the resurrection hazard if violated (:618–629), the loop's
termination (rewritten-monotone). The residual: the "no wrap survives into the final round" argument
depends on wrap-free carry-in across batches — currently prose (§1.3.1). One more audit note: the
carry update's `None` arm banks `Reg { dead: true }` with **zeroed** s_b/π/ρ (:659) — a phantom-u_new
birth propagates as an absent register, matching the pull path's probe-miss. Correct; pin it in the
parity cell when the carry-discipline cell lands.

---

## §5 The systems engineering roadmap (post-200B)

### 5.1 Where the wall stands (the audited numbers)

The fleet record: **73.72B msg/s (368.6B msgs / 5.00 s)**, run 39216129485, workers=2, sweep armed
at mean_width 5.11 — after the R39 sweep (+6.85B same-draw over the k=1 rollback). The system is
**MAIN-BOUND** (the R25b law re-confirmed): one Zen 5 logical core running consume → guard →
re-issue → drain → boundary prices at 63–70B no matter how cheap the workers get (99.998% replay,
~4% duty). The champion's per-pass budget: 8.58 µs = 2.16 wait + 1.92 work + 3.21 fold + 1.06 instr
+ 0.24 boundary (run 39168431150 decomposition).

### 5.2 The bottleneck stack after Champion-4T (ordered by when it binds)

1. **Instruction issue on the producer pair (binds first).** The blueprint's eta: 126–140B from the
   split, 165–200B stacked with the drain diet + RX absorption. Past 200B on 2 cores: 200B ÷ 4.56 GHz
   ≈ 43.9 msg/cycle fleet-wide ≈ 22 msg/cycle/producer — at the affine O(1) steady path (~2 loads +
   8 IMULs per 8-span group ≈ 1–2 instr/span) this is 10–20× the *steady* budget, so the ceiling is
   the NON-steady path: the guards, the boundary ceremony, the journal pushes, the 6-tsc
   instrumentation. The next lever class is **amortization of the non-steady path** (run-drain of
   res records — the `Desc8` compaction pattern applied to the res side; the arriving-last election
   replacing the sequential boundary).
2. **The coherency domain, not DRAM (binds second).** The 9V45's 1 TB/s DRAM ceiling is nowhere near
   binding: the steady regime reads ZERO payload (affine register tags), and the state streams are
   Desc8 8 B/8 spans (1 B/span → 200 GB/s at 200B), res records 16 B/≤64 spans (~50 GB/s), journals
   and epoch slots negligible. What binds is **cross-core line ownership**: every descriptor line,
   res line, and wake word written by one core and read by another pays an L2-to-L2 coherence
   transfer on the die's interconnect. Champion-4T already halves this (per-SMT-pair rings); the
   post-200B law: **state must never cross a physical-core boundary except at epoch slots.** The
   audit's F-14 (pubs[] sharing one line four ways) is this law's cheapest current violation.
3. **NUMA socket scaling (binds at 400B+).** Scaling past 2 cores means either more producers on the
   round-robin chunk grid (the m-way static merge, Theorem 23(b) generalized — already exact) or
   socket-level distribution. The round-robin grid's merge order is POSITION-DERIVED and static —
   it survives sockets — but the golden tables (master, ledger, sigfabric) must shard per socket,
   and the boundary rendezvous becomes a cross-socket epoch election (the futex wake budget again).
   **The static-order merge is the serialization point: at k producers the boundary does a k-way
   position merge of ~172 records/pass — trivial per pass, but it is the ONE sequential surface
   left; the math roadmap (§6, Theorem 37) exists to dissolve exactly this.**
4. **PCIe / NIC ring saturation (binds only in LIVE mode).** Replay has no NIC. A live 200B-class
   ingest is a 200+ Gbps stream — beyond any single venue feed (ITCH ~1 Gbps) but the *multi-venue
   aggregate* of the 1T barrier approaches it. The engine's own xdp/redirect.c and the AF_XDP
   docs (docs/09) are the entry: the NIC's RX/fill ring descriptors become the LAST ring family to
   eliminate (§5.3). The audit's live-mode caveats (F-9 DoS shape, construction-time validation
   moving to runtime) all bind here.

### 5.3 The pure zero-copy kernel (eliminating ring descriptors entirely)

The engine has already eliminated three of four ring families — the trajectory is the spec:

| Ring family | Status | The elimination law |
|---|---|---|
| Per-span payload descriptors | **GONE** | The rxdesc array-plane: RX prefills 8-slot windows, the sink check-and-fixes (check_arr8_packed) — descriptors *become data* |
| Per-span fold work | **GONE** | The affine register tags: zero payload reads, O(1) CRC via the affine law |
| Per-span worker emissions | **GONE** | The composite `Res` records (k ≤ 64 spans per 16 B) + the ResLog phase scripts (99.998% replay) |
| Batch/turn handoff (mailbox) | **REMAINING** | The position protocol: the frozen schedule IS the descriptor — `try_replay` reads only `(rx_start, len)` bounds and re-issues the ledger's recorded calls |
| RX thread itself | **REMAINING** | R40c: producers self-supply from the schedule replay (the R29 self-serve shape per lane) — the zero-thread ingest endpoint |

**The terminal design:** frames stay in DMA-backed hugepages (THP 2 MB grants, already in the
stack); verification state lives in affine registers; boundary composites exchange through epoch
slots; the *only* cross-thread state is the epoch words and the `ready` cursor. No descriptor ever
crosses a cache line owned by another core. The preconditions, in order: R40b's per-lane journals
(state locality), the run-drain (amortized protocol), RX absorption (the schedule self-supply), and —
for live feeds — AF_XDP with the fill-ring prefaulted per lane. **The principal engineer's directive:
price the epoch-slot election and the run-drain on the R40c fleet arms; the zero-copy kernel is the
R42-class program the 165–200B verdict arms.**

### 5.4 The hardening directives that ride along (cheap, release-mode, this corridor)

The §2/§4 fix laws (the reset-liveness trinity, the worker-death flag, the five width asserts, the
`lane.base` generation reset, the `RESLOG_PHASES` assert) are all branch-local compares or
construction-time asserts — single-digit cycles, zero steady-state cost. **Land them WITH R40a** so
the dual-producer interleaving space is instrumented before it exists, not after the first 35-minute
timeout burn.

---

## §6 The mathematical roadmap (preparing solution3.md — the 1T barrier)

**The gap:** single-tape ingestion at 200B (2 Zen 5 cores) → multi-exchange distributed ingestion at
1T (NASDAQ + CME + crypto liquidity under light-speed delay cones). The arithmetic forces
distribution: 1T ÷ 4.56 GHz = 219 msg/cycle — beyond any single socket's issue budget; the 1T
barrier is necessarily a CLUSTER barrier. The mathematics must make distribution FREE: the fold must
compose across venues with the same bit-exactness that Theorem 22 (tree/serial equivalence) gave the
single tape. The theorem numbering continues solution2.md's T19–T32.

### 6.1 The theorem list (in dependency order — what to hand the mathematician first)

1. **Theorem 33 — the multi-venue trace monoid.** Generalize the occurrence-net / trace monoid
   (solution2 §3.2) to venue-indexed generators 𝒢 = ⨆ᵥ 𝒢ᵥ: intra-venue order is TOTAL, inter-venue
   generators commute ONLY when their delay cones are disjoint — the light-speed cone ∂C defines a
   PARTIAL commutation relation, not the equivalence relation Theorem 23 used. Prove: the fold
   algebra factors through the quotient monoid ⟺ the delay graph is acyclic (the obstruction
   vanishes). This is Theorem 23 (lane commutativity) promoted from SMT lanes to venues — the exact
   analog, at the scale where "disjoint support" is a *physical* statement. **Unblocks the 1T
   architecture; hand this over first.**
2. **Theorem 34 — the relativistic causal sheaf over delay cones.** The site is spacetime
   (t, x_venue); the cone cover's nerve is the Čech skeleton; the sheaf 𝒜 of 𝔽₂[x]/Q(x)-monoids
   assigns to each cone the local fold state. Prove the glueing law for composites across cones
   (the epoch-slot machinery generalized to venue-async boundaries) and identify the obstruction
   class in H¹(nerve, 𝒜): **H¹ ≠ 0 is exactly the "no consistent global order exists" regime —
   the arbitrage-free consistency condition as a cohomological statement.** The single-venue case
   (solution2 §3's asynchronous causal sheaf) must degenerate to H¹ = 0 — that degeneration is the
   sanity lemma.
3. **Theorem 35 — non-abelian order cancellation across venues.** The same order resting on two
   venues is a non-abelian cancellation in the transducer semiring: the affine order-transducer
   (T27–T31 over ℤ/2⁶⁴ℤ) lifts to a venue-colored tensor product ⊗ᵥ. The soundness condition: the
   push-scan's O(log B) depth (T29(b)) preserved under venue interleaving — the re-anchor law of
   sigpush.rs at venue scale, with the D-exclusion law (§1.3.1) as the k=1 degenerate case. **The
   carry-discipline cell (§1.3.1) is this theorem's first unit test.**
4. **Theorem 36 — the sub-picosecond single-tape lower bound.** Establish the information-theoretic
   floor: no sequential-equivalent single-tape fold exceeds (issue_width × f) / depth(chains) —
   formalizing WHY the 200B single-socket ceiling is structural (the justification for distribution,
   and the guard against chasing a wall that isn't there). The 0.1975 cyc/msg pure-ingest ceiling
   (docs/37) is the measured anchor; the theorem should reproduce it as a bound, not a number.
5. **Theorem 37 — the 1T composition law.** The k-way Blelloch–Galois reduce/scan (T20's
   decomposition) over the venue cover's nerve skeleton: the tree's levels execute venue-concurrently
   under the Theorem-33 commutation, and the critical path (log₂ of total spans × per-level latency)
   stays under the pass cadence. **This dissolves §5.2's last serialization point (the boundary's
   k-way merge) into the tree.** The 2-way case is already fleet-proven (the epoch-slot merge at
   564k boundaries) — the theorem generalizes the existing machinery, it does not replace it.
6. **Lemma set (the pins):** (a) the CRT phase alignment — per-venue grids with coprime periods
   generalize the ResLog orbit law (grid / gcd(T, grid), hydra.rs:399–409) to lcm-alignment across
   venues; prove bounded-log existence under coprime venue periods. (b) The wrap-free carry
   induction (§1.3.1 / §4.4) as a standalone lemma over batch boundaries. (c) The M-way law's
   boundary partials (solution2's Theorem 23(b)) restated for the venue tensor product.

### 6.2 The calibration (how to direct the Chief Mathematician)

Pose Theorem 33 and 34 as ONE coupled problem — "does the delay-cone partial commutation admit a
monoid quotient, and what is H¹ of its Čech nerve?" — with the two degenerate sanity lemmas
(single-venue → H¹ = 0; SMT lanes → Theorem 23 verbatim) as acceptance tests. The existing
sheaf-cohomology machinery in solution2 §3 is the base camp; the venue cone is the new summit.
Theorem 36 can run in parallel (pure information theory, no dependency). Theorem 35 depends on 33.
Theorem 37 closes the loop with 34. **Do not start implementation of the 1T shape before 33+34 —
the current solution3.md draft's Problem 7 is exactly this pair; the audit's finding is that the
draft is NOT on this branch (queued per the briefing), so the theorems above are the spec it should
land against.**

---

## §7 Production readiness & the live chaos stream (`--random-stream`)

### 7.1 The harness spec

**Goal:** measure TRUE production P99 tail latency under randomized, adversarial, unpredictable
market-data streams — the tail-study discipline (docs/15) applied to the sustained corridor.

**The generator (nf-testkit, seedable, deterministic):**
- PRNG: xoshiro256++ or PCG64, `--seed` pinned; every run reproducible from (seed, profile).
- Stream model knobs (the adversarial distribution suite):
  - `gap_lambda` — Poisson sequence-gap bursts (the sequencer's gap-recovery path under fire);
  - `dup_rate` — duplicate/stale re-transmissions (the dedup lattice under saturation);
  - `reorder_window W` — bounded out-of-order delivery (the reorder buffer's worst shapes);
  - `zero_len` / `max_len` frames — the length-field edge cases (F-7/F-8's runtime faces);
  - `session_flips` — session-id changes mid-stream (the reset/pass machinery);
  - `seq_wrap` — sequence-number wraparound points (RFC-1982 serial arithmetic, hydra.rs:6531);
  - `burst_profile` — bimodal burst/idle cadence (the pace gates' stall shapes — the R29DBG
    2M-iteration spin is the documented witness that pace stalls HAPPEN).
- **The golden reference comes from the GENERATOR's ground truth, not the engine** — the oracle
  independence law (the existing golden.rs discipline, inverted: the tape is random, the truth is
  known a priori).

**The measured surfaces:**
- Per-pass latency histogram: P50 / P99 / P99.9 / P99.99 (the docs/15 tail methodology, per-pass
  not per-msg — the pass is the engine's atomic unit).
- **The three guard counters as chaos canaries**: `seg_guards`, `cursor_resyncs`, `record_snaps`
  > 0 under chaos = desync pressure the containment absorbed — trend them per profile; a rising
  counter under a fixed profile is the family's early-warning radar.
- The end-of-pass tuple assert (vs the generator's golden) as the correctness oracle.
- The allocs=0 gate (the harness must not allocate inside the window either).
- A deadlock watchdog: bounded parks everywhere (the §2 fixes are PREREQUISITES — chaos testing a
  deadlocking transport produces a hung shard, not a P99 number).

**CI integration:** one chaos arm per shard-batch in the 40-shard matrix (seed =
shard-index-derived, so the fleet explores the seed space by construction); a soak mode
(day-scale audit per solution2 §6.3's proof-object discipline); the chaos telemetry lands in the
scoreboard beside the rate columns — **the corridor's honest metric becomes "73.72B at P99 = X µs
under chaos profile P", not a clean-tape rate.**

### 7.2 The live-feed caveats the harness must carry

`--random-stream` moves the validation wall from construction-time to runtime (§4.1): the harness
IS the malformed-packet fuzzer, and every §4.2 silent face (A-2, F-4, F-7, F-8) becomes reachable.
The harness's launch order: (1) land §2's liveness fixes (a hung shard measures nothing), (2) land
the five width asserts + the `lane.base` reset (the divergence faces become loud), (3) then the
generator, (4) then the fleet arms.

---

## §8 The immediate action queue (severity-ranked, priced)

| # | Action | Severity | Cost | Blocks |
|---|---|---|---|---|
| 1 | F-1: EOS double-empty publication fix + the 1024-frame/coalesce-256 regression cell | HIGH (deadlock, deterministic per geometry) | small | R40b (marker arithmetic reuse) |
| 2 | F-2: `CMD_RESET` parity in the marker wait + bounded ack | HIGH (deadlock) | trivial | R40b |
| 3 | A-1: `assert!(n_lanes * CHUNK <= RESLOG_PHASES)` + 9/16-lane cell | HIGH (OOB panic) | trivial | any 9+ lane arm |
| 4 | A-2: `lane.base` per-generation reset (the silent SPANHASH face) | HIGH (silent wrong answer) | small | `--random-stream` |
| 5 | D-2: worker-death flag + bounded `finish()` | MED-HIGH (panic → hang) | small | chaos harness |
| 6 | F-4/F-7/F-8/A-3: the five release-mode width asserts | MEDIUM (live at 200B sustained) | trivial | 200B corridor ops |
| 7 | F-5/F-6: `master_entries` gate + `attach_ledger` zero-store | MEDIUM (UB/panic on reuse) | small | fabric reuse |
| 8 | F-3: bounded consumer parks + heartbeat | MEDIUM (hang) | small | chaos harness |
| 9 | The sigpush carry-discipline cell + `ResLog::push_seg` underflow assert (B-2) | LOW-MED (prose → proof) | small | Theorem 35 |
| 10 | Hygiene: R29DBG removal (A-4), re-home the dangling R39 test, delete `phase_bc` (B-3), the B-4 unset-epoch guard, F-14 pubs[] padding | LOW | trivial | — |

Then the corridor program itself, per the blueprint's staging: **R40a** (lane-filter + script-split
parity cells) → **R40b** (the second producer, the rendezvous, the election) → **R40c** (the fleet
arms 11r40/11r40r/11r40w) — with queue items 1–2 riding R40a's commit and items 3–8 riding R40b's,
so the dual-producer interleaving space is instrumented from its first breath.

---

## §9 Calibration summary (for the two senior peers)

**The Principal Systems Engineer:** the audit's §2/§3 findings are your R40a/R40b checklist — the
reset-liveness trinity and the worker-death flag are the difference between a red shard and a
35-minute timeout burn; §5.3's ring-elimination table is the zero-copy kernel's spec, and its three
preconditions (per-lane journals, run-drain, RX absorption) are the R40c/R41/R42 program in order.
The wake-budget law (§3.3) is the one contention trap the blueprint doesn't yet carry.

**The Chief Mathematician:** §6.1's Theorem 33 + 34 are the coupled summit (the delay-cone partial
commutation and its H¹) — everything else in the 1T program composes under them; Theorem 36 runs in
parallel as the information-theoretic floor; the Lemma-set pins (§6.1.6) are the unit tests that
keep the generalization honest against the fleet-proven degenerate cases. The D-exclusion
carry-discipline lemma is a one-afternoon proof that closes the audit's one remaining prose-only
soundness argument.

**The honest state of the engine:** 73.72B verified, zero-alloc, fail-loud by design, the Shard-28
family closed and telemetry-watched. The residual surface is small, named, and priced above. The
200B corridor is reachable without new mathematics; the 1T barrier is not — and the mathematics it
needs is now specified to theorem granularity.
