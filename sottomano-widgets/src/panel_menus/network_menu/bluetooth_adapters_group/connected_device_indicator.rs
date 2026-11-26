use std::iter::once;

use sottomano_desktop_sources::{
    battery_power_profiles::upower_battery_state::ChargeStatus,
    gamepad_controller_input::accelerometer_report_timing::ReportTiming,
    network_connectivity::bluez_bluetooth::{
        BluetoothAdapter, ConnectedDevice, DeviceBattery, DeviceKind,
    },
    system_protocol_clients::journal_follow::unexpired_events,
    systemd_units_journal::journal_device_kernel_errors::DeviceKernelError,
};
use sottomano_toml_config::severity::Severity;

use super::{BluetoothAdaptersGroup, BluetoothInteraction};
use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    panel_menus::network_menu::bluetooth_adapters_group::reconnect_churn_counting::RECONNECT_HORIZON,
    widgets::{
        input_report_sections::{KernelErrorListing, MeasuredReportTiming, ReportHealth},
        localized_quantities::Quantity,
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

pub struct ConnectedDeviceIndicator<'view> {
    pub device: &'view ConnectedDevice,
    pub short_label: String,
    pub recent_reconnects: usize,
    pub kernel_errors: Vec<&'view DeviceKernelError>,
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

fn charging_description(charging: ChargeStatus) -> Option<&'static str> {
    match charging {
        ChargeStatus::Unreported => None,
        ChargeStatus::Charging => Some("charging"),
        ChargeStatus::Discharging => Some("discharging"),
        ChargeStatus::NotCharging => Some("not charging"),
        ChargeStatus::Full => Some("full"),
    }
}

impl ConnectedDeviceIndicator<'_> {
    pub const PRESS_DOES: &'static str = "disconnect this device";

    fn battery_drain_to(&self, threshold_percent: u8) -> BatteryDrain {
        match self.device.battery {
            DeviceBattery::Reported {
                level_percent,
                charging,
            } if level_percent <= threshold_percent
                && !matches!(charging, ChargeStatus::Charging | ChargeStatus::Full) =>
            {
                BatteryDrain::BelowThreshold
            }
            DeviceBattery::Reported { .. } | DeviceBattery::Unreported => {
                BatteryDrain::AboveThreshold
            }
        }
    }

    fn connection_stability(&self) -> ConnectionStability {
        let lossy_reports = match self.input_reports {
            InputReports::Streamed(report_timing) => {
                report_timing.report_health() == ReportHealth::Degraded
            }
            InputReports::NotStreamed => false,
        };

        if lossy_reports || self.recent_reconnects > 0 || !self.kernel_errors.is_empty() {
            ConnectionStability::Unstable
        } else {
            ConnectionStability::Stable
        }
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
        Section::new("Link", dictionary_table([reconnects]))
    }

    fn battery_section(&self) -> Option<Section> {
        let DeviceBattery::Reported {
            level_percent,
            charging,
        } = self.device.battery
        else {
            return None;
        };
        Some(Section::new(
            "Battery",
            dictionary_table(
                once(LabeledValue::new(
                    "Level",
                    Quantity::whole_percent(level_percent),
                ))
                .chain(
                    charging_description(charging)
                        .map(|description| LabeledValue::new("Status", description)),
                ),
            ),
        ))
    }
}

impl<'view> ConnectedDeviceIndicator<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
        let state =
            if self.battery_drain_to(CRITICAL_BATTERY_PERCENT) == BatteryDrain::BelowThreshold {
                IndicatorState::Critical
            } else if self.battery_drain_to(LOW_BATTERY_PERCENT) == BatteryDrain::BelowThreshold
                || self.connection_stability() == ConnectionStability::Unstable
            {
                IndicatorState::Warning
            } else {
                IndicatorState::Idle
            };
        let gauge = match self.device.battery {
            DeviceBattery::Reported { level_percent, .. } => {
                let fill = percent_fill(i64::from(level_percent));
                ReadoutGauge::Gauged { fill, live: fill }
            }
            DeviceBattery::Unreported => ReadoutGauge::Ungauged,
        };
        let label = if self.short_label.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(self.short_label.clone())
        };
        GroupIndicator::glyph_only(
            format!("{} {}", self.device.name, self.device.address),
            kind_glyph(self.device.kind),
            state,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        [
            Some(Section::new(
                "Device",
                SectionWidget::paragraph(self.device.name.clone()),
            )),
            self.input_report_section(),
            Some(self.link_section()),
            KernelErrorListing {
                kernel_errors: &self.kernel_errors,
            }
            .section(),
            self.battery_section(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionStability {
    Unstable,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatteryDrain {
    BelowThreshold,
    AboveThreshold,
}

impl BluetoothAdaptersGroup {
    pub(super) fn connected_device_rows<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Vec<GroupIndicator<'view>> {
        let siblings = adapter.power.connected_devices();
        siblings
            .iter()
            .map(|device| {
                let input_reports = report_timing_for(&device.address)
                    .map_or(InputReports::NotStreamed, |timing| {
                        InputReports::Streamed(MeasuredReportTiming::Measured(timing))
                    });
                self.device_indicator(device, siblings, input_reports, clock)
                    .indicator()
                    .on_press(
                        ConnectedDeviceIndicator::PRESS_DOES,
                        BluetoothInteraction::Disconnect(device.object_path.clone()),
                    )
                    .with_hover_card(Box::new(move |card_clock| {
                        self.bluetooth.hover_card_with(
                            self.device_indicator(device, siblings, input_reports, card_clock)
                                .hover_card(),
                        )
                    }))
            })
            .collect()
    }

    fn device_indicator<'view>(
        &'view self,
        device: &'view ConnectedDevice,
        siblings: &[ConnectedDevice],
        input_reports: InputReports,
        clock: Clock,
    ) -> ConnectedDeviceIndicator<'view> {
        ConnectedDeviceIndicator {
            device,
            short_label: short_label(device, siblings),
            recent_reconnects: self
                .reconnects
                .recent_reconnects(&device.address, clock.utc()),
            kernel_errors: unexpired_events(&self.kernel_errors, clock.wall)
                .filter(|error| device.kernel_devices.contains(&error.kernel_device))
                .collect(),
            input_reports,
        }
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

    fn device_short_labels(devices: &[ConnectedDevice]) -> Vec<String> {
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
        assert_eq!(device_short_labels(&devices), ["Blue", "Red", "WH-1000X"]);
    }

    #[test]
    fn shared_first_words_extend_to_two_words() {
        let devices = [
            named_device("DualSense Blue"),
            named_device("DualSense Red"),
        ];
        assert_eq!(
            device_short_labels(&devices),
            ["DualSense Blue", "DualSense Red"]
        );
    }
}
