mod adapter_radio_reading;
mod connected_device_reading;
mod reconnect_churn_counting;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::bluez_bluetooth_commands::{
    disconnect_bluetooth_device, toggle_adapter_power,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    gamepad_controller_input::accelerometer_report_timing::ReportTiming,
    network_connectivity::bluez_bluetooth::{Bluetooth, BluetoothAdapter, ConnectedDevice},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};
use zbus::zvariant::OwnedObjectPath;

use self::{
    adapter_radio_reading::{AdapterNaming, AdapterPowerSwitch, AdapterRadioReading},
    connected_device_reading::{ConnectedDeviceReading, InputReports, short_label},
    reconnect_churn_counting::ReconnectHistory,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{KeyedExpansions, WidgetGroupContent, WidgetGroupExpansionMessage},
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        input_report_sections::MeasuredReportTiming,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
        expansion: WidgetGroupExpansionMessage,
    },
}

fn adapter_group_title(naming: AdapterNaming) -> WidgetGroupTitle {
    match naming {
        AdapterNaming::SoleAdapter => {
            WidgetGroupTitle::fixed(WidgetGroupFamily::BluetoothAdapters.title())
        }
        AdapterNaming::OneOfSeveral { label } => WidgetGroupTitle::Named(
            format!("{} {label}", WidgetGroupFamily::BluetoothAdapters.title()).into(),
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

    fn adapter_reading<'a>(&'a self, adapter: &'a BluetoothAdapter) -> AdapterRadioReading<'a> {
        let kernel_device = AdapterRadioReading::kernel_device_of(&adapter.controller_name);
        AdapterRadioReading {
            adapter,
            naming: self.adapter_naming(adapter),
            kernel_errors: self
                .kernel_errors
                .iter()
                .filter(|error| error.kernel_device == kernel_device)
                .collect(),
        }
    }

    fn device_reading<'a>(
        &'a self,
        device: &'a ConnectedDevice,
        siblings: &[ConnectedDevice],
        input_reports: InputReports,
        clock: Clock,
    ) -> ConnectedDeviceReading<'a> {
        ConnectedDeviceReading {
            device,
            short_label: short_label(device, siblings),
            recent_reconnects: self
                .reconnects
                .recent_reconnects(&device.address, clock.utc()),
            kernel_errors: self
                .kernel_errors
                .iter()
                .filter(|error| device.kernel_devices.contains(&error.kernel_device))
                .collect(),
            input_reports,
        }
    }

    fn adapter_group<'a>(
        &'a self,
        adapter: &'a BluetoothAdapter,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Option<WidgetGroupContent<'a>> {
        let expansion = self.expansions.get(&adapter.controller_name)?;
        let group_summary = GroupSummary::summarising(
            Phosphor::BluetoothConnected(Weight::Regular).into(),
            GroupItem::reading(self.adapter_reading(adapter).reading_row()).with_hover_card(
                Box::new(move |_| {
                    self.bluetooth
                        .hover_card_with(self.adapter_reading(adapter).hover_card())
                }),
            ),
        );
        let power_switch = AdapterPowerSwitch { adapter };
        let power_press_does = power_switch.press_does();
        let power_item = GroupItem::reading(power_switch.reading_row())
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
        let device_items: Vec<GroupItem<'a>> = siblings
            .iter()
            .map(|device| {
                let input_reports = report_timing_for(&device.address)
                    .map_or(InputReports::NotStreamed, |timing| {
                        InputReports::Streamed(MeasuredReportTiming::Measured(timing))
                    });
                let reading = self.device_reading(device, siblings, input_reports, clock);
                GroupItem::reading(reading.reading_row())
                    .on_press(
                        ConnectedDeviceReading::PRESS_DOES,
                        BluetoothInteraction::Disconnect(device.object_path.clone()),
                    )
                    .with_hover_card(Box::new(move |card_clock| {
                        self.bluetooth.hover_card_with(
                            self.device_reading(device, siblings, input_reports, card_clock)
                                .hover_card(),
                        )
                    }))
            })
            .collect();
        let controller_name = adapter.controller_name.clone();
        let group_summary = group_summary;
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(
                WidgetGroupFamily::BluetoothAdapters,
                adapter.controller_name.clone(),
            ),
            adapter_group_title(self.adapter_naming(adapter)),
            group_summary,
            expansion,
            move |expansion| BluetoothInteraction::Expansion {
                controller_name: controller_name.clone(),
                expansion,
            },
            [power_item].into_iter().chain(device_items),
        ))
    }

    #[must_use]
    pub fn group_contents(
        &self,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Vec<WidgetGroupContent<'_>> {
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
    use super::{AdapterNaming, WidgetGroupFamily, adapter_group_title};

    fn one_of_several(label: &str) -> AdapterNaming {
        AdapterNaming::OneOfSeveral {
            label: label.to_owned(),
        }
    }

    #[test]
    fn a_sole_adapter_keeps_the_plain_title() {
        assert_eq!(
            adapter_group_title(AdapterNaming::SoleAdapter).to_string(),
            WidgetGroupFamily::BluetoothAdapters.title()
        );
    }

    #[test]
    fn each_of_several_adapters_gets_its_own_row_title() {
        assert_ne!(
            adapter_group_title(one_of_several("hci0")),
            adapter_group_title(one_of_several("hci1"))
        );
    }
}
