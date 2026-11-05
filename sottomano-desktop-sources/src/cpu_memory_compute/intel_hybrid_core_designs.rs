use std::collections::HashMap;

use super::cpu_topology::{CoreDesign, cpulist};
use crate::system_protocol_clients::sysfs_attribute_reads::read_trimmed;

const INTEL_PERFORMANCE_PMU: &str = "/sys/devices/cpu_core/cpus";
const INTEL_EFFICIENT_PMU: &str = "/sys/devices/cpu_atom/cpus";

pub(super) fn read() -> Option<HashMap<u32, CoreDesign>> {
    let performance = read_trimmed(INTEL_PERFORMANCE_PMU)?;
    let efficient = read_trimmed(INTEL_EFFICIENT_PMU)?;
    Some(from_cpulists(&performance, &efficient))
}

fn from_cpulists(performance: &str, efficient: &str) -> HashMap<u32, CoreDesign> {
    let performance = cpulist(performance).map(|cpu| (cpu, CoreDesign::IntelPerformanceCore));
    let efficient = cpulist(efficient).map(|cpu| (cpu, CoreDesign::IntelEfficientCore));
    performance.chain(efficient).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pmu_lists_split_performance_from_efficient() {
        let designs = from_cpulists("0-1", "2-3");
        assert_eq!(designs[&1], CoreDesign::IntelPerformanceCore);
        assert_eq!(designs[&2], CoreDesign::IntelEfficientCore);
    }
}
