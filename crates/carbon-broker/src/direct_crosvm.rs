//! Direct crosvm launch planning for every aarch64 hypervisor crosvm supports on phones.
//!
//! AVF's raw config can only ask for a vCPU count. Launching crosvm directly lets CarbonHV pin
//! each vCPU to one performance core, mirror capacities and frequency domains, and forward guest
//! DVFS, on any of:
//!
//! | Hypervisor | SoC family | Device | Guest SVE / MTE / PMU (crosvm 88c1385) |
//! | --- | --- | --- | --- |
//! | KVM / pKVM | Google Tensor, AVF reference devices | `/dev/kvm` | auto / off / off |
//! | GenieZone | MediaTek Dimensity | `/dev/gzvm` | none / raw cap (left off) / none |
//! | Halla | Samsung Exynos | `/dev/halla` | none / raw cap (left off) / none |
//! | Gunyah | Qualcomm Snapdragon | `/dev/gunyah` | none / none / none |
//!
//! The hypervisor is always named explicitly so a host exposing more than one device can never
//! silently switch backends, and a protected VM is never requested. Unprotected VMs share guest
//! memory directly (no swiotlb bounce buffers) on every backend, so virtio and gfxstream blob
//! memory stay zero-copy. MTE stays off: an MTE-enabled guest cannot map the non-MTE gfxstream
//! host-visible blobs that make presentation zero-copy; enabling it is a measured security
//! decision for a later slice, not a default.

use std::path::{Component, Path, PathBuf};

use crate::avf::{GFXSTREAM_CONTEXT_TYPES, GFXSTREAM_RENDERER_FEATURES};
use crate::{
    AvfGraphicsConfig, BackendError, CpuControl, GuestPerformancePlan, HostResources,
    HypervisorKind, PerformanceProfile, ProgramInvocation, ThpMode, ValidatedVmSpec,
};

/// AVF relocates the aarch64 PCI windows on every aarch64 host for a partner hypervisor; keep
/// the same guest-physical layout on every backend so guests see one CVD1 memory map.
const AARCH64_PCI_WINDOWS: &str =
    "mem=[start=0x2c000000,size=0x2000000],cam=[start=0x2e000000,size=0x1000000]";

/// Inherited descriptors for one session. crosvm opens them as `/proc/self/fd/N`, exactly as
/// virtmgr passes its preserved descriptors, so no socket path is ever guessable or shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectSessionFds {
    pub control: i32,
    pub touch: i32,
    pub keyboard: i32,
    pub mouse: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLaunchPlan {
    pub invocation: ProgramInvocation,
    pub performance: GuestPerformancePlan,
    /// Name under which crosvm registers its Android display service with virtualizationservice;
    /// the host integration attaches the session `Surface` to it.
    pub display_service_name: String,
}

#[derive(Debug, Clone)]
pub struct DirectCrosvmBackend {
    crosvm: PathBuf,
    hypervisor: HypervisorKind,
    hypervisor_device: PathBuf,
    host: Option<HostResources>,
    profile: PerformanceProfile,
    thp: ThpMode,
}

impl DirectCrosvmBackend {
    pub fn new(crosvm: PathBuf, hypervisor: HypervisorKind, hypervisor_device: PathBuf) -> Self {
        Self {
            crosvm,
            hypervisor,
            hypervisor_device,
            host: None,
            profile: PerformanceProfile::default(),
            thp: ThpMode::default(),
        }
    }

    /// The backend's default device node, e.g. `/dev/gzvm` for GenieZone.
    pub fn for_hypervisor(crosvm: PathBuf, hypervisor: HypervisorKind) -> Self {
        let device = hypervisor.default_device().unwrap_or_default();
        Self::new(crosvm, hypervisor, PathBuf::from(device))
    }

    pub fn hypervisor(&self) -> HypervisorKind {
        self.hypervisor
    }

    pub fn with_host_resources(mut self, host: HostResources) -> Self {
        self.host = Some(host);
        self
    }

    pub fn with_performance_profile(mut self, profile: PerformanceProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_host_thp(mut self, thp: ThpMode) -> Self {
        self.thp = thp;
        self
    }

    pub fn build_launch_plan(
        &self,
        spec: &ValidatedVmSpec,
        graphics: &AvfGraphicsConfig,
        fds: DirectSessionFds,
    ) -> Result<DirectLaunchPlan, BackendError> {
        let Some(hypervisor_name) = self.hypervisor.crosvm_name() else {
            return Err(BackendError::NoHypervisor);
        };
        if !is_plain_absolute(&self.crosvm) || !is_plain_absolute(&self.hypervisor_device) {
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
        .map_err(BackendError::Placement)?
        .for_host_thp(self.thp);

        let kernel = utf8(&spec.kernel)?;
        let initrd = utf8(&spec.initrd)?;
        let hypervisor_device = utf8(&self.hypervisor_device)?;
        if hypervisor_device.contains([',', '[', ']', '=']) {
            return Err(BackendError::UnsafeRuntimeDirectory);
        }
        let display_service_name = format!("carbonhv-{}-{}", spec.instance_id, spec.generation.0);

        let mut args: Vec<String> = vec![
            "run".into(),
            "--hypervisor".into(),
            format!("{hypervisor_name}[device={hypervisor_device}]"),
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

        Ok(DirectLaunchPlan {
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
