//! Guest CPU model and machine options for TCG.
//!
//! `-cpu max` keeps the guest's security features (PAC, BTI) present, and only makes the
//! *emulation* of them cheap. Nothing here disables a mitigation the guest would enable on
//! hardware: SVE/SME are absent from the target phone, only their emulated state is skipped.

use crate::EmuProfile;

/// The CPU and machine feature decisions for one profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuModelPlan {
    /// QEMU `-cpu` model name.
    pub model: &'static str,
    /// QEMU `-cpu` property list, in stable order.
    pub options: Vec<&'static str>,
    /// Emulated Memory Tagging Extension (`-machine virt,mte=on`).
    pub mte: bool,
    /// Pointer authentication is present in the guest (emulated with `pauth-impdef`).
    pub pac: bool,
    /// Branch Target Identification is present in the guest (cheap to emulate).
    pub bti: bool,
}

impl CpuModelPlan {
    /// The literal `-cpu` value, e.g. `max,pauth-impdef=on,sve=off,sme=off`.
    pub fn cpu_argument(&self) -> String {
        let mut value = String::from(self.model);
        for option in &self.options {
            value.push(',');
            value.push_str(option);
        }
        value
    }
}

/// The guest CPU model for a profile.
///
/// `pauth-impdef` implements PAC with the architectural "impdef" algorithm instead of QARMA5:
/// the guest still sees PAC and GrapheneOS still enables it, but TCG stops paying QARMA5's cost
/// on every signed pointer. SVE/SME are turned off because the target phone has neither and their
/// emulated state is large and slow. MTE is the one feature that stays off by default: it costs
/// both speed and guest memory, and the hardware path keeps it off for the same reason
/// (`direct_crosvm.rs`: an MTE guest cannot map the non-MTE gfxstream blobs).
pub fn for_profile(profile: EmuProfile) -> CpuModelPlan {
    match profile {
        EmuProfile::Balanced | EmuProfile::LowMemory => CpuModelPlan {
            model: "max",
            options: vec!["pauth-impdef=on", "sve=off", "sme=off"],
            mte: false,
            pac: true,
            bti: true,
        },
        EmuProfile::Hardened => CpuModelPlan {
            model: "max",
            options: vec!["pauth-impdef=on", "sve=off", "sme=off"],
            mte: true,
            pac: true,
            bti: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_keeps_pac_and_bti_but_not_mte() {
        let plan = for_profile(EmuProfile::Balanced);
        assert_eq!(plan.cpu_argument(), "max,pauth-impdef=on,sve=off,sme=off");
        assert!(plan.pac);
        assert!(plan.bti);
        assert!(!plan.mte);
    }

    #[test]
    fn hardened_enables_emulated_mte() {
        assert!(for_profile(EmuProfile::Hardened).mte);
    }
}
