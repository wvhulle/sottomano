use sottomano_desktop_sources::systemd_units_journal::{
    ManagerProxy, connect_systemd_bus, failed_units::FailedUnitRestart,
};

pub async fn restart_failed_unit(failed_unit_restart: FailedUnitRestart) {
    if let Err(restart_failure) = restart_unit_over_bus(&failed_unit_restart).await {
        tracing::warn!(
            "systemd: restarting {} failed: {restart_failure}",
            failed_unit_restart.unit
        );
    }
}

async fn restart_unit_over_bus(failed_unit_restart: &FailedUnitRestart) -> zbus::Result<()> {
    let connection = connect_systemd_bus(failed_unit_restart.unit_scope).await?;
    ManagerProxy::new(&connection)
        .await?
        .restart_unit(&failed_unit_restart.unit, "replace")
        .await?;
    Ok(())
}
