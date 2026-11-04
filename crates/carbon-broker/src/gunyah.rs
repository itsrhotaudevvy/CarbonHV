//! Direct crosvm launch planning for Qualcomm Gunyah hosts.
//!
//! On a Snapdragon phone the hypervisor is Gunyah, not KVM. The AOSP `com.android.virt` APEX
//! ships a crosvm built with the `gunyah` feature (virtmgr uses it for QCOM trusted VMs), and
//! crosvm creates *unprotected* Gunyah VMs with shared (not lent) guest memory and no swiotlb
//! bounce buffers, so virtio and gfxstream blob memory stay zero-copy.
//!
//! This backend gives CarbonHV what AVF's raw config cannot: per-vCPU pinning, mirrored core
//! capacities and frequency domains, and guest-driven DVFS. It always names the hypervisor
//! explicitly so a host that also exposes `/dev/kvm` can never silently switch hypervisors, and it
//! never requests a protected VM.
//!
//! Guest-visible differences on Gunyah with the pinned crosvm revision: no SVE, no MTE and no
//! PMU in the guest. These are recorded in the security-difference matrix, not papered over.

use std::path::{Component, Path, PathBuf};

use crate::avf::{GFXSTREAM_CONTEXT_TYPES, GFXSTREAM_RENDERER_FEATURES};
use crate::{
    AvfGraphicsConfig, BackendError, CpuControl, GuestPerformancePlan, HostResources,
    PerformanceProfile, ProgramInvocation, ValidatedVmSpec,
};

/// AVF relocates the aarch64 PCI windows for a partner hypervisor; keep the same layout.
const AARCH64_PCI_WINDOWS: &str =
    "mem=[start=0x2c000000,size=0x2000000],cam=[start=0x2e000000,size=0x1000000]";

/// Inherited descriptors for one session. crosvm opens them as `/proc/self/fd/N`, exactly as
/// virtmgr passes its preserved descriptors, so no socket path is ever guessable or shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GunyahSessionFds {
    pub control: i32,
    pub touch: i32,
    pub keyboard: i32,
    pub mouse: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GunyahLaunchPlan {
    pub invocation: ProgramInvocation,
    pub performance: GuestPerformancePlan,
    /// Name under which crosvm registers its Android display service with virtualizationservice;
    /// the host integration attaches the session `Surface` to it.
    pub display_service_name: String,
}

#[derive(Debug, Clone)]
pub struct GunyahCrosvmBackend {
    crosvm: PathBuf,
    gunyah_device: PathBuf,
    host: Option<HostResources>,
    profile: PerformanceProfile,
}

impl GunyahCrosvmBackend {
    pub fn new(crosvm: PathBuf, gunyah_device: PathBuf) -> Self {
        Self {
            crosvm,
            gunyah_device,
            host: None,
            profile: PerformanceProfile::default(),
        }
    }

    pub fn with_host_resources(mut self, host: HostResources) -> Self {
        self.host = Some(host);
        self
    }

    pub fn with_performance_profile(mut self, profile: PerformanceProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn build_launch_plan(
        &self,
        spec: &ValidatedVmSpec,
        graphics: &AvfGraphicsConfig,
        fds: GunyahSessionFds,
    ) -> Result<GunyahLaunchPlan, BackendError> {
        if !is_plain_absolute(&self.crosvm) || !is_plain_absolute(&self.gunyah_device) {
            return Err(BackendError::UnsafeRuntimeDirectory);
        }
        let all_fds = [fds.control, fds.touch, fds.keyboard, fds.mouse];
        if all_fds.iter().any(|fd| *fd < 3)
            || (1..all_fds.len()).any(|i| all_fds[..i].contains(&all_fds[i]))
        {
            return Err(BackendError::InvalidSessionDescriptor);
        }
        let performance = GuestPerformancePlan::build(
            self.profile,
            self.host.as_ref(),
            spec.vcpus,
            CpuControl::PerVcpuAffinity,
        )
        .map_err(BackendError::Placement)?;

        let kernel = utf8(&spec.kernel)?;
        let initrd = utf8(&spec.initrd)?;
        let gunyah_device = utf8(&self.gunyah_device)?;
        if gunyah_device.contains([',', '[', ']', '=']) {
            return Err(BackendError::UnsafeRuntimeDirectory);
        }
        let display_service_name = format!("carbonhv-{}-{}", spec.instance_id, spec.generation.0);

        let mut args: Vec<String> = vec![
            "run".into(),
            "--hypervisor".into(),
            format!("gunyah[device={gunyah_device}]"),
            "--name".into(),
            display_service_name.clone(),
            "--disable-sandbox".into(),
            "--no-usb".into(),
            "--no-pmu".into(),
            "--pci".into(),
            AARCH64_PCI_WINDOWS.into(),
            "--socket".into(),
            proc_fd(fds.control),
            "--mem".into(),
            spec.memory_mib.to_string(),
        ];
        args.extend(performance.crosvm_args());

        args.push("--initrd".into());
        args.push(initrd);
        for disk in &spec.disks {
            let path = utf8(&disk.path)?;
            // crosvm's key=value parser treats ',' as a separator; managed IDs never contain it.
            if path.contains(',') {
                return Err(BackendError::UnsafeRuntimeDirectory);
            }
            args.push("--block".into());
            args.push(format!(
                "path={path},ro={},lock=true",
                if disk.writable { "false" } else { "true" }
            ));
        }
        let mut params = spec.kernel_params.clone();
        if !params.iter().any(|param| param == "gfxstream_enabled") {
            params.push("gfxstream_enabled".into());
        }
        for param in params {
            args.push("--params".into());
            args.push(param);
        }

        args.push(format!(
            "--gpu=backend=gfxstream,context-types={},renderer-features={},surfaceless=true,vulkan=true",
            GFXSTREAM_CONTEXT_TYPES.join(":"),
            GFXSTREAM_RENDERER_FEATURES
        ));
        args.push(format!(
            "--gpu-display=mode=windowed[{},{}],dpi=[{},{}],refresh-rate={}",
            graphics.width_pixels(),
            graphics.height_pixels(),
            graphics.density_dpi(),
            graphics.density_dpi(),
            graphics.refresh_rate()
        ));
        args.push(format!("--android-display-service={display_service_name}"));
        args.push("--input".into());
        args.push(format!(
            "multi-touch[path={},width={},height={}]",
            proc_fd(fds.touch),
            graphics.width_pixels(),
            graphics.height_pixels()
        ));
        args.push("--input".into());
        args.push(format!("keyboard[path={}]", proc_fd(fds.keyboard)));
        args.push("--input".into());
        args.push(format!("mouse[path={}]", proc_fd(fds.mouse)));
        args.push(kernel);

        Ok(GunyahLaunchPlan {
            invocation: ProgramInvocation {
                program: self.crosvm.clone(),
                args,
            },
            performance,
            display_service_name,
        })
    }
}

fn proc_fd(fd: i32) -> String {
    format!("/proc/self/fd/{fd}")
}

fn utf8(path: &Path) -> Result<String, BackendError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or(BackendError::NonUtf8Path)
}

fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}
