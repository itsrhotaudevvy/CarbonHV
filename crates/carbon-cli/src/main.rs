use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use carbon_core::{
    BackupManifest, BundleManifest, GuestContract, GuestOsFamily, KernelConfig,
    KernelRequirementClass,
};
use carbon_emu::{
    DEFAULT_DISPLAY, EmuHostFacts, EmuNetwork, EmuProfile, EmuSessionFds, EmuVmSpec,
    PresentationMode,
};

/// Exit codes `carbonctl` promises to automation.
const EXIT_USAGE: u8 = 64;
const EXIT_PLAN_REFUSED: u8 = 65;
const EXIT_IO: u8 = 2;
const EXIT_CONTRACT: u8 = 3;
const EXIT_PERFORMANCE_GAP: u8 = 5;

const USAGE: &str = "\
carbonctl — CarbonHV validation and diagnostic CLI

usage:
  carbonctl bundle verify <bundle.toml>
  carbonctl guest verify <device.toml>
  carbonctl guest kernel-config <android|ubuntu-touch> <config-or-fragment>...
  carbonctl backup verify <manifest>
  carbonctl emu plan --facts <facts.json> [options]
  carbonctl emu boot-assets --boot <boot.img> --vendor-boot <vendor_boot.img> --out <dir>
                            [--init-boot <init_boot.img>] [--bootconfig <file>]
                            [--append <kernel-param>]...

emu plan options:
  --profile <balanced|hardened|low-memory>     planning profile (default: balanced)
  --presentation <zero-copy|readback>          frame path (default: zero-copy)
  --memory <MiB>                               guest RAM (0 = profile default)
  --vcpus <n>                                  0 = profile default
  --qemu <path>                                emulator executable to plan for
  --offline                                    no guest network (default: slirp NAT)
  --no-audio                                   no virtio-snd device
  --disks <n>                                  dry-run disk count (default: 6)

`emu boot-assets` unpacks Android v3/v4 boot images into the kernel, initrd and command line
QEMU's direct-kernel boot needs: <out>/kernel, <out>/initrd (vendor ramdisk, generic ramdisk,
then vendor_boot's bootconfig plus one `key=value` per line of --bootconfig) and
<out>/cmdline. It reads and writes local files only.

`emu plan` is a dry run: it prints the exact QEMU argv the `:vm` service would run, using
placeholder descriptor numbers. No VM is started and nothing is read from the phone.

exit codes: 0 ok, 2 I/O, 3 contract gap, 5 performance gap, 64 usage, 65 plan refused
";

/// `carbonctl emu boot-assets`: the engine's own boot-image code, run on the build host so a
/// qualification boot uses exactly the initrd the `:vm` service would build.
fn emu_boot_assets(args: &[String]) -> ExitCode {
    use carbon_emu::bootimg::{BootImage, VendorBootImage, assemble, kernel_view_of_bootconfig};

    let (mut boot, mut init_boot, mut vendor_boot, mut bootconfig, mut out) =
        (None, None, None, None, None);
    let mut append: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let Some(value) = args.get(index + 1).cloned() else {
            eprintln!("{} needs a value", args[index]);
            return ExitCode::from(EXIT_USAGE);
        };
        match args[index].as_str() {
            "--boot" => boot = Some(PathBuf::from(value)),
            "--init-boot" => init_boot = Some(PathBuf::from(value)),
            "--vendor-boot" => vendor_boot = Some(PathBuf::from(value)),
            "--bootconfig" => bootconfig = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--append" => append.push(value),
            other => {
                eprintln!("unknown emu boot-assets option {other}");
                return ExitCode::from(EXIT_USAGE);
            }
        }
        index += 2;
    }
    let (Some(boot), Some(vendor_boot), Some(out)) = (boot, vendor_boot, out) else {
        eprintln!("emu boot-assets needs --boot, --vendor-boot and --out");
        return ExitCode::from(EXIT_USAGE);
    };
    let read = |path: &PathBuf| {
        fs::read(path).map_err(|error| eprintln!("cannot read {}: {error}", path.display()))
    };
    let Ok(boot_bytes) = read(&boot) else {
        return ExitCode::from(EXIT_IO);
    };
    let Ok(vendor_bytes) = read(&vendor_boot) else {
        return ExitCode::from(EXIT_IO);
    };
    let init_bytes = match init_boot.as_ref().map(read) {
        None => None,
        Some(Ok(bytes)) => Some(bytes),
        Some(Err(())) => return ExitCode::from(EXIT_IO),
    };
    let params_text = match bootconfig.as_ref().map(fs::read_to_string) {
        None => String::new(),
        Some(Ok(text)) => text,
        Some(Err(error)) => {
            eprintln!("cannot read bootconfig file: {error}");
            return ExitCode::from(EXIT_IO);
        }
    };
    let params: Vec<&str> = params_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    let parsed = (|| {
        let boot = BootImage::parse(&boot_bytes)?;
        let init = init_bytes.as_deref().map(BootImage::parse).transpose()?;
        let vendor = VendorBootImage::parse(&vendor_bytes)?;
        assemble(&boot, init.as_ref(), &vendor, &params)
    })();
    let mut assets = match parsed {
        Ok(assets) => assets,
        Err(error) => {
            eprintln!("cannot assemble boot assets: {}", error.message());
            return ExitCode::from(EXIT_CONTRACT);
        }
    };
    for param in append {
        assets.kernel_cmdline.push(' ');
        assets.kernel_cmdline.push_str(&param);
    }
    let Some(seen) = kernel_view_of_bootconfig(&assets.initrd) else {
        eprintln!("assembled initrd carries no bootconfig block the kernel would accept");
        return ExitCode::from(EXIT_CONTRACT);
    };
    let seen_lines = seen
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty() && line.iter().any(|byte| *byte != 0))
        .count();

    if let Err(error) = fs::create_dir_all(&out)
        .and_then(|_| fs::write(out.join("kernel"), &assets.kernel))
        .and_then(|_| fs::write(out.join("initrd"), &assets.initrd))
        .and_then(|_| fs::write(out.join("cmdline"), format!("{}\n", assets.kernel_cmdline)))
        .and_then(|_| fs::write(out.join("bootconfig"), seen))
    {
        eprintln!("cannot write boot assets: {error}");
        return ExitCode::from(EXIT_IO);
    }
    println!(
        "kernel {} bytes, initrd {} bytes, bootconfig {} bytes ({seen_lines} keys) -> {}",
        assets.kernel.len(),
        assets.initrd.len(),
        seen.len(),
        out.display()
    );
    println!("cmdline: {}", assets.kernel_cmdline);
    ExitCode::SUCCESS
}

/// `carbonctl guest kernel-config <family> <config-or-fragment>...`
/// Files are merged in order. Exit 3: CVD1 contract gap; exit 5: near-native performance gap.
fn verify_kernel_config(family: &str, paths: &[String]) -> ExitCode {
    let family = match family {
        "android" => GuestOsFamily::Android,
        "ubuntu-touch" => GuestOsFamily::UbuntuTouch,
        other => {
            eprintln!("unknown guest family {other}; expected android or ubuntu-touch");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let mut config = KernelConfig::default();
    for path in paths {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("failed to read {path}: {error}");
                return ExitCode::from(EXIT_IO);
            }
        };
        if let Err(error) = config.merge(&text) {
            eprintln!("invalid kernel config {path}: {error:?}");
            return ExitCode::from(EXIT_CONTRACT);
        }
    }
    let gaps = config.cvd1_gaps(&family);
    for gap in &gaps {
        eprintln!(
            "{:?} gap: {} must be {:?}, found {:?}",
            gap.class, gap.option, gap.requirement, gap.actual
        );
    }
    if gaps
        .iter()
        .any(|gap| gap.class == KernelRequirementClass::Contract)
    {
        ExitCode::from(EXIT_CONTRACT)
    } else if !gaps.is_empty() {
        ExitCode::from(EXIT_PERFORMANCE_GAP)
    } else {
        println!("verified {family:?} guest kernel config for CVD1 near-native execution");
        ExitCode::SUCCESS
    }
}

#[derive(Debug)]
struct EmuPlanArgs {
    facts: PathBuf,
    profile: EmuProfile,
    presentation: PresentationMode,
    memory_mib: u64,
    vcpus: u16,
    qemu: PathBuf,
    network: EmuNetwork,
    audio: bool,
    disks: usize,
}

impl Default for EmuPlanArgs {
    fn default() -> Self {
        Self {
            facts: PathBuf::new(),
            profile: EmuProfile::Balanced,
            presentation: PresentationMode::ZeroCopy,
            memory_mib: 0,
            vcpus: 0,
            qemu: PathBuf::from("/data/app/carbonhv/libqemu_system_aarch64.so"),
            network: EmuNetwork::Nat,
            audio: true,
            disks: 6,
        }
    }
}

fn parse_emu_plan_args(args: &[String]) -> Result<Option<EmuPlanArgs>, String> {
    let mut parsed = EmuPlanArgs::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let mut value = || -> Result<&str, String> {
            index += 1;
            args.get(index)
                .map(String::as_str)
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag {
            "--help" | "-h" => return Ok(None),
            "--facts" => parsed.facts = PathBuf::from(value()?),
            "--profile" => {
                let name = value()?;
                parsed.profile =
                    EmuProfile::parse(name).ok_or_else(|| format!("unknown profile {name}"))?
            }
            "--presentation" => {
                let name = value()?;
                parsed.presentation = PresentationMode::parse(name)
                    .ok_or_else(|| format!("unknown presentation mode {name}"))?
            }
            "--memory" => {
                parsed.memory_mib = value()?
                    .parse()
                    .map_err(|_| "--memory needs a MiB count".to_string())?
            }
            "--vcpus" => {
                parsed.vcpus = value()?
                    .parse()
                    .map_err(|_| "--vcpus needs a count".to_string())?
            }
            "--qemu" => parsed.qemu = PathBuf::from(value()?),
            "--offline" => parsed.network = EmuNetwork::Offline,
            "--no-audio" => parsed.audio = false,
            "--disks" => {
                parsed.disks = value()?
                    .parse()
                    .map_err(|_| "--disks needs a count".to_string())?
            }
            other => return Err(format!("unknown option {other}")),
        }
        index += 1;
    }
    if parsed.facts.as_os_str().is_empty() {
        return Err("--facts <facts.json> is required".into());
    }
    if parsed.disks == 0 || parsed.disks > 16 {
        return Err("--disks must be between 1 and 16".into());
    }
    Ok(Some(parsed))
}

fn emu_plan(args: &[String]) -> ExitCode {
    let parsed = match parse_emu_plan_args(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n");
            print!("{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let document = match fs::read_to_string(&parsed.facts) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("failed to read {}: {error}", parsed.facts.display());
            return ExitCode::from(EXIT_IO);
        }
    };
    let facts = match EmuHostFacts::parse_json(&document) {
        Ok(facts) => facts,
        Err(error) => {
            eprintln!("invalid host facts {}: {error:?}", parsed.facts.display());
            return ExitCode::from(EXIT_CONTRACT);
        }
    };

    // Dry-run descriptor numbers: the real service passes its own. Disk 0 is the read-only
    // system image; the rest alternate, matching a CVD1 instance (system/vendor/product read-only,
    // userdata/metadata/misc writable).
    let mut next_fd = 10;
    let mut disks = Vec::with_capacity(parsed.disks);
    for index in 0..parsed.disks {
        disks.push(carbon_emu::EmuDisk {
            fd: next_fd,
            writable: index >= 3,
        });
        next_fd += 1;
    }
    let fds = EmuSessionFds {
        qmp: 3,
        frame: 4,
        input: 5,
        vsock: Some(6),
        console: Some(7),
    };
    let spec = EmuVmSpec {
        instance_id: "cvd1-main".into(),
        generation: 1,
        vcpus: parsed.vcpus,
        memory_mib: parsed.memory_mib,
        kernel: PathBuf::from("/data/misc/carbonhv/objects/kernel"),
        initrd: PathBuf::from("/data/misc/carbonhv/objects/initrd"),
        kernel_params: vec!["console=hvc0".into(), "bootconfig".into()],
        disks,
        display: DEFAULT_DISPLAY,
        network: parsed.network,
        audio: parsed.audio,
        presentation: parsed.presentation,
        profile: parsed.profile,
    };

    let plan = match carbon_emu::plan(parsed.qemu, &spec, &facts, &fds) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("plan refused: {}", error.message());
            return ExitCode::from(EXIT_PLAN_REFUSED);
        }
    };

    eprintln!("carbonctl emu plan — dry run, no VM is started");
    eprintln!("engine: Emulated (QEMU TCG) · cpu.native=false");
    eprintln!("host:   {}", facts.describe());
    eprintln!("memory: {}", plan.memory.describe());
    eprintln!("cpu:    {}", plan.placement.describe());
    eprintln!("cpu:    {}", plan.cpu_model.cpu_argument());
    eprintln!("frames: {}", plan.presentation.as_str());
    for warning in &plan.warnings {
        eprintln!("warning: {} — {}", warning.code(), warning.message());
    }
    println!("{}", plan.shell_argv());
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["--help" | "-h" | "help", ..] | [] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["--version" | "-V", ..] => {
            println!("carbonctl {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["bundle", "verify", path] => {
            let text = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!("failed to read {path}: {error}");
                    return ExitCode::from(EXIT_IO);
                }
            };
            match BundleManifest::parse(&text) {
                Ok(manifest) => {
                    println!(
                        "verified {} {} (cvd {}, {} artifacts)",
                        manifest.id,
                        manifest.version,
                        manifest.cvd_major,
                        manifest.artifacts.len()
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("invalid bundle: {error:?}");
                    ExitCode::from(EXIT_CONTRACT)
                }
            }
        }
        ["guest", "verify", path] => {
            let text = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!("failed to read {path}: {error}");
                    return ExitCode::from(EXIT_IO);
                }
            };
            match GuestContract::parse(&text, 1) {
                Ok(contract) => {
                    println!(
                        "verified guest {} (cvd {}, agent protocol {})",
                        contract.id, contract.cvd_major, contract.agent_protocol_major
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("invalid guest contract: {error:?}");
                    ExitCode::from(EXIT_CONTRACT)
                }
            }
        }
        ["guest", "kernel-config", family, paths @ ..] if !paths.is_empty() => {
            let paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
            verify_kernel_config(family, &paths)
        }
        ["backup", "verify", path] => {
            let text = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!("failed to read {path}: {error}");
                    return ExitCode::from(EXIT_IO);
                }
            };
            match BackupManifest::parse(&text) {
                Ok(manifest) => {
                    println!(
                        "verified backup {} generation {} ({} system hashes)",
                        manifest.instance_id,
                        manifest.generation,
                        manifest.system_hashes.len()
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("invalid backup: {error:?}");
                    ExitCode::from(EXIT_CONTRACT)
                }
            }
        }
        ["emu", "boot-assets", rest @ ..] => {
            let rest: Vec<String> = rest.iter().map(|arg| (*arg).to_owned()).collect();
            emu_boot_assets(&rest)
        }
        ["emu", "plan", rest @ ..] => {
            let rest: Vec<String> = rest.iter().map(|arg| (*arg).to_owned()).collect();
            emu_plan(&rest)
        }
        _ => {
            eprintln!("error: unrecognized command\n");
            print!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}
