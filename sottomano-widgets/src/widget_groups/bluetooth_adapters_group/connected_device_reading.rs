use std::iter::once;

use chrono::Locale;
use sottomano_desktop_sources::{
    network_connectivity::bluez_bluetooth::{
        ChargingState, ConnectedDevice, DeviceBattery, DeviceKind,
    },
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::bluetooth_adapters_group::reconnect_churn_counting::RECONNECT_HORIZON,
    widgets::{
        icon_glyph::IconGlyph,
        input_report_sections::{KernelErrorListing, MeasuredReportTiming},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const SHORT_LABEL_CHARACTERS: usize = 8;
const CRITICAL_BATTERY_PERCENT: u8 = 10;
const LOW_BATTERY_PERCENT: u8 = 20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputReports {
    NotStreamed,
    Streamed(MeasuredReportTiming),
}

pub struct ConnectedDeviceReading<'a> {
    pub device: &'a ConnectedDevice,
    pub short_label: String,
    pub recent_reconnects: usize,
    pub kernel_errors: Vec<&'a DeviceKernelError>,
    pub input_reports: InputReports,
}

fn leading_words(name: &str, word_count: usize) -> String {
    let words: Vec<&str> = name.split_whitespace().take(word_count).collect();
    words
        .join(" ")
        .chars()
        .take(SHORT_LABEL_CHARACTERS * word_count)
        .collect()
}

#[must_use]
pub fn short_label(device: &ConnectedDevice, siblings: &[ConnectedDevice]) -> String {
    let first_word = leading_words(&device.name, 1);
    let sharing_first_word = siblings
        .iter()
        .filter(|sibling| leading_words(&sibling.name, 1) == first_word)
        .count();
    if sharing_first_word > 1 {
        leading_words(&device.name, 2)
    } else {
        first_word
    }
}

fn kind_glyph(kind: DeviceKind) -> IconGlyph {
    match kind {
        DeviceKind::Gamepad => Phosphor::GameController(Weight::Regular).into(),
        DeviceKind::Headset => Phosphor::Headset(Weight::Regular).into(),
        DeviceKind::Headphones => Phosphor::Headphones(Weight::Regular).into(),
        DeviceKind::Speaker => Phosphor::SpeakerHigh(Weight::Regular).into(),
        DeviceKind::Keyboard => Phosphor::Keyboard(Weight::Regular).into(),
        DeviceKind::Mouse => Phosphor::Mouse(Weight::Regular).into(),
        DeviceKind::Tablet => Phosphor::DeviceTablet(Weight::Regular).into(),
        DeviceKind::Phone => Phosphor::DeviceMobile(Weight::Regular).into(),
        DeviceKind::Computer => Phosphor::Laptop(Weight::Regular).into(),
        DeviceKind::Unclassified => Phosphor::Devices(Weight::Regular).into(),
    }
}

fn charging_description(charging: ChargingState) -> Option<&'static str> {
    match charging {
        ChargingState::Unknown => None,
        ChargingState::Charging => Some("charging"),
        ChargingState::Discharging => Some("discharging"),
        ChargingState::NotCharging => Some("not charging"),
        ChargingState::Full => Some("full"),
    }
}

impl ConnectedDeviceReading<'_> {
    pub const PRESS_DOES: &'static str = "disconnect this device";

    fn battery_draining_to(&self, threshold_percent: u8) -> bool {
        match self.device.battery {
            DeviceBattery::Reported {
                level_percent,
                charging,
            } => {
                level_percent <= threshold_percent
                    && !matches!(charging, ChargingState::Charging | ChargingState::Full)
            }
            DeviceBattery::Unreported => false,
        }
    }

    fn link_unstable(&self) -> bool {
        let lossy_reports = match self.input_reports {
            InputReports::Streamed(report_timing) => report_timing.lossy_or_stalling(),
            InputReports::NotStreamed => false,
        };
        lossy_reports || self.recent_reconnects > 0 || !self.kernel_errors.is_empty()
    }

    fn input_report_section(&self) -> Option<Section> {
        match self.input_reports {
            InputReports::Streamed(report_timing) => {
                Some(report_timing.report_section("Input reports, last 2 s"))
            }
            InputReports::NotStreamed => None,
        }
    }

    fn link_section(&self) -> Section {
        let horizon_minutes = RECONNECT_HORIZON.num_minutes();
        let reconnects = LabeledValue::new(
            format!("Reconnects, last {horizon_minutes} min"),
            Quantity::count(self.recent_reconnects),
        );
        let reconnects = if self.recent_reconnects > 0 {
            reconnects.level(Severity::Warning)
        } else {
            reconnects
        };
        Section::dictionary([reconnects]).titled("Link")
    }

    fn battery_section(&self) -> Option<Section> {
        let DeviceBattery::Reported {
            level_percent,
            charging,
        } = self.device.battery
        else {
            return None;
        };
        Some(
            Section::dictionary(
                once(LabeledValue::new(
                    "Level",
                    Quantity::whole_percent(level_percent),
                ))
                .chain(
                    charging_description(charging)
                        .map(|description| LabeledValue::new("Status", description)),
                ),
            )
            .titled("Battery"),
        )
    }
}

impl GroupItemReading for ConnectedDeviceReading<'_> {
    fn name(&self) -> &'static str {
        "Bluetooth device"
    }

    fn icon(&self) -> IconGlyph {
        kind_glyph(self.device.kind)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.short_label.clone())
    }

    fn gauge(&self) -> Option<Ratio> {
        match self.device.battery {
            DeviceBattery::Reported { level_percent, .. } => {
                Some(percent_fill(i64::from(level_percent)))
            }
            DeviceBattery::Unreported => None,
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.battery_draining_to(CRITICAL_BATTERY_PERCENT) {
            ReadingDisplayState::Critical
        } else if self.battery_draining_to(LOW_BATTERY_PERCENT) || self.link_unstable() {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(
            [
                Some(Section::note(self.device.name.clone())),
                self.input_report_section(),
                Some(self.link_section()),
                KernelErrorListing {
                    kernel_errors: &self.kernel_errors,
                }
                .section(),
                self.battery_section(),
            ]
            .into_iter()
            .flatten(),
        )
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::network_connectivity::bluez_bluetooth::DeviceBattery;
    use zbus::zvariant::OwnedObjectPath;

    use super::*;

    fn named_device(name: &str) -> ConnectedDevice {
        ConnectedDevice {
            object_path: OwnedObjectPath::try_from("/org/bluez/hci0/dev_any").unwrap(),
            name: name.to_owned(),
            address: "00:00:00:00:00:00".to_owned(),
            kind: DeviceKind::Unclassified,
            battery: DeviceBattery::Unreported,
            kernel_devices: Vec::new(),
        }
    }

    fn labels_of(devices: &[ConnectedDevice]) -> Vec<String> {
        devices
            .iter()
            .map(|device| short_label(device, devices))
            .collect()
    }

    #[test]
    fn distinct_first_words_become_the_labels() {
        let devices = [
            named_device("Blue DualSense Wireless Controller"),
            named_device("Red DualSense Wireless Controller"),
            named_device("WH-1000XM4"),
        ];
        assert_eq!(labels_of(&devices), ["Blue", "Red", "WH-1000X"]);
    }

    #[test]
    fn shared_first_words_extend_to_two_words() {
        let devices = [
            named_device("DualSense Blue"),
            named_device("DualSense Red"),
        ];
        assert_eq!(labels_of(&devices), ["DualSense Blue", "DualSense Red"]);
    }
}
