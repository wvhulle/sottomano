use std::time::Duration;

use sottomano_desktop_sources::{
    container_run_state::RunChange,
    systemd_units_journal::{
        SystemdUnitScope,
        unit_active_state::{UnitChangeFailure, UnitControl},
    },
};

pub const HIBERNATE_DELAY_UNIT_PREFIX: &str = "sottomano-hibernate-delay@";
fn hibernate_delay_unit(asleep_for: Duration) -> String {
    format!(
        "{HIBERNATE_DELAY_UNIT_PREFIX}{}.service",
        asleep_for.as_secs()
    )
}

pub async fn apply_hibernate_delay(asleep_for: Duration) -> Result<(), UnitChangeFailure> {
    UnitControl {
        unit_scope: SystemdUnitScope::SystemWide,
        unit: hibernate_delay_unit(asleep_for),
    }
    .apply(RunChange::Start)
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::hibernate_delay_unit;

    #[test]
    fn the_delay_travels_as_whole_seconds_in_the_instance_name() {
        assert_eq!(
            hibernate_delay_unit(Duration::from_hours(3)),
            "sottomano-hibernate-delay@10800.service"
        );
    }
}
