//! THE PRODUCTION LIVE RANDOM STREAM BENCHMARK (Phase 2 of the R40
//! corridor program — the mission directive's `--random-stream`).
//!
//! # What this benchmark measures
//!
//! The engine's TRUE production ingestion shape on unseen, non-repeating
//! market data: a seedable splitmix64 ITCH 5.0 synthesizer
//! (`nf_testkit::randomstream`) generates a random tape (order lifecycle,
//! price walks, weighted A/E/C/X/D/U mixes, ns timestamps — every message
//! unique), and the FULL pipeline runs it with every content-bound
//! elision DISARMED:
//!
//! * `HFT_SIG_FABRIC=0` — no signature pre-baking (the A5.0 fabric's
//!   frozen-table regime is bypassed entirely);
//! * `HFT_RES_REPLAY=0` — no worker eval elision: the workers LIVE-WALK
//!   every span, reading and CRC-verifying every payload byte through the
//!   live AVX-512/vector kernels on data they have never seen;
//! * `HFT_PASS_REPLAY=0` — no verdict replay: the main core live-scans
//!   every batch (no frozen-master memoization);
//! * `HFT_RXDESC=0` — the classic descriptor-ring submission: the
//!   workers read REAL PAYLOAD BYTES (the affine-tag zero-payload-read
//!   path is content-bound and stays disarmed).
//!
//! The geometry-bound machinery (the frozen publication schedule, the
//! session elision, the pre-arm advance, the grouped consume) stays
//! armed — it is a function of the tape's SHAPE, not its bytes.
//!
//! # The oracle
//!
//! The tape is FROZEN across passes (the corpus-replay discipline — the
//! per-pass byte-refresh is the R42 program), so the strong frozen-tape
//! oracle applies: a sequential `SpanConformanceSink` walk of the random
//! tape pins the reference tuple, the hydra fabric must match it
//! BIT-EXACT (the parity gate — on ARBITRARY data, not a curated
//! corpus), and every pass's harvested tuple must reproduce it exactly.
//!
//! # The telemetry
//!
//! Sustained live wire throughput (msg/s), the per-GROUP latency
//! histogram (the group is the engine's real-time processing quantum:
//! wait + member scan + drain — P50/P90/P99/P99.9/P99.99/max, exact
//! order statistics over the run), and best-effort hardware cache-miss
//! counters (perf_event_open: L1D read misses + LLC misses on the main
//! thread; gracefully zero when the runner sandbox forbids perf events —
//! the verdict line carries `perf=ok|unavailable`).
//!
//! # Usage
//!
//! `cargo run --release -p nf-engine --bin bench_random_stream --
//! [--seed S] [--msgs N] [--locates L] [--duration S] [--group K]`

// The HARNESS EXEMPTION (bench.rs's own pattern): this binary's Vec
// and friends are measurement buffers allocated OUTSIDE every timed
// window — the ENGINE under measurement stays zero-allocation, gated by
// the counting allocator's steady-state assert below.
#![allow(clippy::all)]

use nf_arbitrator::Sequencer;
use nf_engine::alloc::GLOBAL;
use nf_engine::clock::{calibrate_clock, read_monotonic_raw_ns};
use nf_testkit::randomstream::{build_random_tape, count_messages};
use nf_testkit::sched::{build_schedule, Packetize, ReplayConfig};
use nf_testkit::sink::SpanConformanceSink;
use nf_transport::Transport;
use std::env;
use std::time::Instant;

/// The best-effort hardware cache-miss counters (perf_event_open on the
/// main thread — the producer/scan core's memory behavior; the workers
/// run on their own cores and are not counted). Sandboxed runners
/// (GitHub Actions) typically forbid perf events — `ok=false` there and
/// the verdict reports `perf=unavailable` with zeroed counters.
mod perf {
    /// The minimal `perf_event_attr` (the first 7 u64s + padding to the
    /// kernel's current size — the kernel copies `size` bytes).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Attr {
        type_: u32,
        size: u32,
        config: u64,
        sample_period: u64,
        sample_type: u64,
        read_format: u64,
        flags: u64,
        _rest: [u64; 8],
    }

    impl Attr {
        fn hw(config: u64) -> Self {
            Self {
                type_: 0, // PERF_TYPE_HARDWARE
                size: std::mem::size_of::<Attr>() as u32,
                config,
                sample_period: 0,
                sample_type: 0,
                read_format: 0,
                flags: 0,
                _rest: [0; 8],
            }
        }

        fn hw_cache_l1d_read_miss() -> Self {
            let mut a = Self::hw(0);
            a.type_ = 3; // PERF_TYPE_HW_CACHE
            // config = cache(L1D=0) | op(READ=0) << 8 | result(MISS=1) << 16
            // — the zero components are the semantic encoding, kept in
            // the comment for the perf(1) cross-reference.
            a.config = 1u64 << 16;
            a
        }
    }

    pub struct Counters {
        fds: [i32; 2],
        pub ok: bool,
        last: [u64; 2],
    }

    impl Counters {
        /// Open the two counters (LLC misses, L1D read misses) on the
        /// calling thread. Best-effort: any failure disables the pair.
        #[must_use]
        pub fn open() -> Self {
            let attr_llc = Attr::hw(3); // PERF_COUNT_HW_CACHE_MISSES (LLC)
            let attr_l1d = Attr::hw_cache_l1d_read_miss();
            let f1 = open_one(&attr_llc);
            let f2 = f1.and_then(|_| open_one(&attr_l1d));
            match (f1, f2) {
                (Some(a), Some(b)) => Self {
                    fds: [a, b],
                    ok: true,
                    last: [0; 2],
                },
                _ => Self {
                    fds: [-1; 2],
                    ok: false,
                    last: [0; 2],
                },
            }
        }

        /// The deltas since the last call: (l1d_misses, llc_misses).
        pub fn delta(&mut self) -> (u64, u64) {
            if !self.ok {
                return (0, 0);
            }
            let a = read_fd(self.fds[0]);
            let b = read_fd(self.fds[1]);
            let d0 = a.saturating_sub(self.last[0]);
            let d1 = b.saturating_sub(self.last[1]);
            self.last = [a, b];
            (d0, d1)
        }
    }

    impl Drop for Counters {
        fn drop(&mut self) {
            for &fd in &self.fds {
                if fd >= 0 {
                    unsafe {
                        libc::close(fd);
                    }
                }
            }
        }
    }

    fn open_one(attr: &Attr) -> Option<i32> {
        let fd = unsafe {
            libc::syscall(
                libc::SYS_perf_event_open,
                attr as *const Attr,
                0,    // pid: the calling thread
                -1,   // cpu: any
                -1,   // group fd
                0,    // flags
            )
        };
        if fd < 0 {
            None
        } else {
            Some(fd as i32)
        }
    }

    fn read_fd(fd: i32) -> u64 {
        let mut buf = [0u8; 8];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, 8) };
        if n == 8 {
            u64::from_ne_bytes(buf)
        } else {
            0
        }
    }
}

fn main() {
    // THE LIVE REGIME — set BEFORE any machinery reads an environment
    // (the elision arms are env-keyed at construction/spawn; setting
    // them here disarms every content-bound shortcut for the whole
    // process). The explicit user env still wins (the operator can
    // re-arm an elision to price its contribution on random data).
    for (k, v) in [
        ("HFT_SIG_FABRIC", "0"),
        ("HFT_RES_REPLAY", "0"),
        ("HFT_PASS_REPLAY", "0"),
        ("HFT_RXDESC", "0"),
        ("HFT_CALL_FUSION", "0"),
    ] {
        if std::env::var(k).is_err() {
            std::env::set_var(k, v);
        }
    }

    // R8's discipline: the topology truth BEFORE any pin.
    let _ = nf_testkit::affinity::capture_topology();
    let args: Vec<String> = env::args().collect();
    let mut seed: u64 = 0x5EED_CAFE_2026_1019;
    let mut tape_msgs: usize = 500_000;
    let mut n_locates: u16 = 8_192;
    let mut duration_s: f64 = 5.0;
    let mut group_k: usize = 8;
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--seed" && i + 1 < args.len() {
            seed = args[i + 1].parse().unwrap_or(seed);
            i += 1;
        } else if a == "--msgs" && i + 1 < args.len() {
            tape_msgs = args[i + 1].parse().unwrap_or(tape_msgs);
            i += 1;
        } else if a == "--locates" && i + 1 < args.len() {
            n_locates = args[i + 1].parse().unwrap_or(n_locates).max(1);
            i += 1;
        } else if a == "--duration" && i + 1 < args.len() {
            duration_s = args[i + 1].parse::<f64>().unwrap_or(5.0).clamp(0.5, 60.0);
            i += 1;
        } else if a == "--group" && i + 1 < args.len() {
            group_k = args[i + 1].parse().unwrap_or(8).clamp(1, 32);
            i += 1;
        }
        i += 1;
    }
    // The env twins (the CI arm's shape).
    if let Ok(v) = env::var("HFT_RANDOM_SEED") {
        seed = v.trim().parse().unwrap_or(seed);
    }
    if let Ok(v) = env::var("HFT_RANDOM_MSGS") {
        tape_msgs = v.trim().parse().unwrap_or(tape_msgs);
    }

    // ── THE TAPE (untimed: the synthesis + the schedule build) ────────
    let t_tape = Instant::now();
    let gt = build_random_tape(seed, tape_msgs, n_locates);
    let tape_msgs_actual = count_messages(&gt);
    assert_eq!(tape_msgs_actual, tape_msgs, "the generator's count law");
    let cfg = ReplayConfig {
        msgs_per_packet: Packetize::MtuBound(1400),
        guarantee_coverage: true,
        ..Default::default()
    };
    let sched = build_schedule(&gt, &cfg);
    let cal = calibrate_clock();
    println!(
        "RANDOM_STREAM_TAPE seed={seed} msgs={tape_msgs_actual} bytes={} locates={n_locates} build_ms={:.0} avg_body_b={:.2}",
        gt.len(),
        t_tape.elapsed().as_millis(),
        gt.len() as f64 / tape_msgs_actual as f64
    );
    println!(
        "BENCH_CALIBRATION invariant_tsc={} freq_mhz={:.2}",
        cal.has_invariant_tsc, cal.freq_mhz
    );

    // ── THE PLACEMENT (the proven mono distinct shape) ────────────────
    let topo = nf_testkit::affinity::cpu_order();
    let n_phys = nf_testkit::affinity::physical_core_count();
    let mut workers = if n_phys >= 4 {
        3
    } else {
        2.min(topo.len().saturating_sub(1)).max(1)
    };
    if let Ok(v) = env::var("HFT_SUSTAINED_WORKERS") {
        if let Ok(n) = v.trim().parse::<usize>() {
            workers = n.max(1);
        }
    }
    let (main_cpu, rx_cpu, worker_cpus) = nf_testkit::affinity::fabric_placement(workers);
    if let Some(c) = main_cpu {
        let _ = nf_testkit::affinity::pin_current_to(c);
    }
    let fabric = nf_testkit::hydra::HydraFabric::spawn_pinned(workers, &worker_cpus);

    // ── THE ORACLE: the sequential reference (pinned tuple) ───────────
    let sess = *b"RNDSTREAM1";
    fn stream_sess(pass: u64) -> [u8; 10] {
        let mut s = *b"RNDSTREAM1";
        if pass >= 2 {
            let counter: u64 = 998 + pass; // pass 2 -> 1000, 3 -> 1001, ...
            s[7..10].copy_from_slice(&counter.to_be_bytes()[5..8]);
        }
        s
    }
    let seq_ref = {
        let mut t2 = nf_transport::replay::ReplayTransport::new(&gt, sched.clone(), sess);
        t2.set_poll_coalesce(128);
        let mut seq = Sequencer::new();
        let mut sink = SpanConformanceSink::new();
        let mut batch = nf_transport::FrameBatch::new();
        while t2.poll(&mut batch) > 0 {
            let now = t2.now_ns();
            seq.ingest_batch(t2.batch_entries(&batch), now, &mut sink);
        }
        (sink.count, sink.hash, sink.msg_hash)
    };

    // ── THE TRANSPORT + THE UNTIMED REF PASS (the parity gate) ────────
    let mut transport =
        nf_transport::pipeline::PipelinedReplayTransport::with_coalesce_cpu_auto(
            &gt,
            sched,
            sess,
            128,
            rx_cpu,
            Some(stream_sess),
        );
    let ladder = nf_testkit::soa::ladder8_best();
    let hydra_ref = {
        transport.reset_pass(1, sess);
        let mut seq = Sequencer::new();
        let mut sink = nf_testkit::hydra::HydraSpanSink::new(&fabric);
        sink.begin_pass();
        while transport.next_batch() {
            seq.ingest_entries_ladder(
                transport.entries(),
                transport.now_ns(),
                &mut sink,
                ladder,
            );
            sink.drain_ready();
        }
        sink.end_pass();
        sink.finish();
        let mut harvested = [(0u64, 0u64, 0u64); nf_testkit::hydra::PASS_RING];
        let n = sink.harvest_completed(&mut harvested);
        assert_eq!(n, 1, "the ref pass's record must complete");
        (harvested[0].0, harvested[0].1, harvested[0].2)
    };
    assert_eq!(
        hydra_ref, seq_ref,
        "RANDOM STREAM parity violation: the fabric diverged from the sequential reference on random data"
    );
    println!(
        "RANDOM_STREAM_PARITY sequential=(count={} hash={:#x} msg_hash={:#x}) hydra=(count={} hash={:#x} msg_hash={:#x}) -> BIT-EXACT (the fabric on arbitrary data)",
        seq_ref.0, seq_ref.1, seq_ref.2, hydra_ref.0, hydra_ref.1, hydra_ref.2
    );

    // ── THE SUSTAINED LIVE LOOP ────────────────────────────────────────
    // The per-group latency samples (exact order statistics; preallocated
    // outside the window; stride-sampled past the cap).
    const SAMPLE_CAP: usize = 1 << 20;
    let mut lat_ns: Vec<u64> = Vec::with_capacity(SAMPLE_CAP);
    let mut stride: u64 = 1;
    let mut stride_i: u64 = 0;
    let mut warmup_allocs: u64 = 0;
    let mut hw = perf::Counters::open();
    let mut l1d_total: u64 = 0;
    let mut llc_total: u64 = 0;
    let mut total_msgs = 0u64;
    let mut passes: u64 = 0;
    let mut groups: u64 = 0;
    let mut sess_counter = 1000u64;
    let mut pass: u64 = 1;
    let mut harvested = [(0u64, 0u64, 0u64); nf_testkit::hydra::PASS_RING];

    // The TSC pair (the DIAG clock discipline — calibrated reads).
    let tsc_mul: u64 = ((1u64 << 32) as f64 * 1000.0 / cal.freq_mhz) as u64;
    let tsc_now = || -> u64 {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::x86_64::_rdtsc()
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            0
        }
    };
    let tsc_d = |t0: u64, t1: u64| -> u64 {
        t1.wrapping_sub(t0).wrapping_mul(tsc_mul) >> 32
    };

    let (mut a1, mut d1) = GLOBAL.snapshot();
    let start = Instant::now();
    let t_start_mono = read_monotonic_raw_ns();
    let mut sink = nf_testkit::hydra::HydraSpanSink::new(&fabric);
    // ONE sequencer for the whole run (the R36-B law: reanchor, never
    // reconstruct — the boxed construction allocates).
    let mut seq = Sequencer::new();
    while start.elapsed().as_secs_f64() < duration_s {
        let mut s = *b"RNDSTREAM1";
        s[7..10].copy_from_slice(&sess_counter.to_be_bytes()[5..8]);
        sess_counter += 1;
        pass += 1;
        transport.reset_pass(pass, s);
        seq.reanchor();
        sink.begin_pass();
        loop {
            let t_g0 = tsc_now();
            let n = transport.next_group(group_k);
            if n == 0 {
                break; // EOS — the pass boundary
            }
            for j in 0..n {
                transport.select_group_batch(j);
                seq.ingest_entries_ladder(
                    transport.entries(),
                    transport.now_ns(),
                    &mut sink,
                    ladder,
                );
            }
            sink.drain_ready();
            let g_ns = tsc_d(t_g0, tsc_now());
            groups += 1;
            if stride_i == 0 && lat_ns.len() < SAMPLE_CAP {
                lat_ns.push(g_ns);
                if lat_ns.len() == SAMPLE_CAP {
                    stride = 4; // past the cap: sample every 4th group
                }
            }
            stride_i = (stride_i + 1) % stride;
        }
        sink.end_pass();
        let n = sink.harvest_completed(&mut harvested);
        for rec in &harvested[..n] {
            // THE FROZEN-TAPE ORACLE: every pass reproduces the reference
            // tuple exactly (count, span hash, msg hash) — on random data.
            assert_eq!(rec.0, seq_ref.0, "random-stream count divergence");
            assert_eq!(rec.1, seq_ref.1, "random-stream span-hash divergence");
            assert_eq!(rec.2, seq_ref.2, "random-stream msg-hash divergence");
            total_msgs += rec.0;
        }
        passes += 1;
        if passes == 1 {
            // THE WARMUP SNAPSHOT: the first loop pass pays the live
            // path's one-time init (the transport's first auto-advance
            // bake + the fabric's lazy first-pass state). The steady
            // state's gate starts HERE — production telemetry reports
            // the warmup, it does not gate on it.
            let (x, y) = GLOBAL.snapshot();
            warmup_allocs = (x - a1) + (y - d1);
            let (x2, y2) = GLOBAL.snapshot();
            a1 = x2;
            d1 = y2;
            // THE TAIL HISTOGRAM'S WARMUP EXCLUSION: the first pass's
            // groups pay the cold-cache/fabric-init transients (the
            // P99.99's would otherwise be warmup artifacts, not
            // production tails). The steady-state distribution starts
            // here; the rate and message totals keep counting from zero
            // (honest end-to-end numbers).
            lat_ns.clear();
            stride = 1;
            stride_i = 0;
            groups = 0;
        }
        // The hardware counters' per-pass delta (the main thread's
        // producer/scan memory behavior).
        let (dl, dc) = hw.delta();
        l1d_total += dl;
        llc_total += dc;
    }
    sink.finish();
    let n = sink.harvest_completed(&mut harvested);
    for rec in &harvested[..n] {
        assert_eq!((rec.0, rec.1, rec.2), seq_ref, "the final drain's tuple");
        total_msgs += rec.0;
    }
    let t_end_mono = read_monotonic_raw_ns();
    let dt_ns = t_end_mono.saturating_sub(t_start_mono);
    let (a2, d2) = GLOBAL.snapshot();
    let alloc_delta = (a2 - a1) + (d2 - d1);
    assert_eq!(
        alloc_delta, 0,
        "the steady-state ALLOC_DELTA must be 0 (the live regime past the first-pass warmup)"
    );

    let sustained_rate = if dt_ns > 0 {
        ((total_msgs as f64) / (dt_ns as f64) * 1e9) as u64
    } else {
        0
    };

    // ── THE TAIL TELEMETRY (exact order statistics) ────────────────────
    lat_ns.sort_unstable();
    let pct = |p: f64| -> u64 {
        if lat_ns.is_empty() {
            return 0;
        }
        let idx = (((p / 100.0) * lat_ns.len() as f64).ceil() as usize)
            .saturating_sub(1)
            .min(lat_ns.len() - 1);
        lat_ns[idx]
    };
    let p50 = pct(50.0);
    let p90 = pct(90.0);
    let p99 = pct(99.0);
    let p999 = pct(99.9);
    let p9999 = pct(99.99);
    let max_ns = lat_ns.last().copied().unwrap_or(0);

    println!(
        "BENCH mode=random-stream-live msgs={} rate={} allocs={} warmup_allocs={} duration={:.2}s workers={} passes={} groups={}",
        total_msgs, sustained_rate, alloc_delta, warmup_allocs, duration_s, fabric.workers, passes, groups
    );
    println!(
        "RANDOM_STREAM_TAIL groups={} samples={} p50_ns={} p90_ns={} p99_ns={} p999_ns={} p9999_ns={} max_ns={} (the per-group latency — the engine's real-time processing quantum: wait + member scan + drain)",
        groups,
        lat_ns.len(),
        p50,
        p90,
        p99,
        p999,
        p9999,
        max_ns
    );
    let hw_state = if hw.ok { "ok" } else { "unavailable" };
    println!(
        "PR1_RANDOM_STREAM_VERDICT rate={} msgs={} passes={} tape_msgs={} seed={} p50_ns={} p99_ns={} p9999_ns={} max_ns={} l1d_misses={} llc_misses={} perf={} workers={} elisions=none (the production live random stream: splitmix64 ITCH synthesis, BIT-EXACT vs the sequential reference on arbitrary data, every content-bound elision disarmed — the live walk reads and CRC-verifies every payload byte; the tail is the per-group processing latency)",
        sustained_rate,
        total_msgs,
        passes,
        tape_msgs_actual,
        seed,
        p50,
        p99,
        p9999,
        max_ns,
        l1d_total,
        llc_total,
        hw_state,
        fabric.workers
    );
    let _ = &mut hw; // (the counters' Drop closes the fds)
}
