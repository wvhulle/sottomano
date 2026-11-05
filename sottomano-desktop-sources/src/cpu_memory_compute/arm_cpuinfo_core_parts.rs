use std::{collections::HashMap, fs};

use super::{CPU_FREQ, cpu_topology::CoreDesign};
use crate::system_protocol_clients::sysfs_attribute_reads::read_trimmed;

const CPU_INFO: &str = "/proc/cpuinfo";

pub(super) fn read() -> Option<HashMap<u32, CoreDesign>> {
    let cpuinfo = fs::read_to_string(CPU_INFO).ok()?;
    Some(
        CorePart::parse_all(&cpuinfo)
            .filter_map(|core_part| Some((core_part.core, core_part.design_with_capacity()?)))
            .collect(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CorePart {
    core: u32,
    part: String,
}

impl CorePart {
    fn parse_all(cpuinfo: &str) -> impl Iterator<Item = CorePart> + '_ {
        cpuinfo.split("\n\n").filter_map(Self::parse)
    }

    fn parse(block: &str) -> Option<Self> {
        Some(CorePart {
            core: cpuinfo_field(block, "processor")?.parse().ok()?,
            part: cpuinfo_field(block, "CPU part")?.to_owned(),
        })
    }

    fn design_with_capacity(&self) -> Option<CoreDesign> {
        let capacity = read_trimmed(format!("{CPU_FREQ}/cpu{}/cpu_capacity", self.core))?;
        Some(CoreDesign::ArmCorePart {
            part: self.part.clone(),
            capacity: capacity.parse().ok()?,
        })
    }
}

fn cpuinfo_field<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    block.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then(|| value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpuinfo_blocks_yield_each_cores_part() {
        let cpuinfo = "processor\t: 0\nCPU part\t: 0xd05\n\nprocessor\t: 1\nCPU part\t: 0xd0b\n";
        assert_eq!(
            CorePart::parse_all(cpuinfo).collect::<Vec<_>>(),
            [
                CorePart {
                    core: 0,
                    part: "0xd05".to_owned()
                },
                CorePart {
                    core: 1,
                    part: "0xd0b".to_owned()
                },
            ]
        );
    }
}
