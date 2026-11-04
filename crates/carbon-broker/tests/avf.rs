use std::fs;
use std::path::PathBuf;

use carbon_broker::{
    AvfBackend, AvfGraphicsConfig, AvfInputConfig, BackendError, HostVirtualizationClass,
    ProgramInvocation, ValidatedVmDisk, ValidatedVmSpec,
};
use carbon_core::Generation;

fn spec() -> ValidatedVmSpec {
    ValidatedVmSpec {
        request_id: 1,
        instance_id: "vm-demo".into(),
        generation: Generation(3),
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
        memory_mib: 4096,
        vcpus: 4,
        kernel_params: vec!["console=hvc0".into(), "rdinit=/bin/init".into()],
    }
}

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("carbonhv-avf-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn builds_exact_argv_without_shell_interpolation() {
    let root = temp_root("argv");
    let vm_tool = root.join("vm");
    fs::write(&vm_tool, b"tool").unwrap();
    let backend = AvfBackend::new(vm_tool.clone(), root.join("kvm"));
    let mut value = spec();
    value.vcpus = 1;
    let plan = backend
        .build_launch_plan(&value, PathBuf::from("/data/misc/carbonhv/runtime"))
        .unwrap();

    assert_eq!(
        plan.invocation,
        ProgramInvocation {
            program: vm_tool,
            args: vec![
                "run".into(),
                "/data/misc/carbonhv/runtime/vm-demo-3.json".into()
            ],
        }
    );
    assert!(
        plan.config_json
            .contains("\"kernel\":\"/data/misc/carbonhv/objects/kernel-a\"")
    );
    assert!(
        plan.config_json
            .contains("\"params\":\"console=hvc0 rdinit=/bin/init\"")
    );
    assert!(plan.config_json.contains(
        "\"disks\":[{\"image\":\"/data/misc/carbonhv/objects/system-a\",\"writable\":false,\"partitions\":[]},{\"image\":\"/data/misc/carbonhv/objects/data-a\",\"writable\":true,\"partitions\":[]}]"
    ));
    assert!(plan.config_json.contains("\"platform_version\":\"~1.0\""));
    assert!(plan.config_json.contains("\"cpu_topology\":\"one_cpu\""));
    assert!(!plan.config_json.contains("\"vcpus\""));
    assert!(!plan.config_json.contains("\"gpu\""));
    assert!(!plan.config_json.contains("\"display\""));
    assert!(!plan.config_json.contains("\"input\""));
    assert!(!plan.config_json.contains("sh -c"));
}

#[test]
fn graphical_launch_plan_uses_android17_platform_binder_contract() {
    let root = temp_root("graphics");
    let vm_tool = root.join("vm");
    fs::write(&vm_tool, b"tool").unwrap();
    let backend = AvfBackend::new(vm_tool, root.join("kvm"));
    let graphics = AvfGraphicsConfig::gfxstream_vulkan(1080, 2400, 420, 90).unwrap();
    let plan = backend
        .build_graphics_platform_plan(&spec(), graphics)
        .unwrap();

    assert_eq!(plan.platform_version, "~1.0");
    assert_eq!(plan.memory_mib, 4096);
    assert_eq!(plan.vcpus, 4);
    assert_eq!(plan.gpu.backend, "gfxstream");
    assert_eq!(
        plan.gpu.context_types,
        vec!["gfxstream-vulkan", "gfxstream-composer"]
    );
    assert!(!plan.gpu.renderer_use_egl);
    assert!(!plan.gpu.renderer_use_gles);
    assert!(!plan.gpu.renderer_use_glx);
    assert!(plan.gpu.renderer_use_surfaceless);
    assert!(plan.gpu.renderer_use_vulkan);
    assert_eq!(
        plan.gpu.renderer_features,
        "VulkanDisableCoherentMemoryAndEmulate:enabled;VulkanAllocateHostVisibleAsUdmabuf:enabled;ExternalBlob:enabled"
    );
    assert_eq!(plan.display.width_pixels, 1080);
    assert_eq!(plan.display.height_pixels, 2400);
    assert_eq!(plan.display.density_dpi, 420);
    assert_eq!(plan.display.refresh_rate, 90);
    assert_eq!(
        plan.input,
        AvfInputConfig {
            multitouch: true,
            keyboard: true,
            mouse: true,
        }
    );
    assert!(
        plan.kernel_params
            .iter()
            .any(|value| value == "gfxstream_enabled")
    );
}

#[test]
fn graphics_config_rejects_invalid_geometry_or_refresh_rate() {
    assert_eq!(
        AvfGraphicsConfig::gfxstream_vulkan(0, 2400, 420, 90),
        Err(BackendError::InvalidGraphicsConfig),
    );
    assert_eq!(
        AvfGraphicsConfig::gfxstream_vulkan(1080, 2400, 420, 0),
        Err(BackendError::InvalidGraphicsConfig),
    );
    assert_eq!(
        AvfGraphicsConfig::gfxstream_vulkan(1080, 2400, 0, 90),
        Err(BackendError::InvalidGraphicsConfig),
    );
}

#[test]
fn headless_vm_tool_path_rejects_unrepresentable_vcpu_count() {
    let root = temp_root("headless-vcpus");
    let backend = AvfBackend::new(root.join("vm"), root.join("kvm"));

    assert_eq!(
        backend.build_launch_plan(&spec(), root.join("runtime")),
        Err(BackendError::UnsupportedCliVcpuCount { requested: 4 })
    );
}

#[test]
fn escapes_json_control_characters_without_creating_shell_syntax() {
    let root = temp_root("json");
    let backend = AvfBackend::new(root.join("vm"), root.join("kvm"));
    let mut value = spec();
    value.vcpus = 1;
    value.kernel_params = vec!["androidboot.foo=quote\\\"value".into()];
    let plan = backend
        .build_launch_plan(&value, root.join("runtime"))
        .unwrap();

    assert!(plan.config_json.contains("quote\\\\\\\"value"));
    assert_eq!(plan.invocation.args.len(), 2);
}

#[test]
fn probe_reports_missing_avf_tool() {
    let root = temp_root("missing");
    fs::write(root.join("kvm"), b"device").unwrap();
    let backend = AvfBackend::new(root.join("vm"), root.join("kvm"));

    assert_eq!(backend.probe().class, HostVirtualizationClass::Unavailable);
}

#[test]
fn probe_requires_both_vm_tool_and_kvm() {
    let root = temp_root("ready");
    fs::write(root.join("vm"), b"tool").unwrap();
    fs::write(root.join("kvm"), b"device").unwrap();
    let backend = AvfBackend::new(root.join("vm"), root.join("kvm"));

    let probe = backend.probe();
    assert!(probe.vm_tool_present);
    assert!(probe.kvm_present);
    assert_eq!(probe.class, HostVirtualizationClass::DevelopmentReady);
}

#[test]
fn launch_plan_rejects_runtime_directory_with_parent_components() {
    let root = temp_root("runtime-path");
    let backend = AvfBackend::new(root.join("vm"), root.join("kvm"));

    assert_eq!(
        backend.build_launch_plan(&spec(), PathBuf::from("/data/misc/carbonhv/../tmp")),
        Err(BackendError::UnsafeRuntimeDirectory)
    );
}

#[test]
fn gunyah_host_without_kvm_is_reported_distinctly_and_never_ready() {
    let root = temp_root("gunyah");
    fs::write(root.join("vm"), b"tool").unwrap();
    fs::write(root.join("gunyah"), b"device").unwrap();
    let backend =
        AvfBackend::new(root.join("vm"), root.join("kvm")).with_gunyah_device(root.join("gunyah"));

    let probe = backend.probe();
    assert!(probe.gunyah_present);
    assert!(!probe.kvm_present);
    assert_eq!(probe.class, HostVirtualizationClass::GunyahWithoutBackend);
}

#[test]
fn kvm_takes_precedence_over_gunyah_and_absent_gunyah_stays_unavailable() {
    let root = temp_root("kvm-and-gunyah");
    fs::write(root.join("vm"), b"tool").unwrap();
    fs::write(root.join("kvm"), b"device").unwrap();
    fs::write(root.join("gunyah"), b"device").unwrap();
    let both =
        AvfBackend::new(root.join("vm"), root.join("kvm")).with_gunyah_device(root.join("gunyah"));
    assert_eq!(
        both.probe().class,
        HostVirtualizationClass::DevelopmentReady
    );

    let neither = AvfBackend::new(root.join("vm"), root.join("nokvm"))
        .with_gunyah_device(root.join("nogunyah"));
    let probe = neither.probe();
    assert!(!probe.gunyah_present);
    assert_eq!(probe.class, HostVirtualizationClass::Unavailable);
}
