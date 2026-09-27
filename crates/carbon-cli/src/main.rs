use std::env;
use std::fs;
use std::process::ExitCode;

use carbon_core::BundleManifest;

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
    } else {
        eprintln!("usage: carbonctl bundle verify <bundle.toml>");
        ExitCode::from(64)
    }
}

