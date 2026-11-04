use std::fs;
use std::path::{Path, PathBuf};

use carbon_broker::{BrokerPolicy, CpuCore, HostResources, PlacementError, VcpuPin, VcpuPlacement};

fn policy() -> BrokerPolicy {
    BrokerPolicy {
        allowed_uid: 1000,
        allowed_signing_digest: "digest".into(),
        managed_root: PathBuf::from("/data/misc/carbonhv/objects"),
        max_memory_mib: 16 * 1024,
        max_vcpus: 8,
    }
}

/// Measured on the Snapdragon 6 Gen 3 (SM6475) development phone.
fn sm6475() -> HostResources {
    let cores = (0..8)
        .map(|index| CpuCore {
            index,
            capacity: if index < 4 { 405 } else { 1024 },
            freq_domain: if index < 4 { 0 } else { 4 },
        })
        .collect();
    HostResources {
        total_memory_mib: 7391,
        cores,
    }
}

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("carbonhv-res-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_cpu(root: &Path, index: usize, file: &str, value: &str) {
    let dir = root
        .join(format!("cpu{index}"))
        .join(Path::new(file).parent().unwrap());
    fs::create_dir_all(&dir).unwrap();
    fs::write(root.join(format!("cpu{index}")).join(file), value).unwrap();
}

#[test]
fn parses_total_memory_from_meminfo() {
    let meminfo = "MemTotal:        7568976 kB\nMemFree:          100 kB\n";
    assert_eq!(HostResources::parse_total_memory_mib(meminfo), Some(7391));
    assert_eq!(HostResources::parse_total_memory_mib("MemFree: 1 kB"), None);
    assert_eq!(
        HostResources::parse_total_memory_mib("MemTotal: lots"),
        None
    );
}

#[test]
fn big_little_split_selects_only_performance_cores() {
    assert_eq!(sm6475().performance_cpus(), vec![4, 5, 6, 7]);
    assert_eq!(sm6475().max_interactive_vcpus(), 4);
}

#[test]
fn homogeneous_cores_are_all_performance_cores() {
    let host = HostResources {
        total_memory_mib: 16384,
        cores: (0..6)
            .map(|index| CpuCore {
                index,
                capacity: 1024,
                freq_domain: 0,
            })
            .collect(),
    };
    assert_eq!(host.max_interactive_vcpus(), 6);
}

#[test]
fn small_phone_leaves_a_fixed_reserve_to_android() {
    // 7391 MiB phone: 3072 MiB floor beats 40% (2956), so 4319 MiB is grantable.
    assert_eq!(sm6475().max_guest_memory_mib(), 4319);
}

#[test]
fn large_phone_reserve_scales_with_physical_memory() {
    let host = HostResources {
        total_memory_mib: 12288,
        cores: sm6475().cores,
    };
    assert_eq!(host.max_guest_memory_mib(), 12288 - 12288 * 40 / 100);
}

#[test]
fn tiny_host_grants_nothing_instead_of_underflowing() {
    let host = HostResources {
        total_memory_mib: 2048,
        cores: sm6475().cores,
    };
    assert_eq!(host.max_guest_memory_mib(), 0);
}

#[test]
fn policy_is_only_ever_tightened() {
    let tightened = policy().constrained_to(&sm6475());
    assert_eq!(tightened.max_memory_mib, 4319);
    assert_eq!(tightened.max_vcpus, 4);

    let mut already_tight = policy();
    already_tight.max_memory_mib = 2048;
    already_tight.max_vcpus = 2;
    let kept = already_tight.constrained_to(&sm6475());
    assert_eq!(kept.max_memory_mib, 2048);
    assert_eq!(kept.max_vcpus, 2);
}

#[test]
fn reads_capacity_and_cpufreq_domains() {
    let root = temp_root("read");
    let cpu = root.join("cpu");
    let meminfo = root.join("meminfo");
    fs::create_dir_all(&cpu).unwrap();
    fs::write(&meminfo, "MemTotal:        7568976 kB\n").unwrap();
    write_cpu(&cpu, 0, "cpu_capacity", "405\n");
    write_cpu(&cpu, 0, "cpufreq/related_cpus", "0 1\n");
    write_cpu(&cpu, 1, "cpu_capacity", "405\n");
    write_cpu(&cpu, 1, "cpufreq/related_cpus", "0 1\n");
    write_cpu(&cpu, 2, "cpu_capacity", "1024\n");
    write_cpu(&cpu, 2, "cpufreq/related_cpus", "2-3\n");
    write_cpu(&cpu, 3, "cpu_capacity", "1024\n");
    write_cpu(&cpu, 3, "cpufreq/related_cpus", "2-3\n");
    write_cpu(&cpu, 3, "online", "1\n");
    fs::create_dir_all(cpu.join("cpufreq")).unwrap(); // non-core entry is ignored

    let host = HostResources::read(&meminfo, &cpu).unwrap();
    assert_eq!(host.total_memory_mib, 7391);
    assert_eq!(
        host.cores,
        vec![
            CpuCore {
                index: 0,
                capacity: 405,
                freq_domain: 0
            },
            CpuCore {
                index: 1,
                capacity: 405,
                freq_domain: 0
            },
            CpuCore {
                index: 2,
                capacity: 1024,
                freq_domain: 2
            },
            CpuCore {
                index: 3,
                capacity: 1024,
                freq_domain: 2
            },
        ]
    );
}

#[test]
fn capacity_units_are_never_mixed_across_cores() {
    // cpu1 lacks cpu_capacity: every core must then use max frequency, otherwise a 1.8 GHz
    // little core (1804800 kHz) would look ~1700x faster than a 1024-capacity big core.
    let root = temp_root("mixed");
    let cpu = root.join("cpu");
    let meminfo = root.join("meminfo");
    fs::create_dir_all(&cpu).unwrap();
    fs::write(&meminfo, "MemTotal:        7568976 kB\n").unwrap();
    write_cpu(&cpu, 0, "cpu_capacity", "405\n");
    write_cpu(&cpu, 0, "cpufreq/cpuinfo_max_freq", "1804800\n");
    write_cpu(&cpu, 1, "cpufreq/cpuinfo_max_freq", "1804800\n");
    write_cpu(&cpu, 2, "cpu_capacity", "1024\n");
    write_cpu(&cpu, 2, "cpufreq/cpuinfo_max_freq", "2400000\n");

    let host = HostResources::read(&meminfo, &cpu).unwrap();
    let capacities: Vec<u64> = host.cores.iter().map(|core| core.capacity).collect();
    assert_eq!(capacities, vec![1_804_800, 1_804_800, 2_400_000]);
    assert_eq!(host.performance_cpus(), vec![0, 1, 2]);
    // Without cpufreq topology, equal-capacity cores share a domain.
    let domains: Vec<usize> = host.cores.iter().map(|core| core.freq_domain).collect();
    assert_eq!(domains, vec![0, 0, 2]);
}

#[test]
fn a_core_without_any_comparable_capacity_is_an_error() {
    let root = temp_root("nocap");
    let cpu = root.join("cpu");
    fs::create_dir_all(&cpu).unwrap();
    fs::write(root.join("meminfo"), "MemTotal: 8388608 kB\n").unwrap();
    write_cpu(&cpu, 0, "cpufreq/cpuinfo_max_freq", "1804800\n");
    write_cpu(&cpu, 1, "topology/core_id", "1\n");
    assert!(HostResources::read(&root.join("meminfo"), &cpu).is_err());
}

#[test]
fn offline_cores_are_not_offered_to_guests() {
    let root = temp_root("offline");
    let cpu = root.join("cpu");
    fs::create_dir_all(&cpu).unwrap();
    fs::write(root.join("meminfo"), "MemTotal: 8388608 kB\n").unwrap();
    write_cpu(&cpu, 0, "cpu_capacity", "1024\n");
    write_cpu(&cpu, 1, "cpu_capacity", "1024\n");
    write_cpu(&cpu, 1, "online", "0\n");
    let host = HostResources::read(&root.join("meminfo"), &cpu).unwrap();
    assert_eq!(host.performance_cpus(), vec![0]);
}

#[test]
fn parses_related_cpu_lists() {
    assert_eq!(
        HostResources::parse_freq_domain_leader("4 5 6 7\n"),
        Some(4)
    );
    assert_eq!(HostResources::parse_freq_domain_leader("2-4"), Some(2));
    assert_eq!(HostResources::parse_freq_domain_leader("7"), Some(7));
    assert_eq!(HostResources::parse_freq_domain_leader(""), None);
}

#[test]
fn sm6475_pins_one_vcpu_per_big_core_in_one_domain() {
    let placement = sm6475().vcpu_placement(4).unwrap();
    assert_eq!(
        placement,
        VcpuPlacement {
            pins: (0..4)
                .map(|vcpu| VcpuPin {
                    vcpu,
                    host_cpu: vcpu + 4,
                    capacity: 1024
                })
                .collect(),
            freq_domains: vec![vec![0, 1, 2, 3]],
        }
    );
}

/// Snapdragon 8 Gen 3-shaped topology (illustrative capacities, real policy layout):
/// A520 x2 | A720 x3 | A720 x2 (higher clock) | X4 prime.
fn sm8650_shaped() -> HostResources {
    let layout = [
        (325, 0),
        (325, 0),
        (870, 2),
        (870, 2),
        (870, 2),
        (900, 5),
        (900, 5),
        (1024, 7),
    ];
    HostResources {
        total_memory_mib: 11_700,
        cores: layout
            .iter()
            .enumerate()
            .map(|(index, (capacity, freq_domain))| CpuCore {
                index,
                capacity: *capacity,
                freq_domain: *freq_domain,
            })
            .collect(),
    }
}

#[test]
fn placement_prefers_prime_cores_and_mirrors_host_domains() {
    let host = sm8650_shaped();
    assert_eq!(host.max_interactive_vcpus(), 6);
    let placement = host.vcpu_placement(4).unwrap();
    let pinned: Vec<(usize, u32)> = placement
        .pins
        .iter()
        .map(|pin| (pin.host_cpu, pin.capacity))
        .collect();
    assert_eq!(pinned, vec![(2, 870), (5, 900), (6, 900), (7, 1024)]);
    assert_eq!(placement.freq_domains, vec![vec![0], vec![1, 2], vec![3]]);
}

#[test]
fn placement_never_spills_onto_efficiency_cores() {
    assert_eq!(
        sm6475().vcpu_placement(5),
        Err(PlacementError::ExceedsPerformanceCores {
            requested: 5,
            available: 4
        })
    );
    assert_eq!(sm6475().vcpu_placement(0), Err(PlacementError::NoVcpus));
}

#[test]
fn frequency_capacities_are_renormalized_to_the_scheduler_scale() {
    let host = HostResources {
        total_memory_mib: 8192,
        cores: vec![
            CpuCore {
                index: 0,
                capacity: 2_400_000,
                freq_domain: 0,
            },
            CpuCore {
                index: 1,
                capacity: 3_000_000,
                freq_domain: 1,
            },
        ],
    };
    let placement = host.vcpu_placement(2).unwrap();
    let capacities: Vec<u32> = placement.pins.iter().map(|pin| pin.capacity).collect();
    assert_eq!(capacities, vec![819, 1024]);
}

#[test]
fn unreadable_sources_are_errors_not_guesses() {
    let root = temp_root("missing");
    fs::write(root.join("meminfo"), "MemTotal: 1024 kB\n").unwrap();
    fs::create_dir_all(root.join("cpu")).unwrap();
    assert!(HostResources::read(&root.join("meminfo"), &root.join("cpu")).is_err());
    assert!(HostResources::read(&root.join("nope"), &root.join("cpu")).is_err());
}
