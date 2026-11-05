mod adapter_radio_reading;
mod connected_device_reading;
mod reconnect_churn_counting;

use std::collections::BTreeMap;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    gamepad_controller_input::accelerometer_report_timing::ReportTiming,
    network_connectivity::bluez_bluetooth::{
        Bluetooth, BluetoothAdapter, ConnectedDevice, disconnect_bluetooth_device,
        toggle_adapter_power,
    },
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
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        input_report_sections::MeasuredReportTiming,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BluetoothTarget {
    Adapter {
        controller_name: String,
    },
    PowerSwitch {
        controller_name: String,
    },
    Device {
        controller_name: String,
        object_path: OwnedObjectPath,
    },
}

impl From<BluetoothTarget> for HoverTarget {
    fn from(target: BluetoothTarget) -> Self {
        HoverTarget::Bluetooth(target)
    }
}

#[derive(Debug, Clone)]
pub enum BluetoothMessage {
    Snapshot(Box<SnapshotDelivery<Bluetooth>>),
    KernelErrors(Vec<DeviceKernelError>),
    ToggleAdapterPower(OwnedObjectPath),
    Disconnect(OwnedObjectPath),
    Expansion {
        controller_name: String,
        expansion: WidgetGroupExpansionMessage,
    },
}

#[derive(Default)]
struct AdapterSession {
    expansion: WidgetGroupExpansion,
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
    bluetooth: LatestSnapshot<Bluetooth, HidesFetchFailures>,
    sessions: BTreeMap<String, AdapterSession>,
    reconnects: ReconnectHistory,
    kernel_errors: Vec<DeviceKernelError>,
}

async fn logged_bus_call(action: &'static str, bus_call: impl Future<Output = zbus::Result<()>>) {
    if let Err(bus_error) = bus_call.await {
        tracing::warn!("{action}: {bus_error}");
    }
}

impl BluetoothAdaptersGroup {
    pub fn collapse(&mut self) {
        self.sessions.values_mut().for_each(|session| {
            session.expansion.collapse();
        });
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
            BluetoothMessage::ToggleAdapterPower(adapter) => Task::future(logged_bus_call(
                "toggle Bluetooth adapter",
                toggle_adapter_power(adapter),
            ))
            .discard(),
            BluetoothMessage::Disconnect(device) => Task::future(logged_bus_call(
                "disconnect Bluetooth device",
                disconnect_bluetooth_device(device),
            ))
            .discard(),
            BluetoothMessage::Expansion {
                controller_name,
                expansion,
            } => {
                if let Some(session) = self.sessions.get_mut(&controller_name) {
                    session.expansion.update(expansion);
                }
                Task::none()
            }
        }
    }

    fn observe_latest_snapshot(&mut self, clock: Clock) {
        let Some(bluetooth) = self.bluetooth.get() else {
            self.sessions.clear();
            return;
        };
        self.reconnects.observe(bluetooth, clock.utc());
        self.sessions.retain(|controller_name, _| {
            bluetooth
                .adapters
                .iter()
                .any(|adapter| adapter.controller_name == *controller_name)
        });
        bluetooth.adapters.iter().for_each(|adapter| {
            self.sessions
                .entry(adapter.controller_name.clone())
                .or_default();
        });
    }

    fn adapter_named(&self, controller_name: &str) -> Option<&BluetoothAdapter> {
        self.bluetooth
            .get()?
            .adapters
            .iter()
            .find(|adapter| adapter.controller_name == controller_name)
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
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
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
            input_reports: report_timing_for(&device.address)
                .map_or(InputReports::NotStreamed, |timing| {
                    InputReports::Streamed(MeasuredReportTiming::Measured(timing))
                }),
        }
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: &BluetoothTarget,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Option<HoverCard> {
        let body = match target {
            BluetoothTarget::Adapter { controller_name } => self
                .adapter_reading(self.adapter_named(controller_name)?)
                .hover_card(clock),
            BluetoothTarget::PowerSwitch { controller_name } => AdapterPowerSwitch {
                adapter: self.adapter_named(controller_name)?,
            }
            .hover_card(clock),
            BluetoothTarget::Device {
                controller_name,
                object_path,
            } => {
                let siblings = self
                    .adapter_named(controller_name)?
                    .power
                    .connected_devices();
                let device = siblings
                    .iter()
                    .find(|device| device.object_path == *object_path)?;
                self.device_reading(device, siblings, report_timing_for, clock)
                    .hover_card(clock)
            }
        };
        self.bluetooth.hover_card_with(body)
    }

    fn adapter_group<'a>(
        &'a self,
        adapter: &'a BluetoothAdapter,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Option<WidgetGroupContent<'a, BluetoothMessage>> {
        let session = self.sessions.get(&adapter.controller_name)?;
        let summary_item = GroupItem::reading(
            self.adapter_reading(adapter),
            BluetoothTarget::Adapter {
                controller_name: adapter.controller_name.clone(),
            },
        );
        let power_switch = AdapterPowerSwitch { adapter };
        let power_press_does = power_switch.press_does();
        let power_switched_on = power_switch.switched_on();
        let power_item = GroupItem::reading(
            power_switch,
            BluetoothTarget::PowerSwitch {
                controller_name: adapter.controller_name.clone(),
            },
        )
        .on_switch_press(
            power_switched_on,
            power_press_does,
            BluetoothMessage::ToggleAdapterPower(adapter.object_path.clone()),
        );
        let siblings = adapter.power.connected_devices();
        let device_items: Vec<GroupItem<'a, BluetoothMessage>> = siblings
            .iter()
            .map(|device| {
                let reading = self.device_reading(device, siblings, report_timing_for, clock);
                let needs_attention = reading.state(clock) == ReadingDisplayState::Critical;
                GroupItem::reading(
                    reading,
                    BluetoothTarget::Device {
                        controller_name: adapter.controller_name.clone(),
                        object_path: device.object_path.clone(),
                    },
                )
                .on_press(
                    ConnectedDeviceReading::PRESS_DOES,
                    BluetoothMessage::Disconnect(device.object_path.clone()),
                )
                .previewed_on_handle_if(needs_attention)
            })
            .collect();
        let controller_name = adapter.controller_name.clone();
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(
                WidgetGroupFamily::BluetoothAdapters,
                adapter.controller_name.clone(),
            ),
            adapter_group_title(self.adapter_naming(adapter)),
            &session.expansion,
            move |expansion| BluetoothMessage::Expansion {
                controller_name: controller_name.clone(),
                expansion,
            },
            [summary_item, power_item].into_iter().chain(device_items),
        ))
    }

    #[must_use]
    pub fn group_contents(
        &self,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Vec<WidgetGroupContent<'_, BluetoothMessage>> {
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
