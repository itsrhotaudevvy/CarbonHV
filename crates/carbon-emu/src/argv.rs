//! The QEMU argv for one emulated boot.
//!
//! Rules this module keeps, mirroring `crates/carbon-broker/src/direct_crosvm.rs`:
//!
//! - One argument per vector entry. No shell, no `sh -c`, no string joining of guest-controlled
//!   text.
//! - Disks and all control channels are inherited descriptors (`-add-fd`, `-chardev socket,fd=`).
//!   QEMU never receives a path into app storage for a disk.
//! - `-nodefaults -no-user-config` and an explicit device list: no USB, no legacy devices, no
//!   monitor, no `-serial stdio`.
//! - The engine is dynamic binary translation only; `-accel tcg` with multi-threaded TCG.

use std::path::{Component, Path, PathBuf};

use carbon_core::ProgramInvocation;

use crate::{
    EmuHostFacts, EmuNetwork, EmuPlacement, EmuPlanError, EmuProfile, EmuSessionFds, EmuVmSpec,
    EmuWarning, MemoryBudget, PresentationMode, WxMode, cpu_model, cpu_model::CpuModelPlan, memory,
    placement,
};

/// Device names QEMU accepts for this machine, including the packaged executable name.
const ALLOWED_PROGRAMS: [&str; 2] = ["qemu-system-aarch64", "libqemu_system_aarch64.so"];
/// Most kernel parameters forwarded to `-append`; keeps the command line reviewable.
const MAX_KERNEL_PARAMS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmuPlan {
    pub invocation: ProgramInvocation,
    pub placement: EmuPlacement,
    pub memory: MemoryBudget,
    pub cpu_model: CpuModelPlan,
    pub presentation: PresentationMode,
    pub profile: EmuProfile,
    pub warnings: Vec<EmuWarning>,
}

impl EmuPlan {
    /// The argv as a reviewer can paste into a desktop shell (`carbonctl emu plan --shell`).
    /// Quoting is only for display; the real launch passes `invocation.args` verbatim.
    pub fn shell_argv(&self) -> String {
        let mut text = shell_quote(&self.invocation.program.to_string_lossy());
        for arg in &self.invocation.args {
            text.push(' ');
            text.push_str(&shell_quote(arg));
        }
        text
    }

    /// Stable, machine-readable lines for the app's diagnostics screen.
    pub fn diagnostics(&self) -> Vec<String> {
        let mut lines = vec![
            "engine=emulated".to_owned(),
            "accelerator=tcg".to_owned(),
            format!("profile={}", self.profile.as_str()),
            format!("presentation={}", self.presentation.as_str()),
            format!("cpu={}", self.cpu_model.cpu_argument()),
            format!("mte={}", self.cpu_model.mte),
            format!("pac={}", self.cpu_model.pac),
            format!("bti={}", self.cpu_model.bti),
            "cpu.native=false".to_owned(),
            format!("vcpus={}", self.placement.vcpu_count),
            format!(
                "vcpu.cores={}",
                self.placement
                    .vcpu_cores
                    .iter()
                    .map(|core| core.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            format!("tb_size_mib={}", self.memory.tb_size_mib),
            format!("guest_mib={}", self.memory.guest_mib),
            format!("footprint_mib={}", self.memory.footprint_mib),
        ];
        for warning in &self.warnings {
            lines.push(format!("warning={}", warning.code()));
        }
        lines
    }
}

fn shell_quote(value: &str) -> String {
    let safe = !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(
                    ch,
                    '_' | '-' | '.' | '/' | ':' | '=' | ',' | '@' | '%' | '+'
                )
        });
    if safe {
        return value.to_owned();
    }
    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

/// Builds the complete plan for one boot.
pub fn build_plan(
    qemu: PathBuf,
    spec: &EmuVmSpec,
    host: &EmuHostFacts,
    fds: &EmuSessionFds,
) -> Result<EmuPlan, EmuPlanError> {
    if !ALLOWED_PROGRAMS
        .iter()
        .any(|name| qemu.file_name().and_then(|file| file.to_str()) == Some(*name))
        || !is_plain_absolute(&qemu)
    {
        return Err(EmuPlanError::UnexpectedQemuProgram);
    }
    validate_instance_id(&spec.instance_id)?;
    spec.display.validate()?;
    fds.validate(&spec.disks)?;

    // Hardware graphics or nothing (ADR-016): the guest GPU is gfxstream on the host Vulkan
    // device, so a host without Vulkan cannot run this engine at all.
    if !host.vulkan.available {
        return Err(EmuPlanError::NoVulkan);
    }
    if spec.presentation == PresentationMode::ZeroCopy && !host.vulkan.hardware_buffer_export {
        return Err(EmuPlanError::NoHardwareBufferExport);
    }
    let split_wx = match host.w_xor_x {
        WxMode::SplitWx => "on",
        WxMode::AnonymousRwx => "off",
        WxMode::Unknown => return Err(EmuPlanError::UnsupportedWxMode),
    };

    let budget = memory::admit(host, spec.memory_mib, spec.profile, spec.presentation)?;
    let placement = placement::plan(host, spec.vcpus, spec.profile)?;
    let cpu = cpu_model::for_profile(spec.profile);

    let kernel = checked_path("kernel", &spec.kernel)?;
    let initrd = checked_path("initrd", &spec.initrd)?;
    let mut append = Vec::with_capacity(spec.kernel_params.len());
    if spec.kernel_params.len() > MAX_KERNEL_PARAMS {
        return Err(EmuPlanError::InvalidKernelParameter {
            index: MAX_KERNEL_PARAMS,
        });
    }
    for (index, param) in spec.kernel_params.iter().enumerate() {
        if !is_safe_kernel_param(param) {
            return Err(EmuPlanError::InvalidKernelParameter { index });
        }
        append.push(param.clone());
    }

    let mut args: Vec<String> = vec![
        "-nodefaults".into(),
        "-no-user-config".into(),
        "-name".into(),
        qemu_name(&spec.instance_id, spec.generation),
        "-machine".into(),
        format!(
            "virt,gic-version=3,highmem=on,mte={}",
            if cpu.mte { "on" } else { "off" }
        ),
        "-accel".into(),
        format!(
            "tcg,thread=multi,tb-size={},split-wx={split_wx}",
            budget.tb_size_mib
        ),
        "-cpu".into(),
        cpu.cpu_argument(),
        "-smp".into(),
        placement.vcpu_count.to_string(),
        "-m".into(),
        budget.guest_mib.to_string(),
        "-object".into(),
        format!(
            "memory-backend-memfd,id=mem,size={}M,share=on",
            budget.guest_mib
        ),
        "-numa".into(),
        "node,memdev=mem".into(),
        "-rtc".into(),
        "base=utc,clock=host".into(),
        "-display".into(),
        format!("android,frame-fd={},input-fd={}", fds.frame, fds.input),
        "-device".into(),
        format!(
            "virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,gfxstream-gles=on,cross-domain=off,hostmem={}M,wsi=surfaceless",
            budget.gfxstream_hostmem_mib
        ),
        "-device".into(),
        "virtio-multitouch-pci".into(),
        "-device".into(),
        "virtio-keyboard-pci".into(),
    ];

    match spec.network {
        EmuNetwork::Offline => {}
        EmuNetwork::Nat => {
            args.push("-netdev".into());
            args.push("user,id=net0".into());
            args.push("-device".into());
            args.push("virtio-net-pci,netdev=net0".into());
        }
    }
    if spec.audio {
        args.push("-audiodev".into());
        args.push("aaudio,id=snd0".into());
        args.push("-device".into());
        args.push("virtio-sound-pci,audiodev=snd0".into());
    }

    for (index, disk) in spec.disks.iter().enumerate() {
        args.push("-add-fd".into());
        args.push(format!("fd={},set={index}", disk.fd));
        let mut drive = format!(
            "file=/dev/fdset/{index},if=none,id=drive{index},format=raw,aio=threads,cache={}",
            if host.o_direct { "none" } else { "writeback" }
        );
        if !disk.writable {
            drive.push_str(",readonly=on");
        } else {
            drive.push_str(",discard=unmap,detect-zeroes=unmap");
        }
        args.push("-drive".into());
        args.push(drive);
        args.push("-device".into());
        args.push(format!("virtio-blk-pci,drive=drive{index}"));
    }

    if let Some(vsock) = fds.vsock {
        args.push("-chardev".into());
        args.push(format!("socket,id=vsock0,fd={vsock}"));
        args.push("-device".into());
        args.push("vhost-user-vsock-pci,chardev=vsock0".into());
    }
    if let Some(console) = fds.console {
        args.push("-chardev".into());
        args.push(format!("socket,id=console0,fd={console}"));
        args.push("-device".into());
        args.push("virtio-serial-pci".into());
        args.push("-device".into());
        args.push("virtconsole,chardev=console0".into());
    }

    args.push("-chardev".into());
    args.push(format!("socket,id=qmp0,fd={}", fds.qmp));
    args.push("-qmp".into());
    args.push("chardev:qmp0".into());

    args.push("-kernel".into());
    args.push(kernel);
    args.push("-initrd".into());
    args.push(initrd);
    if !append.is_empty() {
        args.push("-append".into());
        args.push(append.join(" "));
    }

    Ok(EmuPlan {
        invocation: ProgramInvocation {
            program: qemu,
            args,
        },
        warnings: warnings_for(&placement, host, spec, &cpu),
        placement,
        memory: budget,
        cpu_model: cpu,
        presentation: spec.presentation,
        profile: spec.profile,
    })
}

/// Warnings never change the argv; they change what the user is told.
fn warnings_for(
    placement: &EmuPlacement,
    host: &EmuHostFacts,
    spec: &EmuVmSpec,
    cpu: &CpuModelPlan,
) -> Vec<EmuWarning> {
    let mut warnings = Vec::new();
    // Always present: there is no software fallback to fall back to.
    warnings.push(EmuWarning::SoftwareFallbackImpossible);
    if spec.presentation == PresentationMode::Readback {
        warnings.push(EmuWarning::ReadbackPresentation);
    }
    if cpu.mte && spec.presentation == PresentationMode::ZeroCopy {
        warnings.push(EmuWarning::MteWithZeroCopyUnverified);
    }
    if placement.spare_cores.is_empty() {
        warnings.push(EmuWarning::NoSparePerformanceCore);
    }
    if !placement.affinity_applied {
        warnings.push(EmuWarning::AffinityNotPermitted);
    }
    if placement.uclamp_min.is_none() && spec.profile != EmuProfile::LowMemory {
        warnings.push(EmuWarning::UclampNotPermitted);
    }
    if host.w_xor_x == WxMode::AnonymousRwx {
        warnings.push(EmuWarning::AnonymousRwxTranslationCache);
    }
    if !host.o_direct {
        warnings.push(EmuWarning::ODirectUnavailable);
    }
    warnings
}

fn qemu_name(instance_id: &str, generation: u64) -> String {
    format!("carbonhv-{instance_id}-{generation}")
}

fn validate_instance_id(instance_id: &str) -> Result<(), EmuPlanError> {
    let valid = !instance_id.is_empty()
        && instance_id.len() <= 64
        && !instance_id.starts_with('-')
        && instance_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if valid {
        Ok(())
    } else {
        Err(EmuPlanError::InvalidInstanceId)
    }
}

fn checked_path(field: &'static str, path: &Path) -> Result<String, EmuPlanError> {
    if !is_plain_absolute(path) {
        return Err(EmuPlanError::UnsafePath { field });
    }
    let text = path
        .to_str()
        .ok_or(EmuPlanError::NonUtf8Path { field })?
        .to_owned();
    // `-drive` and `-chardev` parse commas as separators; QEMU's own file options would too.
    if text.contains(',') {
        return Err(EmuPlanError::UnsafePath { field });
    }
    Ok(text)
}

fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}

/// Kernel parameters land in one `-append` value, where whitespace separates parameters.
fn is_safe_kernel_param(param: &str) -> bool {
    !param.is_empty()
        && param.len() <= 256
        && !param.starts_with('-')
        && param
            .chars()
            .all(|ch| ch.is_ascii_graphic() && ch != ',' && ch != '\'')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::VulkanFacts;

    fn host() -> EmuHostFacts {
        EmuHostFacts {
            mem_available_mib: 4096,
            big_cores: vec![4, 5, 6, 7],
            w_xor_x: WxMode::SplitWx,
            o_direct: true,
            vulkan: VulkanFacts {
                available: true,
                hardware_buffer_export: true,
                device_name: Some("Adreno (TM) 710".into()),
            },
            affinity_allowed: true,
            uclamp_allowed: true,
            source: None,
        }
    }

    fn spec() -> EmuVmSpec {
        EmuVmSpec {
            instance_id: "cvd1-main".into(),
            generation: 3,
            vcpus: 4,
            memory_mib: 2048,
            kernel: PathBuf::from("/data/user/0/dev.carbonhv.app/files/objects/boot-kernel"),
            initrd: PathBuf::from("/data/user/0/dev.carbonhv.app/files/objects/boot-initrd"),
            kernel_params: vec!["console=hvc0".into(), "bootconfig".into()],
            disks: vec![
                crate::EmuDisk {
                    fd: 20,
                    writable: false,
                },
                crate::EmuDisk {
                    fd: 21,
                    writable: true,
                },
            ],
            display: crate::DEFAULT_DISPLAY,
            network: EmuNetwork::Nat,
            audio: true,
            presentation: PresentationMode::ZeroCopy,
            profile: EmuProfile::Balanced,
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

    #[test]
    fn builds_a_hardening_first_argv() {
        let plan = build_plan(
            PathBuf::from("/usr/bin/qemu-system-aarch64"),
            &spec(),
            &host(),
            &fds(),
        )
        .unwrap();
        let args = &plan.invocation.args;
        assert!(args.contains(&"-nodefaults".to_string()));
        assert!(args.contains(&"-no-user-config".to_string()));
        assert!(args.contains(&"tcg,thread=multi,tb-size=256,split-wx=on".to_string()));
        assert!(args.contains(&"max,pauth-impdef=on,sve=off,sme=off".to_string()));
        assert!(args.contains(&"virt,gic-version=3,highmem=on,mte=off".to_string()));
        assert!(args.contains(&"memory-backend-memfd,id=mem,size=2048M,share=on".to_string()));
        assert!(args.contains(&"virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,gfxstream-gles=on,cross-domain=off,hostmem=512M,wsi=surfaceless".to_string()));
        assert!(args.contains(&"file=/dev/fdset/0,if=none,id=drive0,format=raw,aio=threads,cache=none,readonly=on".to_string()));
        assert!(args.contains(&"file=/dev/fdset/1,if=none,id=drive1,format=raw,aio=threads,cache=none,discard=unmap,detect-zeroes=unmap".to_string()));
        assert!(args.contains(&"chardev:qmp0".to_string()));
        assert!(args.contains(&"console=hvc0 bootconfig".to_string()));
        // A TCG engine never reaches a hypervisor accelerator.
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("kvm") || arg.contains("hvf"))
        );
        assert!(
            plan.shell_argv()
                .starts_with("/usr/bin/qemu-system-aarch64 -nodefaults")
        );
        assert!(plan.diagnostics().contains(&"cpu.native=false".to_string()));
    }

    #[test]
    fn refuses_a_software_graphics_path() {
        let mut no_vulkan = host();
        no_vulkan.vulkan.available = false;
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &spec(),
                &no_vulkan,
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::NoVulkan
        );
        let mut no_export = host();
        no_export.vulkan.hardware_buffer_export = false;
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &spec(),
                &no_export,
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::NoHardwareBufferExport
        );
    }

    #[test]
    fn refuses_unsafe_paths_and_names() {
        let mut bad = spec();
        bad.kernel = PathBuf::from("/data/../etc/shadow");
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &bad,
                &host(),
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::UnsafePath { field: "kernel" }
        );
        let mut bad = spec();
        bad.instance_id = "a,b".into();
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &bad,
                &host(),
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::InvalidInstanceId
        );
        let mut bad = spec();
        bad.kernel_params = vec!["console=hvc0 -fs 0".into()];
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &bad,
                &host(),
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::InvalidKernelParameter { index: 0 }
        );
        assert_eq!(
            build_plan(PathBuf::from("/usr/bin/python3"), &spec(), &host(), &fds()).unwrap_err(),
            EmuPlanError::UnexpectedQemuProgram
        );
    }

    #[test]
    fn refuses_duplicate_descriptors_and_unknown_wx() {
        let mut duplicate = fds();
        duplicate.frame = duplicate.qmp;
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &spec(),
                &host(),
                &duplicate
            )
            .unwrap_err(),
            EmuPlanError::InvalidSessionDescriptor
        );
        let mut unknown_wx = host();
        unknown_wx.w_xor_x = WxMode::Unknown;
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &spec(),
                &unknown_wx,
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::UnsupportedWxMode
        );
    }

    #[test]
    fn packs_the_guest_memory_budget() {
        let mut oversized = spec();
        oversized.memory_mib = 4096;
        assert_eq!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &oversized,
                &host(),
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::GuestMemoryExceedsEmulatedMaximum {
                requested_mib: 4096,
                maximum_mib: 3072
            }
        );
        let mut tight = host();
        tight.mem_available_mib = 2400;
        assert!(matches!(
            build_plan(
                PathBuf::from("/usr/bin/qemu-system-aarch64"),
                &spec(),
                &tight,
                &fds()
            )
            .unwrap_err(),
            EmuPlanError::InsufficientMemory { .. }
        ));
    }

    #[test]
    fn readback_is_selectable_and_reported() {
        let mut readback = spec();
        readback.presentation = PresentationMode::Readback;
        let plan = build_plan(
            PathBuf::from("/usr/bin/qemu-system-aarch64"),
            &readback,
            &host(),
            &fds(),
        )
        .unwrap();
        assert!(plan.warnings.contains(&EmuWarning::ReadbackPresentation));
        assert_eq!(plan.presentation.as_str(), "readback");
        assert!(plan.memory.footprint_mib < 2996);
    }
}
