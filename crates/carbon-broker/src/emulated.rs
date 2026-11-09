//! The emulated engine's broker adapter.
//!
//! `carbon-emu` owns the QEMU policy; this module only translates a validated CVD1 spec into the
//! pure crate's input and refuses anything the broker would not have validated itself. Keeping
//! the adapter this thin is what lets `carbonctl emu plan` print the exact argv the phone runs.

use std::path::{Component, Path, PathBuf};

use carbon_core::{EmulationAccelerator, ExecutionEngine};
use carbon_emu::{
    EmuDisplay, EmuHostFacts, EmuNetwork, EmuPlan, EmuProfile, EmuSessionFds, EmuVmSpec,
    PresentationMode,
};

use crate::{AvfGraphicsConfig, BackendError, ValidatedVmSpec};

/// Inherited descriptors for one emulated session. QEMU receives numbers only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmulatedSessionFds {
    pub qmp: i32,
    pub frame: i32,
    pub input: i32,
    /// One descriptor per `spec.disks` entry, in the same order.
    pub disks: Vec<i32>,
    pub vsock: Option<i32>,
    pub console: Option<i32>,
}

/// Per-instance choices that are not part of the CVD1 spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmulatedLaunchOptions {
    pub profile: EmuProfile,
    pub presentation: PresentationMode,
    pub network: EmuNetwork,
    pub audio: bool,
}

impl Default for EmulatedLaunchOptions {
    fn default() -> Self {
        Self {
            profile: EmuProfile::Balanced,
            presentation: PresentationMode::ZeroCopy,
            network: EmuNetwork::Nat,
            audio: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmulatedLaunchPlan {
    pub plan: EmuPlan,
    pub engine: ExecutionEngine,
}

#[derive(Debug, Clone)]
pub struct EmulatedBackend {
    qemu: PathBuf,
}

impl EmulatedBackend {
    /// `qemu` must be the packaged executable in the app's `nativeLibraryDir`.
    pub fn new(qemu: PathBuf) -> Self {
        Self { qemu }
    }

    /// The only engine this adapter can return.
    pub fn engine() -> ExecutionEngine {
        ExecutionEngine::Emulated(EmulationAccelerator::Tcg)
    }

    pub fn build_launch_plan(
        &self,
        spec: &ValidatedVmSpec,
        graphics: &AvfGraphicsConfig,
        facts: &EmuHostFacts,
        options: EmulatedLaunchOptions,
        fds: &EmulatedSessionFds,
    ) -> Result<EmulatedLaunchPlan, BackendError> {
        if spec.disks.len() != fds.disks.len() {
            return Err(BackendError::DiskDescriptorCountMismatch {
                disks: spec.disks.len(),
                descriptors: fds.disks.len(),
            });
        }
        // The pure crate validates these again; doing it here keeps the error attributable to the
        // broker's own path checks.
        for path in [&spec.kernel, &spec.initrd] {
            if !is_plain_absolute(path) {
                return Err(BackendError::UnsafeRuntimeDirectory);
            }
        }
        for disk in &spec.disks {
            if !is_plain_absolute(&disk.path) {
                return Err(BackendError::UnsafeRuntimeDirectory);
            }
        }
        let session = EmuSessionFds {
            qmp: fds.qmp,
            frame: fds.frame,
            input: fds.input,
            vsock: fds.vsock,
            console: fds.console,
        };
        let emu_spec = EmuVmSpec {
            instance_id: spec.instance_id.clone(),
            generation: spec.generation.0,
            vcpus: spec.vcpus,
            memory_mib: spec.memory_mib,
            kernel: spec.kernel.clone(),
            initrd: spec.initrd.clone(),
            kernel_params: spec.kernel_params.clone(),
            disks: spec
                .disks
                .iter()
                .zip(&fds.disks)
                .map(|(disk, fd)| carbon_emu::EmuDisk {
                    fd: *fd,
                    writable: disk.writable,
                })
                .collect(),
            display: EmuDisplay {
                width_pixels: graphics.width_pixels(),
                height_pixels: graphics.height_pixels(),
                density_dpi: graphics.density_dpi(),
                refresh_rate: graphics.refresh_rate(),
            },
            network: options.network,
            audio: options.audio,
            presentation: options.presentation,
            profile: options.profile,
        };
        let plan = carbon_emu::plan(self.qemu.clone(), &emu_spec, facts, &session)
            .map_err(BackendError::Emulation)?;
        Ok(EmulatedLaunchPlan {
            plan,
            engine: Self::engine(),
        })
    }
}

fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use carbon_core::Generation;
    use carbon_emu::{VulkanFacts, WxMode};

    fn spec() -> ValidatedVmSpec {
        ValidatedVmSpec {
            request_id: 1,
            instance_id: "cvd1-main".into(),
            generation: Generation(2),
            kernel: PathBuf::from("/data/misc/carbonhv/objects/kernel"),
            initrd: PathBuf::from("/data/misc/carbonhv/objects/initrd"),
            disks: vec![
                crate::ValidatedVmDisk {
                    path: PathBuf::from("/data/misc/carbonhv/objects/system"),
                    writable: false,
                },
                crate::ValidatedVmDisk {
                    path: PathBuf::from("/data/misc/carbonhv/objects/userdata"),
                    writable: true,
                },
            ],
            memory_mib: 2048,
            vcpus: 4,
            kernel_params: vec!["console=hvc0".into()],
        }
    }

    fn graphics() -> AvfGraphicsConfig {
        AvfGraphicsConfig::gfxstream_vulkan(1080, 2392, 420, 60).unwrap()
    }

    fn facts() -> EmuHostFacts {
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

    #[test]
    fn builds_an_emulated_plan_labelled_emulated() {
        let backend = EmulatedBackend::new(PathBuf::from("/data/app/libqemu_system_aarch64.so"));
        let fds = EmulatedSessionFds {
            qmp: 3,
            frame: 4,
            input: 5,
            disks: vec![10, 11],
            vsock: Some(6),
            console: Some(7),
        };
        let plan = backend
            .build_launch_plan(
                &spec(),
                &graphics(),
                &facts(),
                EmulatedLaunchOptions::default(),
                &fds,
            )
            .unwrap();
        assert_eq!(plan.engine.label(), "Emulated");
        assert!(plan.engine.is_emulated());
        assert_eq!(plan.plan.memory.guest_mib, 2048);
        assert!(plan
            .plan
            .invocation
            .args
            .contains(&"file=/dev/fdset/0,if=none,id=drive0,format=raw,aio=threads,cache=none,readonly=on".to_string()));
    }

    #[test]
    fn refuses_a_disk_without_a_descriptor() {
        let backend = EmulatedBackend::new(PathBuf::from("/data/app/libqemu_system_aarch64.so"));
        let fds = EmulatedSessionFds {
            qmp: 3,
            frame: 4,
            input: 5,
            disks: vec![10],
            vsock: None,
            console: None,
        };
        assert_eq!(
            backend
                .build_launch_plan(
                    &spec(),
                    &graphics(),
                    &facts(),
                    EmulatedLaunchOptions::default(),
                    &fds,
                )
                .unwrap_err(),
            BackendError::DiskDescriptorCountMismatch {
                disks: 2,
                descriptors: 1
            }
        );
    }
}
