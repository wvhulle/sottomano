use std::{iter, time::SystemTime};

use crate::group_indicators::indicator_group_content::IndicatorListing;
mod adapter_radio_indicator;
mod connected_device_indicator;
mod reconnect_churn_counting;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::bluez_bluetooth_commands::{
    disconnect_bluetooth_device, toggle_adapter_power,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    gamepad_controller_input::accelerometer_report_timing::ReportTiming,
    network_connectivity::bluez_bluetooth::{Bluetooth, BluetoothAdapter, ConnectedDevice},
    system_protocol_clients::{dbus_desktop_buses::DesktopBuses, journal_follow::unexpired_events},
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};
use zbus::zvariant::OwnedObjectPath;

use self::{
    adapter_radio_indicator::{AdapterNaming, AdapterPowerSwitch, AdapterRadioIndicator},
    connected_device_indicator::{ConnectedDeviceIndicator, InputReports, short_label},
    reconnect_churn_counting::ReconnectHistory,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansionMessage, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::{
        input_report_sections::MeasuredReportTiming, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug)]
pub enum BluetoothMessage {
    Snapshot(Box<SnapshotDelivery<Bluetooth>>),
    KernelErrors(Vec<DeviceKernelError>),
}

#[derive(Debug, Clone)]
pub enum BluetoothInteraction {
    ToggleAdapterPower(OwnedObjectPath),
    Disconnect(OwnedObjectPath),
    Expansion {
        controller_name: String,
        expansion: IndicatorGroupExpansionMessage,
    },
}

fn adapter_group_title(naming: AdapterNaming) -> IndicatorGroupTitle {
    match naming {
        AdapterNaming::SoleAdapter => {
            IndicatorGroupTitle::fixed(IndicatorGroupFamily::BluetoothAdapters.title())
        }
        AdapterNaming::OneOfSeveral { label } => IndicatorGroupTitle::Named(
            format!(
                "{} {label}",
                IndicatorGroupFamily::BluetoothAdapters.title()
            )
            .into(),
        ),
    }
}

#[derive(Default)]
pub struct BluetoothAdaptersGroup {
    bluetooth: SubscribedSnapshot<Bluetooth>,
    expansions: KeyedExpansions<String>,
    reconnects: ReconnectHistory,
    kernel_errors: Vec<DeviceKernelError>,
}

impl BluetoothAdaptersGroup {
    pub fn collapse(&mut self) {
        self.expansions.collapse();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<BluetoothMessage> {
        Subscription::batch([
            snapshot_subscription::<Bluetooth>(buses.clone())
                .map(|delivery| BluetoothMessage::Snapshot(Box::new(delivery))),
            Subscription::run(device_kernel_error_windows).map(BluetoothMessage::KernelErrors),
        ])
    }

    pub fn update(&mut self, message: BluetoothMessage, clock: Clock) -> Task<BluetoothMessage> {
        match message {
            BluetoothMessage::Snapshot(delivery) => {
                self.bluetooth.receive(*delivery, clock);
                self.observe_latest_snapshot(clock);
                Task::none()
            }
            BluetoothMessage::KernelErrors(kernel_errors) => {
                self.kernel_errors = kernel_errors;
                Task::none()
            }
        }
    }

    pub fn interact(&mut self, interaction: BluetoothInteraction) -> Task<BluetoothMessage> {
        match interaction {
            BluetoothInteraction::ToggleAdapterPower(adapter) => logged_failure_task(
                "toggle Bluetooth adapter".to_owned(),
                toggle_adapter_power(adapter),
            ),
            BluetoothInteraction::Disconnect(device) => logged_failure_task(
                "disconnect Bluetooth device".to_owned(),
                disconnect_bluetooth_device(device),
            ),
            BluetoothInteraction::Expansion {
                controller_name,
                expansion,
            } => {
                self.expansions.update(&controller_name, expansion);
                Task::none()
            }
        }
    }

    fn observe_latest_snapshot(&mut self, clock: Clock) {
        let Some(bluetooth) = self.bluetooth.get() else {
            self.expansions.clear();
            return;
        };
        self.reconnects.observe(bluetooth, clock.utc());
        self.expansions.track(
            bluetooth
                .adapters
                .iter()
                .map(|adapter| adapter.controller_name.clone()),
        );
    }

    fn adapter_naming(&self, adapter: &BluetoothAdapter) -> AdapterNaming {
        let Some(bluetooth) = self.bluetooth.get() else {
            return AdapterNaming::SoleAdapter;
        };
        if bluetooth.adapters.len() < 2 {
            return AdapterNaming::SoleAdapter;
        }
        let placement_label = adapter.placement.label().filter(|label| {
            bluetooth
                .adapters
                .iter()
                .filter(|sibling| sibling.placement.label() == Some(*label))
                .count()
                == 1
        });
        AdapterNaming::OneOfSeveral {
            label: placement_label.map_or_else(|| adapter.controller_name.clone(), str::to_owned),
        }
    }

    fn adapter_indicator<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
        now: SystemTime,
    ) -> AdapterRadioIndicator<'view> {
        let kernel_device =
            AdapterRadioIndicator::controller_kernel_device(&adapter.controller_name);
        AdapterRadioIndicator {
            adapter,
            naming: self.adapter_naming(adapter),
            kernel_errors: unexpired_events(&self.kernel_errors, now)
                .filter(|error| error.kernel_device == kernel_device)
                .collect(),
        }
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

    fn adapter_group<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Option<IndicatorGroupContent<'view>> {
        let expansion = self.expansions.expansions.get(&adapter.controller_name)?;
        let build_adapter_radio_indicator = || {
            self.adapter_indicator(adapter, clock.wall)
                .indicator()
                .with_hover_card(Box::new(move |card_clock| {
                    self.bluetooth.hover_card_with(
                        self.adapter_indicator(adapter, card_clock.wall)
                            .hover_card(),
                    )
                }))
        };
        let headline_indicator = build_adapter_radio_indicator()
            .with_headline_glyph(Phosphor::BluetoothConnected(Weight::Regular).into());
        let power_switch = AdapterPowerSwitch { adapter };
        let power_press_does = power_switch.press_does();
        let wired_power_indicator = power_switch
            .indicator()
            .on_switch_press(
                &adapter.power,
                power_press_does,
                BluetoothInteraction::ToggleAdapterPower(adapter.object_path.clone()),
            )
            .with_hover_card(Box::new(move |_| {
                self.bluetooth
                    .hover_card_with(AdapterPowerSwitch { adapter }.hover_card())
            }));
        let siblings = adapter.power.connected_devices();
        let device_indicators: Vec<GroupIndicator<'view>> = siblings
            .iter()
            .map(|device| {
                let input_reports = report_timing_for(&device.address)
                    .map_or(InputReports::NotStreamed, |timing| {
                        InputReports::Streamed(MeasuredReportTiming::Measured(timing))
                    });
                let connected_device =
                    self.device_indicator(device, siblings, input_reports, clock);
                connected_device
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
            .collect();
        let controller_name = adapter.controller_name.clone();
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::BluetoothAdapters,
                adapter.controller_name.clone(),
            ),
            adapter_group_title(self.adapter_naming(adapter)),
            headline_indicator,
            expansion,
            move |expansion| BluetoothInteraction::Expansion {
                controller_name: controller_name.clone(),
                expansion,
            },
            IndicatorListing::Listed(
                iter::once(build_adapter_radio_indicator())
                    .chain([wired_power_indicator].into_iter().chain(device_indicators))
                    .collect(),
            ),
        ))
    }

    #[must_use]
    pub fn group_contents(
        &self,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'_>> {
        self.bluetooth
            .get()
            .map(|bluetooth| bluetooth.adapters.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|adapter| self.adapter_group(adapter, report_timing_for, clock))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{AdapterNaming, IndicatorGroupFamily, adapter_group_title};

    fn one_of_several(label: &str) -> AdapterNaming {
        AdapterNaming::OneOfSeveral {
            label: label.to_owned(),
        }
    }

    #[test]
    fn a_sole_adapter_keeps_the_plain_title() {
        assert_eq!(
            adapter_group_title(AdapterNaming::SoleAdapter).to_string(),
            IndicatorGroupFamily::BluetoothAdapters.title()
        );
    }

    #[test]
    fn each_of_several_adapters_gets_its_own_indicator_title() {
        assert_ne!(
            adapter_group_title(one_of_several("hci0")),
            adapter_group_title(one_of_several("hci1"))
        );
    }
}
