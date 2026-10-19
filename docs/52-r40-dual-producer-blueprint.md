# R40 — THE DUAL-PRODUCER CHAMPION (Champion-4T): the 200B corridor's structural design

## 0. Where the wall stands (the R39 fleet verdict, run 39201557063)

The producer pivot's Option A (the coarse-grained submission sweep) is fleet-priced:

| arm | rate (same shard, 9V45 draw) | sweep state |
|-----|------------------------------|-------------|
| 11r36s (the 1-lane reference) | 61.82B | armed, mean_width 6.77 |
| 11r38w (the 2-lane live walk) | 57.85B | armed, mean_width 4.55 |
| 11r38r (the order-scan rollback) | 57.66B | armed, mean_width 4.55 |
| **11r37 (THE CHAMPION)** | **70.04B — the fleet record** | **armed, mean_width 5.01** |
| 11r39r (the sweep rollback, k=1) | 63.19B | off, mean_width 1.00 |
| 11r37r (the tree rollback) | 24.25B | — |

The same-draw sweep delta is **+6.85B (+10.9%)**; the champion's absolute
peak moved 66.69B → **70.04B**. The sweep did what it was designed to do:
the wait component (ONE wait + ONE free-cluster per GROUP of ~5 batches)
collapsed. But the ceiling is now unambiguous — the R25b postmortem's law,
re-confirmed by the sweep's modest multiplier: **the system is MAIN-BOUND**.
One Zen 5 logical core running the consume → guard → re-issue → drain →
boundary pipeline prices out at 63–70B msg/s no matter how cheap the
workers get (they are at 99.998% replay — ~0.3 µs/pass against an 8.4 µs
pass cadence, ≈ 4% duty). The road to 200B does not pass through another
diet of the SAME loop; it passes through **splitting the loop**.

## 1. The topology math (why 2 producers fit the silicon)

The 2C/4T draw (9V45: sibling pairs 0-1 / 2-3) currently places, by
`fabric_placement_distinct`:

```
physical core 0:  cpu0 = Worker 0 (~4% duty)   cpu1 = MAIN   (~90% duty)
physical core 1:  cpu2 = Worker 1 (~4% duty)   cpu3 = RX     (~28% duty)
```

The workers — the entire reason the two physical cores were claimed — are
now nearly idle post-elision. Meanwhile the ONE saturated thread (main)
shares an SMT pair with a worker that barely runs, and the RX thread owns
a full sibling while running at a quarter duty. The R25b postmortem
priced the naive rebalances already: stacking main+rx on one sibling
measured **−32.5% median** (118% combined demand on one logical core —
the main timeshares itself into the floor). The placement problem is not
"who shares with main" but "main is one thread doing two lanes' work".

The Champion-4T layout inverts the ownership:

```
physical core 0:  cpu0 = Producer 0 (dedicated)   cpu1 = Worker 0 + RX share
physical core 1:  cpu2 = Producer 1 (dedicated)   cpu3 = Worker 1
```

- Each PRODUCER gets a dedicated physical core — the main-bound thread
  class doubles its silicon.
- The workers (4% duty) and the RX (28% duty) co-locate on one sibling at
  ~32% combined demand — comfortably inside one logical core, and the
  R25b counterfactual already proved worker-sibling sharing is the cheap
  theft (the scalars_stacked failure was MAIN+RX stacked, never
  worker+RX).
- **The coherence dividend**: today main (cpu1) writes BOTH lanes'
  descriptor rings — lane 0's ring is L1/L2-local (worker 0 shares its
  physical core), but every lane-1 descriptor line crosses the physical
  core boundary (an L2 miss + coherence bounce per 64 B of descriptors).
  In Champion-4T, producer i writes ONLY lane i's ring, on its OWN SMT
  pair — every descriptor write becomes L1-local. The same holds for the
  drain side: the res ring producer i drains is its own sibling's.

## 2. The coupling inventory (what the split must untangle, verified in code)

Five sequential dependencies own the main's loop today:

1. **The transport ring (SPSC)** — `PipelinedReplayTransport::next_group`
   has exactly one consumer. BUT in the frozen-master steady state the
   batch payload is a position protocol: `try_replay` reads only
   `batch_bounds()` `(rx_start, len)` for the identity guard and then
   re-issues the LEDGER's recorded calls — the live entries are touched
   only on the 18-in-a-million live scans. The R33 publication-schedule
   replay (`HFT_RX_SCHED_REPLAY`) already memoizes the RX's schedule —
   a second consumer can be fed from the schedule's own frozen
   progression, not from the single ring.
2. **The recorded call stream (sink-logical, not lane-addressed)** — the
   R32 ledger records `Call::Event/Msg/Span/SpanBatch` (the sink's public
   API); the lane partition happens INSIDE `HydraSpanSink`'s chunk-grid
   submit (`submit_packed_batch_rx` targets the lane the chunk grid
   assigns). A per-lane split therefore re-indexes the ledger by LANE:
   each producer's script holds its own lane's ring-write surface
   (position-stamped, in order), while the transport's batch bracket
   (the guards' bounds/watermark operands) stays shared-positional.
3. **The drain (the static-order merge)** — `fold_available` derives the
   lane from `fold_pos` (the R29 derived grid, LaneMod post-R39) and
   consumes records in the strict interleaved tape order. The merge
   order is POSITION-DERIVED and STATIC (the chunk grid is
   round-robin): each producer can drain its own lane's ring into its
   own journal without any cross-lane communication, because the
   interleaving is a pure function of the positions both sides already
   know.
4. **The boundary machine** — `complete_boundaries` + `end_pass` +
   `harvest_completed` fire once per pass, sequentially. The R38
   multi-boundary collector ALREADY runs the epoch-slot protocol per
   lane (the workers stage `X_l = (K, B)` into per-lane epoch slots,
   Release-published, never waited on mid-pass) and the M-WAY LAW pins
   the exact merge: the pass composite is the interleaved tape-order
   product — the 2-factor block form `S ⨂ X_0 ⨂ X_1` is exact only at
   block splits, so the boundary must merge the two journals BY
   POSITION (a 2-way merge of position-stamped leaves, O(records) per
   pass ≈ 172 — trivial), not concatenate.
5. **The verification oracle** — the per-pass tuple assert
   (`assert_sustained_pass` vs the golden ref) and the allocs=0 gate are
   pass-granular, not lane-granular: they run at the boundary, on the
   merged composite, unchanged.

## 3. The design (Champion-4T)

**Thread roles.**

- `P_i` (producer i, dedicated core): consumes the frozen schedule's
  batch progression (its own cursor over the R33 schedule replay),
  replays its lane's script (the per-lane re-indexed call surface — the
  ring writes to lane i's descriptor ring only), drains lane i's res
  ring into its position-stamped journal (the R37 tree journal,
  per-lane), and publishes its pass composite into the existing epoch
  slot machinery (`X_i = (K_i, B_i)`, Release).
- `W_i` (worker i, SMT sibling of P_i): unchanged — the rxdesc walk, the
  res-record replay (99.998%), the epoch-slot publication at the pass
  close. Its ring traffic is now ENTIRELY within its own SMT pair.
- The boundary rendezvous: the last producer to fill its epoch slot
  runs the boundary step — the 2-way position merge of the two journals
  (Theorem 23(b)'s static order made explicit), the tree flush, the
  harvest, the tuple assert. A single `compare_exchange` on a boundary
  epoch word elects the runner (the arriving-last law); the other
  producer proceeds into the next pass without waiting (the tail folds
  during the next pass — the existing non-blocking `end_pass` law).
- The RX thread: keeps feeding the (now position-protocol) batch
  progression, sharing the worker sibling at ≤32% demand. In the limit
  (R40c) the producers absorb the RX role entirely (the R29 self-serve
  shape per lane) and the thread count DROPS — the zero-thread ingest
  endpoint.

**The guards (the soundness chain, per producer).**

- The frozen-master regime word: per-producer view of `master_version`
  (an Acquire load at pass entry; a bump truncates BOTH scripts and
  falls to the live scan — the existing re-record law, applied
  pairwise).
- The bounds/watermark identity: each lane's script carries its own
  recorded `(rx_start, len, w_in)` triples; the live operands come from
  the schedule replay's own positions (the same determinism contract —
  a divergence fails loud exactly as the R32 guard does today).
- The tuple assert: the boundary's merged composite against the golden
  reference — the SAME oracle, one assert per pass, on the merged
  surface. A mis-ordered merge, a corrupt script, or a lost record
  diverges the tuple the same pass. The replay is additionally
  shadowed by the SPANHASH surface (the R36-C merge law) — unchanged.
- Allocs=0: the per-lane scripts and journals are pre-allocated at
  construction (outside the window), same as the R32/R36 ledgers.

**The rollback discipline.** `HFT_DUAL_PRODUCER=0` (default until the
fleet prices it) restores the single-producer loop verbatim — the
champion's current shape, bit-for-bit. Every R40 arm lands behind that
gate.

## 4. The eta math (honest)

Each producer runs the post-sweep per-lane pipeline: consume ~half the
guard ceremony, one lane's ring writes (L1-local now), one lane's drain.
The per-producer budget is roughly HALF the current main's 8.58 µs/pass,
minus the coherence dividend, minus the group-ceremony's second halving:
call it 3.5–4.3 µs/pass → each producer prices at the 63–70B class of
the CURRENT whole main. Combined: **126–140B** before any new lever.
The residual to 200B:

1. **The drain diet** (the R39-sweep analog for the res ring): the
   per-record protocol (head load, tail check, lane derive, journal
   push) amortizes over GROUPS of records — the workers already publish
   runs of consecutive records; a run-drain consumes a whole run with
   one protocol pass (the `Desc8` compaction pattern applied to the res
   side). Est. −30–40% of the drain's 37% share.
2. **The RX absorption** (the R29 self-serve shape per lane): the
   producers generate their own batch progression from the frozen
   schedule — the RX thread retires, its sibling goes to the worker
   full-time.
3. **The SMT dividend**: W_i on P_i's sibling means the worker's record
   publication is L1-visible to its producer — the drain's Acquire
   loads cheapen.

Stacked: 126–140B × (1 + 0.30·0.37 + the RX retirement's ~5% + the
coherence tail) ≈ **165–200B**. The corridor is reachable without new
mathematics — Theorem 25's epoch slots and the static-order merge are
already fleet-proven at 564k boundaries; R40 is their promotion from
the worker plane to the producer plane.

## 5. The staged landing plan

- **R40a — the per-lane script index** (no threading change): the R32
  ledger gains the lane re-index (each batch's recorded calls split by
  the lane the sink's grid assigned them); the sink gains the
  lane-filter parameter (`submit` no-ops for foreign lanes under the
  flag). The single producer runs BOTH scripts interleaved — the
  emitted ring traffic bit-identical (the tuple assert + the ledger
  identity asserts prove the split). Cells: the script-split parity
  (dual scripts vs the mono script, member-for-member), the foreign-
  lane no-op law, the re-record path's pairwise truncation.
- **R40b — the second producer + the boundary rendezvous**: the P1
  thread (the placement of §1), the per-lane journals, the epoch-slot
  election, the position merge at the boundary. Cells: the merge law
  (the interleaved order vs the sequential reference, the strict a,b
  alternation), the election's exactly-once law, the tail-fold overlap.
- **R40c — the fleet arms**: 11r40 (champion, dual producer), 11r40r
  (the HFT_DUAL_PRODUCER=0 rollback), 11r40w (the drain-diet off, the
  attribution split), the sweep interaction (group_k × dual).

## 6. What R40 deliberately does NOT do

- No new fold mathematics: the tree journal, the epoch slots, and the
  m-way law are used exactly as R37/R38 fleet-proved them.
- No change to the workers' walk or the res-replay machinery (the
  99.998% regime is the windfall R40 spends, not a surface it touches).
- No ring protocol changes: the SPSC descriptor/res rings keep their
  laws; R40 only narrows each ring to one producer-consumer SMT pair.
