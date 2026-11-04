use std::fs;
use std::io;
use std::path::Path;

use crate::BrokerPolicy;

/// Without microarchitecture identity, the slowest capacity tier is treated as efficiency cores
/// only when it is below this share of the fastest core.
const EFFICIENCY_TIER_PERCENT: u64 = 50;
/// Memory always left to the Android host, however small the phone.
const HOST_RESERVE_FLOOR_MIB: u64 = 3072;
/// Share of physical memory left to the Android host on larger phones.
const HOST_RESERVE_PERCENT: u64 = 40;
/// Capacity scale used by the Linux scheduler and by crosvm `--cpu-capacity`.
pub const CAPACITY_SCALE: u64 = 1024;

/// Microarchitectural class from `MIDR_EL1`. In-order efficiency cores (Cortex-A53/A55/A510/A520
/// and Qualcomm Kryo "Silver") run vCPU threads far slower than their capacity number suggests
/// for interactive work, so they never host pinned vCPUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreKind {
    Efficiency,
    Performance,
    Unknown,
}

impl CoreKind {
    /// Classifies a raw `MIDR_EL1` value (implementer bits 31:24, part number bits 15:4).
    pub fn from_midr(midr: u64) -> Self {
        let implementer = (midr >> 24) & 0xff;
        let part = (midr >> 4) & 0xfff;
        match (implementer, part) {
            // Arm Ltd: Cortex-A53, A35, A55, A510, A520.
            (0x41, 0xd03 | 0xd04 | 0xd05 | 0xd46 | 0xd80) => CoreKind::Efficiency,
            // Qualcomm Kryo 2xx/3xx/4xx Silver (A53/A55-derived).
            (0x51, 0x801 | 0x803 | 0x805) => CoreKind::Efficiency,
            (0x41 | 0x51 | 0x53 | 0x61, _) => CoreKind::Performance,
            _ => CoreKind::Unknown,
        }
    }

    /// Parses sysfs `regs/identification/midr_el1` text such as `0x00000000410fd050`.
    pub fn parse_midr(text: &str) -> Option<u64> {
        let text = text.trim();
        let hex = text
            .strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))?;
        u64::from_str_radix(hex, 16).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuCore {
    pub index: usize,
    /// Kernel `cpu_capacity` (normalized to 1024) when every core exposes it, otherwise every
    /// core's max frequency in kHz. One host never mixes the two units, so only ratios between
    /// cores of one host are compared, never absolute values.
    pub capacity: u64,
    /// Lowest CPU index of the cpufreq policy (frequency domain) this core belongs to. Cores that
    /// change frequency together must be presented to the guest as one frequency domain.
    pub freq_domain: usize,
    pub kind: CoreKind,
}

impl CpuCore {
    pub fn new(index: usize, capacity: u64, freq_domain: usize, kind: CoreKind) -> Self {
        Self {
            index,
            capacity,
            freq_domain,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostResources {
    pub total_memory_mib: u64,
    pub cores: Vec<CpuCore>,
}

/// One guest vCPU bound to exactly one host CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VcpuPin {
    pub vcpu: usize,
    pub host_cpu: usize,
    /// Host capacity of `host_cpu`, renormalized so the fastest host core is 1024.
    pub capacity: u32,
}

/// A 1:1 vCPU placement on the host's fastest cores, with the guest-visible frequency domains
/// mirroring the host cpufreq policies of the chosen cores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcpuPlacement {
    pub pins: Vec<VcpuPin>,
    /// Guest vCPU indices grouped by shared host frequency domain, ordered by first vCPU.
    pub freq_domains: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementError {
    NoVcpus,
    /// Pinning more vCPUs than performance cores would put latency-sensitive vCPU threads on
    /// efficiency cores; the broker refuses rather than silently degrading.
    ExceedsPerformanceCores {
        requested: u16,
        available: u16,
    },
}

impl HostResources {
    pub fn parse_total_memory_mib(meminfo: &str) -> Option<u64> {
        let line = meminfo.lines().find(|line| line.starts_with("MemTotal:"))?;
        let kib: u64 = line
            .trim_start_matches("MemTotal:")
            .trim()
            .strip_suffix("kB")?
            .trim()
            .parse()
            .ok()?;
        Some(kib / 1024)
    }

    /// Parses a cpufreq `related_cpus` list such as `4 5 6 7` or `4-7` and returns its lowest CPU.
    pub fn parse_freq_domain_leader(related_cpus: &str) -> Option<usize> {
        related_cpus
            .split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter(|token| !token.is_empty())
            .filter_map(|token| token.split('-').next()?.parse::<usize>().ok())
            .min()
    }

    /// Reads `/proc/meminfo`-style memory data and a `/sys/devices/system/cpu`-style tree.
    /// Returns an error rather than guessing when either is unreadable, so callers keep their
    /// conservative static policy.
    pub fn read(meminfo: &Path, cpu_root: &Path) -> io::Result<Self> {
        let total_memory_mib = Self::parse_total_memory_mib(&fs::read_to_string(meminfo)?)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no MemTotal"))?;

        struct RawCore {
            index: usize,
            capacity: Option<u64>,
            max_freq_khz: Option<u64>,
            freq_domain: Option<usize>,
            kind: CoreKind,
        }

        let mut raw = Vec::new();
        for entry in fs::read_dir(cpu_root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(index) = name
                .to_str()
                .and_then(|name| name.strip_prefix("cpu"))
                .and_then(|rest| rest.parse::<usize>().ok())
            else {
                continue;
            };
            let read_text = |relative: &str| fs::read_to_string(entry.path().join(relative)).ok();
            let read_u64 =
                |relative: &str| read_text(relative).and_then(|text| text.trim().parse().ok());
            // Offline cores cannot run vCPU threads; cpu0 usually has no `online` file.
            if read_u64("online") == Some(0) {
                continue;
            }
            raw.push(RawCore {
                index,
                capacity: read_u64("cpu_capacity"),
                max_freq_khz: read_u64("cpufreq/cpuinfo_max_freq"),
                freq_domain: read_text("cpufreq/related_cpus")
                    .as_deref()
                    .and_then(Self::parse_freq_domain_leader),
                kind: read_text("regs/identification/midr_el1")
                    .as_deref()
                    .and_then(CoreKind::parse_midr)
                    .map_or(CoreKind::Unknown, CoreKind::from_midr),
            });
        }
        raw.sort_by_key(|core| core.index);

        // Never mix units: use scheduler capacity only when every core exposes it.
        let use_capacity = raw.iter().all(|core| core.capacity.is_some());
        let mut cores = Vec::new();
        for core in &raw {
            let capacity = if use_capacity {
                core.capacity
            } else {
                core.max_freq_khz
            };
            let Some(capacity) = capacity else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cpu{} has no comparable capacity", core.index),
                ));
            };
            cores.push(CpuCore {
                index: core.index,
                capacity,
                freq_domain: core.freq_domain.unwrap_or(core.index),
                kind: core.kind,
            });
        }
        if cores.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "no readable CPU capacity",
            ));
        }
        // Without cpufreq topology, cores of equal capacity are the best available domain proxy.
        if raw.iter().all(|core| core.freq_domain.is_none()) {
            let snapshot = cores.clone();
            for core in &mut cores {
                core.freq_domain = snapshot
                    .iter()
                    .find(|other| other.capacity == core.capacity)
                    .map_or(core.index, |other| other.index);
            }
        }
        Ok(Self {
            total_memory_mib,
            cores,
        })
    }

    fn fastest_capacity(&self) -> u64 {
        self.cores
            .iter()
            .map(|core| core.capacity)
            .max()
            .unwrap_or(0)
    }

    /// Cores that may host latency-sensitive vCPU threads. Known in-order efficiency cores are
    /// excluded; without microarchitecture identity, only a slow capacity tier is excluded, so
    /// strong mid cores (for example Tensor's A76/A78 tier) remain usable once capacities are
    /// mirrored to the guest. A host made only of efficiency cores keeps all of them.
    fn performance_cores(&self) -> Vec<CpuCore> {
        let identified = self.cores.iter().any(|core| core.kind != CoreKind::Unknown);
        let selected: Vec<CpuCore> = if identified {
            self.cores
                .iter()
                .copied()
                .filter(|core| core.kind != CoreKind::Efficiency)
                .collect()
        } else {
            let fastest = self.fastest_capacity();
            let slowest = self
                .cores
                .iter()
                .map(|core| core.capacity)
                .min()
                .unwrap_or(0);
            let exclude_slowest =
                slowest != fastest && slowest * 100 < fastest * EFFICIENCY_TIER_PERCENT;
            self.cores
                .iter()
                .copied()
                .filter(|core| !(exclude_slowest && core.capacity == slowest))
                .collect()
        };
        if selected.is_empty() {
            self.cores.clone()
        } else {
            selected
        }
    }

    pub fn performance_cpus(&self) -> Vec<usize> {
        self.performance_cores()
            .iter()
            .map(|core| core.index)
            .collect()
    }

    /// Guest memory the broker may grant: physical memory minus what the Android host keeps.
    pub fn max_guest_memory_mib(&self) -> u64 {
        let reserve =
            HOST_RESERVE_FLOOR_MIB.max(self.total_memory_mib * HOST_RESERVE_PERCENT / 100);
        self.total_memory_mib.saturating_sub(reserve)
    }

    /// Interactive guests get one vCPU per performance core so the host scheduler is not asked
    /// to run latency-sensitive vCPU threads on efficiency cores.
    pub fn max_interactive_vcpus(&self) -> u16 {
        u16::try_from(self.performance_cpus().len().max(1)).unwrap_or(u16::MAX)
    }

    /// Pins `vcpus` guest CPUs 1:1 onto the fastest performance cores. Within the chosen set,
    /// vCPU order follows host CPU order so guest CPU numbering is as stable as the host's.
    pub fn vcpu_placement(&self, vcpus: u16) -> Result<VcpuPlacement, PlacementError> {
        if vcpus == 0 {
            return Err(PlacementError::NoVcpus);
        }
        let fastest = self.fastest_capacity().max(1);
        let mut candidates = self.performance_cores();
        let available = u16::try_from(candidates.len()).unwrap_or(u16::MAX);
        if vcpus > available {
            return Err(PlacementError::ExceedsPerformanceCores {
                requested: vcpus,
                available,
            });
        }
        // Prefer the highest capacity (prime cores first), then lower CPU index.
        candidates.sort_by(|a, b| b.capacity.cmp(&a.capacity).then(a.index.cmp(&b.index)));
        candidates.truncate(usize::from(vcpus));
        candidates.sort_by_key(|core| core.index);

        let pins: Vec<VcpuPin> = candidates
            .iter()
            .enumerate()
            .map(|(vcpu, core)| VcpuPin {
                vcpu,
                host_cpu: core.index,
                capacity: u32::try_from((core.capacity * CAPACITY_SCALE / fastest).max(1))
                    .unwrap_or(u32::MAX),
            })
            .collect();

        let mut freq_domains: Vec<(usize, Vec<usize>)> = Vec::new();
        for (vcpu, core) in candidates.iter().enumerate() {
            match freq_domains
                .iter_mut()
                .find(|(leader, _)| *leader == core.freq_domain)
            {
                Some((_, members)) => members.push(vcpu),
                None => freq_domains.push((core.freq_domain, vec![vcpu])),
            }
        }
        Ok(VcpuPlacement {
            pins,
            freq_domains: freq_domains
                .into_iter()
                .map(|(_, members)| members)
                .collect(),
        })
    }
}

impl BrokerPolicy {
    /// Tightens (never loosens) the static limits to what this host can actually give a guest.
    pub fn constrained_to(mut self, host: &HostResources) -> Self {
        self.max_memory_mib = self.max_memory_mib.min(host.max_guest_memory_mib());
        self.max_vcpus = self.max_vcpus.min(host.max_interactive_vcpus());
        self
    }
}
