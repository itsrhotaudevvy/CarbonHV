use std::path::{Component, PathBuf};

use crate::{BackendError, HostProbe, HostVirtualizationClass, ProgramInvocation, ValidatedVmSpec};

const GFXSTREAM_CONTEXT_TYPES: [&str; 2] = ["gfxstream-vulkan", "gfxstream-composer"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfGraphicsConfig {
    width_pixels: u32,
    height_pixels: u32,
    refresh_rate: u32,
}

impl AvfGraphicsConfig {
    pub fn gfxstream_vulkan(
        width_pixels: u32,
        height_pixels: u32,
        refresh_rate: u32,
    ) -> Result<Self, BackendError> {
        if width_pixels == 0
            || height_pixels == 0
            || refresh_rate == 0
            || width_pixels > 16_384
            || height_pixels > 16_384
            || refresh_rate > 1_000
        {
            return Err(BackendError::InvalidGraphicsConfig);
        }
        Ok(Self {
            width_pixels,
            height_pixels,
            refresh_rate,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvfLaunchPlan {
    pub config_path: PathBuf,
    pub config_json: String,
    pub invocation: ProgramInvocation,
}

#[derive(Debug, Clone)]
pub struct AvfBackend {
    vm_tool: PathBuf,
    kvm_device: PathBuf,
}

impl AvfBackend {
    pub fn new(vm_tool: PathBuf, kvm_device: PathBuf) -> Self {
        Self {
            vm_tool,
            kvm_device,
        }
    }

    pub fn probe(&self) -> HostProbe {
        let vm_tool_present = self.vm_tool.is_file();
        let kvm_present = self.kvm_device.exists();
        let class = if vm_tool_present && kvm_present {
            HostVirtualizationClass::DevelopmentReady
        } else {
            HostVirtualizationClass::Unavailable
        };
        HostProbe {
            vm_tool_present,
            kvm_present,
            class,
        }
    }

    pub fn build_launch_plan(
        &self,
        spec: &ValidatedVmSpec,
        runtime_dir: PathBuf,
    ) -> Result<AvfLaunchPlan, BackendError> {
        self.build_launch_plan_inner(spec, runtime_dir, None)
    }

    pub fn build_graphics_launch_plan(
        &self,
        spec: &ValidatedVmSpec,
        runtime_dir: PathBuf,
        graphics: AvfGraphicsConfig,
    ) -> Result<AvfLaunchPlan, BackendError> {
        self.build_launch_plan_inner(spec, runtime_dir, Some(graphics))
    }

    fn build_launch_plan_inner(
        &self,
        spec: &ValidatedVmSpec,
        runtime_dir: PathBuf,
        graphics: Option<AvfGraphicsConfig>,
    ) -> Result<AvfLaunchPlan, BackendError> {
        if runtime_dir
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(BackendError::UnsafeRuntimeDirectory);
        }

        let config_path =
            runtime_dir.join(format!("{}-{}.json", spec.instance_id, spec.generation.0));
        let params = spec.kernel_params.join(" ");
        let mut config_json = String::from("{");
        push_json_field(
            &mut config_json,
            "kernel",
            &spec.kernel.to_string_lossy(),
            true,
        );
        push_json_field(
            &mut config_json,
            "initrd",
            &spec.initrd.to_string_lossy(),
            false,
        );
        push_json_field(&mut config_json, "params", &params, false);
        config_json.push_str(",\"memory_mib\":");
        config_json.push_str(&spec.memory_mib.to_string());
        config_json.push_str(",\"vcpus\":");
        config_json.push_str(&spec.vcpus.to_string());
        config_json.push_str(",\"disks\":[");
        for (index, disk) in spec.disks.iter().enumerate() {
            if index != 0 {
                config_json.push(',');
            }
            config_json.push_str("{\"image\":\"");
            config_json.push_str(&json_escape(&disk.path.to_string_lossy()));
            config_json.push_str("\",\"writable\":");
            config_json.push_str(if disk.writable { "true" } else { "false" });
            config_json.push_str(",\"partitions\":[]}");
        }
        config_json.push(']');

        if let Some(graphics) = graphics {
            append_graphics_config(&mut config_json, &graphics);
        }
        config_json.push('}');

        Ok(AvfLaunchPlan {
            config_path: config_path.clone(),
            config_json,
            invocation: ProgramInvocation {
                program: self.vm_tool.clone(),
                args: vec!["run".into(), config_path.to_string_lossy().into_owned()],
            },
        })
    }
}

fn append_graphics_config(output: &mut String, graphics: &AvfGraphicsConfig) {
    output.push_str(",\"gpu\":{");
    output.push_str("\"backend\":\"gfxstream\"");
    output.push_str(",\"renderer_use_egl\":false");
    output.push_str(",\"renderer_use_gles\":false");
    output.push_str(",\"renderer_use_glx\":false");
    output.push_str(",\"renderer_use_surfaceless\":true");
    output.push_str(",\"renderer_use_vulkan\":true");
    output.push_str(",\"context_types\":[");
    for (index, context_type) in GFXSTREAM_CONTEXT_TYPES.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push('"');
        output.push_str(context_type);
        output.push('"');
    }
    output.push_str("]}");

    output.push_str(",\"display\":{");
    output.push_str("\"width_pixels\":");
    output.push_str(&graphics.width_pixels.to_string());
    output.push_str(",\"height_pixels\":");
    output.push_str(&graphics.height_pixels.to_string());
    output.push_str(",\"refresh_rate\":");
    output.push_str(&graphics.refresh_rate.to_string());
    output.push('}');

    output.push_str(",\"input\":{");
    output.push_str("\"touchscreen\":true");
    output.push_str(",\"keyboard\":true");
    output.push_str(",\"mouse\":true");
    output.push_str(",\"trackpad\":false");
    output.push_str(",\"switches\":false}");
}

fn push_json_field(output: &mut String, key: &str, value: &str, first: bool) {
    if !first {
        output.push(',');
    }
    output.push('"');
    output.push_str(key);
    output.push_str("\":\"");
    output.push_str(&json_escape(value));
    output.push('"');
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}
