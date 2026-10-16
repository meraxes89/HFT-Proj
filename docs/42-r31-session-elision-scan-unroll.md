# 42 — Run 600: The Session Elision & the 16-Frame Scan Step (R31-A / R31-B)

The Run-600 program against PR #15's merged baseline (Run 550, the
R26–R28 stack). PR #15's workers burned through **43,129 passes in 5
seconds** (up from 16.5K) at ~12 cycles/frame — and the wall moved to
the pass boundary:

```
Scan Time:   3,433 ms   (~12 cycles/frame)
Reset Time:  1,561 ms   (~32% of the 5-second window stalling on pass resets)
```

The `HFT_PREPATCH_BUDGET=1024` experiment (arm 11rx) gave the hardware
attribution we needed: draining the per-pass session bake into the pass
dropped `reset_ms` 1,561 → 350, but **degraded** `scan_ms` 3,433 →
4,645 — the ~22,000 session header sites `[0..10]` rewritten per pass
across the shared buffer are RFO (Request-For-Ownership) invalidations
of the frame header lines the workers' L1/L2 want hot. The bake and the
scan are the SAME memory system: paying the reset wall at the boundary
or paying the RFO storm mid-pass are the two faces of one cost.

R31 deletes the cost.

## R31-A — Eliminate the 22K-Site RFO Storm & the Redundant Window Copies

### A.1: The Session Elision (`HFT_SESSION_ELISION`, default ON)

**The law the elision stands on.** In the armed (auto-advance) +
rxbuild + no-split shape, *nobody on the steady path reads the blob's
session bytes*:

* `poll_rxbuild` advances the pacing skeleton without a frame-line read
  — the entries live in the construction-built master.
* The scan's session compare runs on the master entries' inline
  `sess_lo`/`sess_hi` words — which stay construction-frozen.
* The workers' span bodies start at `HEADER_LEN+2` — past the prefix
  (but the prefix shares the frame's FIRST cache line with the first
  span's body head, which is why patching it RFOs the workers).
* The consumer's per-pass sequencer adopts the session from the frame
  bytes — which stay at the construction value; the adoption is
  self-consistent every pass.
* The pass tuple `(count, hash, msg_hash)` is session-value
  independent — the bench's own ref-pass law: *"identical bytes every
  pass; only the session id changes, which cannot affect the tuple."*

**The contract survives.** The RX still stores
`auto_session = sess_fn(next_pass)` and `reset_pass` still fail-stops
on any baked-vs-requested divergence (the program is a schedule, not a
trust substitute). What is knowingly elided is the WIRE fidelity of the
per-pass session bytes. The `HFT_SESSION_ELISION=0` rollback (CI arm
11se) restores the full patch law verbatim.

**Mechanics.** `ReplayTransport::reset_cursors` (render.rs): the
advance's pacing-skeleton restart (event_idx / virtual_clock / session
field / clock_clamp) with ZERO blob writes. The rx_thread's advance
takes it instead of `reset_prepatched`; the master tail patch
(`master_patch_range`) is skipped (the master stays construction-frozen
— `RXBUILD_DIAGNOSTIC patches=0`); the prepatch is disarmed entirely
(there is nothing to patch). The elision is armed at construction iff
armed && rxbuild && no `session_split` (a split schedule's frame session
bytes are load-bearing for the sequencer's mid-pass boundary dispatch).

**The local A/B** (2-vCPU dev VM, 1 worker, shared cores — direction
proof only; the fleet prices it):

| arm | advance total | per advance | master patches | sustained |
|---|---|---|---|---|
| elision ON | 1.6 ms | 0.4 µs | 0 | 436.7M |
| elision OFF | 142.3 ms | 36.3 µs | 86.3M | 396.8M |

+10% sustained and +9.9% pass cadence on a shape where the bake was
only 142 ms of the wall — the fleet's 1,561 ms wall (32% of the window)
projects the sustained arm into the **5.5B–6.2B msg/s band**.

### A.2: The Warm-Epoch Copy Gate (`HFT_WARM_EPOCH`, default ON)

The rotating regime's every window open re-staged the previous window's
~252KB descriptor + tag pair (`copy_arr` + `copy_tags`) — ~10.8 GB of
redundant memcpy over the 5s window on the 43K-pass draw. Once a slot's
own last window verified fix-free at the same span count, the slot
already holds the exact tape (the deterministic-schedule law the copy
itself stands on). The gate (`rx_open_window`) skips the pair when
`rx_slot_epoch[slot] >= 1 && rx_slot_count[slot] == last_window_count`;
the per-span check-and-fix remains the backstop — a wrong skip costs
fix stores, never correctness (the copy's own law, symmetric). After
the first `RX_NARR` warm-filled passes, the rotating regime's opens are
copy-free, exactly like the toroidal epoch's.

Pins: `t_r31a_warm_epoch_copy_gate` (engagement + bit-exact records +
the rollback cell), `t_r31a_session_elision_parity_vs_patched`
(payload-identical / session-frozen / zero-patch-count laws),
`t_r31a_session_elision_contract_failstop` (the contract cell),
`t_r31a_session_elision_mid_pass_abandon`. The I-7 class pins
(`t_prepatch_marker_sentinel_head_invariant`, the batch-parity chaos
soaks) moved to the forced `elision=false` constructors — they pin the
PATCH law, which the elision replaces by design; the elision's
end-to-end soak is `t_rxdesc_multipass_straddle_soak` (400 passes,
4-session rotation, per-pass tuple asserts, env-path construction) and
the bench's own per-pass `assert_sustained_pass`.

## R31-B — Halve the Active Scan Loop (12 → ~6 cycles/frame)

**The 16-frame dual-ZMM step** (`ladder_group16_apply`,
nf-arbitrator): TWO 8-lane `SoaLadder8` kernels fused into ONE
out-of-line call — the full 32-load AoS gather issued up front (group
B's load-to-use latency hides under group A's vector relations), ONE
session gate for all sixteen (the publish-time baked-session law), the
counter folds and emissions batched across both groups. The group-B
anchor is group A's watermark advance (wrap-guarded exact by A's own
kernel — the composed-anchor proof); the cross-group chain link is
implied by A's chain + B's anchor. A failure of either kernel falls the
whole 16 back through the 8-wide step, then the scalar ladder, then the
classic path — nothing is applied unless everything verifies.
`steady_scan_ladder` tries the 16 first, always.

**`check_arr16_packed`** (rxdesc.rs): the 16-wide fused array check —
two 512-bit row loads, two transform+compare chains, one u16 mismatch
mask. Halves the per-group call/bounds overhead on the non-SS commit
path (rotating windows, straddle continuations, inline hole-free
commits). `submit_packed_arr_run` routes 16-wide groups through it with
the same per-lane fix path on mismatch.

**The local evidence** (dev VM, the 11vl shape —
`HFT_RXDESC=1 HFT_VEC_LADDER=1`): the main core's `work_ms` fell
4,325 → 2,444 (−44%), the self-serve epoch engaged on 17,140 of 17,142
windows (vs 0 on the pre-R31 stack — the assist never fired:
`assist_chunks` 373,809 → 16), and the one-worker shared-core sustained
rate hit 1.734B (vs 436M un-laddered on the same VM). The ~12
cycles/frame scan halves on the clean cache state — the R12 verdict's
−5.3% was priced on the PRE-R29 consumer WITH the RFO regime active.

**Pins.** `t_r31b_ladder16_group_parity`: the ENGAGED group path
(hand-built steady tape, elig bits set) — classic vs scalar-steady vs
16-group drivers, plus the foreign-session cell that forces the full
fallback chain; `assert_soa_parity` gained a fourth leg
(`ladder8_scalar_ptr` — the pointer-form scalar kernel, installable
into `SoaLadder8`) that runs the group drivers' relation/fallback logic
on every host. The AVX-512 shapes are pinned by the 11vl/11r6 bench
arms' per-pass tuple asserts.

**A bug the parity cells caught** (worth recording): the first
`check_arr16_packed` draft combined its two `__mmask8` halves with
`(!(eq_lo as u16)) | ((!(eq_hi as u16)) << 8)` — the complement of a
zero-extended u8 sets the OTHER half's bits, so every 16-group reported
its high 8 lanes as mismatches: correctness stayed bit-exact (the
"fixes" wrote the correct words) but every window went dirty and the
toroidal/self-serve epochs never engaged. `t_r27_toroidal_multipass_parity`
caught it in one run. The halves are now masked to their own lanes.

## The Arms

| arm | env | pins |
|---|---|---|
| 11b (default) | — | the R31 defaults: elision + warm epoch |
| 11se | `HFT_SESSION_ELISION=0` | the patch law restored (the elision attribution) |
| 11we | `HFT_WARM_EPOCH=0` | the always-copy rotating regime (the copy-gate attribution) |
| 11r6 | `HFT_RXDESC=1 HFT_VEC_LADDER=1` | the Run-600 stack (elision + warm epoch + ladder16 + check16) |
| 11vl | `+HFT_PREPATCH_BUDGET=1024` | the pre-R31 comparison line (the budget is inert under elision) |

## The Ceiling

R31-A alone (the default arm) collapses the 1,561 ms reset wall to the
pass-handshake floor with zero RFO penalty on the scan — the sustained
band moves to **5.5B–6.2B** on the fleet's dedicated-core shapes. R31-B
(stacked via 11r6/11vl) halves the scan's per-frame cost on the clean
cache state — the combined ceiling of the 6B crossing and the headroom
beyond it. The pass-supply cadence (reset_ms) and the L1/L2 RFO
invalidation are no longer the bottleneck; the next wall is whatever
the fleet's 120-shard matrix says it is.
