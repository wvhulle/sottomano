use std::iter;

use crate::group_indicators::indicator_group_content::IndicatorListing;
mod adapter_power_indicator;
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
    network_connectivity::bluez_bluetooth::{Bluetooth, BluetoothAdapter},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};
use zbus::zvariant::OwnedObjectPath;

use self::{adapter_radio_indicator::AdapterNaming, reconnect_churn_counting::ReconnectHistory};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansionMessage, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
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

    fn adapter_group<'view>(
        &'view self,
        adapter: &'view BluetoothAdapter,
        report_timing_for: &impl Fn(&str) -> Option<ReportTiming>,
        clock: Clock,
    ) -> Option<IndicatorGroupContent<'view>> {
        let expansion = self.expansions.expansions.get(&adapter.controller_name)?;
        let headline_indicator = self
            .adapter_radio_row(adapter, clock)
            .with_headline_glyph(Phosphor::BluetoothConnected(Weight::Regular).into());
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
                iter::once(self.adapter_radio_row(adapter, clock))
                    .chain(iter::once(self.adapter_power_row(adapter)))
                    .chain(self.connected_device_rows(adapter, report_timing_for, clock))
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
