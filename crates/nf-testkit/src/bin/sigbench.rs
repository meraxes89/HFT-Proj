//! sigbench — R34 fold-stage roofline instrument (the 𝒜₅.₀ collapse).
//!
//! # Why this exists
//!
//! docs/solution.md §8.5 demands the fold factor be priced as its own
//! number, separately from decode and cache probes, on the CI runner,
//! with pinning logged: "fold-stage cycles/msg isolated (fold stub vs
//! fold live, subtract)". This instrument prices the three regimes the
//! blueprint's roofline table predicts, on a deterministic 500,000-token
//! stream (the doc's own working set):
//!
//! * `cold` — the regime-1/2 fold stage: 977 × 512-message chunks, each
//!   folded by the dispatching kernel (AVX-512 VPADDQ lane-sums for the
//!   K1 volumes + `vpcmpuq`/mask-blend pairwise tagged max for the K2
//!   price — `_mm512_max_epu64` + `_mm512_mask_mov_epi64`), scalar tail
//!   folded once. Predicted: 0.43–0.60 cyc/msg (§6.5), ~0.1 ms per 500k
//!   pass on one 2.6 GHz core.
//! * `replay` — the regime-3 arithmetic: 976 ⊕ combines over the stored
//!   chunk signatures (the memoized-replay path's whole arithmetic —
//!   ~4k scalar-equivalent ops per 500k-message pass).
//! * `combine` — the ⊕ micro-rate (ops/s of the 6-ALU combine itself),
//!   the per-message floor any orchestration must sit above.
//!
//! The sequential oracle runs alongside in every timed regime and the
//! verdict line asserts bit-exact equality (Theorem 8) — the instrument
//! never prices on faith: `bitexact=true` on the verdict is computed,
//! not assumed. Dual-path equality (AVX-512 vs the portable twin) is
//! asserted over the WHOLE stream — Corollary 2.1 on real hardware.

// Diagnostic tool, not an engine binary: it allocates, prints freely,
// and shares nothing with the measured fabric beyond the kernel API —
// the workspace's Tier-F zero-allocation law does not apply.
#![allow(clippy::all)]
#![allow(warnings)]

use nf_testkit::lobstate::{
    sig_chunk_signatures, sig_combine, sig_fold, sig_fold_scalar, Lob3, Sig, SIG_CHUNK,
};
use std::time::Instant;

const N: usize = 500_000;

/// splitmix64 — the deterministic stream generator (the doc's fuzz PRNG).
struct Sm64(u64);
impl Sm64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The deterministic 500k-token corpus: ~1/4 price-setters with
/// injective 1-based keys (T2/T5), full-range wrapping K1 deltas (the
/// H4 stress: the oracle accumulates identically), dead payload on
/// non-setters (T3).
fn build_stream(n: usize) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
    let mut rng = Sm64(0x0DD0_5EED_00C0_FFEE);
    let mut seq = vec![0u64; n];
    let mut px = vec![0u64; n];
    let mut db = vec![0u64; n];
    let mut da = vec![0u64; n];
    for i in 0..n {
        let is_setter = rng.next() % 4 == 0;
        seq[i] = if is_setter { i as u64 + 1 } else { 0 };
        px[i] = if is_setter { rng.next() >> 32 } else { rng.next() };
        db[i] = rng.next();
        da[i] = rng.next();
    }
    (seq, px, db, da)
}

#[cfg(target_arch = "x86_64")]
fn rdtsc() -> u64 {
    // SAFETY: rdtsc is an unprivileged userspace instruction.
    unsafe { std::arch::x86_64::_rdtsc() }
}

/// Calibrate the TSC against the wall clock over ~200 ms (the invariant
/// TSC law the engine's BENCH_CALIBRATION already relies on).
#[cfg(target_arch = "x86_64")]
fn calibrate_tsc() -> u64 {
    let t0 = Instant::now();
    let c0 = rdtsc();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let dc = rdtsc().wrapping_sub(c0);
    let dt = t0.elapsed();
    (dc as f64 / dt.as_secs_f64()) as u64
}

fn main() {
    #[cfg(target_arch = "x86_64")]
    let avx512 = std::arch::is_x86_feature_detected!("avx512f");
    #[cfg(not(target_arch = "x86_64"))]
    let avx512 = false;

    #[cfg(target_arch = "x86_64")]
    let tsc_hz = calibrate_tsc();
    #[cfg(not(target_arch = "x86_64"))]
    let tsc_hz = 0u64;

    let chunks = (N + SIG_CHUNK - 1) / SIG_CHUNK; // 977 = 976·512 + 288
    let (seq, px, db, da) = build_stream(N);
    let s0 = Lob3 { b: 1 << 20, a: 1 << 20, p: 0 };

    println!("R34_SIGBENCH tokens={N} chunks={chunks} avx512={avx512} tsc_hz={tsc_hz}");

    // ── The oracle: sequential per-token application (Theorem 8's judge).
    let oracle = {
        let mut s = s0;
        for i in 0..N {
            s.apply(Sig { db: db[i], da: da[i], px: px[i], seq: seq[i] });
        }
        s
    };

    // ── Dual-path equality over the WHOLE stream (Corollary 2.1).
    let whole_vec = sig_fold(&seq, &px, &db, &da);
    let whole_scalar = sig_fold_scalar(&seq, &px, &db, &da);
    let dual_path_ok = whole_vec == whole_scalar;

    // ── Regime: cold chunk fold. Warmup 1, best of 5 (min = the
    //    noise-free floor; the doc's protocol: "measured after warmup
    //    of code paths only").
    let _ = sig_chunk_signatures(&seq, &px, &db, &da, SIG_CHUNK);
    let mut cold_best_ns = u128::MAX;
    let mut cold_best_cyc = 0u128;
    for _ in 0..5 {
        let t = Instant::now();
        #[cfg(target_arch = "x86_64")]
        let c0 = rdtsc();
        let sigs = sig_chunk_signatures(&seq, &px, &db, &da, SIG_CHUNK);
        let dt = t.elapsed();
        #[cfg(target_arch = "x86_64")]
        {
            let dc = rdtsc().wrapping_sub(c0) as u128;
            if dt.as_nanos() < cold_best_ns {
                cold_best_ns = dt.as_nanos();
                cold_best_cyc = dc;
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            if dt.as_nanos() < cold_best_ns {
                cold_best_ns = dt.as_nanos();
            }
        }
        // The replay store must survive the loop (the last fold's output
        // is the correctness-checked set below).
        let _keep = &sigs;
    }
    let sigs = sig_chunk_signatures(&seq, &px, &db, &da, SIG_CHUNK);

    // ── Regime: the 976-⊕ replay. 1000 timed reductions of the stored
    //    signatures; best-of reports the arithmetic's floor. The result
    //    feeds a checksum sink — an observable use, so the reduction
    //    cannot be dead-code-eliminated out of the timed region.
    let mut chk: u64 = 0;
    let mut replay_best_ns = u128::MAX;
    for _ in 0..1000 {
        let t = Instant::now();
        let acc = sigs.iter().copied().fold(Sig::IDENTITY, sig_combine);
        let dt = t.elapsed();
        chk = chk.wrapping_add(acc.db).wrapping_add(acc.seq << 1).wrapping_add(acc.px);
        if dt.as_nanos() < replay_best_ns {
            replay_best_ns = dt.as_nanos();
        }
    }

    // ── Regime: the ⊕ micro-rate (ops/s of the combine itself). The
    //    operands are the REAL stored signatures read round-robin (the
    //    regime-3 table-read shape — periodic closed-form inputs would
    //    let LLVM scalar-evolve the loop away), and the accumulator
    //    feeds the checksum sink: no dead-code elimination possible.
    let mut acc = Sig::IDENTITY;
    let mut ops = 0u64;
    let m = sigs.len();
    let mut idx = 0usize;
    let t = Instant::now();
    while t.elapsed().as_nanos() < 50_000_000 {
        for _ in 0..100_000 {
            acc = sig_combine(acc, sigs[idx]);
            idx += 1;
            if idx == m {
                idx = 0;
            }
        }
        chk = chk.wrapping_add(acc.db).wrapping_add(acc.seq);
        ops += 100_000;
    }
    let combine_ns_per_op = t.elapsed().as_nanos() as f64 / ops as f64;

    // ── Correctness: the collapse's three-way equality (Theorem 4 + 8).
    let fwd = sigs.iter().copied().fold(Sig::IDENTITY, sig_combine);
    let mut folded = s0;
    folded.apply(fwd);
    let bitexact = fwd == whole_vec && folded == oracle;

    let cold_msg_per_s = N as f64 / (cold_best_ns as f64 / 1e9);
    let replay_msg_per_s = N as f64 / (replay_best_ns as f64 / 1e9);
    let fold_cyc_per_msg = cold_best_cyc as f64 / N as f64;

    println!(
        "R34_FOLD_COLD best_ns={cold_best_ns} passes=5 msg_per_s={cold_msg_per_s:.0} cyc_per_msg={fold_cyc_per_msg:.4} chunks={chunks}"
    );
    println!(
        "R34_FOLD_REPLAY best_ns={replay_best_ns} iterations=1000 combines={} msg_per_s_equiv={replay_msg_per_s:.0}",
        chunks - 1
    );
    println!(
        "R34_COMBINE ns_per_op={combine_ns_per_op:.3} ops={ops} chk={chk:#x}"
    );
    println!(
        "PR1_R34_AFFINE_FOLD_VERDICT fold_cyc_per_msg={fold_cyc_per_msg:.4} cold_fold_msg_per_s={cold_msg_per_s:.0} replay_msg_per_s_equiv={replay_msg_per_s:.0} chunks={chunks} combines={} avx512={avx512} bitexact={bitexact} dual_path={dual_path_ok} (R34: the A5.0 signature collapse — 500k sequential folds become {chunks} chunk folds + {} O(1) combines; the fold stage leaves the critical path, docs/45)",
        chunks - 1,
        chunks - 1
    );
    assert!(bitexact, "Theorem 8 violated: fold+Psi != sequential oracle");
    assert!(dual_path_ok, "Corollary 2.1 violated: AVX-512 != scalar");
}
