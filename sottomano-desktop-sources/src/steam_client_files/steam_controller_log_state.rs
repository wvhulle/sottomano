use std::{
    collections::BTreeMap,
    mem,
    path::{Path, PathBuf},
};

use super::steam_log_lines::controller_log_lines::{
    ControllerConnectionState, ControllerLogLine, RadioSignalSample,
};

const VALVE_USB_VENDOR: u16 = 0x28de;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum EnumeratedDevice {
    #[default]
    Idle,
    Found,
    Valve,
    ValveAt(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamControllerLogState {
    enumerated_device: EnumeratedDevice,
    hidraw_by_device_index: BTreeMap<u8, PathBuf>,
    connected_device_indices: Vec<u8>,
    establishing_device_index: Option<u8>,
    xinput_slot_by_device_index: BTreeMap<u8, u8>,
    latest_radio_signal: Option<RadioSignalSample>,
}

impl SteamControllerLogState {
    pub fn absorb(&mut self, line: ControllerLogLine) {
        match line {
            ControllerLogLine::LocalDeviceFound => self.enumerated_device = EnumeratedDevice::Found,
            ControllerLogLine::DeviceUsbIds { vendor, .. } => {
                self.enumerated_device = if vendor == VALVE_USB_VENDOR
                    && self.enumerated_device == EnumeratedDevice::Found
                {
                    EnumeratedDevice::Valve
                } else {
                    EnumeratedDevice::Idle
                };
            }
            ControllerLogLine::DevicePath(hidraw_node) => {
                if self.enumerated_device == EnumeratedDevice::Valve {
                    self.enumerated_device = EnumeratedDevice::ValveAt(hidraw_node);
                }
            }
            ControllerLogLine::DeviceOpened { device_index } => {
                if let EnumeratedDevice::ValveAt(hidraw_node) =
                    mem::take(&mut self.enumerated_device)
                {
                    self.hidraw_by_device_index
                        .insert(device_index, hidraw_node);
                }
            }
            ControllerLogLine::ConnectionStateChanged {
                device_index,
                state: ControllerConnectionState::WirelessEstablished,
            } => {
                self.establishing_device_index = Some(device_index);
                if !self.connected_device_indices.contains(&device_index) {
                    self.connected_device_indices.push(device_index);
                }
            }
            ControllerLogLine::ConnectionStateChanged { device_index, .. } => {
                self.connected_device_indices
                    .retain(|connected| *connected != device_index);
                self.xinput_slot_by_device_index.remove(&device_index);
                if self.connected_device_indices.is_empty() {
                    self.latest_radio_signal = None;
                }
            }
            ControllerLogLine::XInputSlotReserved { slot } => {
                if let Some(device_index) = self.establishing_device_index.take() {
                    self.xinput_slot_by_device_index.insert(device_index, slot);
                }
            }
            ControllerLogLine::RadioSignal(sample) => self.latest_radio_signal = Some(sample),
            ControllerLogLine::Unrecognised => {}
        }
    }

    #[must_use]
    pub fn xinput_slot_behind(&self, hidraw_node: &Path) -> Option<u8> {
        self.hidraw_by_device_index
            .iter()
            .find(|(_, logged_node)| logged_node.as_path() == hidraw_node)
            .and_then(|(device_index, _)| self.xinput_slot_by_device_index.get(device_index))
            .copied()
    }

    #[must_use]
    pub fn sole_controller_radio_signal(&self) -> Option<RadioSignalSample> {
        match self.connected_device_indices.as_slice() {
            [_] => self.latest_radio_signal,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam_client_files::steam_log_lines::LogLineShape;

    fn state_after(lines: &[&str]) -> SteamControllerLogState {
        let mut state = SteamControllerLogState::default();
        for line in lines {
            state.absorb(ControllerLogLine::from(LogLineShape::from(*line)));
        }
        state
    }

    const ENUMERATION: [&str; 8] = [
        "[2026-11-19 21:34:11] Local Device Found",
        "  type: 28de 1304",
        "  path: /dev/hidraw10",
        "[2026-11-19 21:34:11] !! Steam controller device opened for index 0.",
        "[2026-11-19 21:34:11] Local Device Found",
        "  type: 28de 1304",
        "  path: /dev/hidraw11",
        "[2026-11-19 21:34:11] !! Steam controller device opened for index 1.",
    ];

    #[test]
    fn a_connected_device_maps_its_hidraw_node_to_its_xinput_slot() {
        let mut lines = ENUMERATION.to_vec();
        lines.extend([
            "[2026-11-20 11:00:17] SetLocalControllerConnectionState for 1 to \
             wireless_established because Event from device",
            "[2026-11-20 11:00:17] Steam Controller reserving XInput slot 0",
        ]);
        let state = state_after(&lines);
        assert_eq!(
            state.xinput_slot_behind(Path::new("/dev/hidraw11")),
            Some(0)
        );
        assert_eq!(state.xinput_slot_behind(Path::new("/dev/hidraw10")), None);
    }

    #[test]
    fn a_disconnect_drops_the_slot_and_the_radio_signal() {
        let mut lines = ENUMERATION.to_vec();
        lines.extend([
            "[2026-11-20 11:00:17] SetLocalControllerConnectionState for 0 to \
             wireless_established because Event from device",
            "[2026-11-20 11:00:17] Steam Controller reserving XInput slot 0",
            "[2026-11-20 11:15:17] 502 pkts 132+  0 int 4 RSSI -52dBm chan 76/ 2 ",
        ]);
        let connected = state_after(&lines);
        assert_eq!(
            connected.sole_controller_radio_signal(),
            Some(RadioSignalSample {
                rssi_dbm: -52,
                channel: 76
            })
        );
        lines.push(
            "[2026-11-20 11:15:17] SetLocalControllerConnectionState for 0 to disconnected \
             because Event from device",
        );
        let disconnected = state_after(&lines);
        assert_eq!(
            disconnected.xinput_slot_behind(Path::new("/dev/hidraw10")),
            None
        );
        assert_eq!(disconnected.sole_controller_radio_signal(), None);
    }

    #[test]
    fn radio_signal_is_unattributed_with_two_controllers() {
        let mut lines = ENUMERATION.to_vec();
        lines.extend([
            "[2026-11-20 11:00:17] SetLocalControllerConnectionState for 0 to \
             wireless_established because Event from device",
            "[2026-11-20 11:00:17] Steam Controller reserving XInput slot 0",
            "[2026-11-20 11:00:18] SetLocalControllerConnectionState for 1 to \
             wireless_established because Event from device",
            "[2026-11-20 11:00:18] Steam Controller reserving XInput slot 1",
            "[2026-11-20 11:15:17] 502 pkts 132+  0 int 4 RSSI -52dBm chan 76/ 2 ",
        ]);
        let state = state_after(&lines);
        assert_eq!(
            state.xinput_slot_behind(Path::new("/dev/hidraw11")),
            Some(1)
        );
        assert_eq!(state.sole_controller_radio_signal(), None);
    }
}
