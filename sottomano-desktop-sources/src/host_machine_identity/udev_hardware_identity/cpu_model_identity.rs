use std::fs;

use serde_json::Value;

use crate::{
    host_machine_identity::udev_hardware_identity::{
        HardwareText,
        cpu_cache_levels::{CacheLevel, cache_levels},
        reported,
    },
    system_protocol_clients::shell_command_output,
};

const CPU_INFO: &str = "/proc/cpuinfo";
const INXI_PROGRAM: &str = "inxi";
const INXI_CPU_REPORT_ARGS: [&str; 8] = [
    "--tty",
    "--cpu",
    "-xxx",
    "-a",
    "--output",
    "json",
    "--output-file",
    "print",
];

fn cpuinfo_field<'cpuinfo>(core: &'cpuinfo str, name: &str) -> Option<&'cpuinfo str> {
    core.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then(|| value.trim())
    })
}

fn cpuinfo_text(core: &str, name: &str) -> HardwareText {
    cpuinfo_field(core, name).map_or(HardwareText::Unreported, reported)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuModel {
    pub identity: CpuIdentity,
    pub architecture: CpuArchitecture,
    pub topology: CpuCoreCounts,
    pub caches: Vec<CacheLevel>,
}

impl CpuModel {
    #[must_use]
    pub fn current() -> Self {
        let cpuinfo = fs::read_to_string(CPU_INFO).unwrap_or_else(|read_failure| {
            tracing::warn!("cpu model: cannot read {CPU_INFO}: {read_failure}");
            String::new()
        });
        let first_core = cpuinfo.split("\n\n").next().unwrap_or_default();
        CpuModel {
            identity: CpuIdentity::parse(first_core),
            architecture: CpuArchitecture::current(),
            topology: CpuCoreCounts::parse(first_core),
            caches: cache_levels(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuIdentity {
    pub vendor: CpuVendor,
    pub name: HardwareText,
    pub microcode: HardwareText,
}

impl CpuIdentity {
    fn parse(first_core: &str) -> Self {
        CpuIdentity {
            vendor: CpuVendor::reported_by_core(first_core),
            name: cpuinfo_text(first_core, "model name"),
            microcode: cpuinfo_text(first_core, "microcode"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CpuVendor {
    Amd,
    Intel,
    Arm,
    Apple,
    Qualcomm,
    HiSilicon,
    #[default]
    Unrecognised,
}

impl CpuVendor {
    fn reported_by_core(first_core: &str) -> Self {
        match cpuinfo_field(first_core, "vendor_id") {
            Some("AuthenticAMD") => return CpuVendor::Amd,
            Some("GenuineIntel") => return CpuVendor::Intel,
            _ => {}
        }
        match cpuinfo_field(first_core, "CPU implementer") {
            Some("0x41") => CpuVendor::Arm,
            Some("0x61") => CpuVendor::Apple,
            Some("0x51") => CpuVendor::Qualcomm,
            Some("0x48") => CpuVendor::HiSilicon,
            _ => CpuVendor::Unrecognised,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CpuCoreCounts {
    Counted {
        cores: u32,
        threads: u32,
    },
    #[default]
    UnlistedInCpuinfo,
}

impl CpuCoreCounts {
    fn parse(first_core: &str) -> Self {
        let count = |name| {
            let count_text = cpuinfo_field(first_core, name)?;
            count_text
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "cpu model: invalid {name} count {count_text:?}: {parse_failure}"
                    );
                })
                .ok()
        };
        let Some(cores) = count("cpu cores") else {
            return CpuCoreCounts::UnlistedInCpuinfo;
        };
        let Some(threads) = count("siblings") else {
            return CpuCoreCounts::UnlistedInCpuinfo;
        };
        CpuCoreCounts::Counted { cores, threads }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuArchitecture {
    pub microarchitecture: HardwareText,
    pub process: HardwareText,
    pub released: HardwareText,
    pub x86_64_level: HardwareText,
}

impl CpuArchitecture {
    fn current() -> Self {
        shell_command_output::blocking::output(INXI_PROGRAM, &INXI_CPU_REPORT_ARGS)
            .inspect_err(|command_failure| {
                tracing::debug!("cpu architecture: cannot run {INXI_PROGRAM}: {command_failure}");
            })
            .ok()
            .and_then(|report| {
                serde_json::from_str(&report)
                    .inspect_err(|json_failure| {
                        tracing::warn!(
                            "cpu architecture: invalid {INXI_PROGRAM} json report: {json_failure}"
                        );
                    })
                    .ok()
            })
            .map(|report| Self::from(&report))
            .unwrap_or_default()
    }
}

impl From<&Value> for CpuArchitecture {
    fn from(report: &Value) -> Self {
        let field = |name| {
            inxi_field(report, name).map_or(HardwareText::Unreported, |text| reported(&text))
        };
        CpuArchitecture {
            microarchitecture: field("arch"),
            process: field("process"),
            released: field("built"),
            x86_64_level: field("level"),
        }
    }
}

fn inxi_field(report: &Value, name: &str) -> Option<String> {
    match report {
        Value::Array(entries) => entries.iter().find_map(|entry| inxi_field(entry, name)),
        Value::Object(fields) => fields.iter().find_map(|(key, value)| {
            match key.rsplit_once('#').map(|(_, field_name)| field_name) {
                Some(field_name) if field_name == name => inxi_scalar(value),
                _ => inxi_field(value, name),
            }
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

fn inxi_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAPHAEL_CPUINFO_CORE: &str = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: \
                                        AMD Ryzen 9 7950X3D 16-Core Processor\nmicrocode\t: \
                                        0xa60120c\nsiblings\t: 32\ncpu cores\t: 16\n";

    const RAPHAEL_INXI_REPORT: &str = r#"[{"000#1#0#CPU":[{"004#1#2#arch":"Zen 4","008#0#2#built":"2022+","009#0#2#process":"TSMC n5 (5nm)","006#1#2#level":"v4","002#0#2#bits":64},{"018#1#3#cores":16}]}]"#;

    #[test]
    fn cpuinfo_yields_name_microcode_and_topology() {
        assert_eq!(
            CpuIdentity::parse(RAPHAEL_CPUINFO_CORE),
            CpuIdentity {
                vendor: CpuVendor::Amd,
                name: HardwareText::Reported("AMD Ryzen 9 7950X3D 16-Core Processor".to_owned()),
                microcode: HardwareText::Reported("0xa60120c".to_owned()),
            }
        );
        assert_eq!(
            CpuCoreCounts::parse(RAPHAEL_CPUINFO_CORE),
            CpuCoreCounts::Counted {
                cores: 16,
                threads: 32,
            }
        );
    }

    #[test]
    fn cpuinfo_vendor_covers_x86_and_arm_implementers() {
        assert_eq!(
            CpuVendor::reported_by_core("vendor_id\t: GenuineIntel\n"),
            CpuVendor::Intel
        );
        assert_eq!(
            CpuVendor::reported_by_core(
                "processor\t: 0\nCPU implementer\t: 0x61\nCPU part\t: 0x022\n"
            ),
            CpuVendor::Apple
        );
        assert_eq!(
            CpuVendor::reported_by_core("vendor_id\t: HygonGenuine\n"),
            CpuVendor::Unrecognised
        );
    }

    #[test]
    fn inxi_report_fields_are_found_by_name() {
        let report = serde_json::from_str(RAPHAEL_INXI_REPORT).unwrap();
        assert_eq!(
            CpuArchitecture::from(&report),
            CpuArchitecture {
                microarchitecture: HardwareText::Reported("Zen 4".to_owned()),
                process: HardwareText::Reported("TSMC n5 (5nm)".to_owned()),
                released: HardwareText::Reported("2022+".to_owned()),
                x86_64_level: HardwareText::Reported("v4".to_owned()),
            }
        );
    }
}
