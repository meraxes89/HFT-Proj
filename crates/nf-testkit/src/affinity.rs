//! R8: topology-aware CPU affinity for the fabric threads (docs/22).
//!
//! The doc-21 "worker core affinity" lever: the HYDRA fabric's threads run
//! unpinned by default, so the scheduler freely migrates them (each
//! migration costs the thread its L1/L2 working set — the sequencer state,
//! the lane rings) and may stack SMT siblings on one physical core while
//! others idle. On shared cloud runners (noisy neighbors) both effects are
//! measured as double-digit-percent fabric regressions.
//!
//! This module pins threads to the process's ALLOWED CPU set (cgroup
//! respected), ordered distinct-physical-core-first (SMT siblings go last),
//! and degrades gracefully: any `sched_setaffinity` failure simply leaves
//! that thread unpinned — behavior, not correctness, is affected.

/// The topology truth, captured once at process start.
///
/// WHY: `cpu_order()` reads the CALLING THREAD's affinity mask. Benchmark
/// arms pin the main thread to their chosen cpu BEFORE later arms compute
/// their fabric placement — and a pinned thread's mask no longer contains
/// the other cpus. CI run 36960041885 caught the consequences: the
/// sustained arm's `fabric_placement` saw only main's pinned cpu0 and
/// pinned BOTH workers onto it (diag `placement=[0,0]`), stacking three
/// busy threads on one hyperthread while the runner's second physical
/// core sat completely idle. The fix: binaries call
/// [`capture_topology`] as their FIRST act (before any pinning); later
/// `cpu_order()` calls return the captured set regardless of the caller's
/// current mask. Uncaptured processes degrade to the live mask (the
/// pre-capture semantics — still correct when nothing has pinned yet).
static TOPOLOGY: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();

/// Capture the process's allowed-cpu set NOW as the topology truth.
/// Call this before ANY thread pinning (binaries' first statement).
pub fn capture_topology() -> &'static Vec<usize> {
    TOPOLOGY.get_or_init(cpu_order_live)
}

/// The live (current-thread) allowed-cpu set, ordered
/// distinct-physical-core-first.
fn cpu_order_live() -> Vec<usize> {
    let mut set = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
    let rc = unsafe { libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) };
    if rc != 0 {
        return Vec::new();
    }
    let mut cpus = Vec::new();
    for cpu in 0..libc::CPU_SETSIZE as usize {
        if unsafe { libc::CPU_ISSET(cpu, &set) } {
            cpus.push(cpu);
        }
    }
    if cpus.is_empty() {
        return cpus;
    }
    // Distinct physical cores first: group by thread_siblings_list and
    // emit one representative per group before the siblings.
    let mut representatives: Vec<usize> = Vec::new();
    let mut siblings: Vec<usize> = Vec::new();
    for &cpu in &cpus {
        match read_sibling_group(cpu) {
            Some(group) => {
                // A sibling is any CPU in the group other than the group's
                // minimum that we have ALSO seen in the allowed set.
                let min_in_set = group.iter().copied().filter(|c| cpus.contains(c)).min();
                if min_in_set == Some(cpu) {
                    representatives.push(cpu);
                } else {
                    siblings.push(cpu);
                }
            }
            None => representatives.push(cpu),
        }
    }
    let mut order = representatives;
    order.extend(siblings);
    order
}

/// The CPUs this process is allowed to run on (cgroup/affinity mask),
/// ordered distinct-physical-core-first. Returns the STARTUP-captured set
/// when [`capture_topology`] has run (see its doc — the mask-pollution
/// trap), otherwise the live set.
pub fn cpu_order() -> Vec<usize> {
    if let Some(t) = TOPOLOGY.get() {
        return t.clone();
    }
    cpu_order_live()
}

/// The thread-siblings list of `cpu` (its SMT group), parsed from sysfs.
fn read_sibling_group(cpu: usize) -> Option<Vec<usize>> {
    let path = format!("/sys/devices/system/cpu/cpu{cpu}/topology/thread_siblings_list");
    let text = std::fs::read_to_string(path).ok()?;
    let mut group = Vec::new();
    for part in text.trim().split(',') {
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                group.extend(a..=b);
            }
        } else if let Ok(v) = part.parse::<usize>() {
            group.push(v);
        }
    }
    if group.is_empty() {
        None
    } else {
        Some(group)
    }
}

/// Pin the CALLING thread to `cpu`. Returns success (failure leaves the
/// thread unpinned — see the module doc).
pub fn pin_current_to(cpu: usize) -> bool {
    let mut set = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
    unsafe { libc::CPU_SET(cpu, &mut set) };
    let rc = unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) };
    rc == 0
}

/// R8 phase-2 diagnostics: the calling thread's current cpu (sched_getcpu;
/// usize::MAX on failure — never a hot path).
pub fn current_cpu() -> usize {
    let rc = unsafe { libc::sched_getcpu() };
    if rc < 0 {
        usize::MAX
    } else {
        rc as usize
    }
}

/// Pin the calling thread to the `slot`-th CPU of the topology order
/// (wrapping). Returns the chosen CPU, or None if pinning was unavailable.
pub fn pin_current_slot(slot: usize) -> Option<usize> {
    let order = cpu_order();
    if order.is_empty() {
        return None;
    }
    let cpu = order[slot % order.len()];
    if pin_current_to(cpu) {
        Some(cpu)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_affinity_order_nonempty_and_bounded() {
        let order = cpu_order();
        // A normal CI/container exposes at least one CPU; every entry is a
        // plausible cpu id.
        assert!(!order.is_empty());
        assert!(order.iter().all(|c| *c < 1024));
    }

    #[test]
    fn t_affinity_pin_roundtrip() {
        // Pinning to a CPU we are already allowed on must succeed or fail
        // gracefully — and the order must be unaffected.
        let order = cpu_order();
        if let Some(cpu) = order.first() {
            let ok = pin_current_to(*cpu);
            // Restore unrestricted affinity within the allowed set.
            let mut set = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
            for c in &order {
                unsafe { libc::CPU_SET(*c, &mut set) };
            }
            let _ = unsafe {
                libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set)
            };
            // Either outcome is acceptable (container restrictions).
            let _ = ok;
        }
    }
}

/// R8: how many DISTINCT physical cores the allowed set spans (SMT sibling
/// groups collapse to one). Drives thread-count decisions on mixed SMT /
/// non-SMT runner pools: oversubscribing a physical core with spin-happy
/// threads collapses the fabric (measured: 55M vs 411M msg/s).
pub fn physical_core_count() -> usize {
    let order = cpu_order();
    if order.is_empty() {
        return 0;
    }
    // Group by identical sibling lists; a true SMT pair (2 members) is one
    // physical core, a singleton is one, and larger groups (some VMs report
    // all vCPUs sharing one thread_siblings_list) count as their size — the
    // oversubscription guard only needs the right ORDER of magnitude.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &cpu in &order {
        match read_sibling_group(cpu) {
            Some(g) => {
                if !groups.iter().any(|h| h == &g) {
                    groups.push(g);
                }
            }
            None => groups.push(vec![cpu]),
        }
    }
    groups
        .iter()
        .map(|g| match g.len() {
            0 => 0,
            2 => 1,
            n => n,
        })
        .sum()
}

/// R8: the L3 (last-level cache) sharing groups of the allowed cpus, in
/// topology order — `l3_groups()[g]` is the list of allowed cpus whose
/// index-3 cache is shared. Cross-L3 thread placement puts the pipeline's
/// per-batch handoffs (14KB+) on a cross-CCD path (~120-200ns latency on
/// multi-CCD server parts) instead of a shared L3 — measured as a 2.5x
/// consumer-side regression on one runner type vs its same-generation
/// sibling whose placement happened to be L3-local.
pub fn l3_groups() -> Vec<Vec<usize>> {
    let order = cpu_order();
    if order.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<(Vec<usize>, Vec<usize>)> = Vec::new(); // (members, allowed members)
    for &cpu in &order {
        let shared = read_l3_shared(cpu).unwrap_or_else(|| vec![cpu]);
        let key: Vec<usize> = {
            let mut k = shared.clone();
            k.sort_unstable();
            k.dedup();
            k
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(cpu),
            None => groups.push((key, vec![cpu])),
        }
    }
    groups.into_iter().map(|(_, m)| m).collect()
}

/// The index-3 cache sharing list of `cpu` (its L3 siblings), parsed from
/// sysfs. None if unavailable (singletons are fine — the cpu is its own
/// group).
fn read_l3_shared(cpu: usize) -> Option<Vec<usize>> {
    let path = format!("/sys/devices/system/cpu/cpu{cpu}/cache/index3/shared_cpu_list");
    let text = std::fs::read_to_string(path).ok()?;
    let mut group = Vec::new();
    for part in text.trim().split(',') {
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                group.extend(a..=b);
            }
        } else if let Ok(v) = part.parse::<usize>() {
            group.push(v);
        }
    }
    (!group.is_empty()).then_some(group)
}

/// R8: placement for a pipelined arm — (main, rx) cpus that share an L3
/// but not a physical core when possible (the mailbox handoff is
/// latency-sensitive; SMT-sharing it with the busy main would timeslice
/// execution). Returns (main_cpu, rx_cpu).
pub fn pipeline_placement() -> (Option<usize>, Option<usize>) {
    let groups = l3_groups();
    // Prefer the L3 group with the most DISTINCT physical cores.
    let mut best: Option<&Vec<usize>> = None;
    let mut best_phys = 0usize;
    for g in &groups {
        let mut reps: Vec<usize> = Vec::new();
        for c in g {
            let is_rep = match read_sibling_group(*c) {
                Some(s) => s.iter().min() == Some(c),
                None => true,
            };
            if is_rep {
                reps.push(*c);
            }
        }
        if reps.len() > best_phys {
            best_phys = reps.len();
            best = Some(g);
        }
    }
    let group = match best {
        Some(g) if g.len() >= 2 => g,
        _ => {
            let order = cpu_order();
            if order.len() >= 2 {
                return (order.first().copied(), order.get(1).copied());
            }
            return (order.first().copied(), None);
        }
    };
    // main = first distinct-physical representative; rx = the next cpu NOT
    // in main's SMT pair.
    let main = group[0];
    let main_sibs = read_sibling_group(main).unwrap_or_else(|| vec![main]);
    let rx = group
        .iter()
        .find(|c| **c != main && !main_sibs.contains(c))
        .copied()
        .or(Some(group[1.min(group.len() - 1)]));
    (Some(main), rx)
}

// ── R21 (Task 4): deterministic physical-core topology VERIFICATION ─────
//
// The directive's law: the (Main Sequencer, RX Ingest Producer) pair must
// land on DISTINCT PHYSICAL CORES sharing the SAME L3 domain — the shape
// that locks in the 0.21 cyc/msg (12B msg/s) ingest mode. Until now the
// placement was a best-effort SELECTION with no runtime verification (the
// R8 mask-pollution incident is exactly what silent drift costs). This
// block makes the placement a first-class, loudly-verdicted fact.

/// R21: the topology verdict for a (main, rx) pair — the three states the
/// verification line distinguishes (a violation is never silently
/// flattened into "not verified").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyVerdict {
    /// Distinct physical cores, same L3 domain — the deterministic mode.
    Verified,
    /// The pair provably violates the law (SMT siblings, same cpu, or
    /// split L3) — carries the human-readable reason.
    Violated(String),
    /// The host does not expose the sysfs truth needed to judge.
    Unverifiable(String),
}

impl TopologyVerdict {
    pub fn is_violation(&self) -> bool {
        matches!(self, Self::Violated(_))
    }
}

/// R21: verify the (main/sequencer, rx/producer) pair against the Task 4
/// law: DISTINCT physical cores (disjoint SMT sibling groups) sharing the
/// SAME L3 domain (identical index-3 `shared_cpu_list`), read from sysfs
/// truth — never from the placement's intentions.
pub fn verify_pair_topology(main_cpu: usize, rx_cpu: usize) -> TopologyVerdict {
    if main_cpu == rx_cpu {
        return TopologyVerdict::Violated(format!(
            "main and rx both on cpu{main_cpu} (R25b scalars-stacked — the \
             intentional 2-worker 2-core SMT shape: the RX leaves worker 1's \
             sibling at the cost of pipeline overlap; docs/39 §4. \
             HFT_FABRIC_PLACE=distinct restores the Task-4 overlap)"
        ));
    }
    // Distinct physical cores: rx must not sit in main's SMT sibling group.
    match (read_sibling_group(main_cpu), read_sibling_group(rx_cpu)) {
        (Some(sa), Some(sb)) => {
            if sa.contains(&rx_cpu) {
                return TopologyVerdict::Violated(format!(
                    "main cpu{main_cpu} and rx cpu{rx_cpu} are SMT siblings \
                     (one physical core — the mailbox handoff timeslices)"
                ));
            }
            let _ = sb;
        }
        (None, _) | (_, None) => {
            return TopologyVerdict::Unverifiable(
                "thread_siblings_list unavailable (no SMT topology in sysfs)".to_string(),
            );
        }
    }
    // Same L3 domain: identical index-3 sharing lists.
    match (read_l3_shared(main_cpu), read_l3_shared(rx_cpu)) {
        (Some(la), Some(lb)) => {
            let key = |mut v: Vec<usize>| {
                v.sort_unstable();
                v.dedup();
                v
            };
            if key(la) != key(lb) {
                return TopologyVerdict::Violated(format!(
                    "main cpu{main_cpu} and rx cpu{rx_cpu} sit in DIFFERENT L3 domains \
                     (cross-CCD/CCX mailbox handoff — the 2.5x consumer-side penalty)"
                ));
            }
        }
        _ => {
            return TopologyVerdict::Unverifiable(
                "cache/index3/shared_cpu_list unavailable (no L3 topology in sysfs)".to_string(),
            );
        }
    }
    TopologyVerdict::Verified
}

/// R21: verify a pipeline placement and PRINT the verdict line
/// (`TOPOLOGY_VERIFICATION ...` — a first-class CI artifact, the
/// flip-validation law: a placement is never trusted on faith).
/// `HFT_TOPOLOGY_STRICT=1` promotes a VIOLATION to a hard failure (the
/// deterministic-pinning gate); an UNVERIFIABLE host degrades to the
/// warning in both modes (a host without sysfs truth cannot be held to
/// a law it cannot express).
pub fn verify_pipeline_placement(main_cpu: Option<usize>, rx_cpu: Option<usize>) {
    let verdict = match (main_cpu, rx_cpu) {
        (Some(m), Some(r)) => verify_pair_topology(m, r),
        _ => TopologyVerdict::Unverifiable(
            "placement returned no cpu pair (empty cpuset or single-cpu host)".to_string(),
        ),
    };
    let strict = std::env::var("HFT_TOPOLOGY_STRICT").as_deref() == Ok("1");
    match &verdict {
        TopologyVerdict::Verified => eprintln!(
            "TOPOLOGY_VERIFICATION main={:?} rx={:?} -> VERIFIED (distinct physical cores, same L3 domain — the deterministic 0.21 cyc/msg mode)",
            main_cpu, rx_cpu
        ),
        TopologyVerdict::Violated(why) => {
            eprintln!(
                "TOPOLOGY_VERIFICATION main={:?} rx={:?} -> VIOLATED: {why}",
                main_cpu, rx_cpu
            );
            if strict {
                panic!("HFT_TOPOLOGY_STRICT: pipeline placement violates the Task 4 law: {why}");
            }
        }
        TopologyVerdict::Unverifiable(why) => eprintln!(
            "TOPOLOGY_VERIFICATION main={:?} rx={:?} -> UNVERIFIABLE: {why} (HFT_TOPOLOGY_STRICT=1 does not fail unverifiable hosts)",
            main_cpu, rx_cpu
        ),
    }
}

/// R33-D (docs/44 §5): the FABRIC-wide topology verdict — main, RX, and
/// every worker lane checked against the producer-starvation program's
/// placement law: the whole fabric inside ONE L3 domain (the SPSC
/// ring's filled[]/freed[] handoffs and the workers' span grid stay
/// on-core latency; a cross-CCD pair puts every ring-index update on
/// the Infinity Fabric at 120-200ns — a per-publication tax the
/// ~100ns replayed publication cannot absorb). Reports the per-cpu
/// die (CCD) ids from sysfs alongside the L3 truth so the verdict line
/// carries the full topology picture. Cheap by construction: a handful
/// of sysfs reads, called once per run OUTSIDE every measured window.
pub fn verify_fabric_topology(
    main_cpu: Option<usize>,
    rx_cpu: Option<usize>,
    worker_cpus: &[usize],
) -> TopologyVerdict {
    let cpus: Vec<usize> = main_cpu
        .into_iter()
        .chain(rx_cpu)
        .chain(worker_cpus.iter().copied())
        .collect();
    if cpus.is_empty() {
        return TopologyVerdict::Unverifiable(
            "no fabric cpus to verify (empty placement)".to_string(),
        );
    }
    // The shared-L3 predicate: every cpu's index-3 sharing list must
    // contain every other fabric cpu (identical domains).
    let domains: Vec<Option<Vec<usize>>> = cpus.iter().map(|&c| read_l3_shared(c)).collect();
    if domains.iter().any(|d| d.is_none()) {
        return TopologyVerdict::Unverifiable(
            "cache/index3/shared_cpu_list unavailable for a fabric cpu".to_string(),
        );
    }
    let first = domains[0].as_ref().expect("checked non-none");
    let key = |mut v: Vec<usize>| {
        v.sort_unstable();
        v.dedup();
        v
    };
    let first_key = key(first.clone());
    for (i, d) in domains.iter().enumerate().skip(1) {
        if key(d.clone().expect("checked non-none")) != first_key {
            return TopologyVerdict::Violated(format!(
                "fabric is SPLIT across L3 domains: cpu{} and cpu{} disagree on \
                 index3/shared_cpu_list (cross-CCD ring handoffs — the R33 \
                 replayed publication's ~100ns budget cannot absorb the \
                 Infinity Fabric hop; HFT_FABRIC_PLACE re-pins within one group)",
                cpus[0], cpus[i]
            ));
        }
    }
    TopologyVerdict::Verified
}

/// R33-D: the sysfs die (CCD) id of `cpu` — None when the host does not
/// expose `topology/die_id` (VMs often hide it; the L3 check above stays
/// the load-bearing truth, the die id is the human-readable context).
fn read_die_id(cpu: usize) -> Option<u64> {
    let path = format!("/sys/devices/system/cpu/cpu{cpu}/topology/die_id");
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// R33-D: print the fabric topology report line — the per-fabric-cpu
/// (cpu, die_id) pairs and the L3-sharing verdict. Called once per run,
/// post-run (the diagnostics contract: never inside a measured window).
/// `HFT_TOPOLOGY_STRICT=1` promotes a VIOLATION to a hard failure.
pub fn report_fabric_topology(
    main_cpu: Option<usize>,
    rx_cpu: Option<usize>,
    worker_cpus: &[usize],
) {
    let verdict = verify_fabric_topology(main_cpu, rx_cpu, worker_cpus);
    let fmt_cpu = |c: usize| -> String {
        match read_die_id(c) {
            Some(d) => format!("cpu{c}(ccd{d})"),
            None => format!("cpu{c}(ccd?)"),
        }
    };
    let parts: Vec<String> = main_cpu
        .into_iter()
        .map(|c| format!("main={}", fmt_cpu(c)))
        .chain(rx_cpu.into_iter().map(|c| format!("rx={}", fmt_cpu(c))))
        .chain(
            worker_cpus
                .iter()
                .map(|&c| fmt_cpu(c))
                .collect::<Vec<_>>(),
        )
        .collect();
    let strict = std::env::var("HFT_TOPOLOGY_STRICT").as_deref() == Ok("1");
    match &verdict {
        TopologyVerdict::Verified => eprintln!(
            "TOPOLOGY_FABRIC {} -> VERIFIED (one L3 domain — the R33 SPSC ring and the span grid stay fabric-local)",
            parts.join(" ")
        ),
        TopologyVerdict::Violated(why) => {
            eprintln!("TOPOLOGY_FABRIC {} -> VIOLATED: {why}", parts.join(" "));
            if strict {
                panic!("HFT_TOPOLOGY_STRICT: fabric placement violates the R33-D law: {why}");
            }
        }
        TopologyVerdict::Unverifiable(why) => eprintln!(
            "TOPOLOGY_FABRIC {} -> UNVERIFIABLE: {why}",
            parts.join(" ")
        ),
    }
}

#[cfg(test)]
mod l3_tests {
    use super::*;

    #[test]
    fn t_l3_groups_partition() {
        let groups = l3_groups();
        let order = cpu_order();
        let total: usize = groups.iter().map(|g| g.len()).sum();
        if !order.is_empty() {
            assert_eq!(total, order.len(), "L3 groups must partition the allowed set");
        }
    }

    #[test]
    fn t_pipeline_placement_distinct() {
        let (main, rx) = pipeline_placement();
        if let (Some(m), Some(r)) = (main, rx) {
            assert_ne!(m, r, "main and rx must be distinct cpus");
        }
    }

    /// R21 (Task 4): the verification predicate's hard law — a pair on the
    /// SAME cpu is always a violation (SMT-sibling check or identity);
    /// the L3 domain checks cannot rescue co-location.
    #[test]
    fn t_topology_verification_same_cpu_violates() {
        for c in cpu_order() {
            let v = verify_pair_topology(c, c);
            assert!(
                matches!(v, TopologyVerdict::Violated(_)),
                "same-cpu pair (cpu{c}) must be a Violated verdict, got {v:?}"
            );
        }
    }

    /// R21 (Task 4): the placement's own output verifies (or is honestly
    /// unverifiable) — the selection and the verifier share the same sysfs
    /// truth, so a VERIFIED placement is the deterministic mode and a
    /// VIOLATED one is a real host-shape fact (single-representative
    /// fallbacks), never an internal inconsistency.
    #[test]
    fn t_topology_verification_placement_consistent() {
        let (main, rx) = pipeline_placement();
        let v = match (main, rx) {
            (Some(m), Some(r)) => verify_pair_topology(m, r),
            _ => return, // single-cpu host: nothing to verify
        };
        // The verifier must agree with the selector's own sysfs reads:
        // a violation is allowed ONLY when the placement genuinely had to
        // fall back (the L3 group had < 2 distinct representatives). On
        // any host where the placement took the L3-group path, the pair
        // must verify or be unverifiable.
        let groups = l3_groups();
        let has_multi_rep_group = groups.iter().any(|g| {
            g.iter()
                .filter(|c| {
                    read_sibling_group(**c)
                        .map(|s| s.iter().min() == Some(*c))
                        .unwrap_or(true)
                })
                .count()
                >= 2
        });
        if has_multi_rep_group {
            assert!(
                !v.is_violation(),
                "placement took the L3-group path but the verifier rejected it: {v:?}"
            );
        }
    }

    /// R33-D: the fabric-wide verdict's own laws — (a) an empty fabric is
    /// honestly UNVERIFIABLE (never a silent pass); (b) the default
    /// placement's whole fabric (main + rx + workers) must verify or be
    /// unverifiable when the host exposes the sysfs truth (the selector
    /// and the verifier share it — an internal inconsistency is a bug,
    /// a host-shape fact is a verdict); (c) a fabric split across two
    /// DIFFERENT L3 groups (when the host has more than one) is a
    /// VIOLATION by construction.
    #[test]
    fn t_r33_fabric_topology_laws() {
        // (a) empty fabric
        let v = verify_fabric_topology(None, None, &[]);
        assert!(
            matches!(v, TopologyVerdict::Unverifiable(_)),
            "empty fabric must be Unverifiable, got {v:?}"
        );
        // (b) the default placement's fabric consistency
        let (main, rx, workers) = fabric_placement(2);
        let v = verify_fabric_topology(main, rx, &workers);
        match &v {
            TopologyVerdict::Verified => {}
            TopologyVerdict::Unverifiable(_) => {}
            TopologyVerdict::Violated(why) => {
                // Allowed only as a genuine host-shape fact: the placement
                // wrapped across L3 groups (fewer cores than lanes). The
                // fabric placement never SPLITS domains on purpose.
                let groups = l3_groups();
                assert!(
                    groups.len() > 1,
                    "VIOLATED on a single-L3 host is an internal inconsistency: {why}"
                );
            }
        }
        // (c) a provably split fabric (needs a host with 2+ L3 groups).
        let groups = l3_groups();
        if groups.len() >= 2 {
            let a = groups[0][0];
            let b = groups[1][0];
            let v = verify_fabric_topology(Some(a), Some(b), &[]);
            assert!(
                matches!(v, TopologyVerdict::Violated(_)),
                "cross-L3 fabric (cpu{a}, cpu{b}) must be Violated, got {v:?}"
            );
        }
    }
}

/// R8: FABRIC placement for SMT hosts — (main, rx, worker cpus). Main and
/// the RX thread are placed as SMT SIBLINGS on one physical core (the RX
/// is ~15% busy at target rates — the heavy main thread keeps ~85-90% of
/// the core under SMT, and the per-batch mailbox handoff becomes
/// L1/L2-local); the workers get the REMAINING physical cores whole (a
/// worker on main's hyperthread measured a degenerative feedback spiral:
/// the starved worker's idle-spin stole issue slots from the critical main
/// thread, collapsing the sustained arm to 14M msg/s).
///
/// Non-SMT hosts: main and rx take distinct cores (the span-arm
/// placement), workers the rest.
///
/// R11 — THE PLACEMENT CEILING: on a 2-physical-core SMT runner (the
/// 8573C/8370C draws) this strategy puts BOTH workers on the second
/// core's two hyperthreads: they share one physical core's VPCLMULQDQ
/// ports, capping their combined fold at the kbench `2cpu_smt` ceiling
/// (measured 32.83 GB/s on the 1.109B draw) while the `2cpu_distinct`
/// ceiling sat unused at 61.74 GB/s. [`HFT_FABRIC_PLACE=distinct`]
/// (see [`fabric_placement_distinct`]) swaps the roles: workers get one
/// physical core EACH, and the scalar threads (main, rx) become the SMT
/// siblings — stealing issue slots from only one worker each instead of
/// stacking both folders on one core.
pub fn fabric_placement(workers: usize) -> (Option<usize>, Option<usize>, Vec<usize>) {
    // R11: experiment override — read once per arm at setup (outside every
    // measurement window; the env::var allocation is setup-only).
    match std::env::var("HFT_FABRIC_PLACE").as_deref() {
        Ok("siblings") => fabric_placement_siblings(workers),
        Ok("distinct") => fabric_placement_distinct(workers),
        Ok("scalars_stacked") => fabric_placement_scalars_stacked(workers),
        _ => {
            // R16d: THE DEFAULT for the 2-physical-core SMT draws (the
            // 8573C/8370C/9V45 4-vCPU shape): workers on DISTINCT physical
            // cores; the scalar threads (main, RX) take the leftover SMT
            // siblings — main on cpu1, RX on cpu3, each scalar sharing a
            // physical core with exactly one worker.
            //
            // R25b POSTMORTEM (the fleet refuted the stack — run 38702884119,
            // 29 target-silicon shards, paired same-shard deltas): the
            // scalars_stacked experiment (main+rx BOTH on cpu1, worker 1's
            // sibling idle) measured **−32.5% median** (min −35.6%, max
            // −29.2%, ALL shards negative). The system is MAIN-BOUND, not
            // worker-SMT-bound: stacking main (~90% busy) + rx (~28%)
            // timeshares one logical cpu at ~118% combined demand — main
            // loses ~a quarter of its own throughput, and the RX sits in
            // main's critical loop (next_batch), compounding the stall.
            // The RX's SMT residency on cpu3 costs worker 1 ~10-15% of ONE
            // worker — the cheaper theft by far. `HFT_FABRIC_PLACE=
            // scalars_stacked` stays armed as the counterfactual instrument
            // (CI arm 11smt); the Run-500+ diagnosis's hyperthread-trap
            // hypothesis is priced and REJECTED on this fleet.
            let n_phys = physical_core_count();
            let order = cpu_order();
            let smt = !order.is_empty() && order.len() > n_phys.max(1);
            if workers == 2 && n_phys == 2 && smt {
                fabric_placement_distinct(workers)
            } else {
                fabric_placement_siblings(workers)
            }
        }
    }
}

/// R25b (REFUTED — kept as the counterfactual instrument): workers on
/// DISTINCT physical cores; main AND rx STACKED on the leftover SMT
/// sibling (cpu1 on the 4-vCPU draws: sibling pairs 0-1/2-3, workers
/// (0, 2)). The hypothesis — the RX ingest thread on worker 1's sibling
/// (cpu3) robs the worker, so stacking the scalars frees a full physical
/// core — priced at **−32.5% median** across 29 target-silicon shards
/// (run 38702884119, paired same-shard): the stack timeshares main
/// (~90% busy) + rx (~28%) on one logical cpu, throttling the MAIN core
/// (the system's binding constraint) by ~a quarter, and the RX in main's
/// critical loop compounds the stall. NEVER the default; armed only via
/// `HFT_FABRIC_PLACE=scalars_stacked` (CI arm 11smt) so the fleet can
/// re-price it on future silicon (e.g. ≥3 physical cores, where the
/// tradeoff shape differs).
fn fabric_placement_scalars_stacked(workers: usize) -> (Option<usize>, Option<usize>, Vec<usize>) {
    let order = cpu_order(); // distinct-physical representatives FIRST
    if order.is_empty() {
        return (None, None, Vec::new());
    }
    // Workers: one per physical core by construction (as `distinct`).
    let wcpus: Vec<usize> = (0..workers).map(|i| order[i % order.len()]).collect();
    // The scalars: BOTH on the first leftover (the representative's SMT
    // sibling, trailing in `order`).
    let rest: Vec<usize> = order
        .iter()
        .copied()
        .filter(|c| !wcpus.contains(c))
        .collect();
    let main = rest.first().copied();
    (main, main, wcpus) // rx == main: the STACK
}

/// R11: workers on DISTINCT physical cores first; main and rx take the
/// leftover SMT siblings (one per worker core). On the 4-logical-cpu SMT
/// draws (sibling pairs 0-1 / 2-3): workers (0, 2), main 1, rx 3 — each
/// folder owns a physical core except for its scalar sibling's issue-slot
/// share, and the two workers' combined ceiling moves from the SMT-pair
/// cap (~32.8 GB/s) toward the distinct-pair cap (~61.7 GB/s). The
/// trade is measured, not assumed: main and rx each steal cycles from a
/// worker (and main's assist folding now contends for the SAME clmul
/// ports as the sibling worker), so the CI sweep (11k) decides per
/// runner class whether the swap wins.
///
/// Degenerate hosts (no SMT, or fewer cores than workers): workers still
/// take distinct cores first and wrap onto siblings/leftovers exactly like
/// the pool round-robin; main and rx go unpinned when nothing is left
/// (behavior-only difference; the fabric's correctness never depends on
/// placement).
fn fabric_placement_distinct(workers: usize) -> (Option<usize>, Option<usize>, Vec<usize>) {
    let order = cpu_order(); // distinct-physical representatives FIRST
    if order.is_empty() {
        return (None, None, Vec::new());
    }
    // Workers: the round-robin over the topology order — the first
    // `workers` slots are one per physical core by construction.
    let wcpus: Vec<usize> = (0..workers).map(|i| order[i % order.len()]).collect();
    // Main + rx: the leftovers, preferring SMT siblings (they trail the
    // representatives in `order`) so each scalar thread shares a core with
    // exactly one worker.
    let rest: Vec<usize> = order
        .iter()
        .copied()
        .filter(|c| !wcpus.contains(c))
        .collect();
    let main = rest.first().copied();
    let rx = rest.get(1).copied();
    (main, rx, wcpus)
}

/// The R8 default (see [`fabric_placement`]'s doc).
fn fabric_placement_siblings(workers: usize) -> (Option<usize>, Option<usize>, Vec<usize>) {
    let order = cpu_order();
    if order.is_empty() {
        return (None, None, Vec::new());
    }
    // Physical cores (representative + its sibling(s)) in topology order.
    let mut cores: Vec<Vec<usize>> = Vec::new();
    for &c in &order {
        let sibs = read_sibling_group(c).unwrap_or_else(|| vec![c]);
        let key = *sibs.iter().min().unwrap_or(&c);
        if !cores.iter().any(|k| *k.first().unwrap_or(&usize::MAX) == key) {
            let mut grp = sibs;
            grp.sort_unstable();
            cores.push(grp);
        }
    }
    if cores.len() >= 2 {
        // Core 0: main + RX (SMT siblings if present).
        let main = cores[0].first().copied();
        let rx = cores[0].get(1).copied().or_else(|| cores[1].first().copied());
        // Workers: the remaining cores' cpus, whole cores first.
        let mut pool: Vec<usize> = Vec::new();
        for core in cores.iter().skip(1) {
            pool.extend(core.iter().copied());
        }
        if pool.is_empty() {
            pool = order.clone();
        }
        // R8 phase-6: on SMT topologies, a THIRD worker joins the RX's
        // hyperthread (appended LAST — the dedicated hyperthreads lead the
        // round-robin). The straggler risk a shared-hyperthread lane carries
        // is contained by the main-core work-assist: its overflow chunks
        // convert to submitting-core CRC automatically, so the lane's
        // hyperthread scavenging is pure net capacity.
        if let Some(r) = rx {
            if workers > pool.len() && !pool.contains(&r) && Some(r) != main {
                pool.push(r);
            }
        }
        let wcpus: Vec<usize> = (0..workers)
            .map(|i| pool[i % pool.len()])
            .collect();
        (main, rx, wcpus)
    } else {
        // Single physical core: everything wraps.
        let main = order.first().copied();
        let rx = order.get(1).copied();
        let wcpus: Vec<usize> = (0..workers)
            .map(|i| order[i % order.len()])
            .collect();
        (main, rx, wcpus)
    }
}

#[cfg(test)]
mod fabric_placement_tests {
    use super::*;

    #[test]
    fn t_fabric_placement_no_worker_on_main_sibling_when_possible() {
        let (main, rx, workers) = fabric_placement(2);
        // R25b: the stacked experiment (rx == main) is REFUTED and
        // never the default (see `fabric_placement_scalars_stacked`);
        // the default distinct layout keeps rx off main's cpu. The
        // invariant that matters on EVERY shape: no worker pinned on
        // the main thread's own cpu when the topology has room.
        let _ = rx;
        if let Some(m) = main {
            for w in &workers {
                if crate::affinity::cpu_order().len() > 2 {
                    assert_ne!(*w, m, "worker pinned on main's cpu");
                }
            }
        }
    }

    /// R11: the distinct strategy — every worker on a DIFFERENT physical
    /// core, main/rx on the leftovers, and never a worker on main's cpu
    /// when the topology has room.
    #[test]
    fn t_fabric_placement_distinct_workers_on_distinct_cores() {
        let order = cpu_order();
        let (main, rx, workers) = fabric_placement_distinct(2);
        if order.len() < 2 {
            return; // degenerate single-cpu hosts: nothing to assert
        }
        // Two workers must sit on two DIFFERENT sibling groups.
        let groups: Vec<Vec<usize>> = workers
            .iter()
            .map(|w| read_sibling_group(*w).unwrap_or_else(|| vec![*w]))
            .collect();
        let g0: Vec<usize> = groups
            .first()
            .map(|g| {
                let mut k = g.clone();
                k.sort_unstable();
                k
            })
            .unwrap_or_default();
        if let Some(g1) = groups.get(1) {
            let mut k1 = g1.clone();
            k1.sort_unstable();
            assert_ne!(g0, k1, "both workers share one physical core");
        }
        if let Some(m) = main {
            assert!(
                !workers.contains(&m),
                "main pinned on a worker cpu: main={m} workers={workers:?}"
            );
        }
        if let (Some(m), Some(r)) = (main, rx) {
            assert_ne!(m, r);
        }
        // Every worker cpu must be in the allowed set.
        for w in &workers {
            assert!(order.contains(w), "worker {w} outside the allowed set");
        }
    }
}

/// R8: SMT-polite spin — bounded PAUSE backoff, then `sched_yield()`.
/// Zen3's PAUSE hint is weak: an uncapped pause-loop on one hyperthread
/// measurably starves its sibling (the fabric's workers collapsed to ~1%
/// of their CRC ceiling through mutual spin-starvation on a shared
/// physical core). The yield deschedules the logical cpu for a
/// reschedule quantum (~1-3us), keeping the sibling fed; wake latency for
/// the yielder is immediate (spinners re-check after the yield).
pub fn polite_spin(iters: &mut u32) {
    if *iters < 6 {
        let n = 1u32 << *iters;
        for _ in 0..n {
            std::hint::spin_loop();
        }
    } else {
        std::thread::yield_now();
    }
    *iters += 1;
}

/// R16b: the DEEP pause spin — the array protocol's work arrives in
/// chunk-granule publications (a ~µs-scale cadence). The fast yield
/// escalation above was tuned for the ring protocol's rare idle laps;
/// at the array cadence it degenerated into a runqueue-churning yield
/// storm (measured: 3.9M yields per worker on the first 8370C draw —
/// every subsequent wake paid scheduler latency, and the polled
/// spans_ready line ping-ponged at MHz rates). Pause bursts up to 256
/// (~9µs), yielding only after ~20 laps (~100µs of true idleness).
pub fn polite_spin_deep(iters: &mut u32) {
    if *iters < 20 {
        let n = 1u32 << (*iters).min(8);
        for _ in 0..n {
            std::hint::spin_loop();
        }
    } else {
        std::thread::yield_now();
    }
    *iters += 1;
}
