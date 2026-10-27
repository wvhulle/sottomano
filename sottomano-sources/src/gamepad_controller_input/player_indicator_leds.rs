use std::{collections::BTreeMap, path::Path};

use super::playstation_player_leds::playstation_player_number;
use crate::system_protocol_clients::sysfs_attribute_reads;

const PLAYER_LED_MARKER: &str = ":white:player-";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LitPlayer {
    pub number: u8,
    pub leds_lit: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlayerIndicator {
    Lit(LitPlayer),
    Unlit,
}

fn player_leds_by_position(hid_device: &Path) -> BTreeMap<u8, bool> {
    sysfs_attribute_reads::list_dir(hid_device.join("leds"))
        .into_iter()
        .filter_map(|led| {
            let name = led.file_name()?.to_str()?.to_owned();
            let position = name.split_once(PLAYER_LED_MARKER)?.1.parse().ok()?;
            let lit = sysfs_attribute_reads::read_i64(led.join("brightness"))? > 0;
            Some((position, lit))
        })
        .collect()
}

fn driver_name(hid_device: &Path) -> Option<String> {
    Some(
        hid_device
            .join("driver")
            .canonicalize()
            .ok()?
            .file_name()?
            .to_str()?
            .to_owned(),
    )
}

impl PlayerIndicator {
    #[must_use]
    pub fn of_input_node(slot: usize) -> Self {
        let hid_device =
            Path::new(&format!("/sys/class/input/event{slot}/device/device")).to_owned();
        let leds = player_leds_by_position(&hid_device);
        let lit_positions = leds
            .iter()
            .filter(|(_, lit)| **lit)
            .map(|(position, _)| *position)
            .collect();
        let player_number = match driver_name(&hid_device).as_deref() {
            Some("playstation") => playstation_player_number(&lit_positions),
            _ => None,
        };
        player_number.map_or(PlayerIndicator::Unlit, |number| {
            PlayerIndicator::Lit(LitPlayer {
                number,
                leds_lit: leds.into_values().collect(),
            })
        })
    }
}
