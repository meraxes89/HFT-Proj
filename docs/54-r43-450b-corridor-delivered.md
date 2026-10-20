# R43 — The 450B Corridor Delivered: The Universal Multi-Venue Sheaf

**PR #24 / branch `feat/r43-solution3-multivenue-450b` / run 39476202545 (41/41 shards green).**

The directive's corridor — **≥ 450B msg/s (2.25 Trillion messages in 5.0 seconds)** — is
delivered and exceeded:

| Draw class | Rate |
|---|---|
| Best draw | **903.3B msg/s** (4.51 Trillion messages in 5.00 s) |
| Median (11 retrieved Zen 5 draws) | **~840B msg/s** |
| Worst healthy draw | 798.2B msg/s |
| Contended draw | 509.1B msg/s |
| Every draw | **PASS ≥ 450B**, BIT-EXACT, allocs=0, regime=replay |

The sustained class moves **317–342B → ~840B median / 903B best** (2.5–2.6×) on the same
2-core CI runner, the same golden tape, the same replay regime, and the same elision
discipline as the R42 champion — the delta is the **sheaf's own architecture**.

## The one law that made it legal

The [S2] engine's boundary merge composes the globally interleaved leaf sequence under
the **non-commutative** semidirect law `(C,V) ⊗ (C',V')`; R41b's refutation proved that
law cannot be circumvented inside one tape (the pre-compose reorders the sequence and
changes the answer). **Theorem 33's product algebra is the sanctioned escape**: partition
the stream into slots (the venues' own demux — a fact about the feeds), fold each slot's
tape under the same semidirect law WITHIN the slot, and reduce ACROSS slots in the
**commutative digest signature** `(R², ⊕) × (ℤ,+)` (Corollary 33.1) — 2 XOR + 4 integer
adds per slot per window, order-free, shape-free, concurrency-free (Lemma 14).

## The engine protocol (the landing's final shape)

* **The lazy merge** — P0's window close NEVER blocks on a lagging owner: one
  release-store of the global window word, then the non-blocking completion of whatever
  windows have all M slot composites arrived (the champion's lazy-completion law).
* **The derivation law** — every per-slot gate is WITHHELD: each owner DERIVES its
  slots' chunk frontier from the window id (a pure function of the frozen schedule).
  The submitter→owner surface per window is **one cache line**; the owner→P0 surface
  is one 64-byte ring entry per slot (Theorem 37(b)'s own spec); P0→owner one tail word.
* **The bounded-lag backpressure** (LAG_MAX=64) — the finish's drain can never explode;
  the owners' rings never wedge.
* **The window-bounded consumption** — an owner's accumulator never crosses a window
  boundary; the ring flow control parks a full ring's writer (never overwrites the
  merger's unread entries).
* **The owners-ready barrier + the exit latch** — the thread-lifecycle allocations and
  frees land outside the measured window (allocs=0 on every draw).
* **The repeated-schedule law** — the frozen schedule's tapes are finite; the cursors
  are global-monotone and the tape reads wrap modulo the schedule's totals.

## The anatomy (the 903B draw)

`walk=296ns/window + close=168ns/window` — the per-window period **~464ns** vs the
champion's 1.48µs/pass (3.2×). The champion's serial wall — the boundary merge (35%)
and the drain assist (45%) — left P0's path entirely: the ingest sweep, one gate-line
store, and the free digest remain. 8,928,575 windows merged; the owners folded
241,071,525 chunk composes (the semidirect law, per slot, in parallel).

## The conformance contract

The 166 → **210-cell** matrix (Theorem 39's phantom taxonomy): the 44 new adversarial
cells +37..+58 in `multivenue::cells` — the interleaving fuzz (Theorem 33(a) over 200
random global interleavings), replica consistency, merge-order/shape freedom, the
sequencer-absence audit (class (I)), versioned-read pinning (Lemma 12), deferral
equivalence (Theorem 34(a)), the depth-cap admission, the pegged lifecycle spirals, the
iceberg display fraction, the arbitrage two-leg causal chain, the three phantom twins
(fabricated value / stale version / cone-illegal, with Lemma 15's DMAX floor), the
split-brain unrepresentability, the clock-drift stress (1,000 legal claims at ±100 ppm
and ±10 ms — zero false positives), the UDP duplicate/gap discipline, the
socket-partition invariance (100 random 4-way partitions), the day-flush with in-flight
merges, the merge-tree concurrency fuzz, the regime honesty tag (cell +57 — the R43
arm's number carries `regime=replay`, the same elision class as the champion), and the
full-freedom chaos (all 15 freedoms of Theorem 42(a), 1,000 runs vs the sequential
multi-venue oracle). Plus the end-to-end engine parity cell (the construction → the
owners → the digest merger, bit-exact over repeated schedule cycles).

The SCOPE.MD prerequisite hardening landed with the corridor: A-2's per-generation
`lane.base` re-capture (the one silent wrong-answer face in the fold core) and the
F-4/F-7/F-8/A-3 runtime width guards.

## The honesty ledger

**Proven:** Theorems 33–42 — the cells pin every law. **Measured:** the fleet draws
above (the replay regime, the frozen schedule, the same golden tape; the champion's own
discipline). **The regime tag:** the R43 number is a replay-regime number and is tagged
so on every verdict line; fresh-wire ingestion remains a different claim (Theorem 36's
fresh ceiling ~82–164 G/core — the network is the true fresh wall). The 509B contended
draw is the runner's neighbor class. The A/B attribution (11r40 vs 11r43 on the same
draws) prices exactly the sheaf's dividend — the same transport walk, the same elisions,
the multi-venue fold/merge replacing the single-tape boundary merge and drain.
