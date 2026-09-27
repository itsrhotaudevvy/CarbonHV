use std::process::ExitCode;

use carbon_renderer::RendererProcessConfig;

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match RendererProcessConfig::parse(&args) {
        Ok(config) => {
            eprintln!(
                "carbon-renderer: profile={} runtime={} virtio_gpu_fd={} presentation_fd={}",
                config.profile_id,
                config.runtime_dir.display(),
                config.virtio_gpu_fd,
                config.presentation_fd,
            );
            eprintln!("carbon-renderer: AVF/crosvm resource transport serve loop is not wired");
            ExitCode::from(69)
        }
        Err(error) => {
            eprintln!("carbon-renderer: invalid process configuration: {error:?}");
            ExitCode::from(64)
        }
    }
}
