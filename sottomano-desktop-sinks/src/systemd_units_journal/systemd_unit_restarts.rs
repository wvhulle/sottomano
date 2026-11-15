use sottomano_desktop_sources::systemd_units_journal::{
    ManagerProxy, connect_systemd_bus, failed_units::FailedUnitRestart,
};

use crate::sink_write_errors::BusWriteError;

pub async fn restart_failed_unit(
    failed_unit_restart: FailedUnitRestart,
) -> Result<(), BusWriteError> {
    let connection = connect_systemd_bus(failed_unit_restart.unit_scope).await?;
    ManagerProxy::new(&connection)
        .await?
        .restart_unit(&failed_unit_restart.unit, "replace")
        .await?;
    Ok(())
}
