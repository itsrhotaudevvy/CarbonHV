use std::fs;
use std::path::PathBuf;

use carbon_broker::{AvfBackend, BackendError, HostVirtualizationClass, ProgramInvocation, ValidatedVmSpec};
use carbon_core::Generation;

fn spec() -> ValidatedVmSpec {
    ValidatedVmSpec {
        request_id: 1,
        instance_id: "vm-demo".into(),
        generation: Generation(3),
        kernel: PathBuf::from("/data/misc/carbonhv/objects/kernel-a"),
        initrd: PathBuf::from("/data/misc/carbonhv/objects/initrd-a"),
        disks: vec![PathBuf::from("/data/misc/carbonhv/objects/disk-a")],
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
    let plan = backend.build_launch_plan(&spec(), PathBuf::from("/data/misc/carbonhv/runtime")).unwrap();

    assert_eq!(
        plan.invocation,
        ProgramInvocation {
            program: vm_tool,
            args: vec!["run".into(), "/data/misc/carbonhv/runtime/vm-demo-3.json".into()],
        }
    );
    assert!(plan.config_json.contains("\"kernel\":\"/data/misc/carbonhv/objects/kernel-a\""));
    assert!(plan.config_json.contains("\"params\":\"console=hvc0 rdinit=/bin/init\""));
    assert!(!plan.config_json.contains("sh -c"));
}

#[test]
fn escapes_json_control_characters_without_creating_shell_syntax() {
    let root = temp_root("json");
    let backend = AvfBackend::new(root.join("vm"), root.join("kvm"));
    let mut value = spec();
    value.kernel_params = vec!["androidboot.foo=quote\\\"value".into()];
    let plan = backend.build_launch_plan(&value, root.join("runtime")).unwrap();

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
