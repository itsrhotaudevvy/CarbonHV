use carbon_broker::{
    CoreKind, CpuControl, CpuCore, CpuFrequencyControl, CpuPlan, GuestPerformancePlan,
    HostResources, PerformanceProfile, PlacementError,
};

fn sm6475() -> HostResources {
    HostResources {
        total_memory_mib: 7391,
        cores: (0..8)
            .map(|index| CpuCore {
                index,
                capacity: if index < 4 { 405 } else { 1024 },
                freq_domain: if index < 4 { 0 } else { 4 },
                kind: CoreKind::Unknown,
            })
            .collect(),
    }
}

#[test]
fn near_native_direct_backend_pins_and_hands_dvfs_to_the_guest() {
    let plan = GuestPerformancePlan::build(
        PerformanceProfile::NearNative,
        Some(&sm6475()),
        4,
        CpuControl::PerVcpuAffinity,
    )
    .unwrap();
    assert!(matches!(plan.cpu, CpuPlan::Pinned(_)));
    assert_eq!(plan.cpu.vcpus(), 4);
    assert_eq!(plan.frequency, CpuFrequencyControl::GuestDvfs);
    assert_eq!(
        plan.crosvm_args(),
        vec![
            "--cpus",
            "num-cores=4,clusters=[[0,1,2,3]],freq-domains=[[0,1,2,3]],sve=[auto=true]",
            "--cpu-affinity",
            "0=4:1=5:2=6:3=7",
            "--cpu-capacity",
            "0=1024,1=1024,2=1024,3=1024",
            "--virt-cpufreq-upstream",
            "--hugepages",
            "--balloon-page-reporting",
        ]
    );
}

#[test]
fn guest_dvfs_and_uclamp_boost_are_never_combined() {
    // crosvm --boost-uclamp pins uclamp.min at full capacity, which would override every
    // guest frequency request; the plan must choose exactly one control.
    for control in [CpuControl::PerVcpuAffinity, CpuControl::CountOnly] {
        for host in [Some(sm6475()), None] {
            for profile in [
                PerformanceProfile::NearNative,
                PerformanceProfile::Efficient,
            ] {
                let args = GuestPerformancePlan::build(profile, host.as_ref(), 2, control)
                    .unwrap()
                    .crosvm_args();
                let dvfs = args.iter().any(|arg| arg == "--virt-cpufreq-upstream");
                let boost = args.iter().any(|arg| arg == "--boost-uclamp");
                assert!(!(dvfs && boost), "{profile:?} {control:?}: {args:?}");
            }
        }
    }
}

#[test]
fn count_only_backends_boost_instead_of_pinning() {
    let plan = GuestPerformancePlan::build(
        PerformanceProfile::NearNative,
        Some(&sm6475()),
        4,
        CpuControl::CountOnly,
    )
    .unwrap();
    assert_eq!(plan.cpu, CpuPlan::Floating { vcpus: 4 });
    assert_eq!(plan.frequency, CpuFrequencyControl::BoostUclamp);
}

#[test]
fn unknown_topology_is_not_guessed_into_a_pin_map() {
    let plan = GuestPerformancePlan::build(
        PerformanceProfile::NearNative,
        None,
        4,
        CpuControl::PerVcpuAffinity,
    )
    .unwrap();
    assert_eq!(plan.cpu, CpuPlan::Floating { vcpus: 4 });
    assert!(!plan.crosvm_args().iter().any(|arg| arg == "--cpu-affinity"));
}

#[test]
fn efficient_profile_leaves_scheduling_to_the_host() {
    let plan = GuestPerformancePlan::build(
        PerformanceProfile::Efficient,
        Some(&sm6475()),
        2,
        CpuControl::PerVcpuAffinity,
    )
    .unwrap();
    assert_eq!(plan.frequency, CpuFrequencyControl::HostDefault);
    assert_eq!(
        plan.crosvm_args(),
        vec![
            "--cpus",
            "num-cores=2,sve=[auto=true]",
            "--hugepages",
            "--balloon-page-reporting"
        ]
    );
}

#[test]
fn oversubscribing_performance_cores_is_refused() {
    assert_eq!(
        GuestPerformancePlan::build(
            PerformanceProfile::NearNative,
            Some(&sm6475()),
            6,
            CpuControl::PerVcpuAffinity,
        ),
        Err(PlacementError::ExceedsPerformanceCores {
            requested: 6,
            available: 4
        })
    );
}
