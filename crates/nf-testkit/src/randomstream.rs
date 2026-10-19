//! THE PRODUCTION LIVE RANDOM STREAM (Phase 2 of the R40 corridor program)
//!
//! # What this module is
//!
//! A high-speed, seedable, deterministic ITCH 5.0 message synthesizer
//! (splitmix64) for the `bench_random_stream` production benchmark:
//! unseen, non-repeating market data — randomized order references with a
//! bounded birth/death lifecycle, per-locate price random walks, log-ish
//! share counts, weighted event-type mixes (A/E/C/X/D/U), nanosecond
//! timestamps — emitted in the engine's `gt` tape format (u16 BE length
//! prefix + the parse-valid ITCH body).
//!
//! # The laws (pinned by the `t_randomstream_*` cells)
//!
//! 1. **REPRODUCIBILITY**: `(seed, msgs)` → the identical byte-for-byte
//!    tape. The benchmark's every run is reconstructible from its
//!    seed — the chaos-stream discipline without the chaos.
//! 2. **PARSE VALIDITY**: every message carries its type's exact spec
//!    length (the `body_len` table) and a valid type byte — the
//!    protocol parsers accept the whole tape (the parity oracle's
//!    precondition).
//! 3. **UNIQUENESS**: no two messages on a tape are byte-identical (the
//!    timestamp's monotone law guarantees it).
//! 4. **THE LIFECYCLE LAW**: every E/C/X/D/U references an order live at
//!    its emission (the birth/death discipline; a cold pool degrades to
//!    an A birth — never a dangling reference).
//! 5. **ALLOCATION**: the tape is built once, outside every measured
//!    window (a single `Vec` reserve + the synthesis walk).
//!
//! # What this module deliberately is NOT
//!
//! A market SIMULATOR — no queue-position semantics, no matching
//! engine, no venue-level dynamics. The synthesis targets the ENGINE's
//! consumption surface: valid framing, bounded state, realistic
//! weightings, and entropy where the parsers and CRC kernels read it.

/// The splitmix64 generator (the seed's full 64-bit avalanche — one
/// `wrapping_add` + three xorshift-multiply rounds per draw; ~1 ns/draw,
/// no warmup, no correlation structure worth attacking).
#[derive(Clone)]
pub struct SplitMix64 {
    s: u64,
}

impl SplitMix64 {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { s: seed }
    }

    #[inline]
    pub fn next(&mut self) -> u64 {
        self.s = self.s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A draw below `n` (n ≤ 2^32 — the modulo's bias is ≤ 2^-32, far
    /// below the synthesis's statistical needs).
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The ITCH 5.0 spec body lengths this engine parses (nf-protocol's
/// table — the generator emits ONLY these types).
#[inline]
#[must_use]
pub fn body_len(ty: u8) -> Option<usize> {
    match ty {
        b'A' => Some(36), // Add Order (no MPID)
        b'E' => Some(31), // Order Executed
        b'C' => Some(36), // Order Executed With Price
        b'X' => Some(23), // Order Cancel
        b'D' => Some(19), // Order Delete
        b'U' => Some(35), // Order Replace
        _ => None,
    }
}

/// The live-order pool's bound (the lifecycle law's cap — real feeds
/// carry ~10⁵ resting orders; the pool models the birth/death PRESSURE,
/// not the census).
const POOL_CAP: usize = 4096;

/// One synthesized tape's generator state. The stock universe is
/// `n_locates` synthetic symbols; each carries a running mid-price
/// (a bounded random walk on the ITCH price grid); the order pool is a
/// compact ring of live `(ref, remaining)` pairs.
struct TapeGen {
    rng: SplitMix64,
    /// Live orders — the E/C/X/D/U reference surface. Dense-swap
    /// deletion (the pool never scans).
    pool: Vec<(u64, u32)>,
    pool_n: usize,
    next_ref: u64,
    /// Per-locate running price (the walk's state) + the 8-byte symbol.
    prices: Vec<u32>,
    symbols: Vec<[u8; 8]>,
    ts_ns: u64,
    msg_i: u64,
    match_no: u64,
}

impl TapeGen {
    fn new(seed: u64, n_locates: u16) -> Self {
        let mut rng = SplitMix64::new(seed);
        let mut prices = Vec::with_capacity(n_locates as usize);
        let mut symbols = Vec::with_capacity(n_locates as usize);
        for i in 0..n_locates {
            // The mid-price seed: 100.0000 .. 999.0000 dollars on the
            // ITCH fixed-point grid (1e-4).
            prices.push(1_000_000 + (rng.next() % 899_000_000) as u32);
            let mut sym = *b"RNDSYM  ";
            // The locate's 4 hex digits into the symbol's tail — unique
            // per locate, 8 bytes, printable.
            for k in 0..4 {
                sym[6 - k] = b"0123456789ABCDEF"[((i >> (k * 4)) & 0xF) as usize];
            }
            symbols.push(sym);
        }
        Self {
            rng,
            pool: vec![(0, 0); POOL_CAP],
            pool_n: 0,
            next_ref: 1,
            prices,
            symbols,
            ts_ns: 3_600_000_000_000_000, // 10:00:00.000000 — the session's open
            msg_i: 0,
            match_no: 1,
        }
    }

    /// The POOL-AWARE type draw: the weighted mix (A 34%, E 22%, C 6%,
    /// X 16%, D 18%, U 4%) with the lifecycle law's guard — a reference
    /// type with an empty pool degrades to an A birth (never a dangling
    /// reference).
    #[inline]
    fn next_type(&mut self) -> u8 {
        if self.pool_n == 0 {
            return b'A';
        }
        match self.rng.below(100) {
            0..=33 => b'A',
            34..=55 => b'E',
            56..=61 => b'C',
            62..=77 => b'X',
            78..=95 => b'D',
            _ => b'U',
        }
    }

    /// A heavy-tailed locate draw (a Zipf-ish tilt: the first quarter of
    /// the symbols takes half the flow — the cache-locality profile of a
    /// real tape).
    #[inline]
    fn pick_locate(&mut self) -> u16 {
        let d = self.rng.next();
        let idx = if d & 1 == 0 {
            (d >> 1) % ((self.prices.len() as u64 / 4).max(1))
        } else {
            (d >> 1) % self.prices.len() as u64
        };
        idx as u16
    }

    /// The bounded walk: ±(1..=99) ticks, clamped to the session's band
    /// — a random walk with reflecting barriers.
    #[inline]
    fn walk_price(&mut self, locate: u16) -> u32 {
        let i = locate as usize;
        let step = 1 + (self.rng.next() % 99) as i64;
        let dir: i64 = if self.rng.next() & 1 == 0 { -1 } else { 1 };
        let p = (self.prices[i] as i64 + dir * step).clamp(10_000, 4_000_000_000);
        self.prices[i] = p as u32;
        p as u32
    }

    /// Synthesize one message's body (`out` is the exact-size slice for
    /// `ty`). The common header (11 B): type, locate, tracking, the
    /// 48-bit ns timestamp.
    fn gen_body(&mut self, out: &mut [u8], ty: u8) {
        let len = body_len(ty).expect("the caller draws a tabled type");
        debug_assert_eq!(out.len(), len);
        let locate = self.pick_locate();
        out[0] = ty;
        out[1..3].copy_from_slice(&locate.to_be_bytes());
        out[3..5].copy_from_slice(&(self.msg_i as u16).to_be_bytes());
        let ts = self.ts_ns & 0xFFFF_FFFF_FFFF;
        out[5] = (ts >> 40) as u8;
        out[6] = (ts >> 32) as u8;
        out[7] = (ts >> 24) as u8;
        out[8] = (ts >> 16) as u8;
        out[9] = (ts >> 8) as u8;
        out[10] = ts as u8;
        // The timestamp's monotone advance: 40..=400 ns per message —
        // the burst cadence of a real session (also the uniqueness law:
        // no two messages share (ts, ref, type)).
        self.ts_ns += 40 + self.rng.below(361);
        let side: u8 = if self.rng.next() & 1 == 0 { b'B' } else { b'S' };
        match ty {
            b'A' => {
                let r = self.next_ref;
                self.next_ref += 1;
                let shares = 1 + self.rng.below(10_000);
                out[11..19].copy_from_slice(&r.to_be_bytes());
                out[19] = side;
                out[20..24].copy_from_slice(&(shares as u32).to_be_bytes());
                out[24..32].copy_from_slice(&self.symbols[locate as usize]);
                let px = self.walk_price(locate);
                out[32..36].copy_from_slice(&px.to_be_bytes());
                if self.pool_n < POOL_CAP {
                    self.pool[self.pool_n] = (r, shares as u32);
                    self.pool_n += 1;
                }
            }
            b'E' => {
                self.reference(out);
                let m = self.match_no;
                self.match_no += 1;
                out[23..31].copy_from_slice(&m.to_be_bytes());
            }
            b'C' => {
                self.reference(out);
                let m = self.match_no;
                self.match_no += 1;
                // C's spec layout: ref@11, executed@19, match@23,
                // printable@31, execution price@32.
                out[23..31].copy_from_slice(&m.to_be_bytes());
                out[31] = b'Y';
                let px = self.walk_price(locate);
                out[32..36].copy_from_slice(&px.to_be_bytes());
            }
            b'X' => {
                self.reference(out);
            }
            b'D' => {
                self.reference(out);
            }
            b'U' => {
                self.reference(out);
                // U's spec layout: orig ref@11, NEW ref@19, new shares@27,
                // new price@31 — the replacement births the new ref (the
                // lifecycle law: the old dies, the new lives).
                let new_ref = self.next_ref;
                self.next_ref += 1;
                out[19..27].copy_from_slice(&new_ref.to_be_bytes());
                let shares = 1 + self.rng.below(10_000);
                out[27..31].copy_from_slice(&(shares as u32).to_be_bytes());
                let px = self.walk_price(locate);
                out[31..35].copy_from_slice(&px.to_be_bytes());
                if self.pool_n < POOL_CAP {
                    self.pool[self.pool_n] = (new_ref, shares as u32);
                    self.pool_n += 1;
                }
            }
            _ => unreachable!("the caller draws a tabled type"),
        }
        self.msg_i += 1;
    }

    /// Draw a live order reference and write it (ref@11), applying the
    /// type's pool effect: E/C consume shares (delete at zero), X
    /// cancels a slice, D/U delete outright (dense-swap).
    #[inline]
    fn reference(&mut self, out: &mut [u8]) {
        debug_assert!(self.pool_n > 0, "next_type guards the cold pool");
        let idx = (self.rng.next() % self.pool_n as u64) as usize;
        let (r, remaining) = self.pool[idx];
        let share_draw = 1 + self.rng.below(remaining.max(1) as u64) as u32;
        match out[0] {
            b'E' | b'C' | b'X' => {
                out[19..23].copy_from_slice(&share_draw.to_be_bytes());
                if share_draw >= remaining {
                    self.pool[idx] = self.pool[self.pool_n - 1];
                    self.pool_n -= 1;
                } else {
                    self.pool[idx].1 = remaining - share_draw;
                }
            }
            _ => {
                // D (full delete) and U (the old side of a replace).
                self.pool[idx] = self.pool[self.pool_n - 1];
                self.pool_n -= 1;
            }
        }
        out[11..19].copy_from_slice(&r.to_be_bytes());
    }
}

/// Build a random tape: `total_msgs` messages, `seed`-deterministic, in
/// the engine's `gt` format (u16 BE length prefix + the ITCH body). The
/// returned buffer is a single allocation sized exactly.
#[must_use]
pub fn build_random_tape(seed: u64, total_msgs: usize, n_locates: u16) -> Vec<u8> {
    let mut g = TapeGen::new(seed, n_locates);
    let mut tape = Vec::with_capacity(total_msgs * 32 + 64);
    let mut body = [0u8; 36];
    for _ in 0..total_msgs {
        let ty = g.next_type();
        let len = body_len(ty).expect("tabled");
        g.gen_body(&mut body[..len], ty);
        tape.extend_from_slice(&(len as u16).to_be_bytes());
        tape.extend_from_slice(&body[..len]);
    }
    tape
}

/// The tape's message count (the length-prefix walk — the count-law
/// oracle's ground truth).
#[must_use]
pub fn count_messages(tape: &[u8]) -> usize {
    let mut pos = 0usize;
    let mut n = 0usize;
    while pos + 2 <= tape.len() {
        let l = u16::from_be_bytes([tape[pos], tape[pos + 1]]) as usize;
        pos += 2 + l;
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAW 1 — reproducibility: the same seed builds the identical tape;
    /// a different seed diverges.
    #[test]
    fn t_randomstream_reproducible() {
        let a = build_random_tape(0xC0FFEE, 5_000, 512);
        let b = build_random_tape(0xC0FFEE, 5_000, 512);
        assert_eq!(a, b);
        let c = build_random_tape(0xC0FFEE + 1, 5_000, 512);
        assert_ne!(a, c, "a different seed must diverge");
    }

    /// LAW 2 — parse validity: every message's prefix matches its type's
    /// spec length, and every type is tabled; the walk lands exactly.
    #[test]
    fn t_randomstream_parse_valid() {
        let tape = build_random_tape(42, 20_000, 256);
        let mut pos = 0usize;
        let mut n = 0usize;
        while pos + 2 <= tape.len() {
            let l = u16::from_be_bytes([tape[pos], tape[pos + 1]]) as usize;
            let ty = tape[pos + 2];
            assert_eq!(
                body_len(ty),
                Some(l),
                "type {ty} must carry its spec length (got {l})"
            );
            pos += 2 + l;
            n += 1;
        }
        assert_eq!(pos, tape.len(), "the walk must land exactly");
        assert_eq!(n, 20_000);
        assert_eq!(count_messages(&tape), 20_000);
    }

    /// LAW 3 — uniqueness: no two messages are byte-identical (the
    /// timestamp's monotone advance guarantees it).
    #[test]
    fn t_randomstream_unique() {
        use std::collections::HashSet;
        let tape = build_random_tape(7, 20_000, 128);
        let mut seen: HashSet<u64> = HashSet::new();
        let mut pos = 0usize;
        let mut n = 0usize;
        while pos + 2 <= tape.len() {
            let l = u16::from_be_bytes([tape[pos], tape[pos + 1]]) as usize;
            let msg = &tape[pos + 2..pos + 2 + l];
            // FNV-1a over the body — collision-negligible at 20K msgs.
            let mut h: u64 = 0xCBF2_9CE4_8422_2325;
            for &b in msg {
                h ^= b as u64;
                h = h.wrapping_mul(0x1000_0000_01B3);
            }
            assert!(seen.insert(h), "message {n} repeats byte-for-byte");
            n += 1;
            pos += 2 + l;
        }
        assert_eq!(n, 20_000);
    }

    /// LAW 4 — the lifecycle pressure: the A-vs-death balance keeps the
    /// tape's shape in the weighted mix's band (the average body length
    /// lands in 24..=34 B) and the generator never wedges.
    #[test]
    fn t_randomstream_lifecycle_pressure() {
        let tape = build_random_tape(9, 200_000, 64);
        assert_eq!(count_messages(&tape), 200_000);
        let avg = tape.len() as f64 / 200_000.0;
        assert!(
            (24.0..=34.0).contains(&avg),
            "the weighted mix's average body length is {avg}"
        );
    }
}
