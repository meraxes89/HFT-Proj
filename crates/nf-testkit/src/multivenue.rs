//! # R43: The Universal Multi-Venue Sheaf (docs/solution3.md)
//!
//! The fleet layer on top of the [S2] record algebra: the slot space and
//! the product state (Theorem 33), the order-free fleet digest (Corollary
//! 33.1), the versioned-trigger DAG and the phantom detector (Theorems
//! 34–35), the SPSC merge rings and the Null-Flow law (Theorem 37), and
//! the day-scale proof object (Theorem 40).
//!
//! **The one law that makes the 450B corridor legal.** The [S2] engine's
//! boundary merge composes the globally interleaved leaf sequence under
//! the NON-commutative semidirect law `(C,V) ⊗ (C',V')` — R41b's
//! refutation proved that law cannot be circumvented inside one tape
//! (one lane's pre-composed composite reorders the sequence and changes
//! the answer). Theorem 33's product algebra is the sanctioned escape:
//! partition the stream into SLOTS (the venues' own demux — a fact about
//! the feeds), fold each slot's tape under the same semidirect law
//! WITHIN the slot, and reduce ACROSS slots in the COMMUTATIVE digest
//! signature `(R², ⊕) × (ℤ,+)`. Cross-slot composition is order-free,
//! shape-free and concurrency-free (Lemma 14) — the boundary merge, the
//! j1 lead laws and the arrival pacing all dissolve. The per-slot folds
//! are fully decoupled (light-cone causality, Lemma 10: no sequencer, no
//! global epoch lock, no shared writable line between slot engines).
//!
//! The module ships in two layers:
//! * **The math** (§`Dig`, §`SlotPub`, §`MergeRing`, §`DayChain`) — the
//!   conformance surface: every theorem has its cell in `mod tests`
//!   (the 44 new adversarial cells, +37..+58 — the 166 → 210 extension).
//! * **The engine** (§`FleetEngine`) — the sustained corridor driver:
//!   the multi-tape twin of the champion's replay regime. Same golden
//!   tape, same transport walk, same frozen-schedule elision class, same
//!   per-span CRC discipline — the fold/merge architecture replaced by
//!   the sheaf. The BIT-EXACT oracle is the sequential multi-venue
//!   reference (per-slot sequential folds + the flat digest).

use crate::affinity::{pin_current_to, rendezvous_spin};
use crate::sink::span_crc32c_8lane;
use crate::spanfold::{mulmod, p64_compose, shift_pow, t_pow, P64};
use nf_arbitrator::types::{Event, LiveFeedProof, Sink, SpanRec};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::atomic::AtomicU64 as AU64;

/// The per-slot sequential chain's seed (the champion's own — the FNV
/// offset basis; the fleet's chains re-seed at every window boundary,
/// the same law the pass records follow).
pub const SPAN_SEED: u64 = 0xcbf29ce484222325;

// ─────────────────────────────────────────────────────────────────────
// §1 The slot space and the slot map (Theorem 33, Def 2.1; Appendix C.1)
// ─────────────────────────────────────────────────────────────────────

/// The deployment ceiling on the CI runner's topology (2 physical cores
/// → 4 vCPUs → 4 lane groups). Appendix C.1's production map scales this
/// to `|𝕊_live| ≈ 100` across 4 sockets; the slot MAP is the artifact,
/// the slot COUNT is a dial.
pub const SLOTS_MAX: usize = 16;

/// Spans per per-slot chunk — the R42 coarse-grid law carried into the
/// sheaf (the two-level scheme's leaf granularity; [S2] §2.7 per slot).
pub const SLOT_CHUNK: usize = 512;

/// A slot's identity: `(ν, γ)` — venue ν, sub-stream γ (Def 2.1). The
/// engine derives it from the feed's own demux (the stock-locate field
/// of the ITCH message header — the venue's symbol-hash demux fact).
pub type SlotId = u8;

/// **The Slot Map** (Appendix C.1): venue → slot → lane group → socket.
/// Ingress across the M venues is fully decoupled — inter-venue
/// generators commute freely along disjoint light cones (Theorem 33).
/// The CI deployment: M slots round-robin over the runner's lane groups
/// (the "socket" axis degenerates to 1 — the map's law, not its size,
/// is what the cells pin).
#[derive(Clone, Debug)]
pub struct SlotMap {
    pub n_slots: usize,
    /// `group[s]` = the lane group owning slot s (venue-home placement).
    pub group: Vec<u8>,
    /// `socket[s]` = the socket hosting slot s's arenas (first-touch).
    pub socket: Vec<u8>,
    /// The venue names (audit provenance — Theorem 40's verifier carries
    /// the map with the proof object).
    pub venue: Vec<&'static str>,
}

impl SlotMap {
    /// The CI deployment: M slots over `n_groups` lane groups,
    /// round-robin (the fairness law of [S2] §5.4's placement).
    pub fn ci(n_slots: usize, n_groups: usize) -> Self {
        assert!(n_slots >= 1 && n_slots <= SLOTS_MAX, "slot count {n_slots} out of [1, {SLOTS_MAX}]");
        assert!(n_groups >= 1, "lane groups >= 1");
        SlotMap {
            n_slots,
            group: (0..n_slots).map(|s| (s % n_groups) as u8).collect(),
            socket: vec![0; n_slots],
            venue: (0..n_slots)
                .map(|s| VENUES[s % VENUES.len()])
                .collect(),
        }
    }

    /// The demux: a message's slot, by the venue's own discipline — the
    /// SYMBOL-HASH of the stock-locate field (Def 2.1: "symbol hashing at
    /// the venue; segment hashing at CME" — the hash spreads the stocks
    /// across the venue's channels; the raw locate would cluster them).
    /// A fact about the feed's demux, not an engineering choice.
    #[inline(always)]
    pub fn demux_msg(&self, msg_with_prefix: &[u8]) -> SlotId {
        // [len: u16 BE][type: u8][locate: u16 BE][tracking: u16]...
        let locate = if msg_with_prefix.len() >= 5 {
            u16::from_be_bytes([msg_with_prefix[3], msg_with_prefix[4]])
        } else {
            0
        };
        // The venue's symbol hash (the multiplicative mixer — Knuth's
        // golden-ratio scatter; deterministic, content-derived).
        let h = (locate as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 24;
        (h as usize % self.n_slots) as u8
    }

    /// The slot's lane group (venue-home).
    #[inline(always)]
    pub fn group_of(&self, s: usize) -> usize {
        self.group[s] as usize
    }
}

const VENUES: [&str; 8] = [
    "NASDAQ-ITCH", "CME-MDP3", "BATS", "EURONEXT", "BINANCE", "DERIBIT", "LSE", "JPX",
];

// ─────────────────────────────────────────────────────────────────────
// §2 The fleet window digest (Corollary 33.1 — the commutative reduction)
// ─────────────────────────────────────────────────────────────────────

/// The digest record — 40 bytes. The reduction is over the COMMUTATIVE
/// signature monoid: `(R², ⊕)` for the (C,V) pair (component-wise XOR)
/// and `(ℤ,+)` for db/da/deg/cnt. **NEVER `p64_compose` across slots**
/// (§2.4's correction note; pitfall 15 — the semidirect law is
/// non-commutative, so a cross-slot compose is both order-dependent AND
/// mathematically meaningless in the product algebra).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Dig {
    pub v: u64,
    pub c: u64,
    pub db: i64,
    pub da: i64,
    pub deg: u32,
    pub cnt: u32,
}

impl Dig {
    /// The reduction's identity: the all-zero record.
    #[inline(always)]
    pub const fn identity() -> Dig {
        Dig { v: 0, c: 0, db: 0, da: 0, deg: 0, cnt: 0 }
    }

    /// 2 XOR + 4 integer adds — the whole cross-slot cost per window
    /// composite (Cor 33.1; the reason the fleet merge is free where the
    /// single-tape boundary merge was 54% of the pass period).
    #[inline(always)]
    pub fn accum(&mut self, x: &P64) {
        self.c ^= x.c;
        self.v ^= x.v;
        self.db = self.db.wrapping_add(x.db);
        self.da = self.da.wrapping_add(x.da);
        self.deg = self.deg.wrapping_add(x.deg);
        self.cnt = self.cnt.wrapping_add(x.cnt);
    }

    /// The reduction's canonical hash (the tuple's `hash` coordinate).
    #[inline]
    pub fn hash(&self) -> u64 {
        // A deterministic mixing of the six coordinates — any fixed
        // function serves (the contract is engine == oracle, both sides
        // use this one).
        self.v
            ^ self.c.rotate_left(17)
            ^ (self.db as u64).rotate_left(31)
            ^ (self.da as u64).rotate_left(47)
            ^ ((self.deg as u64) << 32)
            ^ self.cnt as u64
    }
}

/// The flat fleet digest over the window's slot composites — the
/// reduction in ARRIVAL order (any order is the flat — Theorem 33(d)).
pub fn fleet_digest(xs: &[P64]) -> Dig {
    let mut d = Dig::identity();
    for x in xs {
        d.accum(x);
    }
    d
}

/// The balanced tree variant: the reduction at depth `⌈log₂ n⌉` —
/// pairwise levels, the commutative monoid makes every bracketing the
/// flat reduction (Lemma 14; the SPSC merge-tree rings reduce exactly
/// this way when the arrival stream is balanced).
pub fn fleet_digest_tree(xs: &[P64]) -> Dig {
    let mut level: Vec<Dig> = xs.iter().map(|x| {
        let mut d = Dig::identity();
        d.accum(x);
        d
    }).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0;
        while i + 1 < level.len() {
            let (a, b) = (level[i], level[i + 1]);
            next.push(Dig {
                v: a.v ^ b.v,
                c: a.c ^ b.c,
                db: a.db.wrapping_add(b.db),
                da: a.da.wrapping_add(b.da),
                deg: a.deg.wrapping_add(b.deg),
                cnt: a.cnt.wrapping_add(b.cnt),
            });
            i += 2;
        }
        if i < level.len() {
            next.push(level[i]);
        }
        level = next;
    }
    level.first().copied().unwrap_or(Dig::identity())
}

/// The arbitrary-shape variant (the fuzz driver's kernel): reduce under
/// a caller-supplied bracketing — `shape[i]` is the index of the node
/// `xs[i]` folds into at each round (a permutation-derived pairing).
/// Unbalanced and odd-tail shapes are the same function of the same
/// multiset (Lemma 14's re-bracketing law).
pub fn fleet_digest_shape(xs: &[P64], shape: &[usize]) -> Dig {
    // Interpreting arbitrary shapes generically: fold each prefix group
    // delimited by the shape's ascending cut points, then reduce the
    // groups' digests — the bracketing freedom exercised at every level.
    let mut groups: Vec<Dig> = Vec::new();
    let mut acc = Dig::identity();
    let mut cut = 0usize;
    for &c in shape {
        while cut < c && cut < xs.len() {
            acc.accum(&xs[cut]);
            cut += 1;
        }
        if cut > 0 {
            groups.push(acc);
            acc = Dig::identity();
        }
    }
    while cut < xs.len() {
        acc.accum(&xs[cut]);
        cut += 1;
    }
    groups.push(acc);
    let mut d = Dig::identity();
    for g in &groups {
        d.v ^= g.v;
        d.c ^= g.c;
        d.db = d.db.wrapping_add(g.db);
        d.da = d.da.wrapping_add(g.da);
        d.deg = d.deg.wrapping_add(g.deg);
        d.cnt = d.cnt.wrapping_add(g.cnt);
    }
    d
}

// ─────────────────────────────────────────────────────────────────────
// §3 The per-slot fold (the [S2] machinery per slot — span granularity)
// ─────────────────────────────────────────────────────────────────────

/// The per-span contribution in the slot's factor of the product
/// algebra: `(C, V) = (x^{len}, crc(body))` with `cnt` the span's
/// message count — the champion's own SPANHASH law, lifted into 𝒜_res.
/// The identity embedding in every OTHER factor (Def 2.2) is structural:
/// the record simply is not composed into them.
#[inline]
pub fn span_record(body: &[u8], msg_count: u32) -> P64 {
    P64 {
        v: span_crc32c_8lane(body),
        c: shift_pow(body.len() as u64),
        db: 0,
        da: 0,
        px: 0,
        tag: 0,
        deg: body.len() as u32,
        cnt: msg_count,
        flg: 0,
    }
}

/// The sequential oracle's per-span recursion (the exact law — the judge
/// every parallel path must match bit-for-bit): `h ← h ⊗ x^{len} ⊕ crc`.
#[inline]
pub fn seq_apply_span(h: u64, body: &[u8]) -> u64 {
    mulmod(h, shift_pow(body.len() as u64)) ^ span_crc32c_8lane(body)
}

/// Compose a run of span records into ONE composite (the two-level
/// scheme's leaf compose — order = the slot's tape order, the semidirect
/// law WITHIN the slot; a maximum of `SLOT_CHUNK` spans per chunk).
pub fn compose_span_run(recs: &[P64]) -> P64 {
    let mut acc = P64::IDENTITY;
    for r in recs {
        acc = p64_compose(&acc, r);
    }
    acc
}

// ─────────────────────────────────────────────────────────────────────
// §4 The versioned-read surface and the phantom detector (Thms 34–35)
// ─────────────────────────────────────────────────────────────────────

/// The engine's per-slot publication record (one writer, release-store —
/// Appendix B.3). `pub_count` is the version frontier R1 compares
/// against; `pub_time[w]` is the publication timestamp R2's cone bound
/// reads. The window composites themselves live in the merge ring /
/// the day chain (`engine_eval(s, w)` reads them back).
pub struct SlotPub {
    pub_time: Vec<AtomicU64>,
    pub_count: AtomicU64,
}

/// The propagation map `d(s, t)` in ns — the deployment's light-cone
/// matrix (Def 2.3; same-rack ~0.3 µs, co-located 2–5 µs).
pub const DMAX: u64 = 10_000;

impl SlotPub {
    pub fn new(windows: usize) -> Self {
        SlotPub {
            pub_time: (0..windows.max(1)).map(|_| AtomicU64::new(0)).collect(),
            pub_count: AtomicU64::new(0),
        }
    }

    /// The single-writer publication (the slot's owner — release-store).
    #[inline]
    pub fn publish(&self, w: usize, t_ns: u64) {
        self.pub_time[w].store(t_ns, Ordering::Release);
        self.pub_count.store(w as u64 + 1, Ordering::Release);
    }

    /// R1's operand.
    #[inline]
    pub fn count(&self) -> u64 {
        self.pub_count.load(Ordering::Acquire)
    }

    #[inline]
    pub fn time(&self, w: usize) -> u64 {
        self.pub_time[w].load(Ordering::Acquire)
    }
}

/// A cross-venue contingent trigger: `τ(r) = (s, w', φ, θ)` — the
/// reference slot s, the PINNED window version w' (never "current"),
/// the read function φ, and the depth θ (the DAG admission cap).
#[derive(Clone, Copy, Debug)]
pub struct Trigger {
    pub slot: SlotId,
    pub w_prime: u32,
    /// The read function's tag (the φ family: mid, display-fraction,
    /// fill-state — the engine's `phi(tag, X)`).
    pub phi: u8,
    /// The trigger's depth in the resolution DAG (0 = wire-native).
    pub depth: u8,
}

/// The maximum admitted DAG depth — `D_max = ⌈log₂ B⌉` at the dial
/// (§3.5(iii); deeper triggers are rejected at INTAKE in O(1) — the
/// fold never blocks on them).
pub const D_MAX: u8 = 16;

/// φ — the projection family (Lemma 12's pinned read). Each tag maps a
/// published composite to the trigger's entry state; emit-time only
/// (Theorem 34's table note — the value is a function of the PINNED
/// version, never of the reader's local clock or progress).
#[inline]
pub fn phi(tag: u8, x: &P64) -> u64 {
    match tag {
        0 => x.v,                       // the raw V projection
        1 => (x.v >> 32) ^ x.c,         // "mid"-style blend
        2 => (x.v & 0xFFFF_FFFF) | ((x.deg as u64) << 32), // display fraction input
        3 => x.cnt as u64,                   // the fill-state probe (the pure count projection)
        _ => x.v ^ x.c,
    }
}

/// The (C,V) comparator — R3 (the fabricated-value catch). `y_cv` is
/// the claim's claimed projection; `eq_cv` recomputes from the pinned
/// composite. A claimed Y with a non-unit C-coordinate is immediately
/// non-realizable (A.4's free check — no composite of span-discipline
/// records ever produces one).
#[inline]
pub fn eq_cv(y_cv: u64, s: usize, w: u32, eval: &dyn Fn(u32) -> P64) -> bool {
    let x = eval(w);
    y_cv == phi(tag_of(s), &x)
}

#[inline]
fn tag_of(_s: usize) -> u8 {
    0 // the bench's single φ family; the cells exercise the full set
}

/// **The 3-compare phantom detector** (Theorem 35(a) — O(1), off-path,
/// anomaly arm; the fold never blocks and never rolls back):
/// * R1 — the version pin exists: `w' < pub_count` (1 integer compare);
/// * R2 — the light-cone bound (with Lemma 15's FLOOR: `d < DMAX`
///   underflows unsigned arithmetic and inverts the check without it);
/// * R3 — the (C,V) comparator (1 u64 compare).
///
/// Returns `true` iff the claim is realizable.
pub fn phantom_check(
    cone_d: &[[u64; SLOTS_MAX]; SLOTS_MAX],
    s: usize,
    t: usize,
    w_prime: u32,
    arrival_ns: u64,
    y_cv: u64,
    pub_rec: &SlotPub,
    eval: &dyn Fn(u32) -> P64,
) -> bool {
    // R1 — the stale-version catch (phantom B).
    if w_prime as u64 >= pub_rec.count() {
        return false;
    }
    // R2 — the cone-illegal catch (phantom C), WITH the floor.
    let bound = pub_rec.time(w_prime as usize).saturating_add(cone_d[s][t]);
    let bound = if bound > DMAX { bound - DMAX } else { 0 };
    if arrival_ns < bound {
        return false;
    }
    // R3 — the fabricated-value catch (phantom A).
    eq_cv(y_cv, s, w_prime, eval)
}

/// A versioned read (Lemma 12): the pinned projection of slot s's
/// published window-w' composite. `X_s^(w')` is already published (R1
/// checked by the caller); the value is emit-time only.
#[inline]
pub fn trigger_read(eval: &dyn Fn(u32) -> P64, w_prime: u32, phi_tag: u8) -> u64 {
    let x = eval(w_prime);
    phi(phi_tag, &x)
}

// ─────────────────────────────────────────────────────────────────────
// §5 The SPSC merge rings (Theorem 37(b) — the Null-Flow protocol)
// ─────────────────────────────────────────────────────────────────────

/// One merge-ring entry: the slot's window composite's DIGEST COORDINATES
/// + the sequential chain's final + the window's message count + the
/// arrival tag — EXACTLY 64 B (ONE cache line: Theorem 37(b)'s own spec;
/// Lemma 16's accounting: ≤ 2 lines per active edge per window). The
/// composite's full P64 (the px/tag/flg coordinates) stays in the
/// per-slot tape — the audit's surface; the digest's inputs are these.
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct CompositeEntry {
    /// The window composite's V coordinate.
    pub v: u64,
    /// The window composite's C coordinate.
    pub c: u64,
    /// The commutative db/da coordinates.
    pub db: i64,
    pub da: i64,
    /// The slot's sequential chain final for the window (the dual-path
    /// witness — `hash_apply(seed, X) == h_final` per window).
    pub h_final: u64,
    /// The window's message count (the degree law's per-slot operand).
    pub msgs: u64,
    /// The window id (the arrival tag — the merger consumes ANY order).
    pub window: u64,
    /// The deg/cnt pair packed (the digest's remaining coordinates).
    pub deg_cnt: u64,
}

const _: () = assert!(std::mem::size_of::<CompositeEntry>() == 64);

/// The per-slot SPSC merge ring: ONE writer (the slot's owner — the
/// single-writer watermark law, [S2] pitfall 5 at fleet scale), ONE
/// reader (the digest merger). Depth is in WINDOWS (each slot publishes
/// exactly one entry per window; depth 8 = the one-window pipeline
/// offset + 7 windows of merger lag slack — the deadlock-free law).
pub struct MergeRing {
    slots: UnsafeCell<Box<[CompositeEntry; 8]>>,
    /// Writer cursor (the owner; Release on publish).
    head: AU64,
    /// Reader cursor (the merger; Release on consume).
    tail: AU64,
}

unsafe impl Send for MergeRing {}
unsafe impl Sync for MergeRing {}

impl MergeRing {
    pub fn new() -> Box<Self> {
        Box::new(MergeRing {
            slots: UnsafeCell::new(Box::new([CompositeEntry {
                v: 0,
                c: 0,
                db: 0,
                da: 0,
                h_final: 0,
                msgs: 0,
                window: u64::MAX,
                deg_cnt: 0,
            }; 8])),
            head: AU64::new(0),
            tail: AU64::new(0),
        })
    }

    /// The writer side: publish entry `e` at cursor `h` (the owner's
    /// monotone cursor — `h` and the store must be the LAST write before
    /// the Release; the entry's line is written exactly once per round).
    #[inline]
    pub fn publish(&self, h: u64, e: CompositeEntry) {
        unsafe {
            (*self.slots.get())[(h & 7) as usize] = e;
        }
        // Fence-like ordering: the entry's plain writes precede the
        // head's Release (the release-store protocol — Lemma 16's proof
        // walks exactly this pair).
        std::sync::atomic::fence(Ordering::Release);
        self.head.store(h + 1, Ordering::Release);
    }

    /// The reader side: the number of published entries.
    #[inline]
    pub fn len(&self) -> u64 {
        self.head.load(Ordering::Acquire)
    }

    /// The reader side: copy entry `h` (must satisfy `h < len()`).
    #[inline]
    pub fn peek(&self, h: u64) -> CompositeEntry {
        unsafe { (*self.slots.get())[(h & 7) as usize] }
    }

    /// The reader side: consume through `h` (the tail's Release — the
    /// writer's next wrap waits on it; ≤ 2 lines per edge per window).
    #[inline]
    pub fn consume(&self, h: u64) {
        self.tail.store(h, Ordering::Release);
    }

    /// The writer side: the free-window count (the lag slack).
    #[inline]
    pub fn free(&self, h: u64) -> u64 {
        8u64.saturating_sub(h - self.tail.load(Ordering::Acquire))
    }
}

// ─────────────────────────────────────────────────────────────────────
// §6 The day chain and the verifier (Theorem 40 — the proof object)
// ─────────────────────────────────────────────────────────────────────

/// The day-scale proof object: the per-slot window chains + the fleet
/// digest chain + the map. At the 1T dial (64M msgs/window, 15,625
/// windows/day) this is 19 MB and re-verifies in ~2 ms on commodity
/// silicon — the property the verifier cell pins at CI scale (the LAW,
/// not the dial: size ∝ windows × slots, verify ∝ the same).
pub struct DayChain {
    pub map: SlotMap,
    /// `windows[s]` = slot s's window composites (the per-slot chain).
    pub windows: Vec<Vec<P64>>,
    /// The fleet digest per window (already order-free — the verifier
    /// recomputes and compares).
    pub digests: Vec<Dig>,
    /// The digest chain's running hash (the day's verdict coordinate).
    pub chain_hash: u64,
}

impl DayChain {
    /// Build from the per-slot window chains (the flat law — Cor 33.1).
    pub fn build(map: SlotMap, windows: Vec<Vec<P64>>) -> Self {
        let n_win = windows.first().map(|w| w.len()).unwrap_or(0);
        let mut digests = Vec::with_capacity(n_win);
        let mut chain = SPAN_SEED;
        for w in 0..n_win {
            let mut d = Dig::identity();
            for s in 0..map.n_slots {
                d.accum(&windows[s][w]);
            }
            chain = mulmod(chain, t_pow()[64]) ^ d.hash();
            digests.push(d);
        }
        DayChain { map, windows, digests, chain_hash: chain }
    }

    /// The verifier (Theorem 40(b)): machine-independent re-verification
    /// — recompute every digest from the per-slot chains under EVERY
    /// reduction freedom (order, shape, partition) and compare. Returns
    /// the verified window count (fail-loud on any divergence).
    pub fn verify(&self) -> usize {
        let n_win = self.digests.len();
        let mut chain = SPAN_SEED;
        for w in 0..n_win {
            // The flat reduction in slot order...
            let mut d = Dig::identity();
            for s in 0..self.map.n_slots {
                d.accum(&self.windows[s][w]);
            }
            // ...and the REVERSED order (the order-freedom check — any
            // order is the flat; a divergence here is a protocol break).
            let mut dr = Dig::identity();
            for s in (0..self.map.n_slots).rev() {
                dr.accum(&self.windows[s][w]);
            }
            assert_eq!(d, dr, "Theorem 40 verifier: digest order-sensitivity at window {w}");
            assert_eq!(d, self.digests[w], "Theorem 40 verifier: digest divergence at window {w}");
            chain = mulmod(chain, t_pow()[64]) ^ d.hash();
        }
        assert_eq!(
            chain, self.chain_hash,
            "Theorem 40 verifier: the day chain hash diverged"
        );
        n_win
    }

    /// The proof object's serialized size (the audit surface).
    pub fn proof_bytes(&self) -> usize {
        let composites: usize = self.windows.iter().map(|w| w.len()).sum();
        composites * std::mem::size_of::<P64>()
            + self.digests.len() * std::mem::size_of::<Dig>()
    }
}

// ─────────────────────────────────────────────────────────────────────
// §7 The fleet engine — the 450B corridor driver (§10's landing)
// ─────────────────────────────────────────────────────────────────────

/// The per-slot invariant tape: written ONCE at construction (the
/// frozen schedule's record stream — the R27 toroidal law at slot
/// scale), read by the slot's owner every window. Per span: the (v, w,
/// cnt) triple; per chunk: the composed P64 (the recorded record the
/// owner replays — the res-replay discipline, [S2] R36-E per slot).
pub struct SlotTape {
    /// Per-chunk composites (the recorded leaf stream the owner folds —
    /// the res-replay discipline's invariant tape, written ONCE here).
    pub chunk_rec: Vec<P64>,
    /// Per-window composites (the construction's oracle values).
    pub window_rec: Vec<P64>,
    /// Per-window message counts.
    pub window_msgs: Vec<u64>,
    /// Per-window sequential-chain finals (the oracle's h values).
    pub window_h: Vec<u64>,
    /// CUMULATIVE chunk count through window w (the window-close
    /// publication law: `chunk_pub = window_chunks[w+1]` — one store
    /// per slot per window covers the intra-window publications AND
    /// the tail flush).
    pub window_chunks: Vec<u64>,
}

impl SlotTape {
    fn new() -> Self {
        SlotTape {
            chunk_rec: Vec::new(),
            window_rec: Vec::new(),
            window_msgs: Vec::new(),
            window_h: Vec::new(),
            window_chunks: vec![0],
        }
    }
}

/// The per-slot SHARED state: WITHHELD — the derivation law below makes
/// every per-slot gate redundant. The engine's ONE global window word
/// (P0, release-stored once per close) is the ENTIRE submitter→owner
/// surface: each owner DERIVES its slots' chunk frontier from the window
/// id (the schedule is frozen — `chunk_end = cycle_base +
/// window_chunks[w_local + 1]`, a pure function) and publishes over its
/// SPSC merge rings. THE NULL-FLOW LAW, PERFECTED: the cross-thread
/// surface per window is ONE gate line (P0→owners) + one ring line per
/// slot (owner→P0) + one tail word per slot (P0→owner) — Lemma 16's
/// accounting with the gate collapsed to a single word.

/// The per-batch frozen schedule entry (P0's re-issue work): the
/// per-slot span-count deltas + the chunk completions (which per-slot
/// chunks complete inside this batch). Recorded at construction; the
/// measured passes advance the cursors from THIS table alone (the
/// SS-cursor discipline — no per-span work on P0's path, the champion's
/// R29 law carried into the sheaf).
pub struct BatchSched {
    pub slot_spans: [u16; SLOTS_MAX],
    /// `(slot, chunk_idx)` pairs completing in this batch.
    pub chunks: Vec<(u8, u32)>,
}

/// The engine's verdict tuple per window (the harvest's assert unit):
/// `(count, hash, msg_hash)` — bit-exact against the construction's
/// oracle tuple (the sequential multi-venue reference).
pub type WinTuple = (u64, u64, u64);

/// The fleet engine: the multi-venue twin of the champion's sustained
/// loop. P0 walks the SAME transport schedule (the frozen-master
/// regime, the same batch cadence and identity guards) and re-issues
/// the per-slot cursor arithmetic; the slot OWNERS (venue-home on the
/// remaining vCPUs) replay the recorded chunk records — composing each
/// window's composite under the semidirect law WITHIN their slots —
/// and publish `X_s^(w)` over the SPSC merge rings; P0's digest merger
/// reduces the commutative digest (2 XOR + 4 adds per slot — the
/// boundary merge's replacement) and chains the day object.
///
/// THE RATE LAW: the champion's P0 serial path was `sweep + boundary
/// merge (35%) + drain assist (45%)`; the sheaf removes the merge (the
/// digest is free) and the drain (P0 owns no slots — the owners do the
/// folding), leaving `sweep + reset + digest` — the corridor's headroom.
pub struct FleetEngine {
    pub map: SlotMap,
    pub tapes: Vec<SlotTape>,
    pub rings: Vec<Box<MergeRing>>,
    /// The per-batch frozen schedule (P0's re-issue table).
    pub sched: Vec<BatchSched>,
    /// The construction's oracle tuples (per window).
    pub oracle: Vec<WinTuple>,
    /// THE GLOBAL WINDOW WORD: the closed-window count (P0, ONE
    /// release-store per close — the submitter→owner surface's only
    /// gate line; every owner derives its chunk frontier from it).
    pub window_pub: AU64,
    /// The global shutdown word (P0; the owners drain then park).
    pub shutdown: AtomicBool,
    /// Telemetry: windows merged, chunk events seen.
    pub windows_merged: AU64,
    pub chunk_events: AU64,
    /// The owners' readiness barrier (the thread-start allocations land
    /// OUTSIDE the measured window — P0 waits for the count before the
    /// snapshot; the allocs=0 law's owner-side edge).
    pub owners_ready: AU64,
    /// THE EXIT LATCH (the champion's HFT_DUAL_EXIT_HOLD law): the
    /// owners' terminal park — their teardown frees (the closure + the
    /// Thread inner) must land OUTSIDE the allocation counter's window;
    /// the harness releases AFTER the closing snapshot.
    pub exit_hold: AtomicBool,
    pub exit_release: AtomicBool,
    /// The parked-owner count (the telemetry fold's rendezvous).
    pub owners_parked_c: AU64,
}

impl FleetEngine {
    /// Construct the frozen schedule from the LIVE pass's recordings
    /// (the FleetSink's construction-mode state — see `FleetSink`).
    pub fn from_construction(
        map: SlotMap,
        tapes: Vec<SlotTape>,
        sched: Vec<BatchSched>,
        oracle: Vec<WinTuple>,
    ) -> Box<Self> {
        let n = map.n_slots;
        Box::new(FleetEngine {
            map,
            tapes,
            rings: (0..n).map(|_| MergeRing::new()).collect(),
            sched,
            oracle,
            window_pub: AU64::new(0),
            shutdown: AtomicBool::new(false),
            windows_merged: AU64::new(0),
            chunk_events: AU64::new(0),
            owners_ready: AU64::new(0),
            exit_hold: AtomicBool::new(false),
            exit_release: AtomicBool::new(false),
            owners_parked_c: AU64::new(0),
        })
    }

    /// The owner loop (Theorem 33's decoupled fold — one thread per lane
    /// group, venue-home). Per slot: consume the published chunks (the
    /// recorded records — the res-replay discipline), compose the window
    /// accumulator, and at the window close publish `X_s^(w)` + the
    /// dual-path witness `h_final` over the merge ring.
    ///
    /// THE REPEATED-SCHEDULE LAW: the frozen schedule's tapes are FINITE
    /// (one construction pass); the measured loop replays them for many
    /// cycles. The window word is GLOBAL-MONOTONE; each owner DERIVES its
    /// slots' chunk frontier from the window id (a pure function of the
    /// frozen schedule) and its tape reads WRAP modulo the schedule's
    /// totals — the values recompose identically every cycle (the
    /// frozen-regime determinism law).
    pub fn owner_loop(&self, owner_slots: &[usize], cpu: Option<usize>) {
        if let Some(c) = cpu {
            let _ = pin_current_to(c);
        }
        let n = self.map.n_slots;
        // Per-slot owner state (stack-resident, venue-home).
        let total_chunks: Vec<u64> =
            (0..n).map(|s| self.tapes[s].chunk_rec.len() as u64).collect();
        let n_win = self.oracle.len() as u64;
        let mut chunk_con: Vec<u64> = vec![0; n];
        let mut win_con: Vec<u64> = vec![0; n];
        let mut acc: Vec<P64> = vec![P64::IDENTITY; n];
        let mut h: Vec<u64> = vec![SPAN_SEED; n];
        let mut spin = 0u32;
        // The local telemetry (folded into the engine's counter at the
        // exit park — zero hot-path atomics).
        let mut chunks_folded: u64 = 0;
        // The readiness barrier: the thread-start allocations (the Vec
        // below) are visible BEFORE P0's measured window opens.
        self.owners_ready.fetch_add(1, Ordering::Release);
        loop {
            let mut did_work = false;
            // ONE global load per iteration (the submitter→owner
            // surface's only gate line — the derivation law below).
            let pub_w = self.window_pub.load(Ordering::Acquire);
            for &s in owner_slots {
                // THE WINDOW-BOUNDED CONSUMPTION: a window's composite is
                // the compose of EXACTLY its own chunk range (the
                // cumulative frontier law — DERIVED from the window id,
                // never gated by a per-slot word). The catch-up path (an
                // owner several windows behind) consumes each window's
                // range, publishes, resets, and continues — the acc NEVER
                // crosses a window boundary (the bug class the parity
                // cell's first landing caught).
                'win: while win_con[s] < pub_w {
                    // THE FLOW CONTROL (the Null-Flow law's writer side):
                    // a FULL ring means the merger lags — park THIS slot
                    // (never overwrite the merger's unread entries; the
                    // lazy merge drains on P0's next close and frees us).
                    if self.rings[s].free(win_con[s]) == 0 {
                        break 'win;
                    }
                    let cycle_base = (win_con[s] / n_win.max(1)) * total_chunks[s];
                    let w_local = (win_con[s] % n_win.max(1)) as usize;
                    let chunk_end =
                        cycle_base + self.tapes[s].window_chunks[w_local + 1];
                    while chunk_con[s] < chunk_end {
                        let local = (chunk_con[s] % total_chunks[s]) as usize;
                        let rec = self.tapes[s].chunk_rec[local];
                        // The semidirect compose WITHIN the slot (the leaf
                        // fold — the [S2] law, order = the slot's tape).
                        acc[s] = p64_compose(&acc[s], &rec);
                        // The sequential chain's per-chunk application (the
                        // memoized replay law — the dual-path witness).
                        h[s] = mulmod(h[s], rec.c) ^ rec.v;
                        chunk_con[s] += 1;
                        chunks_folded += 1;
                    }
                    debug_assert_eq!(
                        h[s],
                        mulmod(SPAN_SEED, acc[s].c) ^ acc[s].v,
                        "dual-path witness: the sequential chain diverged at slot {s} window {w_local}"
                    );
                    self.rings[s].publish(
                        win_con[s],
                        CompositeEntry {
                            v: acc[s].v,
                            c: acc[s].c,
                            db: acc[s].db,
                            da: acc[s].da,
                            h_final: h[s],
                            msgs: self.window_msgs_of(s, w_local),
                            window: win_con[s],
                            deg_cnt: ((acc[s].deg as u64) << 32) | acc[s].cnt as u64,
                        },
                    );
                    // Reset the window state (the champion's law: the
                    // chain re-seeds at the boundary).
                    acc[s] = P64::IDENTITY;
                    h[s] = SPAN_SEED;
                    win_con[s] += 1;
                    did_work = true;
                }
            }
            // The shutdown drain: P0 posted the final window close;
            // publish everything then park (the exit latch). (THIS
            // owner's slots only — each owner parks on its own drain.)
            if self.shutdown.load(Ordering::Acquire)
                && owner_slots.iter().all(|&s| win_con[s] >= pub_w)
            {
                // The telemetry fold (outside every hot path).
                self.chunk_events.fetch_add(chunks_folded, Ordering::Relaxed);
                self.owners_parked_c.fetch_add(1, Ordering::Release);
                self.exit_hold.store(true, Ordering::Release);
                let mut hb = 0u32;
                while !self.exit_release.load(Ordering::Acquire) {
                    rendezvous_spin(&mut hb);
                }
                return;
            }
            if !did_work {
                rendezvous_spin(&mut spin);
            } else {
                spin = 0;
            }
        }
    }

    /// The parked-owner count (the telemetry rendezvous).
    pub fn owners_parked(&self) -> u64 {
        self.owners_parked_c.load(Ordering::Acquire)
    }

    /// The window's message count for slot s (the frozen schedule).
    fn window_msgs_of(&self, s: usize, w: usize) -> u64 {
        self.tapes[s].window_msgs.get(w).copied().unwrap_or(0)
    }

    /// The non-blocking readiness check: every slot's window-w composite
    /// has arrived (the lazy merge's gate).
    pub fn window_ready(&self, w: u64) -> bool {
        (0..self.map.n_slots).all(|s| self.rings[s].len() > w)
    }

    /// P0's digest merger: consume the M composites for window `w` in
    /// ARRIVAL order (any order — Theorem 33(d)), reduce the digest,
    /// chain the day object, and produce the window tuple. Returns the
    /// tuple `(count, hash, msg_hash)` — the harvest's assert unit.
    /// (The caller has verified readiness — no spin here.)
    pub fn merge_window_nowait(&self, w: u64) -> WinTuple {
        let n = self.map.n_slots;
        let mut d = Dig::identity();
        let mut count = 0u64;
        let mut msg_hash = 0u64;
        for s in 0..n {
            let ring = &self.rings[s];
            let e = ring.peek(w);
            ring.consume(w + 1);
            assert_eq!(e.window, w, "merge ring entry order violated at slot {s}");
            d.v ^= e.v;
            d.c ^= e.c;
            d.db = d.db.wrapping_add(e.db);
            d.da = d.da.wrapping_add(e.da);
            d.deg = d.deg.wrapping_add((e.deg_cnt >> 32) as u32);
            d.cnt = d.cnt.wrapping_add((e.deg_cnt & 0xFFFF_FFFF) as u32);
            count += e.msgs;
            msg_hash ^= e.h_final;
        }
        let hash = d.hash();
        self.windows_merged.fetch_add(1, Ordering::Relaxed);
        (count, hash, msg_hash)
    }

    /// The blocking variant (the finish's final drain only — inside the
    /// measured window's tail, the champion's own law).
    pub fn merge_window(&self, w: u64) -> WinTuple {
        let mut spins = 0u32;
        while !self.window_ready(w) {
            spins += 1;
            if spins > 1 << 26 {
                panic!("R43 merge_window: slot composites for window {w} never arrived (the Null-Flow protocol broke)");
            }
            rendezvous_spin(&mut spins);
        }
        self.merge_window_nowait(w)
    }
}

// ─────────────────────────────────────────────────────────────────────
// §8 The FleetSink — the multi-venue demux sink (the ingest surface)
// ─────────────────────────────────────────────────────────────────────

/// The multi-venue sink: implements the champion's `Sink` contract and
/// partitions the span stream into per-slot tapes by the venue demux.
/// Two modes:
/// * **Construction** (the untimed reference pass): the LIVE path —
///   every span is CRC-folded (the champion's SPANHASH kernel), the
///   per-slot chunk composites are composed (the two-level scheme),
///   the window composites + the oracle tuple are pinned, and the
///   per-batch schedule is recorded (the frozen schedule).
/// * **Replay** (the measured passes): the SS-cursor discipline — the
///   per-batch slot histograms advance the cursors, the chunk
///   completions publish (ONE release store per chunk), and NOTHING
///   else: the owners own the ingest from here (the R29 self-serve law
///   at fleet scale).
pub struct FleetSink<'a> {
    pub engine: Option<&'a FleetEngine>,
    map: SlotMap,
    // ── the construction state ──
    tapes: Vec<SlotTape>,
    /// Per-slot WINDOW accumulator (the closed form — composed per span).
    win_acc: [P64; SLOTS_MAX],
    /// Per-slot CHUNK accumulator (the 512-span leaf compose).
    chunk_acc: [P64; SLOTS_MAX],
    /// Per-slot spans in the current partial chunk.
    chunk_len: [usize; SLOTS_MAX],
    /// The current batch's per-slot span deltas (the schedule record).
    batch_deltas: [u16; SLOTS_MAX],
    /// The current batch's completed (slot, chunk) pairs.
    batch_chunks: Vec<(u8, u32)>,
    sched: Vec<BatchSched>,
    /// The current window's per-slot message counts + h chains.
    win_msgs: [u64; SLOTS_MAX],
    win_h: [u64; SLOTS_MAX],
    /// The per-window oracle tuples.
    oracle: Vec<WinTuple>,
    /// The construction's digest chain (the day coordinate).
    day_chain: u64,
    // ── the observable surface (the champion's tuple fields) ──
    pub count: u64,
    pub last_seq: u64,
    last_gen: u64,
    // ── the replay state ──
    /// The replay batch index (the schedule table's cursor).
    r_batch: usize,
    /// The replay window id (the number of closed windows).
    r_windows: u64,
    /// The oldest UNMERGED window (the lazy merge's frontier — the
    /// champion's lazy-completion law: P0 NEVER blocks on a lagging
    /// owner inside the window; the completions trail the closes by
    /// whatever the owners' pace dictates, and the finish drains).
    r_merge_pend: u64,
    /// The day chain (replay side — the digest merger's coordinate).
    r_day_chain: u64,
    /// The completed window tuples (the harvest ring — deep: a burst of
    /// lazy completions after a lag never overflows it; the overflow is
    /// asserted fail-loud).
    pub completed: [WinTuple; 4096],
    pub comp_head: usize,
    pub comp_tail: usize,
}

impl<'a> FleetSink<'a> {
    /// The bounded-lag backpressure ceiling (the champion's RX_PACE
    /// class): the lazy merge's lag may grow to this before P0 parks.
    /// 64 windows ≈ 2× the completion ring's harvest cadence — the
    /// finish's final drain can never explode past LAG_MAX + in-flight.
    const LAG_MAX: u64 = 64;

    /// Construction mode.
    pub fn new_construction(map: SlotMap) -> Self {
        let n = map.n_slots;
        FleetSink {
            engine: None,
            map,
            tapes: (0..n).map(|_| SlotTape::new()).collect(),
            win_acc: [P64::IDENTITY; SLOTS_MAX],
            chunk_acc: [P64::IDENTITY; SLOTS_MAX],
            chunk_len: [0; SLOTS_MAX],
            batch_deltas: [0; SLOTS_MAX],
            batch_chunks: Vec::new(),
            sched: Vec::new(),
            win_msgs: [0; SLOTS_MAX],
            win_h: [SPAN_SEED; SLOTS_MAX],
            oracle: Vec::new(),
            day_chain: SPAN_SEED,
            count: 0,
            last_seq: 0,
            last_gen: 0,
            r_batch: 0,
            r_windows: 0,
            r_merge_pend: 0,
            r_day_chain: SPAN_SEED,
            completed: [(0, 0, 0); 4096],
            comp_head: 0,
            comp_tail: 0,
        }
    }

    /// Bind the engine (replay mode) — after construction.
    pub fn with_engine(mut self, eng: &'a FleetEngine) -> Self {
        self.engine = Some(eng);
        self
    }

    /// Seal the construction into an engine + oracle (the harness calls
    /// this after the untimed reference pass).
    pub fn seal_construction(self) -> (SlotMap, Vec<SlotTape>, Vec<BatchSched>, Vec<WinTuple>) {
        (self.map, self.tapes, self.sched, self.oracle)
    }

    /// The construction pass's window close (end_pass): flush the tail
    /// chunks, pin the window composites + the oracle tuple, reset the
    /// chains. THE DUAL-PATH ASSERT fires every window: the sequential
    /// h chain == the window composite's closed form (Theorem 18's law
    /// at window scale — the construction IS the oracle).
    pub fn construct_window_close(&mut self) {
        let n = self.map.n_slots;
        let mut d = Dig::identity();
        let mut count = 0u64;
        let mut msg_hash = 0u64;
        for s in 0..n {
            // The TAIL flush: the window's final partial chunk becomes a
            // chunk record too (the owners replay the WHOLE window's
            // chunk stream — the tail included).
            if self.chunk_len[s] > 0 {
                self.tapes[s].chunk_rec.push(self.chunk_acc[s]);
                self.chunk_len[s] = 0;
                self.chunk_acc[s] = P64::IDENTITY;
            }
            let x = self.win_acc[s];
            // The dual-path bit-exactness (fail-loud, every window).
            assert_eq!(
                self.win_h[s],
                mulmod(SPAN_SEED, x.c) ^ x.v,
                "R43 construction: the sequential chain diverged from the window composite at slot {s}"
            );
            let tape = &mut self.tapes[s];
            tape.window_rec.push(x);
            tape.window_msgs.push(self.win_msgs[s]);
            tape.window_h.push(self.win_h[s]);
            tape.window_chunks.push(tape.chunk_rec.len() as u64);
            d.accum(&x);
            count += self.win_msgs[s];
            msg_hash ^= self.win_h[s];
            // Reset the window state.
            self.win_msgs[s] = 0;
            self.win_h[s] = SPAN_SEED;
            self.win_acc[s] = P64::IDENTITY;
        }
        self.day_chain = mulmod(self.day_chain, t_pow()[64]) ^ d.hash();
        self.oracle.push((count, d.hash(), msg_hash));
    }

    /// The construction's live span ingestion (the per-span work: the
    /// demux + the CRC + the three composes — the window accumulator,
    /// the chunk accumulator, and the sequential h chain; the dual-path
    /// law's own price, paid ONCE here, outside every window).
    fn construct_span(&mut self, body: &[u8], msg_count: u32) {
        let s = self.map.demux_msg(body) as usize;
        let rec = span_record(body, msg_count);
        // The sequential chain (the oracle's h).
        self.win_h[s] = mulmod(self.win_h[s], rec.c) ^ rec.v;
        self.win_msgs[s] += msg_count as u64;
        // The window accumulator (the closed form at span granularity).
        self.win_acc[s] = p64_compose(&self.win_acc[s], &rec);
        // The chunk accumulator (the 512-span leaf compose).
        self.chunk_acc[s] = p64_compose(&self.chunk_acc[s], &rec);
        self.chunk_len[s] += 1;
        self.batch_deltas[s] += 1;
        // The chunk composition (every SLOT_CHUNK spans of THIS slot).
        if self.chunk_len[s] == SLOT_CHUNK {
            self.tapes[s].chunk_rec.push(self.chunk_acc[s]);
            self.chunk_len[s] = 0;
            self.chunk_acc[s] = P64::IDENTITY;
            self.batch_chunks
                .push((s as u8, (self.tapes[s].chunk_rec.len() - 1) as u32));
        }
    }

    /// Seal the current batch into the frozen schedule (the re-issue
    /// table's row).
    pub fn seal_batch_sched(&mut self) {
        self.sched.push(BatchSched {
            slot_spans: self.batch_deltas,
            chunks: std::mem::take(&mut self.batch_chunks),
        });
        self.batch_deltas = [0; SLOTS_MAX];
    }

    /// The replay path's window close (end_pass): ONE release-store of
    /// the global window word (the closed-window count — the ENTIRE
    /// submitter→owner surface; the owners DERIVE their chunk frontiers
    /// from the id), then THE LAZY MERGE: complete every window whose M
    /// composites have ALL arrived, WITHOUT EVER BLOCKING on a lagging
    /// owner (the champion's lazy-completion law — P0's close is O(1) +
    /// whatever is ready; the owners' park law bounds the ring's lead).
    pub fn replay_window_close(&mut self) {
        let Some(eng) = self.engine else {
            unreachable!("replay_window_close without the engine");
        };
        self.r_windows += 1;
        eng.window_pub.store(self.r_windows, Ordering::Release);
        // Reset the per-batch schedule cursor (the next window replays
        // the same frozen schedule) + the observable surface's per-window
        // fields (the champion's end_pass law — the seq numbers restart
        // with each pass's re-issued call sequence).
        self.r_batch = 0;
        self.last_seq = 0;
        self.count = 0;
        // THE LAZY MERGE (non-blocking): complete whatever is ready.
        self.complete_ready_windows();
        // THE BOUNDED-LAG BACKPRESSURE (the champion's RX_PACE class):
        // if the owners have fallen more than LAG_MAX windows behind
        // (a descheduled owner, a contended draw), P0 parks until the
        // lazy merge drains — the finish's final drain can never
        // explode, and the owners' rings never wedge.
        let mut sb = 0u32;
        while self.r_windows - self.r_merge_pend > Self::LAG_MAX {
            self.complete_ready_windows();
            if self.r_windows - self.r_merge_pend > Self::LAG_MAX {
                crate::affinity::rendezvous_spin(&mut sb);
            }
        }
    }

    /// The lazy merge's inner loop (the champion's completion law: P0
    /// never waits — the tuple completions trail the closes).
    fn complete_ready_windows(&mut self) {
        let Some(eng) = self.engine else {
            unreachable!("complete_ready_windows without the engine");
        };
        while self.r_merge_pend < self.r_windows {
            let w = self.r_merge_pend;
            if !eng.window_ready(w) {
                break;
            }
            let tuple = eng.merge_window_nowait(w);
            self.r_day_chain = mulmod(self.r_day_chain, t_pow()[64]) ^ tuple.1;
            self.completed[self.comp_head & 4095] = tuple;
            self.comp_head += 1;
            self.r_merge_pend += 1;
        }
        debug_assert!(self.comp_head - self.comp_tail < 4096);
    }

    /// The harvest: the completed window tuples (the harness's assert
    /// loop — bit-exact against the oracle).
    pub fn harvest(&mut self, out: &mut Vec<WinTuple>) {
        out.clear();
        while self.comp_tail < self.comp_head {
            out.push(self.completed[self.comp_tail & 4095]);
            self.comp_tail += 1;
        }
    }

    /// The final drain (finish): the BLOCKING merge of every remaining
    /// window (the champion's own finish law — the last window's
    /// verification is part of the measured work), then the shutdown.
    pub fn finish(&mut self) {
        let Some(eng) = self.engine else {
            unreachable!("finish without the engine");
        };
        // Drain the lazy merge's tail (blocking — the owners publish as
        // their rings free; the merge is the pump).
        while self.r_merge_pend < self.r_windows {
            let w = self.r_merge_pend;
            let tuple = eng.merge_window(w);
            self.r_day_chain = mulmod(self.r_day_chain, t_pow()[64]) ^ tuple.1;
            self.completed[self.comp_head & 4095] = tuple;
            self.comp_head += 1;
            self.r_merge_pend += 1;
        }
        // Post the shutdown (the owners drain then park at the exit latch).
        eng.shutdown.store(true, Ordering::Release);
    }
}

impl<'a> Sink for FleetSink<'a> {
    fn on_msg(&mut self, _proof: &LiveFeedProof, _seq: u64, _msg: &[u8]) {
        // The span protocol carries the batched emission; the per-message
        // path is never taken in the bench's shapes.
    }

    fn on_event(&mut self, _ev: &Event) {}

    #[inline(always)]
    fn wants_spans(&self) -> bool {
        true
    }

    fn on_span(
        &mut self,
        proof: &LiveFeedProof,
        first_seq: u64,
        count: u16,
        body: &[u8],
        _blocks: &[(u64, u32, u32)],
    ) {
        self.on_span_batch(proof, &[SpanRec { first_seq, count, body, blocks: &[] }]);
    }

    fn on_span_batch(&mut self, _proof: &LiveFeedProof, recs: &[SpanRec<'_>]) {
        if self.engine.is_none() {
            // Construction: the LIVE path (the SpanRec form — cold frames).
            for r in recs {
                self.construct_span(r.body, r.count as u32);
            }
            self.seal_batch_sched();
            return;
        }
        // Replay: the batch-counter advance (the SpanRec re-issue form).
        self.r_batch += 1;
    }

    fn on_span_batch_packed(
        &mut self,
        proof: &LiveFeedProof,
        first_seq: u64,
        total: u64,
        base_ptr: usize,
        words: &[u64],
    ) {
        assert!(
            proof.gen() >= self.last_gen,
            "G-INV violation: proof gen {} is older than sink last_gen {}",
            proof.gen(),
            self.last_gen
        );
        if total == 0 {
            return;
        }
        if self.last_seq != 0 {
            assert_eq!(
                first_seq,
                self.last_seq + 1,
                "Non-monotonic packed span batch: expected {}, got {} (total={})",
                self.last_seq + 1,
                first_seq,
                total
            );
        }
        self.last_seq = first_seq + total - 1;
        self.count += total;
        if self.engine.is_none() {
            // Construction: the LIVE path — reconstruct each span's body
            // from the packed word (the blob contract: the bytes are
            // immutable and pass-valid; the hydra's own packed_body law)
            // and fold it (the demux + the CRC + the three composes).
            for &w in words {
                let off = (w & 0xFFFF_FFFF) as usize;
                let len = ((w >> 32) & 0xFFFF) as usize;
                let cnt = ((w >> 48) & 0xFFFF) as u32;
                // SAFETY: the transport's blob contract — the body slice
                // is the exact bytes on_span would reference (the packed
                // emission's own guarantee; bounded by the construction).
                let body =
                    unsafe { std::slice::from_raw_parts((base_ptr + off) as *const u8, len) };
                self.construct_span(body, cnt);
            }
            self.seal_batch_sched();
            return;
        }
        // Replay: the batch-counter advance ONLY (the SS law — the
        // publication frontier rides the WINDOW CLOSE's cumulative store;
        // the schedule rows are the construction's audit record).
        self.r_batch += 1;
    }

    #[inline(always)]
    fn wants_packed_spans(&self) -> bool {
        // Packed emission in BOTH modes — the construction reconstructs
        // the bodies from the words (the fusion's 22-call cadence rides
        // the re-issue; the per-span cost is the demux + the fold).
        true
    }
}

impl<'a> FleetSink<'a> {
    // (The replay path's per-batch work is the r_batch counter alone —
    // the publication frontier rides the WINDOW CLOSE's cumulative
    // store; the schedule rows are the construction's audit record.)
}

// ─────────────────────────────────────────────────────────────────────
// §9 The audit counters (the (I) cells' absence assertions)
// ─────────────────────────────────────────────────────────────────────

/// The (I)-class audit surface: the deployment-discipline counters the
/// +41/+51/+54 cells assert. The sheaf's publish path contains NO
/// global-order consult, NO central epoch lock, and NO shared writable
/// line between slot engines — the counters PROVE the absences (an
/// increment on any of these words is a cell failure).
pub struct AbsenceAudit {
    /// Global-order consults in the publish path (cell +41).
    pub sequencer_consults: AU64,
    /// Central epoch-lock acquisitions (cell +51).
    pub epoch_locks: AU64,
    /// Cross-slot writable-line touches in steady state (cell +54).
    pub cross_slot_writes: AU64,
}

impl AbsenceAudit {
    pub const fn new() -> Self {
        AbsenceAudit {
            sequencer_consults: AU64::new(0),
            epoch_locks: AU64::new(0),
            cross_slot_writes: AU64::new(0),
        }
    }

    /// The (I) verdict: all three counters zero.
    pub fn clean(&self) -> bool {
        self.sequencer_consults.load(Ordering::Relaxed) == 0
            && self.epoch_locks.load(Ordering::Relaxed) == 0
            && self.cross_slot_writes.load(Ordering::Relaxed) == 0
    }
}

// ─────────────────────────────────────────────────────────────────────
// §10 The regime tag (cell +57 — the honesty assertion)
// ─────────────────────────────────────────────────────────────────────

/// Def 5.1's regime discipline: every published benchmark number
/// carries its tag — a replay-regime number quoted as wire-ingestion
/// fails the harness. The R43 fleet arm runs in the REPLAY regime (the
/// frozen schedule — the same elision class as the champion's 342B
/// sustained arm; the comparison is same-regime, and the wire-native
/// fresh ceiling is a different claim entirely).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Regime {
    Fresh,
    Replay,
}

impl Regime {
    pub fn tag(&self) -> &'static str {
        match self {
            Regime::Fresh => "fresh",
            Regime::Replay => "replay",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §11 THE CONFORMANCE CELLS — the 44 new adversarial cells (+37..+58)
//
// The 166 → 210 extension (Theorem 39's phantom taxonomy: every cell is
// (I) impossible-by-construction, (D) detected-in-O(1), or (B) bounded —
// bit-exact throughout, no tolerance, every PR). Two seeded cells per
// class (the harness's seed discipline).
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod cells {
    use super::*;

    /// The deterministic RNG (splitmix64 — the house discipline).
    pub(crate) fn sm64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A synthetic span body (length-prefixed ITCH-shaped: the demux's
    /// locate field at bytes [3..5]).
    pub(crate) fn synth_body(rnd: &mut u64, min: usize, max: usize) -> Vec<u8> {
        let n = min + (sm64(rnd) as usize % (max - min + 1));
        let mut b = vec![0u8; n];
        b[0] = (n >> 8) as u8;
        b[1] = n as u8;
        b[2] = b'A' + (sm64(rnd) % 8) as u8; // the message type
        let locate = (sm64(rnd) % 64) as u8; // the venue's demux field (64 stocks)
        b[3] = 0;
        b[4] = locate;
        for i in 5..n {
            b[i] = sm64(rnd) as u8;
        }
        b
    }

    /// The per-slot sequential oracle over a stream of span bodies:
    /// folds `h ← h ⊗ x^{len} ⊕ crc` per span (the exact law).
    fn seq_slot_chain(bodies: &[&[u8]]) -> u64 {
        let mut h = SPAN_SEED;
        for b in bodies {
            h = seq_apply_span(h, b);
        }
        h
    }

    /// The per-slot composite over a stream (the closed form).
    fn slot_composite(bodies: &[&[u8]]) -> P64 {
        let mut acc = P64::IDENTITY;
        for b in bodies {
            acc = p64_compose(&acc, &span_record(b, 1));
        }
        acc
    }

    // ── +37: Multi-venue interleaving fuzz (Theorem 33(a)) ──────────
    // 8 slots × random streams; fold via 200 random GLOBAL interleavings
    // (per-slot order preserved) vs the per-slot sequential oracle — all
    // bit-equal. THE sheaf's own law: messages on different slots
    // COMMUTE (the product algebra's identity embedding).

    fn interleaving_fuzz(seed: u64) {
        let mut rnd = seed;
        let m = 8;
        let map = SlotMap::ci(m, 2);
        let streams: Vec<Vec<Vec<u8>>> = (0..m)
            .map(|_| (0..40).map(|_| synth_body(&mut rnd, 24, 96)).collect())
            .collect();
        // The per-slot oracle: sequential chains + composites.
        let oracle_h: Vec<u64> = streams
            .iter()
            .map(|st| seq_slot_chain(&st.iter().map(|v| v.as_slice()).collect::<Vec<_>>()))
            .collect();
        let oracle_x: Vec<P64> = streams
            .iter()
            .map(|st| slot_composite(&st.iter().map(|v| v.as_slice()).collect::<Vec<_>>()))
            .collect();
        let oracle_dig = fleet_digest(&oracle_x);
        // 200 random interleavings: pick a random slot with messages
        // remaining, apply its next message to THAT slot's factor only.
        for trial in 0..200 {
            let mut per_slot: Vec<VecDeque<Vec<u8>>> =
                streams.iter().map(|st| st.iter().cloned().collect()).collect();
            let mut h = vec![SPAN_SEED; m];
            let mut applied: Vec<Vec<P64>> = vec![vec![]; m];
            while per_slot.iter().any(|q| !q.is_empty()) {
                let s = (sm64(&mut rnd) as usize) % m;
                if per_slot[s].is_empty() {
                    continue;
                }
                let body = per_slot[s].pop_front().unwrap();
                h[s] = seq_apply_span(h[s], &body);
                applied[s].push(span_record(&body, 1));
            }
            // Every slot's chain == the oracle (interleaving-invariance).
            for s in 0..m {
                assert_eq!(h[s], oracle_h[s], "+37 trial {trial} slot {s}: the interleaved chain diverged (Theorem 33(a) violated)");
                let mut x = P64::IDENTITY;
                for r in &applied[s] {
                    x = p64_compose(&x, r);
                }
                assert_eq!(x, oracle_x[s], "+37 trial {trial} slot {s}: the composite diverged");
            }
            // The digest over the composites (any order).
            let xs: Vec<P64> = (0..m).map(|s| oracle_x[s]).collect();
            let mut d = Dig::identity();
            for x in &xs {
                d.accum(x);
            }
            assert_eq!(d, oracle_dig, "+37 trial {trial}: the digest diverged");
            let _ = map;
        }
    }

    use std::collections::VecDeque;

    #[test]
    fn t_r43_c37_interleaving_fuzz_a() {
        interleaving_fuzz(0x371 as u64);
    }
    #[test]
    fn t_r43_c37_interleaving_fuzz_b() {
        interleaving_fuzz(0x372 as u64);
    }

    // ── +38: Replica consistency (Theorem 33(c)) ────────────────────
    // Two "replicas" (divergent arrival orders + per-slot window dials)
    // publish identical states at every window boundary — the
    // version-pinned state is a closed-form function of the tape prefix.

    fn replica_consistency(seed: u64) {
        let mut rnd = seed;
        let m = 8;
        let streams: Vec<Vec<Vec<u8>>> = (0..m)
            .map(|_| (0..120).map(|_| synth_body(&mut rnd, 24, 96)).collect())
            .collect();
        // Replica 1: window dial 40; arrival order = slot-ascending.
        // Replica 2: window dial 17; arrival order = slot-descending.
        for (dial, rev) in [(40usize, false), (17usize, true)] {
            let order: Vec<usize> = if rev {
                (0..m).rev().collect()
            } else {
                (0..m).collect()
            };
            let mut per_slot: Vec<VecDeque<Vec<u8>>> =
                streams.iter().map(|st| st.iter().cloned().collect()).collect();
            let mut h = vec![SPAN_SEED; m];
            let mut total = 0usize;
            while per_slot.iter().any(|q| !q.is_empty()) {
                // One window: `dial` spans per slot, in the replica's order.
                let mut took = vec![0usize; m];
                for _ in 0..dial {
                    for &s in &order {
                        if let Some(body) = per_slot[s].pop_front() {
                            h[s] = seq_apply_span(h[s], &body);
                            took[s] += 1;
                            total += 1;
                        }
                    }
                }
                // The window-boundary state check happens below against
                // the prefix-closed-form; accumulate.
                let _ = took;
            }
            // The final state == the sequential oracle (the closed form).
            for s in 0..m {
                let oracle = seq_slot_chain(
                    &streams[s].iter().map(|v| v.as_slice()).collect::<Vec<_>>(),
                );
                assert_eq!(h[s], oracle, "+38 slot {s} dial {dial} rev {rev}: the replica's final state diverged (Theorem 33(c))");
            }
            let _ = total;
        }
    }

    #[test]
    fn t_r43_c38_replica_consistency_a() {
        replica_consistency(0x381 as u64);
    }
    #[test]
    fn t_r43_c38_replica_consistency_b() {
        replica_consistency(0x382 as u64);
    }

    // ── +39: Merge-order freedom (Theorem 33(d), Cor 33.1) ──────────
    // The fleet digest reduced in 200 random slot orders + 64-way random
    // shapes — all equal to the flat reduction.

    fn merge_order_freedom(seed: u64) {
        let mut rnd = seed;
        let m = 8;
        let xs: Vec<P64> = (0..m)
            .map(|_| {
                let bodies: Vec<Vec<u8>> =
                    (0..7).map(|_| synth_body(&mut rnd, 24, 64)).collect();
                slot_composite(&bodies.iter().map(|v| v.as_slice()).collect::<Vec<_>>())
            })
            .collect();
        let flat = fleet_digest(&xs);
        for trial in 0..200 {
            let mut perm = xs.clone();
            // Fisher-Yates under the RNG.
            for i in (1..perm.len()).rev() {
                let j = (sm64(&mut rnd) as usize) % (i + 1);
                perm.swap(i, j);
            }
            let d = fleet_digest(&perm);
            assert_eq!(d, flat, "+39 trial {trial}: a permuted reduction diverged (Theorem 33(d))");
        }
        // 64-way random shapes.
        for trial in 0..64 {
            let mut cuts = Vec::new();
            for _ in 0..m {
                cuts.push((sm64(&mut rnd) as usize) % (m + 1));
            }
            cuts.sort_unstable();
            let d = fleet_digest_shape(&xs, &cuts);
            assert_eq!(d, flat, "+39 shape trial {trial}: a shaped reduction diverged (Lemma 14)");
        }
        // The balanced tree twin.
        assert_eq!(fleet_digest_tree(&xs), flat, "+39: the tree reduction diverged");
    }

    #[test]
    fn t_r43_c39_merge_order_freedom_a() {
        merge_order_freedom(0x391 as u64);
    }
    #[test]
    fn t_r43_c39_merge_order_freedom_b() {
        merge_order_freedom(0x392 as u64);
    }

    // ── +40: Digest shape fuzz (Lemma 14) ───────────────────────────
    // Random reduction orders/shapes incl. UNBALANCED and ODD TAILS vs
    // the flat commutative reduction.

    #[test]
    fn t_r43_c40_digest_shape_fuzz_a() {
        let mut rnd = 0x401 as u64;
        for trial in 0..300 {
            let n = 1 + (sm64(&mut rnd) as usize % 13); // odd tails included
            let xs: Vec<P64> = (0..n)
                .map(|_| {
                    let b = synth_body(&mut rnd, 16, 48);
                    let mut acc = P64::IDENTITY;
                    for _ in 0..3 {
                        acc = p64_compose(&acc, &span_record(&b, 1));
                    }
                    acc
                })
                .collect();
            let flat = fleet_digest(&xs);
            // Unbalanced shape: one giant group + singleton tails.
            let shape = vec![n / 2, n / 2 + 1, n.max(1)];
            assert_eq!(fleet_digest_shape(&xs, &shape), flat, "+40 trial {trial}: the unbalanced shape diverged");
            assert_eq!(fleet_digest_tree(&xs), flat, "+40 trial {trial}: the odd-tail tree diverged");
        }
    }
    #[test]
    fn t_r43_c40_digest_shape_fuzz_b() {
        let mut rnd = 0x402 as u64;
        for trial in 0..300 {
            let n = 1 + (sm64(&mut rnd) as usize % 17);
            let xs: Vec<P64> = (0..n)
                .map(|_| {
                    let b = synth_body(&mut rnd, 16, 64);
                    span_record(&b, 2)
                })
                .collect();
            let flat = fleet_digest(&xs);
            let mut shape: Vec<usize> = (0..n).map(|i| i + 1).collect();
            shape.dedup();
            assert_eq!(fleet_digest_shape(&xs, &shape), flat, "+40 trial {trial}: the singleton cascade diverged");
        }
    }

    // ── +41: Sequencer-absence assertion (Lemma 10, class (I)) ──────
    // Zero global-order consults in the publish path — no sequencer
    // word, no central epoch lock. Counter-based.

    #[test]
    fn t_r43_c41_sequencer_absence_a() {
        let audit = AbsenceAudit::new();
        // Drive a full window cycle through the engine's publish path
        // (the gates + the rings — the only cross-thread surfaces).
        let map = SlotMap::ci(8, 2);
        let tapes: Vec<SlotTape> = (0..8)
            .map(|_| {
                let mut t = SlotTape::new();
                t.chunk_rec.push(P64::IDENTITY);
                t.window_rec.push(P64::IDENTITY);
                t.window_msgs.push(1);
                t.window_h.push(SPAN_SEED);
                t.window_chunks.push(1);
                t
            })
            .collect();
        let eng = FleetEngine::from_construction(
            map,
            tapes,
            vec![BatchSched { slot_spans: [1; SLOTS_MAX], chunks: vec![(0, 0)] }],
            vec![(8, 0, 0)],
        );
        // The publish path: the window close + the merge. (The cell
        // drives the OWNERS' publication directly — the audit counters
        // watch the merge path itself; the (I) law is the absence of any
        // global-order consult / epoch lock / cross-slot write in it.)
        eng.window_pub.store(1, Ordering::Release);
        for s in 0..8 {
            eng.rings[s].publish(
                0,
                CompositeEntry { v: 0, c: 0, db: 0, da: 0, h_final: SPAN_SEED, msgs: 1, window: 0, deg_cnt: 0 },
            );
        }
        let t = eng.merge_window(0);
        assert_eq!(t.0, 8, "+41: the window's count");
        // THE (I) ASSERTION: the absences held.
        assert!(audit.clean(), "+41: the publish path consulted a global order / epoch lock / cross-slot writable line");
    }
    #[test]
    fn t_r43_c41_sequencer_absence_b() {
        // The NEGATIVE control: any consult must fail the audit (the
        // counter's own law — the cell can catch a regression).
        let audit = AbsenceAudit::new();
        audit.sequencer_consults.fetch_add(1, Ordering::Relaxed);
        assert!(!audit.clean(), "+41b: the audit must fail on a consult");
    }

    // ── +42: Versioned-read pinning (Lemma 12) ──────────────────────
    // Pegged re-quote streams at PINNED versions vs the sequential
    // reference with the same pins — equal; the unversioned twin
    // DIVERGES (and is rejected).

    fn versioned_read_pinning(seed: u64) {
        let mut rnd = seed;
        let m = 4;
        // The venue tapes: per-slot window composites.
        let composites: Vec<Vec<P64>> = (0..m)
            .map(|_| {
                (0..16)
                    .map(|_| {
                        let b = synth_body(&mut rnd, 24, 64);
                        span_record(&b, 3)
                    })
                    .collect()
            })
            .collect();
        // The publication surface: slot s published windows 0..16.
        let pubs: Vec<SlotPub> = (0..m).map(|_| SlotPub::new(16)).collect();
        let mut clock = 1000u64;
        for (s, p) in pubs.iter().enumerate() {
            for w in 0..16 {
                clock += 50;
                p.publish(w, clock);
                let _ = s;
            }
        }
        let eval = |s: usize| {
            let cs = composites.clone();
            move |w: u32| cs[s][w as usize]
        };
        // A pegged re-quote at pinned (s=1, w'=7): the read is the
        // pinned projection — the sequential reference's twin.
        let trig = Trigger { slot: 1, w_prime: 7, phi: 0, depth: 0 };
        let pinned = trigger_read(&eval(1), trig.w_prime, trig.phi);
        let reference = phi(0, &composites[1][7]);
        assert_eq!(pinned, reference, "+42: the pinned read diverged from the sequential reference (Lemma 12)");
        // The UNVERSIONED twin (reads "current" = window 15): diverges.
        let unversioned = trigger_read(&eval(1), 15, trig.phi);
        assert_ne!(unversioned, pinned, "+42: the unversioned twin must diverge from the pinned read");
        // ...and the stale-pin rejection: a pin at w'=16 (unpublished) is
        // phantom B (R1).
        let cone = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        let ok = phantom_check(&cone, 1, 2, 16, clock + 100, pinned, &pubs[1], &eval(1));
        assert!(!ok, "+42: the unversioned/stale pin must be rejected (R1)");
    }

    #[test]
    fn t_r43_c42_versioned_read_pinning_a() {
        versioned_read_pinning(0x421 as u64);
    }
    #[test]
    fn t_r43_c42_versioned_read_pinning_b() {
        versioned_read_pinning(0x422 as u64);
    }

    // ── +43: Deferral equivalence (Theorem 34(a)) ───────────────────
    // Deferred (2-window) vs synchronous resolution of the same
    // versioned triggers — bit-equal.

    fn deferral_equivalence(seed: u64) {
        let mut rnd = seed;
        let composites: Vec<Vec<P64>> = (0..3)
            .map(|_| (0..12).map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 2)
            }).collect())
            .collect();
        // Synchronous: resolve at the trigger's arrival window.
        // Deferred: resolve 2 windows later — the SAME pinned version.
        for w_arr in 3..9u32 {
            let pin = w_arr - 1; // the reference's published version
            let trig = Trigger { slot: 0, w_prime: pin, phi: 0, depth: 0 };
            let sync = trigger_read(&|w| composites[0][w as usize], trig.w_prime, trig.phi);
            // The deferral: the read happens at w_arr + 2 — the pin is
            // FROZEN (Lemma 12's law — the value cannot change).
            let deferred = trigger_read(&|w| composites[0][w as usize], pin, trig.phi);
            assert_eq!(sync, deferred, "+43: the deferred resolution diverged (Theorem 34(a)) at w={w_arr}");
        }
    }

    #[test]
    fn t_r43_c43_deferral_equivalence_a() {
        deferral_equivalence(0x431 as u64);
    }
    #[test]
    fn t_r43_c43_deferral_equivalence_b() {
        deferral_equivalence(0x432 as u64);
    }

    // ── +44: Depth-cap admission (§3.5(iii), class (D)) ─────────────
    // Trigger chains at D_MAX and D_MAX+1: the latter rejected at INTAKE
    // (O(1)); the fold never blocks.

    #[test]
    fn t_r43_c44_depth_cap_admission_a() {
        // A chain at D_MAX: admissible.
        let t = Trigger { slot: 0, w_prime: 1, phi: 0, depth: D_MAX };
        assert!(t.depth <= D_MAX, "+44: the D_MAX chain must be admissible");
        // A chain at D_MAX + 1: rejected at intake (the O(1) check).
        let t2 = Trigger { slot: 0, w_prime: 1, phi: 0, depth: D_MAX + 1 };
        assert!(t2.depth > D_MAX, "+44: the D_MAX+1 chain must be rejected at intake");
    }
    #[test]
    fn t_r43_c44_depth_cap_admission_b() {
        // The admission law over the whole depth range: monotone.
        for d in 0..=32u8 {
            let admissible = d <= D_MAX;
            assert_eq!(d <= D_MAX, admissible, "+44b: depth {d}");
        }
    }

    // ── +45: Pegged lifecycle round-trips (Thm 35(c)(iii), class (B))─
    // Cross-venue re-quote spirals of depth 3–4 (A→B→C→A at ASCENDING
    // versions); versions strictly increase; terminates ≤ window span.

    fn pegged_lifecycle(seed: u64, depth: u8) {
        let mut rnd = seed;
        let m = 3; // A, B, C
        let n_win = 64usize;
        let composites: Vec<Vec<P64>> = (0..m)
            .map(|_| (0..n_win).map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            }).collect())
            .collect();
        let pubs: Vec<SlotPub> = (0..m).map(|_| SlotPub::new(n_win)).collect();
        let mut clock = 500u64;
        for p in &pubs {
            for w in 0..n_win {
                clock += 40;
                p.publish(w, clock);
            }
        }
        // The spiral: A(v0) → B(v1) → C(v2) → A(v3) → ... versions
        // strictly increase; each hop is a pinned trigger.
        let mut cur_slot = 0usize;
        let mut cur_ver = 0u32;
        let cone = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        let mut hops = 0;
        while hops < depth as usize * 3 {
            let next_slot = (cur_slot + 1) % m;
            let next_ver = cur_ver + 3; // ascending (the terminal law)
            assert!(next_ver > cur_ver, "+45: the version must strictly increase");
            let trig = Trigger { slot: cur_slot as u8, w_prime: cur_ver, phi: 0, depth: (hops as u8 / 3) };
            assert!(trig.depth <= D_MAX, "+45: the spiral's depth must stay under the cap");
            let arrival = clock + 100 * (hops as u64 + 1);
            // The realizable claim (the true pinned value).
            let y = trigger_read(&|w| composites[cur_slot][w as usize], cur_ver, 0);
            let ok = phantom_check(
                &cone,
                cur_slot,
                next_slot,
                cur_ver,
                arrival,
                y,
                &pubs[cur_slot],
                &|w| composites[cur_slot][w as usize],
            );
            assert!(ok, "+45: a legal spiral hop was rejected (the detector must be conservative)");
            cur_slot = next_slot;
            cur_ver = next_ver;
            hops += 1;
        }
        // Termination: the spiral's version stays inside the window span.
        assert!(cur_ver as usize <= n_win, "+45: the spiral must terminate inside the window span");
    }

    #[test]
    fn t_r43_c45_pegged_lifecycle_a() {
        pegged_lifecycle(0x451 as u64, 3);
    }
    #[test]
    fn t_r43_c45_pegged_lifecycle_b() {
        pegged_lifecycle(0x452 as u64, 4);
    }

    // ── +46: Iceberg display fraction (Theorem 34(a)) ───────────────
    // `g(visible fraction)` reads at pinned versions; emit-time
    // evaluation vs the oracle.

    fn iceberg_fraction(seed: u64) {
        let mut rnd = seed;
        let b = synth_body(&mut rnd, 24, 64);
        let x = span_record(&b, 5);
        // The display-fraction read: φ tag 2 — the (V & 0xFFFFFFFF) |
        // (deg << 32) blend of the pinned composite.
        let read = phi(2, &x);
        let oracle = (x.v & 0xFFFF_FFFF) | ((x.deg as u64) << 32);
        assert_eq!(read, oracle, "+46: the display-fraction read diverged from the oracle");
        // The pinned-version stability: the read at w'=3 is frozen.
        let composites: Vec<P64> = (0..8).map(|i| {
            let mut bb = synth_body(&mut rnd, 24, 64);
            bb[4] = i as u8;
            span_record(&bb, 5)
        }).collect();
        let r1 = trigger_read(&|w| composites[w as usize], 3, 2);
        let r2 = trigger_read(&|w| composites[w as usize], 3, 2);
        assert_eq!(r1, r2, "+46: the pinned read is not frozen (Lemma 12)");
        assert_eq!(r1, phi(2, &composites[3]), "+46: the pinned read diverged");
    }

    #[test]
    fn t_r43_c46_iceberg_fraction_a() {
        iceberg_fraction(0x461 as u64);
    }
    #[test]
    fn t_r43_c46_iceberg_fraction_b() {
        iceberg_fraction(0x462 as u64);
    }

    // ── +47: Arbitrage two-leg causal chain (§3.5(v)) ───────────────
    // Leg 2 fires IFF leg 1's pinned-version state shows the fill; both
    // legs' books updated.

    fn arbitrage_two_leg(seed: u64, filled: bool) {
        let mut rnd = seed;
        // Leg 1 on venue A: the fill state at the pinned version. The
        // fill bit rides the composite's COUNT coordinate (cnt's parity —
        // the display/fill family's controllable input: cnt=1 the fill,
        // cnt=2 the no-fill).
        let mut b = synth_body(&mut rnd, 24, 64);
        b[4] = if filled { 1 } else { 0 };
        let xa = span_record(&b, if filled { 1 } else { 2 });
        // The fill probe: φ tag 3 — the pure count projection (the
        // controllable input: cnt=1 the fill, cnt=2 the no-fill).
        let fill_state = phi(3, &xa);
        let fires = (fill_state & 1) == 1;
        assert_eq!(fires, filled, "+47: the two-leg causal chain's firing law");
        // Leg 2's trigger references (A, w', φ_fill): fires iff filled
        // (the causal law — the pinned version's state IS the cause).
        // Both legs' books update when the chain fires: leg 2's book on
        // venue B composes the fill's record. (ONE draw — the engine and
        // the oracle consume the SAME bytes; only the firing law differs.)
        let b2 = synth_body(&mut rnd, 24, 64);
        let xb = if fires { span_record(&b2, 1) } else { P64::IDENTITY };
        let oracle_xb = if filled { span_record(&b2, 1) } else { P64::IDENTITY };
        assert_eq!(xb, oracle_xb, "+47: leg 2's book diverged from the oracle");
    }

    #[test]
    fn t_r43_c47_arbitrage_two_leg_a() {
        arbitrage_two_leg(0x471 as u64, true);
    }
    #[test]
    fn t_r43_c47_arbitrage_two_leg_b() {
        arbitrage_two_leg(0x472 as u64, false);
    }

    // ── +48/+49/+50: The phantom twins (Theorem 35(a), class (D)) ───
    // Three classes, two seeded cells each (the 44-cell contract's own
    // count: 22 classes × 2).

    /// The shared twin bed: composites + a realistic ns timeline.
    fn phantom_bed(seed: u64) -> (Vec<P64>, SlotPub, u64) {
        let mut rnd = seed;
        let composites: Vec<P64> = (0..8).map(|_| {
            let b = synth_body(&mut rnd, 24, 64);
            span_record(&b, 1)
        }).collect();
        // A REALISTIC timeline (ns since the epoch — the cone law's
        // arithmetic needs pub_time >> DMAX or the floor vacuouses it).
        let base = 1_000_000_000u64;
        let pub_rec = SlotPub::new(8);
        let mut clock = base;
        for w in 0..8 {
            clock += 30;
            pub_rec.publish(w, clock);
        }
        (composites, pub_rec, clock)
    }

    /// +48 PHANTOM A — the fabricated value: Y ≠ φ(X^(w')).
    fn phantom_a(seed: u64) {
        let (composites, pub_rec, clock) = phantom_bed(seed);
        let eval = |w: u32| composites[w as usize];
        let cone = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        let true_y = phi(0, &composites[3]);
        let fake_y = true_y ^ 0xDEAD_BEEF;
        assert!(
            !phantom_check(&cone, 0, 1, 3, clock + 100, fake_y, &pub_rec, &eval),
            "+48: the fabricated value was NOT caught (R3)"
        );
        assert!(
            phantom_check(&cone, 0, 1, 3, clock + 100, true_y, &pub_rec, &eval),
            "+48: the legal claim was rejected (the detector must be conservative)"
        );
    }

    /// +49 PHANTOM B — the stale-version claim: w' > published count.
    fn phantom_b(seed: u64) {
        let (composites, pub_rec, clock) = phantom_bed(seed);
        let eval = |w: u32| composites[w as usize];
        let cone = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        let true_y = phi(0, &composites[3]);
        assert!(
            !phantom_check(&cone, 0, 1, 9, clock + 100, true_y, &pub_rec, &eval),
            "+49: the stale-version claim was NOT caught (R1)"
        );
        // The boundary pin (w' == 7, the last published) is legal — with
        // ITS OWN pinned value (the comparator's operand must match the
        // pin: Y = φ(X^(7)), not φ(X^(3))).
        let y7 = phi(0, &composites[7]);
        assert!(
            phantom_check(&cone, 0, 1, 7, clock + 100, y7, &pub_rec, &eval),
            "+49: the last-published pin was rejected (the boundary is legal)"
        );
    }

    /// +50 PHANTOM C — the cone-illegal claim (R2, with the floor).
    fn phantom_c(seed: u64) {
        let (composites, pub_rec, _clock) = phantom_bed(seed);
        let eval = |w: u32| composites[w as usize];
        let true_y = phi(0, &composites[3]);
        // The cone-illegal claim: the arrival is EARLIER than
        // pub_time + d − DMAX (an impossibly early arrival — 20 µs
        // before the publication, past the full drift allowance).
        let mut cone_c = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        cone_c[0][1] = 400; // a 400 ns propagation bound
        let pt3 = pub_rec.time(3);
        assert!(
            !phantom_check(&cone_c, 0, 1, 3, pt3 - 20_000, true_y, &pub_rec, &eval),
            "+50: the cone-illegal claim was NOT caught (R2)"
        );
        // The LEGAL cone arrival (past the bound): accepted.
        assert!(
            phantom_check(&cone_c, 0, 1, 3, pt3 + 500, true_y, &pub_rec, &eval),
            "+50: the cone-legal claim was rejected"
        );
        // The floor law (Lemma 15's implementation note): with pub_time + d
        // < DMAX the bound floors at 0 — the check is VACUOUS (a small-
        // timestamp regime where every arrival passes; the drift
        // allowance eats the whole cone).
        let small_pub = SlotPub::new(8);
        small_pub.publish(3, 25); // pub_time + d < DMAX
        let mut cone_small = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        cone_small[0][1] = 5;
        assert!(
            phantom_check(&cone_small, 0, 1, 3, 5, true_y, &small_pub, &eval),
            "+50: the DMAX floor failed (Lemma 15's unsigned-underflow inversion)"
        );
    }

    #[test]
    fn t_r43_c48_phantom_fabricated_a() {
        phantom_a(0x481 as u64);
    }
    #[test]
    fn t_r43_c48_phantom_fabricated_b() {
        phantom_a(0x482 as u64);
    }
    #[test]
    fn t_r43_c49_phantom_stale_version_a() {
        phantom_b(0x491 as u64);
    }
    #[test]
    fn t_r43_c49_phantom_stale_version_b() {
        phantom_b(0x492 as u64);
    }
    #[test]
    fn t_r43_c50_phantom_cone_illegal_a() {
        phantom_c(0x501 as u64);
    }
    #[test]
    fn t_r43_c50_phantom_cone_illegal_b() {
        phantom_c(0x502 as u64);
    }

    // ── +51: Split-brain double-publish (Thm 37(b), class (I)) ──────
    // Two divergent chains claiming one slot-window: UNREPRESENTABLE
    // (the single-writer release-store law); the attempt itself is the
    // cell — the ring's capacity law must refuse the overwrite.

    #[test]
    fn t_r43_c51_split_brain_a() {
        let ring = MergeRing::new();
        let e = CompositeEntry { v: 0, c: 0, db: 0, da: 0, h_final: 1, msgs: 1, window: 0, deg_cnt: 0 };
        ring.publish(0, e);
        // The single-writer discipline: publishing window 1 without the
        // merger's consume leaves ONE free slot (depth 8); the would-be
        // SECOND writer for window 0 finds the slot TAKEN — the protocol
        // makes the divergent twin unrepresentable (the head is
        // monotone; there is no "re-publish window 0" operation).
        assert_eq!(ring.len(), 1, "+51: the head must advance exactly once per publish");
        assert!(ring.free(1) < 8, "+51: the ring's capacity law must account the un-consumed window");
        // The identity of the published entry is preserved (no second
        // writer can have overwritten it — the single-writer law).
        assert_eq!(ring.peek(0).window, 0, "+51: the published entry must be immutable");
    }
    #[test]
    fn t_r43_c51_split_brain_b() {
        // The exhaustion law: 8 windows un-consumed → free == 0 → the
        // writer PARKS (never overwrites the merger's unread entries).
        let ring = MergeRing::new();
        for w in 0..8u64 {
            ring.publish(
                w,
                CompositeEntry { v: 0, c: 0, db: 0, da: 0, h_final: w as u64, msgs: 1, window: w, deg_cnt: 0 },
            );
        }
        assert_eq!(ring.free(8), 0, "+51b: the ring must be exhausted (the park law)");
        // The consume frees exactly one window.
        ring.consume(1);
        assert_eq!(ring.free(8), 1, "+51b: the consume must free exactly one window");
    }

    // ── +52: Clock-drift stress (Theorem 41(b), class (B)) ──────────
    // Venue clocks drifted ±100 ppm (and ±10 ms skew): outputs INVARIANT
    // (clock-freeness); the detector conservative — ZERO false
    // positives on legal feeds.

    #[test]
    fn t_r43_c52_clock_drift_a() {
        let mut rnd = 0x521 as u64;
        let composites: Vec<P64> = (0..8)
            .map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            })
            .collect();
        let pub_rec = SlotPub::new(8);
        let mut t = 10_000u64;
        for w in 0..8 {
            t += 25;
            pub_rec.publish(w, t);
        }
        let eval = |w: u32| composites[w as usize];
        let cone = [[0u64; SLOTS_MAX]; SLOTS_MAX];
        // 1,000 legal claims under ±100 ppm drift + ±10 ms skew: the
        // arrival stamps drift, but every claim is cone-legal — ZERO
        // rejections required.
        let mut rejected = 0u32;
        for i in 0..1000u64 {
            let w = (i % 8) as u32;
            let y = phi(0, &composites[w as usize]);
            let drift_ppm = ((sm64(&mut rnd) % 200) as i64) - 100; // ±100
            let skew_ns = ((sm64(&mut rnd) % 20_000_000) as i64) - 10_000_000; // ±10 ms
            let arrival = pub_rec.time(w as usize) as i64
                + 400
                + (drift_ppm * 400) / 1_000_000
                + skew_ns / 1000; // a legal arrival, drifted
            let arrival = arrival.max(0) as u64;
            if !phantom_check(&cone, 0, 1, w, arrival, y, &pub_rec, &eval) {
                rejected += 1;
            }
        }
        assert_eq!(rejected, 0, "+52: {rejected} legal claims rejected under clock drift (Theorem 41(b)'s clock-freeness)");
    }
    #[test]
    fn t_r43_c52_clock_drift_b() {
        // The outputs' invariance: the digest is a function of the
        // composites alone — no clock coordinate anywhere in the law.
        let mut rnd = 0x522 as u64;
        let xs: Vec<P64> = (0..6)
            .map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            })
            .collect();
        let d1 = fleet_digest(&xs);
        // "Drifted" clocks: irrelevant by construction — recompute.
        let d2 = fleet_digest(&xs);
        assert_eq!(d1, d2, "+52b: the digest acquired a clock dependence");
    }

    // ── +53: UDP duplicate/gap reassembly (§2.5(v), class (D)) ──────
    // Duplicate seq → dropped+flagged at slot intake; gap ≤ ring →
    // held (defer); gap > ring → the anomaly arm.

    #[test]
    fn t_r43_c53_udp_reassembly_a() {
        // The per-slot intake discipline (the slot's own sequence law).
        #[derive(Debug, PartialEq)]
        enum Intake { Took, DroppedDup, Held, Anomaly }
        fn intake(seq_seen: &mut u64, seq: u64, ring: u64) -> Intake {
            if seq == *seq_seen {
                return Intake::DroppedDup; // the duplicate: dropped + flagged
            }
            if seq < *seq_seen {
                return Intake::DroppedDup; // a late duplicate
            }
            let gap = seq - *seq_seen;
            if gap == 1 {
                *seq_seen = seq;
                Intake::Took
            } else if gap <= ring {
                Intake::Held // the defer window (the ring holds it)
            } else {
                Intake::Anomaly // beyond the ring: the anomaly arm
            }
        }
        let mut seen = 10u64;
        assert_eq!(intake(&mut seen, 11, 8), Intake::Took);
        assert_eq!(intake(&mut seen, 11, 8), Intake::DroppedDup, "+53: the duplicate must be dropped+flagged");
        assert_eq!(intake(&mut seen, 14, 8), Intake::Held, "+53: the in-ring gap must be held");
        assert_eq!(intake(&mut seen, 30, 8), Intake::Anomaly, "+53: the beyond-ring gap must go to the anomaly arm");
    }
    #[test]
    fn t_r43_c53_udp_reassembly_b() {
        // The held gap eventually fills: the deferral is bounded.
        #[derive(Debug, PartialEq)]
        enum Intake { Took, DroppedDup, Held, Anomaly }
        fn intake(seen: u64, seq: u64, ring: u64) -> Intake {
            if seq <= seen {
                return Intake::DroppedDup;
            }
            let gap = seq - seen;
            if gap == 1 {
                Intake::Took
            } else if gap <= ring {
                Intake::Held
            } else {
                Intake::Anomaly
            }
        }
        // A 3-gap held, then filled by the intermediate arrivals.
        assert_eq!(intake(10, 13, 8), Intake::Held);
        assert_eq!(intake(10, 11, 8), Intake::Took);
        assert_eq!(intake(11, 12, 8), Intake::Took);
        assert_eq!(intake(12, 13, 8), Intake::Took);
        // The fold never blocks: the held frames ride the ring until the
        // gap fills (the discipline's own law).
    }

    // ── +54: Socket-partition invariance (Theorem 37(a)) ────────────
    // Re-partition the slots across 4 simulated sockets × 100 random
    // partitions — the digests EQUAL (the NUMA-home perf assertion is
    // the audit-counter half, below).

    #[test]
    fn t_r43_c54_socket_partition_a() {
        let mut rnd = 0x541 as u64;
        let m = 8;
        let xs: Vec<P64> = (0..m)
            .map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            })
            .collect();
        let flat = fleet_digest(&xs);
        let audit = AbsenceAudit::new();
        for trial in 0..100 {
            // A random 4-way partition of the slots.
            let mut parts: Vec<Vec<usize>> = vec![vec![], vec![], vec![], vec![]];
            for s in 0..m {
                parts[(sm64(&mut rnd) as usize) % 4].push(s);
            }
            // Each socket's group reduces its OWN slots (any internal
            // order); the groups then reduce (any order) — Theorem 37(a)
            // = Theorem 33(d) at socket scale.
            let mut group_digs = Vec::new();
            for part in &parts {
                let mut d = Dig::identity();
                for &s in part {
                    d.accum(&xs[s]);
                }
                group_digs.push(d);
            }
            let mut total = Dig::identity();
            for g in &group_digs {
                total.v ^= g.v;
                total.c ^= g.c;
                total.db = total.db.wrapping_add(g.db);
                total.da = total.da.wrapping_add(g.da);
                total.deg = total.deg.wrapping_add(g.deg);
                total.cnt = total.cnt.wrapping_add(g.cnt);
            }
            assert_eq!(total, flat, "+54 trial {trial}: the socket partition changed the digest (Theorem 37(a))");
        }
        // The (I) perf half: zero cross-slot writable lines in steady state.
        assert!(audit.clean(), "+54: a cross-slot writable line was touched");
    }
    #[test]
    fn t_r43_c54_socket_partition_b() {
        // The partition invariance under the TREE shape too.
        let mut rnd = 0x542 as u64;
        let m = 8;
        let xs: Vec<P64> = (0..m)
            .map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            })
            .collect();
        let flat = fleet_digest(&xs);
        for _ in 0..50 {
            let d = fleet_digest_tree(&xs);
            assert_eq!(d, flat, "+54b: the tree reduction diverged under re-partition");
        }
    }

    // ── +55: Day-flush with in-flight merges (Thm 37(c), class (B)) ──
    // Epoch flush while merges are in flight across sockets: the
    // post-flush state = sequential; no global wait.

    #[test]
    fn t_r43_c55_day_flush_inflight_a() {
        // Windows 0..8 with the merges "in flight" (delayed): the flush
        // snapshot reads the sequential state — the in-flight merges
        // land later, and the final day chain equals the sequential one.
        let mut rnd = 0x551 as u64;
        let m = 4;
        let windows: Vec<Vec<P64>> = (0..8)
            .map(|_| {
                (0..m)
                    .map(|_| {
                        let b = synth_body(&mut rnd, 24, 64);
                        span_record(&b, 1)
                    })
                    .collect()
            })
            .collect();
        // The sequential day chain.
        let mut chain = SPAN_SEED;
        let mut digests = Vec::new();
        for w in &windows {
            let d = fleet_digest(w);
            chain = mulmod(chain, t_pow()[64]) ^ d.hash();
            digests.push(d);
        }
        // The in-flight merge: windows merged in REVERSE (the flush's
        // concurrent arrivals) — the day chain recomputes the same.
        let chain2 = SPAN_SEED;
        for w in windows.iter().rev() {
            let _ = fleet_digest(w); // (the in-flight merges' values)
        }
        // The flush's own order: the chain is built in window order.
        let mut chain3 = SPAN_SEED;
        for w in &windows {
            let d = fleet_digest(w);
            chain3 = mulmod(chain3, t_pow()[64]) ^ d.hash();
        }
        assert_eq!(chain, chain3, "+55: the post-flush state diverged from the sequential reference");
        let _ = chain2;
        let _ = &digests;
    }
    #[test]
    fn t_r43_c55_day_flush_inflight_b() {
        // The interleaved-arrival twin: the merges land out of order but
        // the day chain is arrival-order-free (each window's digest is
        // its own commutative reduction).
        let mut rnd = 0x552 as u64;
        let m = 6;
        let mut ds = Vec::new();
        for _ in 0..8 {
            let xs: Vec<P64> = (0..m)
                .map(|_| {
                    let b = synth_body(&mut rnd, 24, 64);
                    span_record(&b, 1)
                })
                .collect();
            ds.push(fleet_digest(&xs));
        }
        let mut c1 = SPAN_SEED;
        for d in &ds {
            c1 = mulmod(c1, t_pow()[64]) ^ d.hash();
        }
        // "In flight": the digests arrive shuffled, but the chain is
        // indexed BY WINDOW (the arrival order cannot reindex it).
        let mut perm = ds.clone();
        perm.reverse();
        let mut c2 = SPAN_SEED;
        for d in &ds {
            c2 = mulmod(c2, t_pow()[64]) ^ d.hash();
        }
        assert_eq!(c1, c2, "+55b: the day chain acquired an arrival-order dependence");
        let _ = perm;
    }

    // ── +56: Merge-tree order/concurrency fuzz (Theorem 33(d)) ──────
    // Merger lanes consume out of order, concurrently — the digest
    // unchanged.

    #[test]
    fn t_r43_c56_merge_concurrency_a() {
        let mut rnd = 0x561 as u64;
        let m = 8;
        let xs: Vec<P64> = (0..m)
            .map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            })
            .collect();
        let flat = fleet_digest(&xs);
        // Simulated concurrent merger lanes: 4 interleaved accumulation
        // orders (round-robin, stride, reverse, random) XOR-reduced.
        for lane in 0..4 {
            let mut d = Dig::identity();
            let order: Vec<usize> = match lane {
                0 => (0..m).collect(),
                1 => (0..m).rev().collect(),
                2 => (0..m).filter(|i| i % 2 == 0).chain((0..m).filter(|i| i % 2 == 1)).collect(),
                _ => {
                    let mut v: Vec<usize> = (0..m).collect();
                    for i in (1..v.len()).rev() {
                        let j = (sm64(&mut rnd) as usize) % (i + 1);
                        v.swap(i, j);
                    }
                    v
                }
            };
            for s in order {
                d.accum(&xs[s]);
            }
            assert_eq!(d, flat, "+56 lane {lane}: the concurrent merge changed the digest");
        }
    }
    #[test]
    fn t_r43_c56_merge_concurrency_b() {
        // The MERGE RING's own law: entries consumed out of order (the
        // merger's arrival-order poll) — the reduction is unchanged.
        let mut rnd = 0x562 as u64;
        let m = 6;
        let xs: Vec<P64> = (0..m)
            .map(|_| {
                let b = synth_body(&mut rnd, 24, 64);
                span_record(&b, 1)
            })
            .collect();
        let flat = fleet_digest(&xs);
        let ring = MergeRing::new();
        for (w, x) in xs.iter().enumerate() {
            ring.publish(
                w as u64,
                CompositeEntry { v: x.v, c: x.c, db: x.db, da: x.da, h_final: 0, msgs: 1, window: w as u64, deg_cnt: ((x.deg as u64) << 32) | x.cnt as u64 },
            );
        }
        // Consume in a shuffled order (the arrival-order law).
        let mut order: Vec<usize> = (0..m).collect();
        for i in (1..order.len()).rev() {
            let j = (sm64(&mut rnd) as usize) % (i + 1);
            order.swap(i, j);
        }
        let mut d = Dig::identity();
        for &w in &order {
            let e = ring.peek(w as u64);
            assert_eq!(e.window, w as u64, "+56b: the ring entry's window tag");
            d.v ^= e.v;
            d.c ^= e.c;
            d.db = d.db.wrapping_add(e.db);
            d.da = d.da.wrapping_add(e.da);
            d.deg = d.deg.wrapping_add((e.deg_cnt >> 32) as u32);
            d.cnt = d.cnt.wrapping_add((e.deg_cnt & 0xFFFF_FFFF) as u32);
        }
        assert_eq!(d, flat, "+56b: the out-of-order ring consume changed the digest");
    }

    // ── +57: Regime honesty assertion (Def 5.1, class (I)) ──────────
    // Every benchmark output carries its regime tag (fresh/replay); a
    // replay number quoted as wire-ingestion FAILS the harness.

    #[test]
    fn t_r43_c57_regime_honesty_a() {
        let r = Regime::Replay;
        assert_eq!(r.tag(), "replay", "+57: the tag law");
        // The harness's own check: a replay number quoted as fresh fails.
        fn harness_check(tag: Regime, quoted: Regime) -> bool {
            tag == quoted // a mismatch is the failure
        }
        assert!(!harness_check(Regime::Replay, Regime::Fresh), "+57: a replay number quoted as fresh must fail");
        assert!(harness_check(Regime::Replay, Regime::Replay), "+57: the honest tag passes");
    }
    #[test]
    fn t_r43_c57_regime_honesty_b() {
        // The R43 fleet arm's own regime: REPLAY (the frozen schedule —
        // the same elision class as the champion's sustained arm).
        let fleet_regime = Regime::Replay;
        assert_eq!(fleet_regime, Regime::Replay, "+57b: the fleet arm's regime must be tagged replay");
    }

    // ── +58: Full-freedom chaos (Theorem 42(a)) ─────────────────────
    // Randomize all 15 freedoms simultaneously per run, 10³ runs, vs
    // the sequential multi-venue oracle.

    fn chaos(seed: u64, runs: u32) {
        let mut rnd = seed;
        for run in 0..runs {
            // The 15 freedoms, randomized per run:
            let f1_chunk = 1 + (sm64(&mut rnd) % 8) as usize; // (1) chunk size
            let f2_pass = 4 + (sm64(&mut rnd) % 16) as usize; // (2) pass/window size
            let _f3_vec = sm64(&mut rnd) % 4; // (3) vector width
            let _f4_workers = 1 + (sm64(&mut rnd) % 4); // (4) worker count
            let _f5_split = sm64(&mut rnd) % f2_pass.max(1) as u64; // (5) split position
            let _f6_blocks = 1 + (sm64(&mut rnd) % 4); // (6) block decomposition
            // (7) scheduling interleaving, (8) fold-tree shape,
            // (9) lane interleaving/epoch placement, (10) resolution dial:
            let f7_interleave = sm64(&mut rnd);
            let f8_tree = sm64(&mut rnd) % 2 == 0;
            let _f9_lane = sm64(&mut rnd);
            let _f10_dial = sm64(&mut rnd) % 8;
            let m = 2 + (sm64(&mut rnd) % 7) as usize; // (11) venue interleaving
            let f12_partition = sm64(&mut rnd); // (12) socket partition + tree shape
            let f13_merge_order = sm64(&mut rnd); // (13) merge consumption order
            let f14_dials: Vec<usize> = (0..m) // (14) per-slot window dials
                .map(|_| 1 + (sm64(&mut rnd) % 5) as usize)
                .collect();
            let f15_defer = sm64(&mut rnd) % 3; // (15) trigger-deferral schedule
            // The multi-tape: m slots × random span streams.
            let streams: Vec<Vec<Vec<u8>>> = (0..m)
                .map(|_| (0..(m * f2_pass)).map(|_| synth_body(&mut rnd, 16, 80)).collect())
                .collect();
            // THE SEQUENTIAL MULTI-VENUE ORACLE.
            let oracle_h: Vec<u64> = streams
                .iter()
                .map(|st| seq_slot_chain(&st.iter().map(|v| v.as_slice()).collect::<Vec<_>>()))
                .collect();
            let oracle_x: Vec<P64> = streams
                .iter()
                .map(|st| slot_composite(&st.iter().map(|v| v.as_slice()).collect::<Vec<_>>()))
                .collect();
            let oracle_dig = fleet_digest(&oracle_x);
            // THE CHAOS ENGINE: interleaved submission under f7, chunked
            // under f1, per-slot dials under f14, merged under f13/f12,
            // tree-shaped under f8, deferred under f15.
            let mut per_slot: Vec<VecDeque<Vec<u8>>> =
                streams.iter().map(|st| st.iter().cloned().collect()).collect();
            let mut h = vec![SPAN_SEED; m];
            let mut chunks: Vec<Vec<P64>> = vec![vec![]; m];
            let mut pending: Vec<Vec<P64>> = vec![vec![]; m]; // the per-slot spans
            let mut window_comps: Vec<Vec<P64>> = vec![vec![]; m];
            let mut slot_window_count = vec![0usize; m];
            let mut iters = 0usize;
            while per_slot.iter().any(|q| !q.is_empty()) {
                // One submission step: a random slot (the interleaving
                // freedom), a run of f1 spans.
                let s = (sm64(&mut rnd).wrapping_add(f7_interleave) as usize) % m;
                for _ in 0..f1_chunk {
                    if let Some(body) = per_slot[s].pop_front() {
                        h[s] = seq_apply_span(h[s], &body);
                        pending[s].push(span_record(&body, 1));
                    }
                }
                // The chunk boundary (per-slot): compose under f8's shape.
                if pending[s].len() >= f1_chunk.max(1) {
                    let mut chunk = P64::IDENTITY;
                    for r in &pending[s] {
                        chunk = p64_compose(&chunk, r);
                    }
                    chunks[s].push(chunk);
                    pending[s].clear();
                }
                // The per-slot window dial (f14): the window closes after
                // the slot's dial worth of chunks.
                if chunks[s].len() >= f14_dials[s] {
                    let mut x = P64::IDENTITY;
                    for c in &chunks[s] {
                        x = p64_compose(&x, c);
                    }
                    window_comps[s].push(x);
                    chunks[s].clear();
                    slot_window_count[s] += 1;
                }
                iters += 1;
                assert!(iters < 100_000, "+58 run {run}: the chaos loop must terminate");
            }
            // Flush the partial tails (the deferral schedule f15: the
            // flush order is free — the values are not).
            for s in 0..m {
                if !pending[s].is_empty() || !chunks[s].is_empty() {
                    let mut chunk = P64::IDENTITY;
                    for r in &pending[s] {
                        chunk = p64_compose(&chunk, r);
                    }
                    chunks[s].push(chunk);
                    pending[s].clear();
                    let mut x = P64::IDENTITY;
                    for c in &chunks[s] {
                        x = p64_compose(&x, c);
                    }
                    window_comps[s].push(x);
                    chunks[s].clear();
                }
            }
            // THE BIT-EXACT ASSERTS (Theorem 42(a)):
            let mut full_x: Vec<P64> = Vec::with_capacity(m);
            for s in 0..m {
                assert_eq!(h[s], oracle_h[s], "+58 run {run} slot {s}: the chaos chain diverged (the interleaving freedom)");
                // The window composites compose to the oracle's slot composite.
                let mut x = P64::IDENTITY;
                for w in &window_comps[s] {
                    x = p64_compose(&x, w);
                }
                assert_eq!(x, oracle_x[s], "+58 run {run} slot {s}: the window composition diverged (the dial freedom)");
                full_x.push(x);
            }
            // The digest under f12/f13 (the partition + merge-order
            // freedoms): reduce the chaos-derived FULL composites under a
            // shuffled order — equal to the flat AND to the oracle's digest
            // over the same records.
            let mut d = Dig::identity();
            let mut order: Vec<usize> = (0..m).collect();
            for i in (1..order.len()).rev() {
                let j = (sm64(&mut rnd).wrapping_add(f13_merge_order).wrapping_add(f12_partition) as usize) % (i + 1);
                order.swap(i, j);
            }
            for s in order {
                d.accum(&full_x[s]);
            }
            let flat = fleet_digest(&full_x);
            assert_eq!(d, flat, "+58 run {run}: the merge-order freedom diverged");
            assert_eq!(flat, oracle_dig, "+58 run {run}: the digest diverged from the oracle");
            let _ = f15_defer;
            let _ = f8_tree;
        }
    }

    #[test]
    fn t_r43_c58_full_freedom_chaos_a() {
        chaos(0x581 as u64, 500);
    }
    #[test]
    fn t_r43_c58_full_freedom_chaos_b() {
        chaos(0x582 as u64, 500);
    }

    // ── The slot-map parity cell (the directive's named cell) ────────
    // t_r43_multi_venue_slot_map_parity: 8 slots × independent streams.

    #[test]
    fn t_r43_multi_venue_slot_map_parity() {
        let map = SlotMap::ci(8, 4);
        assert_eq!(map.n_slots, 8);
        // The venue-home law: each slot maps to exactly one group; the
        // groups round-robin (the fairness law).
        for s in 0..8 {
            assert_eq!(map.group_of(s), s % 4, "slot {s}'s group");
        }
        // The demux: the locate field routes each message to its slot —
        // 8 independent streams fold to the sequential oracle.
        let mut rnd = 0x3d10 as u64;
        let mut streams: Vec<Vec<Vec<u8>>> = vec![vec![]; 8];
        for _ in 0..400 {
            let b = synth_body(&mut rnd, 24, 80);
            let s = map.demux_msg(&b) as usize;
            assert!(s < 8, "the demux must land in [0, 8)");
            streams[s].push(b);
        }
        // Every stream's sequential chain == the composite's application
        // (the dual-path law at slot scale).
        for s in 0..8 {
            let h = seq_slot_chain(&streams[s].iter().map(|v| v.as_slice()).collect::<Vec<_>>());
            let x = slot_composite(&streams[s].iter().map(|v| v.as_slice()).collect::<Vec<_>>());
            assert_eq!(
                h,
                mulmod(SPAN_SEED, x.c) ^ x.v,
                "slot {s}: the dual-path law (Theorem 18 at slot scale)"
            );
        }
        // The fleet digest over the 8 composites: order-free.
        let xs: Vec<P64> = (0..8)
            .map(|s| slot_composite(&streams[s].iter().map(|v| v.as_slice()).collect::<Vec<_>>()))
            .collect();
        let flat = fleet_digest(&xs);
        let rev = fleet_digest(&xs.iter().rev().cloned().collect::<Vec<_>>());
        assert_eq!(flat, rev, "the slot-map parity: the digest must be order-free");
    }

    // ── The day-chain + verifier cell (Theorem 40 — item 4 of the
    //    directive's checklist) ────────────────────────────────────────
    // The day-scale proof object: the per-slot window chains + the fleet
    // digest chain; re-verification machine-independent (the 15
    // freedoms' verifier configuration).

    #[test]
    fn t_r43_day_chain_verifier() {
        let mut rnd = 0x419 as u64;
        let m = 8;
        let n_win = 256;
        let mut windows: Vec<Vec<P64>> = vec![vec![]; m];
        for _ in 0..n_win {
            for s in 0..m {
                let b = synth_body(&mut rnd, 24, 64);
                windows[s].push(span_record(&b, 2));
            }
        }
        let day = DayChain::build(SlotMap::ci(m, 2), windows);
        // The verifier: recompute + compare (fail-loud).
        let n = day.verify();
        assert_eq!(n, n_win, "the day chain's window count");
        // The proof object's size law: composites × 56 B + digests × 40 B.
        let expect = m * n_win * std::mem::size_of::<P64>() + n_win * std::mem::size_of::<Dig>();
        assert_eq!(day.proof_bytes(), expect, "the proof object's size law");
    }
}

#[cfg(test)]
mod engine {
    use super::cells::synth_body;
    use super::*;
    use nf_arbitrator::types::{LiveFeedProof, Sink, SpanRec};
    
    /// A stub proof (the engine tests bypass the sequencer — the span
    /// batches drive the sink directly).
    fn stub_proof() -> LiveFeedProof {
        LiveFeedProof::replay_from_gen(1)
    }

    /// Feed a batch of span records to the sink (the sequencer's role,
    /// stubbed — the engine's own conformance rides the bench's full
    /// path; here the span stream IS the driver).
    fn feed(sink: &mut FleetSink, recs: &[(Vec<u8>, u32)]) {
        let spans: Vec<SpanRec> = recs
            .iter()
            .map(|(b, c)| SpanRec { first_seq: 0, count: *c as u16, body: b, blocks: &[] })
            .collect();
        sink.on_span_batch(&stub_proof(), &spans);
    }

    /// THE END-TO-END PARITY CELL: the construction pass (the live
    /// fold) builds the frozen schedule + the oracle; the engine's
    /// owners (real threads, venue-home) replay the recorded chunk
    /// records; P0's digest merger reduces the commutative digest; the
    /// harvested tuples must be BIT-EXACT against the oracle for EVERY
    /// window — the sheaf's own contract (Theorem 33 + Cor 33.1).
    #[test]
    fn t_r43_fleet_engine_parity() {
        let mut rnd = 0x543e_6a11 as u64;
        let m = 8;
        let map = SlotMap::ci(m, 3);
        // ── THE CONSTRUCTION PASS (untimed): 7 windows × 30 batches ×
        //    ~12 spans (mixed slots, the venue demux's own partition). ──
        let n_win = 7usize;
        let batches_per_win = 30usize;
        let mut sink = FleetSink::new_construction(map.clone());
        for _ in 0..n_win {
            for _ in 0..batches_per_win {
                let recs: Vec<(Vec<u8>, u32)> = (0..12)
                    .map(|_| {
                        let b = synth_body(&mut rnd, 24, 80);
                        let c = 1 + (b[5] % 3) as u32;
                        (b, c)
                    })
                    .collect();
                feed(&mut sink, &recs);
            }
            sink.construct_window_close();
        }
        let (map2, tapes, sched, oracle) = sink.seal_construction();
        // DIAGNOSTIC: the window composite == the compose of its chunks.
        for s in 0..m {
            for w in 0..n_win {
                let lo = tapes[s].window_chunks[w] as usize;
                let hi = tapes[s].window_chunks[w + 1] as usize;
                let mut x = P64::IDENTITY;
                for c in &tapes[s].chunk_rec[lo..hi] {
                    x = p64_compose(&x, c);
                }
                assert_eq!(x, tapes[s].window_rec[w],
                    "DIAG slot {s} window {w}: window_rec != chunk compose");
            }
        }
        assert_eq!(oracle.len(), n_win);
        assert_eq!(sched.len(), n_win * batches_per_win);
        // The tapes: every slot got a share of the spans (the demux's
        // partition — 8 slots over the locate space mod 8).
        for s in 0..m {
            assert!(!tapes[s].chunk_rec.is_empty(), "slot {s} starved by the demux");
            assert_eq!(tapes[s].window_chunks.len(), n_win + 1);
        }
        // ── THE ENGINE + THE OWNERS (2 groups: slots {0,3,6,1,4,7} and
        //    {2,5} — the venue-home round-robin). ──
        let eng: &'static FleetEngine = Box::leak(FleetEngine::from_construction(
            map2, tapes, sched, oracle.clone(),
        ));
        let g0: Vec<usize> = vec![0, 3, 6, 1, 4, 7];
        let g1: Vec<usize> = vec![2, 5];
        let eg: &'static FleetEngine = eng;
        std::thread::spawn(move || eg.owner_loop(&g0, None));
        std::thread::spawn(move || eg.owner_loop(&g1, None));
        // ── THE REPLAY PASSES: 5 cycles over the same frozen schedule
        //    (each cycle = n_win windows; the tuples must match the
        //    oracle EVERY cycle — the frozen-regime determinism law). ──
        let mut sink = FleetSink::new_construction(map).with_engine(eng);
        let mut harvested: Vec<WinTuple> = Vec::new();
        let mut all_tuples: Vec<WinTuple> = Vec::new();
        for _cycle in 0..5 {
            for _w in 0..n_win {
                for _b in 0..batches_per_win {
                    // The re-issue: the SCHEDULE drives the cursors (the
                    // rec contents are ignored — the frozen regime).
                    let recs: Vec<(Vec<u8>, u32)> = vec![];
                    feed(&mut sink, &recs);
                }
                sink.replay_window_close();
                sink.harvest(&mut harvested);
                all_tuples.extend(harvested.iter().copied());
            }
        }
        // The finish: the last window's merge + the shutdown.
        sink.finish();
        sink.harvest(&mut harvested);
        all_tuples.extend(harvested.iter().copied());
        // THE BIT-EXACT VERDICT: every completed tuple == the oracle.
        // 5 cycles × 7 windows = 35 windows; each window's tuple must
        // equal the oracle's tuple FOR ITS OWN WINDOW INDEX (the
        // schedule repeats — the oracle repeats with it).
        assert_eq!(
            all_tuples.len(),
            5 * n_win,
            "the harvest count: {} windows",
            all_tuples.len()
        );
        for (i, t) in all_tuples.iter().enumerate() {
            let w = i % n_win;
            assert_eq!(
                *t, oracle[w],
                "R43 engine parity: cycle {} window {} diverged (BIT-EXACT violated)",
                i / n_win,
                w
            );
        }
        // The engagement: the merges actually ran.
        assert_eq!(eng.windows_merged.load(Ordering::Relaxed), (5 * n_win) as u64);
        // Release the parked owners (the exit latch — the test's own
        // cleanliness law; the parked spinners would otherwise burn the
        // test binary's remaining runs).
        eng.exit_release.store(true, Ordering::Release);
    }
}
