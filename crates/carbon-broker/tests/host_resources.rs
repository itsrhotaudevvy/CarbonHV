use std::fs;
use std::path::{Path, PathBuf};

use carbon_broker::{BrokerPolicy, CpuCore, HostResources};

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
fn reads_cpu_capacity_and_falls_back_to_max_frequency() {
    let root = temp_root("read");
    let cpu = root.join("cpu");
    let meminfo = root.join("meminfo");
    fs::create_dir_all(&cpu).unwrap();
    fs::write(&meminfo, "MemTotal:        7568976 kB\n").unwrap();
    write_cpu(&cpu, 0, "cpu_capacity", "405\n");
    write_cpu(&cpu, 1, "cpufreq/cpuinfo_max_freq", "1804800\n");
    write_cpu(&cpu, 2, "cpu_capacity", "1024\n");
    fs::create_dir_all(cpu.join("cpufreq")).unwrap(); // non-core entry is ignored

    let host = HostResources::read(&meminfo, &cpu).unwrap();
    assert_eq!(host.total_memory_mib, 7391);
    assert_eq!(
        host.cores,
        vec![
            CpuCore {
                index: 0,
                capacity: 405
            },
            CpuCore {
                index: 1,
                capacity: 1804800
            },
            CpuCore {
                index: 2,
                capacity: 1024
            },
        ]
    );
}

#[test]
fn unreadable_sources_are_errors_not_guesses() {
    let root = temp_root("missing");
    fs::write(root.join("meminfo"), "MemTotal: 1024 kB\n").unwrap();
    fs::create_dir_all(root.join("cpu")).unwrap();
    assert!(HostResources::read(&root.join("meminfo"), &root.join("cpu")).is_err());
    assert!(HostResources::read(&root.join("nope"), &root.join("cpu")).is_err());
}
