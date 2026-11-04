use std::path::{Component, PathBuf};

use crate::{
    BackendError, CpuControl, CpuFrequencyControl, GuestPerformancePlan, HostProbe,
    HostVirtualizationClass, HypervisorKind, HypervisorProperties, PerformanceProfile,
    ProgramInvocation, ThpMode, ValidatedVmSpec,
};

pub(crate) const GFXSTREAM_CONTEXT_TYPES: [&str; 2] = ["gfxstream-vulkan", "gfxstream-composer"];
pub(crate) const GFXSTREAM_RENDERER_FEATURES: &str = "VulkanDisableCoherentMemoryAndEmulate:enabled;VulkanAllocateHostVisibleAsUdmabuf:enabled;ExternalBlob:enabled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfGraphicsConfig {
    width_pixels: u32,
    height_pixels: u32,
    density_dpi: u32,
    refresh_rate: u32,
}

impl AvfGraphicsConfig {
    pub fn gfxstream_vulkan(
        width_pixels: u32,
        height_pixels: u32,
        density_dpi: u32,
        refresh_rate: u32,
    ) -> Result<Self, BackendError> {
        if width_pixels == 0
            || height_pixels == 0
            || density_dpi == 0
            || refresh_rate == 0
            || width_pixels > 16_384
            || height_pixels > 16_384
            || density_dpi > 10_000
            || refresh_rate > 1_000
        {
            return Err(BackendError::InvalidGraphicsConfig);
        }
        Ok(Self {
            width_pixels,
            height_pixels,
            density_dpi,
            refresh_rate,
        })
    }

    pub fn width_pixels(&self) -> u32 {
        self.width_pixels
    }

    pub fn height_pixels(&self) -> u32 {
        self.height_pixels
    }

    pub fn density_dpi(&self) -> u32 {
        self.density_dpi
    }

    pub fn refresh_rate(&self) -> u32 {
        self.refresh_rate
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfGpuConfig {
    pub backend: &'static str,
    pub renderer_use_egl: bool,
    pub renderer_use_gles: bool,
    pub renderer_use_glx: bool,
    pub renderer_use_surfaceless: bool,
    pub renderer_use_vulkan: bool,
    pub context_types: Vec<&'static str>,
    pub renderer_features: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfDisplayConfig {
    pub width_pixels: u32,
    pub height_pixels: u32,
    pub density_dpi: u32,
    pub refresh_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfInputConfig {
    pub multitouch: bool,
    pub keyboard: bool,
    pub mouse: bool,
}

/// The AVF `VirtualMachineRawConfig` performance fields CarbonHV controls. AVF cannot pin vCPUs,
/// so near-native sessions use `boostUclamp` to keep vCPU threads on performance cores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvfPerformanceConfig {
    pub huge_pages: bool,
    pub boost_uclamp: bool,
    pub balloon: bool,
}

impl AvfPerformanceConfig {
    pub fn from_plan(plan: &GuestPerformancePlan) -> Self {
        Self {
            huge_pages: plan.hugepages,
            boost_uclamp: plan.frequency == CpuFrequencyControl::BoostUclamp,
            balloon: plan.balloon_page_reporting,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfPlatformVmPlan {
    pub platform_version: &'static str,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub disks: Vec<crate::ValidatedVmDisk>,
    pub memory_mib: u64,
    pub vcpus: u16,
    pub kernel_params: Vec<String>,
    pub gpu: AvfGpuConfig,
    pub display: AvfDisplayConfig,
    pub input: AvfInputConfig,
    pub performance: AvfPerformanceConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfLaunchPlan {
    pub config_path: PathBuf,
    pub config_json: String,
    pub invocation: ProgramInvocation,
}

#[derive(Debug, Clone)]
pub struct AvfBackend {
    vm_tool: PathBuf,
    kvm_device: PathBuf,
    vendor_devices: Vec<(HypervisorKind, PathBuf)>,
    profile: PerformanceProfile,
    thp: ThpMode,
}

impl AvfBackend {
    pub fn new(vm_tool: PathBuf, kvm_device: PathBuf) -> Self {
        Self {
            vm_tool,
            kvm_device,
            vendor_devices: Vec::new(),
            profile: PerformanceProfile::default(),
            thp: ThpMode::default(),
        }
    }

    pub fn with_performance_profile(mut self, profile: PerformanceProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_host_thp(mut self, thp: ThpMode) -> Self {
        self.thp = thp;
        self
    }

    /// Lets the probe recognise a vendor hypervisor device (Gunyah, GenieZone, Halla) so such
    /// hosts are reported as such instead of as a generic missing-KVM failure.
    pub fn with_vendor_hypervisor_device(mut self, kind: HypervisorKind, device: PathBuf) -> Self {
        if kind != HypervisorKind::Kvm && kind != HypervisorKind::None {
            self.vendor_devices
                .retain(|(existing, _)| *existing != kind);
            self.vendor_devices.push((kind, device));
        }
        self
    }

    pub fn with_gunyah_device(self, device: PathBuf) -> Self {
        self.with_vendor_hypervisor_device(HypervisorKind::Gunyah, device)
    }

    /// Probe without hypervisor system properties (non-Android hosts and tests).
    pub fn probe(&self) -> HostProbe {
        self.probe_with_properties(&HypervisorProperties::default())
    }

    /// Classifies the host. A vendor-hypervisor host is ready for AVF only when its build
    /// declares non-protected VM support (`ro.boot.hypervisor.vm.supported=1`); otherwise
    /// virtualizationservice refuses every VM, as observed on stock Snapdragon firmware.
    pub fn probe_with_properties(&self, properties: &HypervisorProperties) -> HostProbe {
        let vm_tool_present = self.vm_tool.is_file();
        let kvm_present = self.kvm_device.exists();
        // Mirror crosvm's default-hypervisor probe: /dev/kvm, then gzvm, halla, gunyah.
        let hypervisor = if kvm_present {
            HypervisorKind::Kvm
        } else {
            HypervisorKind::VENDOR_PROBE_ORDER
                .into_iter()
                .find(|kind| {
                    self.vendor_devices
                        .iter()
                        .any(|(candidate, device)| candidate == kind && device.exists())
                })
                .unwrap_or(HypervisorKind::None)
        };
        let class = match hypervisor {
            HypervisorKind::Kvm if vm_tool_present => HostVirtualizationClass::DevelopmentReady,
            HypervisorKind::Kvm | HypervisorKind::None => HostVirtualizationClass::Unavailable,
            vendor
                if vm_tool_present
                    && properties.vm_supported == Some(true)
                    && properties.consistent_with(vendor) =>
            {
                HostVirtualizationClass::VendorHypervisorAvfReady
            }
            _ => HostVirtualizationClass::VendorHypervisorWithoutAvf,
        };
        HostProbe {
            vm_tool_present,
            kvm_present,
            hypervisor,
            class,
        }
    }

    pub fn build_launch_plan(
        &self,
        spec: &ValidatedVmSpec,
        runtime_dir: PathBuf,
    ) -> Result<AvfLaunchPlan, BackendError> {
        if runtime_dir
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(BackendError::UnsafeRuntimeDirectory);
        }
        if spec.vcpus != 1 {
            return Err(BackendError::UnsupportedCliVcpuCount {
                requested: spec.vcpus,
            });
        }

        let config_path =
            runtime_dir.join(format!("{}-{}.json", spec.instance_id, spec.generation.0));
        let params = spec.kernel_params.join(" ");
        let mut config_json = String::from("{");
        push_json_field(
            &mut config_json,
            "kernel",
            &spec.kernel.to_string_lossy(),
            true,
        );
        push_json_field(
            &mut config_json,
            "initrd",
            &spec.initrd.to_string_lossy(),
            false,
        );
        push_json_field(&mut config_json, "params", &params, false);
        config_json.push_str(",\"memory_mib\":");
        config_json.push_str(&spec.memory_mib.to_string());
        push_json_field(&mut config_json, "cpu_topology", "one_cpu", false);
        push_json_field(&mut config_json, "platform_version", "~1.0", false);
        config_json.push_str(",\"disks\":[");
        for (index, disk) in spec.disks.iter().enumerate() {
            if index != 0 {
                config_json.push(',');
            }
            config_json.push_str("{\"image\":\"");
            config_json.push_str(&json_escape(&disk.path.to_string_lossy()));
            config_json.push_str("\",\"writable\":");
            config_json.push_str(if disk.writable { "true" } else { "false" });
            config_json.push_str(",\"partitions\":[]}");
        }
        config_json.push(']');
        config_json.push('}');

        Ok(AvfLaunchPlan {
            config_path: config_path.clone(),
            config_json,
            invocation: ProgramInvocation {
                program: self.vm_tool.clone(),
                args: vec!["run".into(), config_path.to_string_lossy().into_owned()],
            },
        })
    }

    pub fn build_graphics_platform_plan(
        &self,
        spec: &ValidatedVmSpec,
        graphics: AvfGraphicsConfig,
    ) -> Result<AvfPlatformVmPlan, BackendError> {
        let mut kernel_params = spec.kernel_params.clone();
        if !kernel_params
            .iter()
            .any(|param| param == "gfxstream_enabled")
        {
            kernel_params.push("gfxstream_enabled".into());
        }

        let performance =
            GuestPerformancePlan::build(self.profile, None, spec.vcpus, CpuControl::CountOnly)
                .map_err(BackendError::Placement)?
                .for_host_thp(self.thp);

        Ok(AvfPlatformVmPlan {
            platform_version: "~1.0",
            kernel: spec.kernel.clone(),
            initrd: spec.initrd.clone(),
            disks: spec.disks.clone(),
            memory_mib: spec.memory_mib,
            vcpus: spec.vcpus,
            kernel_params,
            gpu: AvfGpuConfig {
                backend: "gfxstream",
                renderer_use_egl: false,
                renderer_use_gles: false,
                renderer_use_glx: false,
                renderer_use_surfaceless: true,
                renderer_use_vulkan: true,
                context_types: GFXSTREAM_CONTEXT_TYPES.to_vec(),
                renderer_features: GFXSTREAM_RENDERER_FEATURES,
            },
            display: AvfDisplayConfig {
                width_pixels: graphics.width_pixels,
                height_pixels: graphics.height_pixels,
                density_dpi: graphics.density_dpi,
                refresh_rate: graphics.refresh_rate,
            },
            input: AvfInputConfig {
                multitouch: true,
                keyboard: true,
                mouse: true,
            },
            performance: AvfPerformanceConfig::from_plan(&performance),
        })
    }
}

fn push_json_field(output: &mut String, key: &str, value: &str, first: bool) {
    if !first {
        output.push(',');
    }
    output.push('"');
    output.push_str(key);
    output.push_str("\":\"");
    output.push_str(&json_escape(value));
    output.push('"');
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}
