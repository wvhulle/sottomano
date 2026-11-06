use std::collections::HashMap;

#[cfg(feature = "libvirt-domains")]
use virt::{
    connect::Connect,
    domain::Domain,
    sys::{
        VIR_DOMAIN_PAUSED, VIR_DOMAIN_PMSUSPENDED, VIR_DOMAIN_RUNNING, VIR_DOMAIN_SHUTDOWN,
        virDomainState,
    },
};

use crate::qemu_virtual_machines::qmp_monitor_events::GuestRunState;

#[cfg(feature = "libvirt-domains")]
const LIBVIRT_SYSTEM_URI: &str = "qemu:///system";

pub const LIBVIRT_DOMAIN_CONTROL_BUILT: bool = cfg!(feature = "libvirt-domains");

#[cfg(feature = "libvirt-domains")]
fn run_state_of_domain_state(domain_state: virDomainState) -> GuestRunState {
    match domain_state {
        VIR_DOMAIN_RUNNING => GuestRunState::Running,
        VIR_DOMAIN_PAUSED | VIR_DOMAIN_PMSUSPENDED => GuestRunState::Paused,
        VIR_DOMAIN_SHUTDOWN => GuestRunState::ShuttingDown,
        _ => GuestRunState::Unreported,
    }
}

#[cfg(feature = "libvirt-domains")]
fn with_system_connection<T>(use_connection: impl FnOnce(&Connect) -> T) -> Result<T, String> {
    let mut connection =
        Connect::open(Some(LIBVIRT_SYSTEM_URI)).map_err(|failure| failure.to_string())?;
    let outcome = use_connection(&connection);
    let _ = connection.close();
    Ok(outcome)
}

#[cfg(feature = "libvirt-domains")]
#[must_use]
pub(crate) fn libvirt_run_states(domain_names: &[String]) -> HashMap<String, GuestRunState> {
    with_system_connection(|connection| {
        domain_names
            .iter()
            .filter_map(|domain_name| {
                let domain = Domain::lookup_by_name(connection, domain_name).ok()?;
                let (domain_state, _reason) = domain.get_state().ok()?;
                Some((domain_name.clone(), run_state_of_domain_state(domain_state)))
            })
            .collect()
    })
    .unwrap_or_else(|failure| {
        tracing::debug!("libvirt states: {failure}");
        HashMap::new()
    })
}

#[cfg(not(feature = "libvirt-domains"))]
#[must_use]
pub(crate) fn libvirt_run_states(_domain_names: &[String]) -> HashMap<String, GuestRunState> {
    HashMap::new()
}

#[cfg(feature = "libvirt-domains")]
pub fn request_libvirt_shutdown(domain_name: &str) -> Result<(), String> {
    with_system_connection(|connection| {
        Domain::lookup_by_name(connection, domain_name)
            .and_then(|domain| domain.shutdown())
            .map(drop)
            .map_err(|failure| failure.to_string())
    })?
}

#[cfg(not(feature = "libvirt-domains"))]
pub fn request_libvirt_shutdown(_domain_name: &str) -> Result<(), String> {
    Err("sottomano was built without the libvirt-domains feature".to_owned())
}
