use std::env;
use std::fs;
use std::process::ExitCode;

use carbon_core::{
    BackupManifest, BundleManifest, GuestContract, GuestOsFamily, KernelConfig,
    KernelRequirementClass,
};

/// `carbonctl guest kernel-config <android|ubuntu-touch> <config-or-fragment>...`
/// Files are merged in order. Exit 3: CVD1 contract gap; exit 5: near-native performance gap.
fn verify_kernel_config(family: &str, paths: &[String]) -> ExitCode {
    let family = match family {
        "android" => GuestOsFamily::Android,
        "ubuntu-touch" => GuestOsFamily::UbuntuTouch,
        other => {
            eprintln!("unknown guest family {other}; expected android or ubuntu-touch");
            return ExitCode::from(64);
        }
    };
    let mut config = KernelConfig::default();
    for path in paths {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("failed to read {path}: {error}");
                return ExitCode::from(2);
            }
        };
        if let Err(error) = config.merge(&text) {
            eprintln!("invalid kernel config {path}: {error:?}");
            return ExitCode::from(3);
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
        ExitCode::from(3)
    } else if !gaps.is_empty() {
        ExitCode::from(5)
    } else {
        println!("verified {family:?} guest kernel config for CVD1 near-native execution");
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() == 4 && args[1] == "bundle" && args[2] == "verify" {
        let path = &args[3];
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("failed to read {path}: {error}");
                return ExitCode::from(2);
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
                ExitCode::from(3)
            }
        }
    } else if args.len() == 4 && args[1] == "guest" && args[2] == "verify" {
        let path = &args[3];
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("failed to read {path}: {error}");
                return ExitCode::from(2);
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
                ExitCode::from(3)
            }
        }
    } else if args.len() >= 5 && args[1] == "guest" && args[2] == "kernel-config" {
        verify_kernel_config(&args[3], &args[4..])
    } else if args.len() == 4 && args[1] == "backup" && args[2] == "verify" {
        let path = &args[3];
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("failed to read {path}: {error}");
                return ExitCode::from(2);
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
                ExitCode::from(3)
            }
        }
    } else {
        eprintln!(
            "usage: carbonctl bundle verify <bundle.toml> | guest verify <device.toml> | guest kernel-config <android|ubuntu-touch> <config>... | backup verify <manifest>"
        );
        ExitCode::from(64)
    }
}
