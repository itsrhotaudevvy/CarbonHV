//! Placement of TCG vCPU threads on the host's performance cores.
//!
//! QEMU cannot pin its own threads, so the `:vm` service applies the returned mask after spawn by
//! matching QEMU's vCPU thread names (`CPU 0/TCG`, `CPU 1/TCG`, ...). Keeping the decision here
//! makes the placement testable on a laptop and identical to the probe the broker already uses
//! for crosvm (`crates/carbon-broker/src/host_resources.rs` classifies cores by `MIDR_EL1`).

use crate::{EmuHostFacts, EmuPlanError, EmuProfile};

/// Most vCPUs the engine will run: the reference phone's four Cortex-A78 cores.
pub const MAX_VCPUS: u16 = 4;
/// `uclamp.min` value that marks a thread as maximum-capacity (`SCHED_CAPACITY_SCALE`).
pub const FULL_CAPACITY_UCLAMP: u16 = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmuPlacement {
    pub vcpu_count: u16,
    /// One host CPU per guest vCPU, in vCPU order.
    pub vcpu_cores: Vec<u32>,
    /// Performance cores deliberately left to QEMU's I/O thread, gfxstream and the app UI.
    pub spare_cores: Vec<u32>,
    /// Whether the service may apply `vcpu_cores` with `sched_setaffinity`.
    pub affinity_applied: bool,
    /// `uclamp.min` to request for vCPU threads while the session is foreground.
    pub uclamp_min: Option<u16>,
}

impl EmuPlacement {
    pub fn describe(&self) -> String {
        format!(
            "{} vCPU thread(s) on CPUs [{}], spare [{}], affinity={}, uclamp={}",
            self.vcpu_count,
            self.vcpu_cores
                .iter()
                .map(|core| core.to_string())
                .collect::<Vec<_>>()
                .join(","),
            self.spare_cores
                .iter()
                .map(|core| core.to_string())
                .collect::<Vec<_>>()
                .join(","),
            if self.affinity_applied {
                "on"
            } else {
                "blocked"
            },
            match self.uclamp_min {
                Some(value) => value.to_string(),
                None => "off".into(),
            }
        )
    }
}

/// Plans vCPU placement.
///
/// The vCPU count is `min(requested or MAX_VCPUS, performance cores, profile cap)`. The engine
/// never silently moves a vCPU thread onto an in-order efficiency core: if no performance core is
/// available it refuses, because that would be slower than fewer vCPUs.
pub fn plan(
    host: &EmuHostFacts,
    requested: u16,
    profile: EmuProfile,
) -> Result<EmuPlacement, EmuPlanError> {
    if host.big_cores.is_empty() {
        return Err(EmuPlanError::NoPerformanceCores);
    }
    let cap = match profile {
        // One core back for the service, gfxstream and the UI.
        EmuProfile::Balanced | EmuProfile::Hardened => MAX_VCPUS,
        EmuProfile::LowMemory => MAX_VCPUS - 1,
    };
    let available = host.big_cores.len().min(usize::from(u16::MAX)) as u16;
    let requested = if requested == 0 { cap } else { requested };
    let vcpu_count = requested.min(available).min(cap).max(1);
    let vcpu_cores = host
        .big_cores
        .iter()
        .take(usize::from(vcpu_count))
        .copied()
        .collect();
    let spare_cores = host
        .big_cores
        .iter()
        .skip(usize::from(vcpu_count))
        .copied()
        .collect();
    Ok(EmuPlacement {
        vcpu_count,
        vcpu_cores,
        spare_cores,
        affinity_applied: host.affinity_allowed,
        uclamp_min: match (host.uclamp_allowed, profile) {
            (false, _) | (true, EmuProfile::LowMemory) => None,
            (true, _) => Some(FULL_CAPACITY_UCLAMP),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VulkanFacts, WxMode};

    fn host(cores: Vec<u32>) -> EmuHostFacts {
        EmuHostFacts {
            mem_available_mib: 4096,
            big_cores: cores,
            w_xor_x: WxMode::SplitWx,
            o_direct: true,
            vulkan: VulkanFacts {
                available: true,
                hardware_buffer_export: true,
                device_name: None,
            },
            affinity_allowed: true,
            uclamp_allowed: true,
            source: None,
        }
    }

    #[test]
    fn pins_one_vcpu_per_big_core_and_keeps_the_rest_spare() {
        let placement = plan(&host(vec![4, 5, 6, 7]), 0, EmuProfile::Balanced).unwrap();
        assert_eq!(placement.vcpu_count, 4);
        assert_eq!(placement.vcpu_cores, vec![4, 5, 6, 7]);
        assert!(placement.spare_cores.is_empty());
        assert_eq!(placement.uclamp_min, Some(FULL_CAPACITY_UCLAMP));
    }

    #[test]
    fn low_memory_profile_keeps_one_core_for_the_service() {
        let placement = plan(&host(vec![4, 5, 6, 7]), 0, EmuProfile::LowMemory).unwrap();
        assert_eq!(placement.vcpu_count, 3);
        assert_eq!(placement.spare_cores, vec![7]);
        assert_eq!(placement.uclamp_min, None);
    }

    #[test]
    fn request_above_the_cores_is_clamped_not_errored() {
        let placement = plan(&host(vec![2, 3]), 8, EmuProfile::Balanced).unwrap();
        assert_eq!(placement.vcpu_count, 2);
    }

    #[test]
    fn refuses_to_run_without_a_performance_core() {
        assert_eq!(
            plan(&host(Vec::new()), 4, EmuProfile::Balanced).unwrap_err(),
            EmuPlanError::NoPerformanceCores
        );
    }
}
