use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr},
};

use futures::try_join;
use zbus::{
    Connection,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use super::{DeviceInfo, Ip4ConfigProxy, Ip6ConfigProxy, assigned_object_path, uncached};
use crate::system_protocol_clients::dbus_desktop_buses::prop;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterGateway {
    Routed(String),
    Unrouted,
}

impl From<String> for RouterGateway {
    fn from(gateway: String) -> Self {
        if gateway.is_empty() {
            Self::Unrouted
        } else {
            Self::Routed(gateway)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressBlock {
    pub address: String,
    pub mask: String,
    pub gateway: RouterGateway,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressAssignment {
    Assigned(AddressBlock),
    Unassigned,
}

#[derive(Debug)]
pub struct IpInfo {
    pub v4: AddressAssignment,
    pub v6: AddressAssignment,
    pub dns: Vec<String>,
}

pub struct FamilyConfig {
    pub(super) block: AddressAssignment,
    pub(super) dns: Vec<String>,
}

impl FamilyConfig {
    fn unconfigured() -> Self {
        Self {
            block: AddressAssignment::Unassigned,
            dns: Vec::new(),
        }
    }
}

pub async fn ip(connection: &Connection, device: &DeviceInfo) -> zbus::Result<IpInfo> {
    let (v4_path, v6_path) = try_join!(device.proxy.ip4_config(), device.proxy.ip6_config())?;
    let (v4, v6) = try_join!(
        ip4_config(connection, v4_path),
        ip6_config(connection, v6_path)
    )?;
    Ok(IpInfo {
        v4: v4.block,
        v6: v6.block,
        dns: v4.dns.into_iter().chain(v6.dns).collect(),
    })
}

fn first_address(entries: &[HashMap<String, OwnedValue>]) -> Option<(String, u32)> {
    let global = entries.iter().find(|entry| {
        prop::<String>(entry, "address").is_some_and(|address| !address.starts_with("fe80"))
    });
    let entry = global.or_else(|| entries.first())?;
    let address = prop::<String>(entry, "address")?;
    let prefix = prop::<u32>(entry, "prefix")?;
    Some((address, prefix))
}

pub(super) async fn ip4_config(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<FamilyConfig> {
    let Some(path) = assigned_object_path(path) else {
        return Ok(FamilyConfig::unconfigured());
    };
    let config: Ip4ConfigProxy = uncached(connection, path).await?;
    let (servers, addresses, gateway) = try_join!(
        config.nameserver_data(),
        config.address_data(),
        config.gateway()
    )?;
    Ok(FamilyConfig {
        block: first_address(&addresses).map_or(
            AddressAssignment::Unassigned,
            |(address, prefix)| {
                AddressAssignment::Assigned(AddressBlock {
                    address,
                    mask: prefix_to_netmask(prefix),
                    gateway: RouterGateway::from(gateway),
                })
            },
        ),
        dns: servers
            .into_iter()
            .filter_map(|entry| prop::<String>(&entry, "address"))
            .collect(),
    })
}

pub(super) async fn ip6_config(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<FamilyConfig> {
    let Some(path) = assigned_object_path(path) else {
        return Ok(FamilyConfig::unconfigured());
    };
    let config: Ip6ConfigProxy = uncached(connection, path).await?;
    let (servers, addresses, gateway) = try_join!(
        config.nameservers(),
        config.address_data(),
        config.gateway()
    )?;
    Ok(FamilyConfig {
        block: first_address(&addresses).map_or(
            AddressAssignment::Unassigned,
            |(address, prefix)| {
                AddressAssignment::Assigned(AddressBlock {
                    address,
                    mask: format!("/{prefix}"),
                    gateway: RouterGateway::from(gateway),
                })
            },
        ),
        dns: servers
            .into_iter()
            .filter_map(|bytes| {
                <[u8; 16]>::try_from(bytes.as_slice())
                    .inspect_err(|_| {
                        tracing::warn!(
                            "network manager: ipv6 name server has {} bytes instead of 16",
                            bytes.len()
                        );
                    })
                    .ok()
                    .map(|raw_address| Ipv6Addr::from(raw_address).to_string())
            })
            .collect(),
    })
}

fn prefix_to_netmask(prefix: u32) -> String {
    let bits = match prefix {
        0 => 0,
        32.. => !0,
        _ => !0u32 << (32 - prefix),
    };
    Ipv4Addr::from(bits).to_string()
}
