use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io,
    os::fd::{AsFd, BorrowedFd},
    path::Path,
    time::Duration,
};

use drm::control::{Device as ControlDevice, connector};

use crate::data::source::{Panicked, PolledSource, Source, blocking_poll};

const DRM_DEVICES: &str = "/dev/dri";
const DPMS_ON: u64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenEdge {
    Off,
    On,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorPower {
    pub name: String,
    pub powered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenPower {
    pub connectors: Vec<ConnectorPower>,
}

impl ScreenPower {
    pub fn edge(&self) -> ScreenEdge {
        let all_dark = !self.connectors.is_empty() && self.connectors.iter().all(|c| !c.powered);
        if all_dark {
            ScreenEdge::Off
        } else {
            ScreenEdge::On
        }
    }
}

impl Source for ScreenPower {
    type Error = String;
    type Key = BTreeSet<String>;

    const NAME: &'static str = "drm connector power";
}

impl From<Panicked> for String {
    fn from(panicked: Panicked) -> Self {
        panicked.to_string()
    }
}

impl PolledSource for ScreenPower {
    const INTERVAL: Duration = Duration::from_secs(1);

    async fn poll(outputs: BTreeSet<String>) -> Result<Self, String> {
        blocking_poll(move || read_all_cards(&outputs)).await
    }
}

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Card {}

impl ControlDevice for Card {}

fn read_all_cards(outputs: &BTreeSet<String>) -> Result<ScreenPower, String> {
    let entries = fs::read_dir(DRM_DEVICES).map_err(|e| format!("{DRM_DEVICES}: {e}"))?;
    let connectors = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("card"))
        })
        .filter_map(|path| open_card(&path).ok())
        .flat_map(|card| connectors_of(&card, outputs))
        .collect::<Vec<ConnectorPower>>();
    let unaccounted: Vec<&str> = outputs
        .iter()
        .filter(|output| !connectors.iter().any(|c| &c.name == *output))
        .map(String::as_str)
        .collect();
    if unaccounted.is_empty() {
        Ok(ScreenPower { connectors })
    } else {
        Err(format!(
            "no DRM connector with a DPMS property for {}",
            unaccounted.join(", ")
        ))
    }
}

fn open_card(path: &Path) -> io::Result<Card> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map(Card)
}

fn connectors_of(card: &Card, outputs: &BTreeSet<String>) -> Vec<ConnectorPower> {
    let Ok(resources) = card.resource_handles() else {
        return Vec::new();
    };
    resources
        .connectors()
        .iter()
        .filter_map(|handle| card.get_connector(*handle, false).ok())
        .filter(|info| info.state() == connector::State::Connected)
        .map(|info| {
            let name = format!("{}-{}", info.interface().as_str(), info.interface_id());
            (name, info.handle())
        })
        .filter(|(name, _)| outputs.contains(name))
        .filter_map(|(name, handle)| {
            Some(ConnectorPower {
                powered: dpms_value(card, handle)? == DPMS_ON,
                name,
            })
        })
        .collect()
}

fn dpms_value(card: &Card, handle: connector::Handle) -> Option<u64> {
    let properties = card.get_properties(handle).ok()?;
    let (handles, values) = properties.as_props_and_values();
    handles
        .iter()
        .zip(values)
        .find(|(property, _)| {
            card.get_property(**property)
                .is_ok_and(|info| info.name().to_bytes() == b"DPMS")
        })
        .map(|(_, value)| *value)
}
