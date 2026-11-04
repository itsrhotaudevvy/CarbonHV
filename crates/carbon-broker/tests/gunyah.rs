use std::path::PathBuf;

use carbon_broker::{
    AvfGraphicsConfig, BackendError, CpuCore, GunyahCrosvmBackend, GunyahSessionFds, HostResources,
    PerformanceProfile, PlacementError, ValidatedVmDisk, ValidatedVmSpec,
};
use carbon_core::Generation;

fn spec() -> ValidatedVmSpec {
    ValidatedVmSpec {
        request_id: 7,
        instance_id: "ut-main".into(),
        generation: Generation(5),
        kernel: PathBuf::from("/data/misc/carbonhv/objects/ut-kernel"),
        initrd: PathBuf::from("/data/misc/carbonhv/objects/ut-initrd"),
        disks: vec![
            ValidatedVmDisk {
                path: PathBuf::from("/data/misc/carbonhv/objects/ut-rootfs"),
                writable: false,
            },
            ValidatedVmDisk {
                path: PathBuf::from("/data/misc/carbonhv/objects/ut-main-userdata"),
                writable: true,
            },
        ],
        memory_mib: 4096,
        vcpus: 4,
        kernel_params: vec!["console=hvc0".into()],
    }
}

fn sm6475() -> HostResources {
    HostResources {
        total_memory_mib: 7391,
        cores: (0..8)
            .map(|index| CpuCore {
                index,
                capacity: if index < 4 { 405 } else { 1024 },
                freq_domain: if index < 4 { 0 } else { 4 },
            })
            .collect(),
    }
}

fn backend() -> GunyahCrosvmBackend {
    GunyahCrosvmBackend::new(
        PathBuf::from("/apex/com.android.virt/bin/crosvm"),
        PathBuf::from("/dev/gunyah"),
    )
    .with_host_resources(sm6475())
}

fn fds() -> GunyahSessionFds {
    GunyahSessionFds {
        control: 10,
        touch: 11,
        keyboard: 12,
        mouse: 13,
    }
}

fn graphics() -> AvfGraphicsConfig {
    AvfGraphicsConfig::gfxstream_vulkan(1080, 2392, 440, 120).unwrap()
}

#[test]
fn sm6475_near_native_plan_is_an_exact_argv() {
    let plan = backend()
        .build_launch_plan(&spec(), &graphics(), fds())
        .unwrap();
    assert_eq!(
        plan.invocation.program,
        PathBuf::from("/apex/com.android.virt/bin/crosvm")
    );
    let expected: Vec<String> = [
        "run",
        "--hypervisor",
        "gunyah[device=/dev/gunyah]",
        "--name",
        "carbonhv-ut-main-5",
        "--disable-sandbox",
        "--no-usb",
        "--no-pmu",
        "--pci",
        "mem=[start=0x2c000000,size=0x2000000],cam=[start=0x2e000000,size=0x1000000]",
        "--socket",
        "/proc/self/fd/10",
        "--mem",
        "4096",
        "--cpus",
        "num-cores=4,clusters=[[0,1,2,3]],freq-domains=[[0,1,2,3]]",
        "--cpu-affinity",
        "0=4:1=5:2=6:3=7",
        "--cpu-capacity",
        "0=1024,1=1024,2=1024,3=1024",
        "--virt-cpufreq-upstream",
        "--hugepages",
        "--balloon-page-reporting",
        "--initrd",
        "/data/misc/carbonhv/objects/ut-initrd",
        "--block",
        "path=/data/misc/carbonhv/objects/ut-rootfs,ro=true,lock=true",
        "--block",
        "path=/data/misc/carbonhv/objects/ut-main-userdata,ro=false,lock=true",
        "--params",
        "console=hvc0",
        "--params",
        "gfxstream_enabled",
        "--gpu=backend=gfxstream,context-types=gfxstream-vulkan:gfxstream-composer,renderer-features=VulkanDisableCoherentMemoryAndEmulate:enabled;VulkanAllocateHostVisibleAsUdmabuf:enabled;ExternalBlob:enabled,surfaceless=true,vulkan=true",
        "--gpu-display=mode=windowed[1080,2392],dpi=[440,440],refresh-rate=120",
        "--android-display-service=carbonhv-ut-main-5",
        "--input",
        "multi-touch[path=/proc/self/fd/11,width=1080,height=2392]",
        "--input",
        "keyboard[path=/proc/self/fd/12]",
        "--input",
        "mouse[path=/proc/self/fd/13]",
        "/data/misc/carbonhv/objects/ut-kernel",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(plan.invocation.args, expected);
    assert_eq!(plan.display_service_name, "carbonhv-ut-main-5");
}

#[test]
fn never_requests_a_protected_vm_or_swiotlb_bounce_buffers() {
    let args = backend()
        .build_launch_plan(&spec(), &graphics(), fds())
        .unwrap()
        .invocation
        .args;
    assert!(!args.iter().any(|arg| arg.starts_with("--protected-vm")));
    assert!(!args.iter().any(|arg| arg == "--swiotlb"));
    assert!(!args.iter().any(|arg| arg.contains("qcom_trusted_vm")));
    // The hypervisor is always explicit, never crosvm's /dev/kvm-first default probe.
    assert_eq!(args[1..3], ["--hypervisor", "gunyah[device=/dev/gunyah]"]);
}

#[test]
fn stdio_or_duplicate_descriptors_are_rejected() {
    let mut stdio = fds();
    stdio.touch = 2;
    assert_eq!(
        backend().build_launch_plan(&spec(), &graphics(), stdio),
        Err(BackendError::InvalidSessionDescriptor)
    );
    let mut duplicate = fds();
    duplicate.mouse = duplicate.keyboard;
    assert_eq!(
        backend().build_launch_plan(&spec(), &graphics(), duplicate),
        Err(BackendError::InvalidSessionDescriptor)
    );
}

#[test]
fn relative_or_key_value_breaking_paths_are_rejected() {
    let relative = GunyahCrosvmBackend::new(PathBuf::from("crosvm"), PathBuf::from("/dev/gunyah"));
    assert_eq!(
        relative.build_launch_plan(&spec(), &graphics(), fds()),
        Err(BackendError::UnsafeRuntimeDirectory)
    );
    let device = GunyahCrosvmBackend::new(
        PathBuf::from("/apex/com.android.virt/bin/crosvm"),
        PathBuf::from("/dev/gunyah],protected=true"),
    );
    assert_eq!(
        device.build_launch_plan(&spec(), &graphics(), fds()),
        Err(BackendError::UnsafeRuntimeDirectory)
    );
    let mut comma_disk = spec();
    comma_disk.disks[0].path = PathBuf::from("/data/misc/carbonhv/objects/a,ro=false");
    assert_eq!(
        backend().build_launch_plan(&comma_disk, &graphics(), fds()),
        Err(BackendError::UnsafeRuntimeDirectory)
    );
}

#[test]
fn vcpus_beyond_performance_cores_are_refused_not_spilled() {
    let mut wide = spec();
    wide.vcpus = 8;
    assert_eq!(
        backend().build_launch_plan(&wide, &graphics(), fds()),
        Err(BackendError::Placement(
            PlacementError::ExceedsPerformanceCores {
                requested: 8,
                available: 4
            }
        ))
    );
}

#[test]
fn efficient_profile_drops_pinning_but_keeps_the_explicit_hypervisor() {
    let args = backend()
        .with_performance_profile(PerformanceProfile::Efficient)
        .build_launch_plan(&spec(), &graphics(), fds())
        .unwrap()
        .invocation
        .args;
    assert!(args.iter().any(|arg| arg == "gunyah[device=/dev/gunyah]"));
    assert!(!args.iter().any(|arg| arg == "--cpu-affinity"));
    assert!(!args.iter().any(|arg| arg == "--virt-cpufreq-upstream"));
    assert!(!args.iter().any(|arg| arg == "--boost-uclamp"));
}
