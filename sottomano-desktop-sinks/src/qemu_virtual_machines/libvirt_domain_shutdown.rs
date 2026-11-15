use std::path::{Path, PathBuf};

#[cfg(feature = "libvirt-domains")]
use sottomano_desktop_sources::qemu_virtual_machines::libvirt_domain_control::with_system_connection;
use sottomano_desktop_sources::qemu_virtual_machines::qmp_monitor_events::send_qmp_powerdown;
#[cfg(feature = "libvirt-domains")]
use tokio::task::spawn_blocking;
#[cfg(feature = "libvirt-domains")]
use virt::domain::Domain;

use crate::sink_write_errors::GuestPowerError;

#[cfg(feature = "libvirt-domains")]
pub async fn request_libvirt_shutdown(domain_name: String) -> Result<(), GuestPowerError> {
    spawn_blocking(move || {
        with_system_connection(|connection| {
            Domain::lookup_by_name(connection, &domain_name)?.shutdown()
        })??;
        Ok(())
    })
    .await?
}

#[cfg(feature = "libvirt-domains")]
pub async fn request_libvirt_start(domain_name: String) -> Result<(), GuestPowerError> {
    spawn_blocking(move || {
        with_system_connection(|connection| {
            Domain::lookup_by_name(connection, &domain_name)?.create()
        })??;
        Ok(())
    })
    .await?
}

#[cfg(not(feature = "libvirt-domains"))]
pub async fn request_libvirt_start(_domain_name: String) -> Result<(), GuestPowerError> {
    Err(GuestPowerError::LibvirtNotBuilt)
}

#[cfg(not(feature = "libvirt-domains"))]
pub async fn request_libvirt_shutdown(_domain_name: String) -> Result<(), GuestPowerError> {
    Err(GuestPowerError::LibvirtNotBuilt)
}

pub async fn request_qmp_powerdown(socket_path: &Path) -> Result<(), GuestPowerError> {
    send_qmp_powerdown(socket_path)
        .await
        .map_err(|source| GuestPowerError::Monitor {
            socket_path: PathBuf::from(socket_path),
            source,
        })
}
