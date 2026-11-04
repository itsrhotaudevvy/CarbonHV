//! Host-side performance planning for interactive guests.
//!
//! "Near-native" here means the guest's own scheduler and cpufreq governor see, and drive, real
//! host cores: each vCPU is a host thread pinned to one performance core, the guest sees that
//! core's relative capacity and frequency-domain grouping, and the guest's DVFS requests reach the
//! host through crosvm's upstream virtual cpufreq device (`qemu,virtual-cpufreq`, Linux
//! `CONFIG_CPUFREQ_VIRT`). Backends that cannot pin vCPUs fall back to uclamp boosting, which keeps
//! vCPU threads on fast cores at the cost of thermal headroom. Nothing here weakens isolation.

use crate::{HostResources, PlacementError, VcpuPlacement};

/// Runtime transparent-huge-page mode of the host (`/sys/kernel/mm/transparent_hugepage/enabled`).
/// `--hugepages` / AVF `hugePages` only *advise* THP, so they do nothing when the host runs with
/// `never`, as the stock Redmi kunzite (Snapdragon 6 Gen 3) firmware does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThpMode {
    Always,
    Madvise,
    Never,
    #[default]
    Unknown,
}

impl ThpMode {
    /// Parses the sysfs format, where the active mode is bracketed: `always [madvise] never`.
    pub fn parse(text: &str) -> Self {
        let active = text
            .split_whitespace()
            .find_map(|token| token.strip_prefix('[')?.strip_suffix(']'));
        match active {
            Some("always") => ThpMode::Always,
            Some("madvise") => ThpMode::Madvise,
            Some("never") => ThpMode::Never,
            _ => ThpMode::Unknown,
        }
    }

    pub fn read(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .map(|text| Self::parse(&text))
            .unwrap_or(ThpMode::Unknown)
    }

    /// Whether advising huge pages can have any effect on this host.
    pub fn honours_advice(self) -> bool {
        self != ThpMode::Never
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerformanceProfile {
    /// Pinned vCPUs with guest-driven DVFS where the backend allows it; maximum responsiveness.
    #[default]
    NearNative,
    /// Host scheduler decides placement and frequency; for thermally constrained sessions.
    Efficient,
}

/// What a backend can express about vCPU placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuControl {
    /// Direct crosvm: `--cpu-affinity`, `--cpu-capacity`, `--cpus freq-domains=[...]`.
    PerVcpuAffinity,
    /// AVF raw config: only a vCPU count (`CpuOptions.cpuTopology.cpuCount`).
    CountOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFrequencyControl {
    /// Guest cpufreq requests are forwarded to the host (`--virt-cpufreq-upstream`).
    GuestDvfs,
    /// vCPU threads run with `uclamp.min` at full capacity (`--boost-uclamp` / AVF `boostUclamp`).
    BoostUclamp,
    /// The host scheduler alone decides.
    HostDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuPlan {
    Pinned(VcpuPlacement),
    Floating { vcpus: u16 },
}

impl CpuPlan {
    pub fn vcpus(&self) -> u16 {
        match self {
            CpuPlan::Pinned(placement) => u16::try_from(placement.pins.len()).unwrap_or(u16::MAX),
            CpuPlan::Floating { vcpus } => *vcpus,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestPerformancePlan {
    pub profile: PerformanceProfile,
    pub cpu: CpuPlan,
    pub frequency: CpuFrequencyControl,
    /// Advise transparent huge pages for guest RAM (`--hugepages` / AVF `hugePages`); fewer
    /// stage-2 TLB misses for the guest's working set.
    pub hugepages: bool,
    /// Return guest-freed pages to Android (`--balloon-page-reporting` / AVF `balloon`) so a large
    /// guest does not push the host into low-memory kills during long sessions.
    pub balloon_page_reporting: bool,
}

impl GuestPerformancePlan {
    pub fn build(
        profile: PerformanceProfile,
        host: Option<&HostResources>,
        vcpus: u16,
        control: CpuControl,
    ) -> Result<Self, PlacementError> {
        if vcpus == 0 {
            return Err(PlacementError::NoVcpus);
        }
        let (cpu, frequency) = match (profile, control, host) {
            (PerformanceProfile::NearNative, CpuControl::PerVcpuAffinity, Some(host)) => (
                CpuPlan::Pinned(host.vcpu_placement(vcpus)?),
                CpuFrequencyControl::GuestDvfs,
            ),
            // Without pinning (or without a readable topology to mirror), boosting is the only
            // lever that keeps vCPU threads off efficiency cores.
            (PerformanceProfile::NearNative, _, _) => (
                CpuPlan::Floating { vcpus },
                CpuFrequencyControl::BoostUclamp,
            ),
            (PerformanceProfile::Efficient, _, _) => (
                CpuPlan::Floating { vcpus },
                CpuFrequencyControl::HostDefault,
            ),
        };
        Ok(Self {
            profile,
            cpu,
            frequency,
            hugepages: true,
            balloon_page_reporting: true,
        })
    }

    /// Drops huge-page advice the host would ignore, so the plan (and its evidence report) never
    /// claims a lever that cannot take effect.
    pub fn for_host_thp(mut self, thp: ThpMode) -> Self {
        self.hugepages &= thp.honours_advice();
        self
    }

    /// crosvm `run` arguments that realize this plan. Only flags present in the pinned crosvm
    /// revision are emitted; each value is a separate argv element (no shell).
    pub fn crosvm_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        match &self.cpu {
            CpuPlan::Pinned(placement) => {
                let domains = cpu_set_list(&placement.freq_domains);
                args.push("--cpus".into());
                args.push(format!(
                    "num-cores={},clusters={domains},freq-domains={domains},sve=[auto=true]",
                    placement.pins.len()
                ));
                args.push("--cpu-affinity".into());
                args.push(
                    placement
                        .pins
                        .iter()
                        .map(|pin| format!("{}={}", pin.vcpu, pin.host_cpu))
                        .collect::<Vec<_>>()
                        .join(":"),
                );
                args.push("--cpu-capacity".into());
                args.push(
                    placement
                        .pins
                        .iter()
                        .map(|pin| format!("{}={}", pin.vcpu, pin.capacity))
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            CpuPlan::Floating { vcpus } => {
                args.push("--cpus".into());
                args.push(format!("num-cores={vcpus},sve=[auto=true]"));
            }
        }
        match self.frequency {
            CpuFrequencyControl::GuestDvfs => args.push("--virt-cpufreq-upstream".into()),
            CpuFrequencyControl::BoostUclamp => args.push("--boost-uclamp".into()),
            CpuFrequencyControl::HostDefault => {}
        }
        if self.hugepages {
            args.push("--hugepages".into());
        }
        if self.balloon_page_reporting {
            args.push("--balloon-page-reporting".into());
        }
        args
    }
}

/// Formats vCPU groups as crosvm `CpuSet` lists: `[[0,1],[2,3]]`.
fn cpu_set_list(groups: &[Vec<usize>]) -> String {
    let inner = groups
        .iter()
        .map(|group| {
            format!(
                "[{}]",
                group
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{inner}]")
}
