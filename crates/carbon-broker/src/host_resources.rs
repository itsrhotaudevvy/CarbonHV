use std::fs;
use std::io;
use std::path::Path;

use crate::BrokerPolicy;

/// Cores at or above this share of the fastest core's capacity are treated as performance cores.
const PERFORMANCE_CAPACITY_PERCENT: u64 = 70;
/// Memory always left to the Android host, however small the phone.
const HOST_RESERVE_FLOOR_MIB: u64 = 3072;
/// Share of physical memory left to the Android host on larger phones.
const HOST_RESERVE_PERCENT: u64 = 40;
/// Capacity scale used by the Linux scheduler and by crosvm `--cpu-capacity`.
pub const CAPACITY_SCALE: u64 = 1024;

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

    pub fn performance_cpus(&self) -> Vec<usize> {
        let fastest = self.fastest_capacity();
        self.cores
            .iter()
            .filter(|core| core.capacity * 100 >= fastest * PERFORMANCE_CAPACITY_PERCENT)
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
        let mut candidates: Vec<CpuCore> = self
            .cores
            .iter()
            .copied()
            .filter(|core| core.capacity * 100 >= fastest * PERFORMANCE_CAPACITY_PERCENT)
            .collect();
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
