//! The memory budget for an emulated guest, and the admission rule that guards it.
//!
//! `MemAvailable` on the reference phone is about 2.7 GiB, and Android's LMK is the real
//! deadline. The budget therefore counts every resident component we control (guest RAM, TCG
//! translation cache, gfxstream host window, QEMU and the `:vm` service) and refuses to start
//! unless the phone can hold the guest *plus* a fixed reserve that LMK cannot be expected to
//! respect.

use crate::{EmuHostFacts, EmuPlanError, EmuProfile, PresentationMode};

/// Guest RAM when the caller does not ask for a size (the bundle's 4 GiB is a hardware-VM value).
pub const DEFAULT_GUEST_MIB: u64 = 2048;
/// Largest guest RAM the emulated engine will plan for, whatever the bundle says.
pub const MAX_GUEST_MIB: u64 = 3072;
/// Reserve the phone must still have free after the guest is resident.
pub const HOST_RESERVE_MIB: u64 = 768;
/// QEMU, glib, slirp, device state and the gfxstream host, resident.
const EMULATOR_OVERHEAD_MIB: u64 = 150;
/// The `:vm` service, its JNI library and Binder plumbing.
const SERVICE_OVERHEAD_MIB: u64 = 60;

/// The TCG translation cache and gfxstream host window chosen by a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudget {
    pub guest_mib: u64,
    pub tb_size_mib: u64,
    pub gfxstream_hostmem_mib: u64,
    pub emulator_overhead_mib: u64,
    pub service_overhead_mib: u64,
    /// Total resident estimate, for diagnostics.
    pub footprint_mib: u64,
    /// `MemAvailable` the phone must report before a start is allowed.
    pub required_available_mib: u64,
}

impl MemoryBudget {
    pub fn describe(&self) -> String {
        format!(
            "guest {} MiB + TCG cache {} MiB + gfxstream {} MiB + emulator {} MiB + service {} MiB = ~{} MiB resident; needs {} MiB free",
            self.guest_mib,
            self.tb_size_mib,
            self.gfxstream_hostmem_mib,
            self.emulator_overhead_mib,
            self.service_overhead_mib,
            self.footprint_mib,
            self.required_available_mib
        )
    }
}

/// Guest RAM for a request: the profile default when `requested_mib` is zero.
///
/// `LowMemory` deliberately starts below the `Balanced` default. On the reference phone the
/// measured `MemAvailable` (2.75 GiB) cannot hold a 2 GiB guest plus the 768 MiB reserve, so the
/// honest default for that reading is the low-memory profile, not a silently clamped guest.
pub fn guest_memory_mib(profile: EmuProfile, requested_mib: u64) -> u64 {
    if requested_mib != 0 {
        return requested_mib;
    }
    match profile {
        EmuProfile::Balanced | EmuProfile::Hardened => DEFAULT_GUEST_MIB,
        EmuProfile::LowMemory => 1536,
    }
}

pub fn budget(profile: EmuProfile, presentation: PresentationMode) -> MemoryBudget {
    let (tb_size_mib, gfxstream_hostmem_mib) = match profile {
        // The guest's code footprint is large; 256 MiB keeps the translation cache warm.
        EmuProfile::Balanced | EmuProfile::Hardened => (256, 512),
        EmuProfile::LowMemory => (128, 256),
    };
    // Readback presentation does not need gfxstream's host-visible window for scanout, but the
    // guest GPU still runs on gfxstream, so the window is kept; only the reservoir shrinks.
    let gfxstream_hostmem_mib = match presentation {
        PresentationMode::ZeroCopy => gfxstream_hostmem_mib,
        PresentationMode::Readback => gfxstream_hostmem_mib / 2,
    };
    budget_with(0, tb_size_mib, gfxstream_hostmem_mib)
}

fn budget_with(guest_mib: u64, tb_size_mib: u64, gfxstream_hostmem_mib: u64) -> MemoryBudget {
    let footprint_mib = guest_mib
        + tb_size_mib
        + gfxstream_hostmem_mib
        + EMULATOR_OVERHEAD_MIB
        + SERVICE_OVERHEAD_MIB;
    MemoryBudget {
        guest_mib,
        tb_size_mib,
        gfxstream_hostmem_mib,
        emulator_overhead_mib: EMULATOR_OVERHEAD_MIB,
        service_overhead_mib: SERVICE_OVERHEAD_MIB,
        footprint_mib,
        required_available_mib: guest_mib + HOST_RESERVE_MIB,
    }
}

/// Computes and admits the budget for a request.
pub fn admit(
    host: &EmuHostFacts,
    requested_mib: u64,
    profile: EmuProfile,
    presentation: PresentationMode,
) -> Result<MemoryBudget, EmuPlanError> {
    let guest_mib = guest_memory_mib(profile, requested_mib);
    if guest_mib > MAX_GUEST_MIB {
        return Err(EmuPlanError::GuestMemoryExceedsEmulatedMaximum {
            requested_mib: guest_mib,
            maximum_mib: MAX_GUEST_MIB,
        });
    }
    let base = budget(profile, presentation);
    let budget = budget_with(guest_mib, base.tb_size_mib, base.gfxstream_hostmem_mib);
    if host.mem_available_mib < budget.required_available_mib {
        return Err(EmuPlanError::InsufficientMemory {
            required_mib: budget.required_available_mib,
            available_mib: host.mem_available_mib,
        });
    }
    Ok(budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VulkanFacts;

    fn host(mem_available_mib: u64) -> EmuHostFacts {
        EmuHostFacts {
            mem_available_mib,
            big_cores: vec![4, 5, 6, 7],
            w_xor_x: crate::WxMode::SplitWx,
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
    fn default_two_gib_guest_reserves_768_mib() {
        let budget = admit(
            &host(3584),
            0,
            EmuProfile::Balanced,
            PresentationMode::ZeroCopy,
        )
        .unwrap();
        assert_eq!(budget.guest_mib, 2048);
        assert_eq!(budget.required_available_mib, 2816);
        assert_eq!(budget.footprint_mib, 3026);
    }

    #[test]
    fn reference_phone_reading_refuses_the_balanced_default() {
        // 2026-11-04 probe: MemAvailable 2750 MiB, i.e. below 2048 + 768. The engine must refuse
        // with numbers the setup screen can show rather than let LMK decide.
        let error = admit(
            &host(2750),
            0,
            EmuProfile::Balanced,
            PresentationMode::ZeroCopy,
        )
        .unwrap_err();
        assert_eq!(
            error,
            EmuPlanError::InsufficientMemory {
                required_mib: 2816,
                available_mib: 2750
            }
        );
    }

    #[test]
    fn reference_phone_reading_admits_the_low_memory_profile() {
        let budget = admit(
            &host(2750),
            0,
            EmuProfile::LowMemory,
            PresentationMode::ZeroCopy,
        )
        .unwrap();
        assert_eq!(budget.guest_mib, 1536);
        assert_eq!(budget.required_available_mib, 2304);
        assert!(budget.footprint_mib <= 2750);
    }

    #[test]
    fn readback_presentation_needs_less_gfxstream_memory() {
        let zero_copy = budget(EmuProfile::Balanced, PresentationMode::ZeroCopy);
        let readback = budget(EmuProfile::Balanced, PresentationMode::Readback);
        assert!(readback.gfxstream_hostmem_mib < zero_copy.gfxstream_hostmem_mib);
        assert!(readback.footprint_mib < zero_copy.footprint_mib);
    }

    #[test]
    fn refuses_an_oversized_guest() {
        assert_eq!(
            admit(
                &host(16_384),
                4096,
                EmuProfile::Balanced,
                PresentationMode::ZeroCopy
            )
            .unwrap_err(),
            EmuPlanError::GuestMemoryExceedsEmulatedMaximum {
                requested_mib: 4096,
                maximum_mib: MAX_GUEST_MIB
            }
        );
    }
}
