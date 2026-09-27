use std::path::{Component, Path, PathBuf};

use crate::renderer_entry_point;

pub const RENDERER_RUNTIME_ROOT: &str = "/data/misc/carbonhv/runtime";
pub const VIRTIO_GPU_FD: i32 = 3;
pub const PRESENTATION_FD: i32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererProcessConfig {
    pub profile_id: String,
    pub runtime_dir: PathBuf,
    pub virtio_gpu_fd: i32,
    pub presentation_fd: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererProcessConfigError {
    MissingArgument,
    DuplicateArgument,
    UnknownArgument,
    UnsupportedProfile,
    UnsafeRuntimeDirectory,
    InvalidFileDescriptor,
    FileDescriptorCollision,
}

impl RendererProcessConfig {
    pub fn parse(args: &[String]) -> Result<Self, RendererProcessConfigError> {
        let mut profile_id = None;
        let mut runtime_dir = None;
        let mut virtio_gpu_fd = None;
        let mut presentation_fd = None;
        let mut index = 0;

        while index < args.len() {
            let key = args[index].as_str();
            let value = args
                .get(index + 1)
                .ok_or(RendererProcessConfigError::MissingArgument)?;
            match key {
                "--profile" => set_once(&mut profile_id, value.clone())?,
                "--runtime-dir" => set_once(&mut runtime_dir, PathBuf::from(value))?,
                "--virtio-gpu-fd" => {
                    let fd = parse_fd(value)?;
                    set_once(&mut virtio_gpu_fd, fd)?;
                }
                "--presentation-fd" => {
                    let fd = parse_fd(value)?;
                    set_once(&mut presentation_fd, fd)?;
                }
                _ => return Err(RendererProcessConfigError::UnknownArgument),
            }
            index += 2;
        }

        let profile_id = profile_id.ok_or(RendererProcessConfigError::MissingArgument)?;
        if renderer_entry_point(&profile_id).is_none() {
            return Err(RendererProcessConfigError::UnsupportedProfile);
        }

        let runtime_dir = runtime_dir.ok_or(RendererProcessConfigError::MissingArgument)?;
        if !runtime_dir.is_absolute()
            || !runtime_dir.starts_with(Path::new(RENDERER_RUNTIME_ROOT))
            || runtime_dir
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(RendererProcessConfigError::UnsafeRuntimeDirectory);
        }

        let virtio_gpu_fd = virtio_gpu_fd.ok_or(RendererProcessConfigError::MissingArgument)?;
        let presentation_fd = presentation_fd.ok_or(RendererProcessConfigError::MissingArgument)?;
        if virtio_gpu_fd == presentation_fd {
            return Err(RendererProcessConfigError::FileDescriptorCollision);
        }

        Ok(Self {
            profile_id,
            runtime_dir,
            virtio_gpu_fd,
            presentation_fd,
        })
    }
}

fn parse_fd(value: &str) -> Result<i32, RendererProcessConfigError> {
    let fd = value
        .parse::<i32>()
        .map_err(|_| RendererProcessConfigError::InvalidFileDescriptor)?;
    if fd < 3 {
        return Err(RendererProcessConfigError::InvalidFileDescriptor);
    }
    Ok(fd)
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), RendererProcessConfigError> {
    if slot.is_some() {
        return Err(RendererProcessConfigError::DuplicateArgument);
    }
    *slot = Some(value);
    Ok(())
}
