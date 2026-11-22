use std::collections::HashMap;

use futures::{StreamExt, stream, try_join};
use uom::si::{
    f64::{Frequency, InformationRate},
    frequency::megahertz,
    information_rate::kilobit_per_second,
};
use zbus::{Connection, zvariant::OwnedObjectPath};

use super::{
    AccessPointProxy, BUS_OBJECT_READS_IN_FLIGHT, DeviceInfo, SettingsConnectionProxy,
    WirelessProxy, readable_objects, settings_text, shown_ssid, uncached,
};
use crate::{non_empty_text::non_empty, system_protocol_clients::dbus_desktop_buses::prop};

pub async fn wireless_rate(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<InformationRate> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    let kilobits_per_second = wireless.bitrate().await?;
    Ok(InformationRate::new::<kilobit_per_second>(f64::from(
        kilobits_per_second,
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkName {
    Broadcast(String),
    Hidden,
}

impl From<Vec<u8>> for NetworkName {
    fn from(ssid_bytes: Vec<u8>) -> Self {
        let shown = shown_ssid(ssid_bytes);
        if shown.is_empty() {
            Self::Hidden
        } else {
            Self::Broadcast(shown)
        }
    }
}

pub struct ApInfo {
    pub ssid: NetworkName,
    pub signal: i64,
    pub frequency: Frequency,
    pub security: String,
}

pub async fn active_access_point(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<OwnedObjectPath> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    wireless.active_access_point().await
}

pub async fn access_point(
    connection: &Connection,
    ap_path: OwnedObjectPath,
) -> zbus::Result<ApInfo> {
    let ap: AccessPointProxy = uncached(connection, ap_path).await?;
    let (ssid, strength, frequency, flags, wpa, rsn) = try_join!(
        ap.ssid(),
        ap.strength(),
        ap.frequency(),
        ap.flags(),
        ap.wpa_flags(),
        ap.rsn_flags()
    )?;
    Ok(ApInfo {
        ssid: NetworkName::from(ssid),
        signal: i64::from(strength),
        frequency: Frequency::new::<megahertz>(f64::from(frequency)),
        security: security(flags, wpa, rsn),
    })
}

pub async fn visible_access_points(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<Vec<ApInfo>> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    let paths = wireless.get_all_access_points().await?;
    let access_points = stream::iter(paths)
        .map(|path| access_point(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await;
    Ok(readable_objects(access_points, "access point").collect())
}

pub async fn request_wireless_scan(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<()> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    wireless.request_scan(HashMap::new()).await
}

pub struct WirelessProfile {
    pub settings_connection: OwnedObjectPath,
    pub profile_name: String,
    pub ssid: String,
}

pub async fn available_wireless_profiles(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<Vec<WirelessProfile>> {
    let paths = device.proxy.available_connections().await?;
    let profiles = stream::iter(paths)
        .map(|path| wireless_profile(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await;
    Ok(readable_objects(profiles, "wireless profile")
        .flatten()
        .collect())
}

async fn wireless_profile(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<Option<WirelessProfile>> {
    let settings: SettingsConnectionProxy = uncached(connection, path.clone()).await?;
    let profile = settings.get_settings().await?;
    let ssid = profile
        .get("802-11-wireless")
        .and_then(|section| prop::<Vec<u8>>(section, "ssid"))
        .and_then(|ssid_bytes| non_empty(shown_ssid(ssid_bytes)));
    let Some(ssid) = ssid else {
        return Ok(None);
    };
    let profile_name = profile
        .get("connection")
        .and_then(|section| settings_text(section, "id"))
        .unwrap_or_else(|| ssid.clone());
    Ok(Some(WirelessProfile {
        settings_connection: path,
        profile_name,
        ssid,
    }))
}

const AP_PRIVACY: u32 = 0x1;
const KEY_MGMT_8021X: u32 = 0x200;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

fn security(flags: u32, wpa: u32, rsn: u32) -> String {
    let parts = [
        (flags & AP_PRIVACY != 0 && wpa == 0 && rsn == 0).then_some("WEP"),
        (wpa != 0).then_some("WPA1"),
        (rsn & (KEY_MGMT_PSK | KEY_MGMT_8021X) != 0).then_some("WPA2"),
        (rsn & KEY_MGMT_SAE != 0).then_some("WPA3"),
        (rsn & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0).then_some("OWE"),
        (wpa & KEY_MGMT_8021X != 0 || rsn & (KEY_MGMT_8021X | KEY_MGMT_EAP_SUITE_B_192) != 0)
            .then_some("802.1X"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if parts.is_empty() {
        "Open".to_owned()
    } else {
        parts
    }
}
