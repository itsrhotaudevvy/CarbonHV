use std::path::{Component, PathBuf};

use crate::{BackendError, HostProbe, HostVirtualizationClass, ProgramInvocation, ValidatedVmSpec};

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
            config_json.push('"');
            config_json.push_str(&json_escape(&disk.to_string_lossy()));
            config_json.push('"');
        }
        config_json.push_str("]}");

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
