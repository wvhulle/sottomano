use std::{io, path::Path};

#[cfg(feature = "libvirt-domains")]
use sottomano_desktop_sources::qemu_virtual_machines::libvirt_domain_control::with_system_connection;
use sottomano_desktop_sources::qemu_virtual_machines::qmp_monitor_events::send_qmp_powerdown;
#[cfg(feature = "libvirt-domains")]
use virt::domain::Domain;

#[cfg(feature = "libvirt-domains")]
pub fn request_libvirt_shutdown(domain_name: &str) -> Result<(), String> {
    with_system_connection(|connection| {
        Domain::lookup_by_name(connection, domain_name)
            .and_then(|domain| domain.shutdown())
            .map(drop)
            .map_err(|failure| failure.to_string())
    })?
}

#[cfg(feature = "libvirt-domains")]
pub fn request_libvirt_start(domain_name: &str) -> Result<(), String> {
    with_system_connection(|connection| {
        Domain::lookup_by_name(connection, domain_name)
            .and_then(|domain| domain.create())
            .map(drop)
            .map_err(|failure| failure.to_string())
    })?
}

#[cfg(not(feature = "libvirt-domains"))]
pub fn request_libvirt_start(_domain_name: &str) -> Result<(), String> {
    Err("sottomano was built without the libvirt-domains feature".to_owned())
}

#[cfg(not(feature = "libvirt-domains"))]
pub fn request_libvirt_shutdown(_domain_name: &str) -> Result<(), String> {
    Err("sottomano was built without the libvirt-domains feature".to_owned())
}

pub async fn request_qmp_powerdown(socket_path: &Path) -> io::Result<()> {
    send_qmp_powerdown(socket_path).await
}
