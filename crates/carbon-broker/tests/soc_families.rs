//! Topology fixtures for the SoC families CarbonHV targets. Core counts, core types (MIDR part
//! numbers), and cpufreq policy layout follow each SoC's public configuration. Capacity values
//! are illustrative placeholders until a physical probe records the real `cpu_capacity` table;
//! the assertions only depend on their ordering.

use std::fs;
use std::path::Path;

use carbon_broker::{CoreKind, CpuCore, HostResources};

const A55: u64 = 0x410f_d050;
const A510: u64 = 0x410f_d460;
const A520: u64 = 0x410f_d800;
const A715: u64 = 0x410f_d4d0;
const A720: u64 = 0x410f_d810;
const A725: u64 = 0x410f_d870;
const A78: u64 = 0x410f_d410;
const X1: u64 = 0x410f_d440;
const X3: u64 = 0x410f_d4e0;
const X4: u64 = 0x410f_d820;
const X925: u64 = 0x410f_d850;
const KRYO_SILVER_4XX: u64 = 0x517f_8050;
const ORYON: u64 = 0x511f_0010;

/// `(count, midr, capacity)` clusters in CPU-index order; each cluster is one cpufreq policy.
fn soc(clusters: &[(usize, u64, u64)]) -> HostResources {
    let mut cores = Vec::new();
    for &(count, midr, capacity) in clusters {
        let leader = cores.len();
        for _ in 0..count {
            cores.push(CpuCore::new(
                cores.len(),
                capacity,
                leader,
                CoreKind::from_midr(midr),
            ));
        }
    }
    HostResources {
        total_memory_mib: 12 * 1024,
        cores,
    }
}

fn pinned_cpus(host: &HostResources, vcpus: u16) -> Vec<usize> {
    host.vcpu_placement(vcpus)
        .unwrap()
        .pins
        .iter()
        .map(|pin| pin.host_cpu)
        .collect()
}

#[test]
fn midr_classifies_in_order_efficiency_cores() {
    for midr in [A55, A510, A520, KRYO_SILVER_4XX, 0x410f_d030] {
        assert_eq!(CoreKind::from_midr(midr), CoreKind::Efficiency, "{midr:#x}");
    }
    for midr in [A78, X1, A715, X3, A720, X4, A725, X925, ORYON] {
        assert_eq!(
            CoreKind::from_midr(midr),
            CoreKind::Performance,
            "{midr:#x}"
        );
    }
    assert_eq!(CoreKind::from_midr(0x4e0f_0000), CoreKind::Unknown);
    assert_eq!(CoreKind::parse_midr("0x00000000410fd050\n"), Some(A55));
    assert_eq!(CoreKind::parse_midr("410fd050"), None);
}

#[test]
fn tensor_g2_keeps_its_a78_mid_cores() {
    // Pixel 7/7a: 4x A55 | 2x A78 | 2x X1. A fixed 70% cut would leave only the two X1 cores.
    let host = soc(&[(4, A55, 230), (2, A78, 600), (2, X1, 1024)]);
    assert_eq!(host.performance_cpus(), vec![4, 5, 6, 7]);
    let placement = host.vcpu_placement(4).unwrap();
    let capacities: Vec<u32> = placement.pins.iter().map(|pin| pin.capacity).collect();
    assert_eq!(capacities, vec![600, 600, 1024, 1024]);
    assert_eq!(placement.freq_domains, vec![vec![0, 1], vec![2, 3]]);
}

#[test]
fn tensor_g2_without_midr_falls_back_to_the_capacity_tier_rule() {
    let mut host = soc(&[(4, A55, 230), (2, A78, 600), (2, X1, 1024)]);
    host.cores
        .iter_mut()
        .for_each(|core| core.kind = CoreKind::Unknown);
    assert_eq!(host.performance_cpus(), vec![4, 5, 6, 7]);
}

#[test]
fn tensor_g3_prefers_the_prime_core_then_big_cores() {
    // Pixel 8: 4x A510 | 4x A715 | 1x X3 (nine cores).
    let host = soc(&[(4, A510, 250), (4, A715, 650), (1, X3, 1024)]);
    assert_eq!(host.max_interactive_vcpus(), 5);
    assert_eq!(pinned_cpus(&host, 4), vec![4, 5, 6, 8]);
}

#[test]
fn tensor_g4_and_g5_exclude_only_a520() {
    // Pixel 9: 4x A520 | 3x A720 | 1x X4.  Pixel 10: 2x A520 | 5x A725 | 1x X4.
    let g4 = soc(&[(4, A520, 260), (3, A720, 760), (1, X4, 1024)]);
    assert_eq!(g4.performance_cpus(), vec![4, 5, 6, 7]);
    let g5 = soc(&[(2, A520, 260), (5, A725, 800), (1, X4, 1024)]);
    assert_eq!(g5.max_interactive_vcpus(), 6);
    assert_eq!(pinned_cpus(&g5, 4), vec![2, 3, 4, 7]);
}

#[test]
fn exynos_2400_mirrors_three_performance_frequency_domains() {
    // Galaxy S24 (Exynos): 4x A520 | 3x A720 @2.6 | 2x A720 @2.9 | 1x X4 (ten cores).
    let host = soc(&[
        (4, A520, 260),
        (3, A720, 700),
        (2, A720, 780),
        (1, X4, 1024),
    ]);
    assert_eq!(host.max_interactive_vcpus(), 6);
    let placement = host.vcpu_placement(4).unwrap();
    let pinned: Vec<usize> = placement.pins.iter().map(|pin| pin.host_cpu).collect();
    assert_eq!(pinned, vec![4, 7, 8, 9]);
    assert_eq!(placement.freq_domains, vec![vec![0], vec![1, 2], vec![3]]);
}

#[test]
fn exynos_2500_offers_eight_non_efficiency_cores() {
    // Galaxy Z Flip7: 2x A520 | 5x A725 @2.36 | 2x A725 @2.74 | 1x X925.
    let host = soc(&[
        (2, A520, 250),
        (5, A725, 690),
        (2, A725, 800),
        (1, X925, 1024),
    ]);
    assert_eq!(host.max_interactive_vcpus(), 8);
}

#[test]
fn dimensity_all_big_core_designs_pin_every_core() {
    // Dimensity 9300: 4x A720 | 4x X4.  Dimensity 9400: 4x A720 | 3x X4 | 1x X925.
    let d9300 = soc(&[(4, A720, 560), (4, X4, 1024)]);
    assert_eq!(d9300.max_interactive_vcpus(), 8);
    assert_eq!(pinned_cpus(&d9300, 4), vec![4, 5, 6, 7]);
    let d9400 = soc(&[(4, A720, 560), (3, X4, 900), (1, X925, 1024)]);
    assert_eq!(d9400.max_interactive_vcpus(), 8);
    assert_eq!(pinned_cpus(&d9400, 2), vec![4, 7]);
}

#[test]
fn snapdragon_oryon_and_silver_cores_are_classified_by_implementer() {
    // Snapdragon 8 Elite: 6x Oryon M + 2x Oryon L (no efficiency cores).
    let elite = soc(&[(6, ORYON, 870), (2, ORYON, 1024)]);
    assert_eq!(elite.max_interactive_vcpus(), 8);
    // Older Kryo designs: Silver cores are excluded even if their capacity tier is high.
    let kryo = soc(&[(4, KRYO_SILVER_4XX, 600), (4, A78, 1024)]);
    assert_eq!(kryo.performance_cpus(), vec![4, 5, 6, 7]);
}

#[test]
fn an_all_efficiency_host_still_offers_its_cores() {
    let host = soc(&[(8, A55, 1024)]);
    assert_eq!(host.max_interactive_vcpus(), 8);
}

#[test]
fn reads_midr_from_sysfs() {
    let root = std::env::temp_dir().join(format!("carbonhv-midr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let cpu = root.join("cpu");
    for (index, midr, capacity) in [(0, A55, 405), (1, A78, 1024)] {
        let dir = cpu.join(format!("cpu{index}"));
        fs::create_dir_all(dir.join("regs/identification")).unwrap();
        fs::write(dir.join("cpu_capacity"), format!("{capacity}\n")).unwrap();
        fs::write(
            dir.join("regs/identification/midr_el1"),
            format!("0x{midr:016x}\n"),
        )
        .unwrap();
    }
    fs::write(root.join("meminfo"), "MemTotal: 8388608 kB\n").unwrap();
    let host = HostResources::read(&root.join("meminfo"), Path::new(&cpu)).unwrap();
    let kinds: Vec<CoreKind> = host.cores.iter().map(|core| core.kind).collect();
    assert_eq!(kinds, vec![CoreKind::Efficiency, CoreKind::Performance]);
    assert_eq!(host.performance_cpus(), vec![1]);
}
