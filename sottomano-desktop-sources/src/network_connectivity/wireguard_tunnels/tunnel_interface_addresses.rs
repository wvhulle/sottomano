use futures::{Stream, TryStreamExt};
use serde_json::Value as Json;

use super::TunnelAddress;
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeTriggerStreams, ChangeWatchFailure},
    network_connectivity::network_manager_proxies,
    system_protocol_clients::{
        dbus_desktop_buses::OnBus,
        shell_command_output::{self, WatchedCommandError},
    },
};

impl SnapshotSource for TunnelAddress {
    type Error = WatchedCommandError;
    type StreamInputs = OnBus<String>;

    const NAME: &'static str = "Tunnel address";

    fn delivery_stream(
        watched: OnBus<String>,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(watched, address_changes)
    }
}

async fn address_changes(
    watched: OnBus<String>,
) -> Result<
    impl Stream<Item = Result<TunnelAddress, WatchedCommandError>> + Send,
    WatchedCommandError,
> {
    let addressing = network_manager_proxies::changes(&watched.buses.system, ADDRESSING_PROPERTIES)
        .await
        .map_err(ChangeWatchFailure::from)?
        .map_err(ChangeWatchFailure::from);
    Ok(addressing.triggered_rereads(move || {
        let tunnel = watched.inputs.clone();
        async move { Ok(interface_ip(&tunnel).await) }
    }))
}

const ADDRESSING_PROPERTIES: &[&str] = &[
    network_manager_proxies::DEVICE_PROPERTIES,
    network_manager_proxies::IP4_CONFIG_PROPERTIES,
];

async fn interface_ip(iface: &str) -> TunnelAddress {
    let Ok(address_listing) =
        shell_command_output::output("ip", &["-j", "-4", "addr", "show", iface])
            .await
            .inspect_err(|listing_error| {
                tracing::debug!("wireguard: cannot list addresses of {iface}: {listing_error}");
            })
    else {
        return TunnelAddress::Unassigned;
    };
    serde_json::from_str::<Json>(&address_listing)
        .inspect_err(|parse_failure| {
            tracing::warn!("wireguard: invalid address listing for {iface}: {parse_failure}");
        })
        .ok()
        .and_then(|json| {
            let entry = json.as_array()?.first()?;
            let addr = entry.get("addr_info")?.as_array()?.first()?;
            let local = addr.get("local")?.as_str()?;
            let prefix = addr.get("prefixlen").and_then(Json::as_u64).unwrap_or(32);
            Some(format!("{local}/{prefix}"))
        })
        .map_or(TunnelAddress::Unassigned, TunnelAddress::Assigned)
}
