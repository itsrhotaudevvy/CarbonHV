use std::env;
use std::fs;
use std::process::ExitCode;

use carbon_core::{BackupManifest, BundleManifest, GuestContract};

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
        eprintln!("usage: carbonctl bundle verify <bundle.toml> | guest verify <device.toml> | backup verify <manifest>");
        ExitCode::from(64)
    }
}
