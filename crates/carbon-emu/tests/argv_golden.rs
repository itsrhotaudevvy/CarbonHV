//! Golden argv for each profile, from the reference host's measured topology.
//!
//! These are the review surface for the whole engine: if the QEMU invocation changes, this test
//! changes, and the change has to be justified. Facts come from
//! `qualification/emu/fixtures/kunzite.json`; the memory reading is raised only where the
//! admission rule would otherwise refuse the profile (the measured 2.75 GiB reading itself is
//! asserted separately as a refusal).

use std::path::PathBuf;

use carbon_emu::{
    EmuDisk, EmuHostFacts, EmuNetwork, EmuPlanError, EmuProfile, EmuSessionFds, EmuVmSpec,
    PresentationMode, VulkanFacts, WxMode,
};

fn kunzite_facts(mem_available_mib: u64) -> EmuHostFacts {
    let fixture = EmuHostFacts::parse_json(include_str!(
        "../../../qualification/emu/fixtures/kunzite.json"
    ))
    .expect("fixture parses");
    EmuHostFacts {
        mem_available_mib,
        ..fixture
    }
}

fn spec(profile: EmuProfile) -> EmuVmSpec {
    EmuVmSpec {
        instance_id: "cvd1-main".into(),
        generation: 1,
        vcpus: 0,
        memory_mib: 0,
        kernel: PathBuf::from("/data/misc/carbonhv/objects/kernel"),
        initrd: PathBuf::from("/data/misc/carbonhv/objects/initrd"),
        kernel_params: vec!["console=hvc0".into(), "bootconfig".into()],
        disks: vec![
            EmuDisk {
                fd: 10,
                writable: false,
            },
            EmuDisk {
                fd: 11,
                writable: true,
            },
        ],
        display: carbon_emu::DEFAULT_DISPLAY,
        network: EmuNetwork::Nat,
        audio: true,
        presentation: PresentationMode::ZeroCopy,
        profile,
    }
}

fn fds() -> EmuSessionFds {
    EmuSessionFds {
        qmp: 3,
        frame: 4,
        input: 5,
        vsock: Some(6),
        console: Some(7),
    }
}

fn qemu() -> PathBuf {
    PathBuf::from("/data/app/carbonhv/libqemu_system_aarch64.so")
}

#[test]
fn low_memory_profile_argv_from_the_measured_host() {
    let plan = carbon_emu::plan(
        qemu(),
        &spec(EmuProfile::LowMemory),
        &kunzite_facts(2750),
        &fds(),
    )
    .unwrap();
    assert_eq!(plan.invocation.args, expected_low_memory_argv());
}

#[test]
fn balanced_profile_argv_at_the_admission_boundary() {
    // 2816 MiB is exactly the plan's own admission floor for a 2048 MiB guest.
    let plan = carbon_emu::plan(
        qemu(),
        &spec(EmuProfile::Balanced),
        &kunzite_facts(2816),
        &fds(),
    )
    .unwrap();
    assert_eq!(plan.invocation.args, expected_balanced_argv());
}

#[test]
fn the_measured_reading_refuses_the_balanced_default() {
    let error = carbon_emu::plan(
        qemu(),
        &spec(EmuProfile::Balanced),
        &kunzite_facts(2750),
        &fds(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        EmuPlanError::InsufficientMemory {
            required_mib: 2816,
            available_mib: 2750
        }
    );
    // ... and the low-memory profile is the honest default for that reading.
    assert!(
        carbon_emu::plan(
            qemu(),
            &spec(EmuProfile::LowMemory),
            &kunzite_facts(2750),
            &fds()
        )
        .is_ok()
    );
}

#[test]
fn hardened_profile_enables_emulated_mte_and_says_it_is_unproven() {
    let plan = carbon_emu::plan(
        qemu(),
        &spec(EmuProfile::Hardened),
        &kunzite_facts(3584),
        &fds(),
    )
    .unwrap();
    assert!(
        plan.invocation
            .args
            .contains(&"virt,gic-version=3,highmem=on,mte=on".to_string())
    );
    assert!(
        plan.warnings
            .contains(&carbon_emu::EmuWarning::MteWithZeroCopyUnverified)
    );
}

#[test]
fn readback_works_without_hardware_buffer_export_and_is_labelled() {
    let mut facts = kunzite_facts(4096);
    facts.vulkan.hardware_buffer_export = false;
    let mut spec = spec(EmuProfile::Balanced);
    spec.presentation = PresentationMode::Readback;
    let plan = carbon_emu::plan(qemu(), &spec, &facts, &fds()).unwrap();
    assert!(
        plan.warnings
            .contains(&carbon_emu::EmuWarning::ReadbackPresentation)
    );
    assert_eq!(plan.presentation, PresentationMode::Readback);
    // Zero-copy on the same host is refused rather than silently downgraded.
    let mut zero_copy = spec.clone();
    zero_copy.presentation = PresentationMode::ZeroCopy;
    assert_eq!(
        carbon_emu::plan(qemu(), &zero_copy, &facts, &fds()).unwrap_err(),
        EmuPlanError::NoHardwareBufferExport
    );
}

#[test]
fn no_vulkan_host_never_gets_a_software_gpu() {
    let mut facts = kunzite_facts(4096);
    facts.vulkan = VulkanFacts {
        available: false,
        hardware_buffer_export: false,
        device_name: None,
    };
    assert_eq!(
        carbon_emu::plan(qemu(), &spec(EmuProfile::Balanced), &facts, &fds()).unwrap_err(),
        EmuPlanError::NoVulkan
    );
}

#[test]
fn anonymous_rwx_and_blocked_affinity_are_reported_not_hidden() {
    let mut facts = kunzite_facts(4096);
    facts.w_xor_x = WxMode::AnonymousRwx;
    facts.affinity_allowed = false;
    facts.o_direct = false;
    let plan = carbon_emu::plan(qemu(), &spec(EmuProfile::Balanced), &facts, &fds()).unwrap();
    assert!(
        plan.invocation
            .args
            .contains(&"tcg,thread=multi,tb-size=256,split-wx=off".to_string())
    );
    for expected in [
        carbon_emu::EmuWarning::AnonymousRwxTranslationCache,
        carbon_emu::EmuWarning::AffinityNotPermitted,
        carbon_emu::EmuWarning::ODirectUnavailable,
    ] {
        assert!(plan.warnings.contains(&expected), "missing {expected:?}");
    }
}

fn expected_low_memory_argv() -> Vec<String> {
    strings(&[
        "-nodefaults",
        "-no-user-config",
        "-name",
        "carbonhv-cvd1-main-1",
        "-machine",
        "virt,gic-version=3,highmem=on,mte=off",
        "-accel",
        "tcg,thread=multi,tb-size=128,split-wx=on",
        "-cpu",
        "max,pauth-impdef=on,sve=off,sme=off",
        "-smp",
        "3",
        "-m",
        "1536",
        "-object",
        "memory-backend-memfd,id=mem,size=1536M,share=on",
        "-numa",
        "node,memdev=mem",
        "-rtc",
        "base=utc,clock=host",
        "-display",
        "android,frame-fd=4,input-fd=5",
        "-device",
        "virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off,hostmem=256M,wsi=surfaceless",
        "-device",
        "virtio-multitouch-pci",
        "-device",
        "virtio-keyboard-pci",
        "-netdev",
        "user,id=net0",
        "-device",
        "virtio-net-pci,netdev=net0,romfile=",
        "-audiodev",
        "aaudio,id=snd0",
        "-device",
        "virtio-sound-pci,audiodev=snd0",
        "-add-fd",
        "fd=10,set=0",
        "-drive",
        "file=/dev/fdset/0,if=none,id=drive0,format=raw,aio=threads,cache=none,readonly=on",
        "-device",
        "virtio-blk-pci,drive=drive0",
        "-add-fd",
        "fd=11,set=1",
        "-drive",
        "file=/dev/fdset/1,if=none,id=drive1,format=raw,aio=threads,cache=none,auto-read-only=off,discard=unmap,detect-zeroes=unmap",
        "-device",
        "virtio-blk-pci,drive=drive1",
        "-chardev",
        "socket,id=vsock0,fd=6",
        "-device",
        "vhost-user-vsock-pci,chardev=vsock0",
        "-chardev",
        "socket,id=console0,fd=7",
        "-device",
        "virtio-serial-pci",
        "-device",
        "virtconsole,chardev=console0",
        "-chardev",
        "socket,id=qmp0,fd=3",
        "-qmp",
        "chardev:qmp0",
        "-kernel",
        "/data/misc/carbonhv/objects/kernel",
        "-initrd",
        "/data/misc/carbonhv/objects/initrd",
        "-append",
        "console=hvc0 bootconfig",
    ])
}

fn expected_balanced_argv() -> Vec<String> {
    strings(&[
        "-nodefaults",
        "-no-user-config",
        "-name",
        "carbonhv-cvd1-main-1",
        "-machine",
        "virt,gic-version=3,highmem=on,mte=off",
        "-accel",
        "tcg,thread=multi,tb-size=256,split-wx=on",
        "-cpu",
        "max,pauth-impdef=on,sve=off,sme=off",
        "-smp",
        "4",
        "-m",
        "2048",
        "-object",
        "memory-backend-memfd,id=mem,size=2048M,share=on",
        "-numa",
        "node,memdev=mem",
        "-rtc",
        "base=utc,clock=host",
        "-display",
        "android,frame-fd=4,input-fd=5",
        "-device",
        "virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off,hostmem=512M,wsi=surfaceless",
        "-device",
        "virtio-multitouch-pci",
        "-device",
        "virtio-keyboard-pci",
        "-netdev",
        "user,id=net0",
        "-device",
        "virtio-net-pci,netdev=net0,romfile=",
        "-audiodev",
        "aaudio,id=snd0",
        "-device",
        "virtio-sound-pci,audiodev=snd0",
        "-add-fd",
        "fd=10,set=0",
        "-drive",
        "file=/dev/fdset/0,if=none,id=drive0,format=raw,aio=threads,cache=none,readonly=on",
        "-device",
        "virtio-blk-pci,drive=drive0",
        "-add-fd",
        "fd=11,set=1",
        "-drive",
        "file=/dev/fdset/1,if=none,id=drive1,format=raw,aio=threads,cache=none,auto-read-only=off,discard=unmap,detect-zeroes=unmap",
        "-device",
        "virtio-blk-pci,drive=drive1",
        "-chardev",
        "socket,id=vsock0,fd=6",
        "-device",
        "vhost-user-vsock-pci,chardev=vsock0",
        "-chardev",
        "socket,id=console0,fd=7",
        "-device",
        "virtio-serial-pci",
        "-device",
        "virtconsole,chardev=console0",
        "-chardev",
        "socket,id=qmp0,fd=3",
        "-qmp",
        "chardev:qmp0",
        "-kernel",
        "/data/misc/carbonhv/objects/kernel",
        "-initrd",
        "/data/misc/carbonhv/objects/initrd",
        "-append",
        "console=hvc0 bootconfig",
    ])
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
