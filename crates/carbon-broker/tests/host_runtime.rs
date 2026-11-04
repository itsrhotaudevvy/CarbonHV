use std::ffi::OsString;
use std::path::PathBuf;

use carbon_broker::{
    AvfBackend, AvfGraphicsConfig, DisplaySessionIdentity, HostRuntimeGraphicalVmRequest,
    HostRuntimeRequestError, PerformanceProfile, ValidatedVmDisk, ValidatedVmSpec,
};
use carbon_core::Generation;

fn spec() -> ValidatedVmSpec {
    ValidatedVmSpec {
        request_id: 17,
        instance_id: "vm-demo".into(),
        generation: Generation(23),
        kernel: PathBuf::from("/data/misc/carbonhv/objects/kernel-a"),
        initrd: PathBuf::from("/data/misc/carbonhv/objects/initrd-a"),
        disks: vec![
            ValidatedVmDisk {
                path: PathBuf::from("/data/misc/carbonhv/objects/system-a"),
                writable: false,
            },
            ValidatedVmDisk {
                path: PathBuf::from("/data/misc/carbonhv/objects/data-a"),
                writable: true,
            },
        ],
        memory_mib: 6144,
        vcpus: 6,
        kernel_params: vec!["console=hvc0".into(), "androidboot.foo=bar".into()],
    }
}

fn platform_plan(spec: &ValidatedVmSpec) -> carbon_broker::AvfPlatformVmPlan {
    let backend = AvfBackend::new(
        PathBuf::from("/apex/com.android.virt/bin/vm"),
        PathBuf::from("/dev/kvm"),
    );
    backend
        .build_graphics_platform_plan(
            spec,
            AvfGraphicsConfig::gfxstream_vulkan(1080, 2400, 420, 90).unwrap(),
        )
        .unwrap()
}

#[test]
fn runtime_publication_preserves_broker_identity_and_platform_plan_exactly() {
    let spec = spec();
    let plan = platform_plan(&spec);
    let identity = DisplaySessionIdentity {
        instance_id: spec.instance_id.clone(),
        generation: spec.generation,
    };

    let request = HostRuntimeGraphicalVmRequest::try_from_plan(&identity, &plan).unwrap();

    assert_eq!(request.instance_id, "vm-demo");
    assert_eq!(request.generation, 23);
    assert_eq!(request.kernel_path, "/data/misc/carbonhv/objects/kernel-a");
    assert_eq!(request.initrd_path, "/data/misc/carbonhv/objects/initrd-a");
    assert_eq!(request.disks.len(), 2);
    assert_eq!(
        request.disks[0].path,
        "/data/misc/carbonhv/objects/system-a"
    );
    assert!(!request.disks[0].writable);
    assert_eq!(request.disks[1].path, "/data/misc/carbonhv/objects/data-a");
    assert!(request.disks[1].writable);
    assert_eq!(request.memory_mib, 6144);
    assert_eq!(request.vcpus, 6);
    assert_eq!(
        request.kernel_params,
        vec![
            "console=hvc0".to_string(),
            "androidboot.foo=bar".to_string(),
            "gfxstream_enabled".to_string(),
        ]
    );
    assert_eq!(request.width_pixels, 1080);
    assert_eq!(request.height_pixels, 2400);
    assert_eq!(request.density_dpi, 420);
    assert_eq!(request.refresh_rate, 90);
    // Near-native default on AVF: no pinning is expressible, so boost vCPU threads instead.
    assert!(request.huge_pages);
    assert!(request.boost_uclamp);
    assert!(request.balloon);
}

#[test]
fn efficient_profile_reaches_the_runtime_request_without_boost() {
    let spec = spec();
    let plan = AvfBackend::new(
        PathBuf::from("/apex/com.android.virt/bin/vm"),
        PathBuf::from("/dev/kvm"),
    )
    .with_performance_profile(PerformanceProfile::Efficient)
    .build_graphics_platform_plan(
        &spec,
        AvfGraphicsConfig::gfxstream_vulkan(1080, 2400, 420, 90).unwrap(),
    )
    .unwrap();
    let identity = DisplaySessionIdentity {
        instance_id: spec.instance_id.clone(),
        generation: spec.generation,
    };
    let request = HostRuntimeGraphicalVmRequest::try_from_plan(&identity, &plan).unwrap();
    assert!(!request.boost_uclamp);
    assert!(request.huge_pages);
    assert!(request.balloon);
}

#[test]
fn runtime_publication_rejects_generation_that_does_not_fit_android_long() {
    let spec = spec();
    let plan = platform_plan(&spec);
    let identity = DisplaySessionIdentity {
        instance_id: spec.instance_id,
        generation: Generation(i64::MAX as u64 + 1),
    };

    assert_eq!(
        HostRuntimeGraphicalVmRequest::try_from_plan(&identity, &plan),
        Err(HostRuntimeRequestError::GenerationOutOfRange),
    );
}

#[cfg(unix)]
#[test]
fn runtime_publication_rejects_non_utf8_managed_paths_without_lossy_conversion() {
    use std::os::unix::ffi::OsStringExt;

    let mut spec = spec();
    spec.kernel = PathBuf::from(OsString::from_vec(vec![
        b'/', b'd', b'a', b't', b'a', b'/', 0xff,
    ]));
    let plan = platform_plan(&spec);
    let identity = DisplaySessionIdentity {
        instance_id: spec.instance_id,
        generation: spec.generation,
    };

    assert_eq!(
        HostRuntimeGraphicalVmRequest::try_from_plan(&identity, &plan),
        Err(HostRuntimeRequestError::NonUtf8Path),
    );
}

#[test]
fn avf_hosts_running_thp_never_do_not_request_huge_pages() {
    let spec = spec();
    let plan = AvfBackend::new(
        PathBuf::from("/apex/com.android.virt/bin/vm"),
        PathBuf::from("/dev/kvm"),
    )
    .with_host_thp(carbon_broker::ThpMode::Never)
    .build_graphics_platform_plan(
        &spec,
        AvfGraphicsConfig::gfxstream_vulkan(1080, 2392, 440, 120).unwrap(),
    )
    .unwrap();
    assert!(!plan.performance.huge_pages);
    assert!(plan.performance.boost_uclamp);
}
