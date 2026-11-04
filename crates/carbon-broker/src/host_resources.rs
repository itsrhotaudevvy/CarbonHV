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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuCore {
    pub index: usize,
    /// Kernel `cpu_capacity` (normalized to 1024) when exposed, otherwise max frequency in kHz.
    /// Only ratios between cores of one host are compared, never absolute values.
    pub capacity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostResources {
    pub total_memory_mib: u64,
    pub cores: Vec<CpuCore>,
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

    /// Reads `/proc/meminfo`-style memory data and a `/sys/devices/system/cpu`-style tree.
    /// Returns an error rather than guessing when either is unreadable, so callers keep their
    /// conservative static policy.
    pub fn read(meminfo: &Path, cpu_root: &Path) -> io::Result<Self> {
        let total_memory_mib = Self::parse_total_memory_mib(&fs::read_to_string(meminfo)?)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no MemTotal"))?;

        let mut cores = Vec::new();
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
            let read_u64 = |relative: &str| {
                fs::read_to_string(entry.path().join(relative))
                    .ok()
                    .and_then(|text| text.trim().parse::<u64>().ok())
            };
            if let Some(capacity) =
                read_u64("cpu_capacity").or_else(|| read_u64("cpufreq/cpuinfo_max_freq"))
            {
                cores.push(CpuCore { index, capacity });
            }
        }
        cores.sort_by_key(|core| core.index);
        if cores.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "no readable CPU capacity",
            ));
        }
        Ok(Self {
            total_memory_mib,
            cores,
        })
    }

    pub fn performance_cpus(&self) -> Vec<usize> {
        let fastest = self
            .cores
            .iter()
            .map(|core| core.capacity)
            .max()
            .unwrap_or(0);
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
}

impl BrokerPolicy {
    /// Tightens (never loosens) the static limits to what this host can actually give a guest.
    pub fn constrained_to(mut self, host: &HostResources) -> Self {
        self.max_memory_mib = self.max_memory_mib.min(host.max_guest_memory_mib());
        self.max_vcpus = self.max_vcpus.min(host.max_interactive_vcpus());
        self
    }
}
