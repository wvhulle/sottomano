use std::path::PathBuf;

use winnow::{
    Parser, Result,
    ascii::{dec_int, dec_uint, hex_uint, space0, space1},
    combinator::{alt, delimited, preceded, separated_pair, terminated},
    token::{rest, take_till},
};

use super::LogLineShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerConnectionState {
    WirelessEstablished,
    Disconnected,
    OtherState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioSignalSample {
    pub rssi_dbm: i16,
    pub channel: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerLogLine {
    LocalDeviceFound,
    DeviceUsbIds { vendor: u16, product: u16 },
    DevicePath(PathBuf),
    DeviceOpened { device_index: u8 },
    ConnectionStateChanged {
        device_index: u8,
        state: ControllerConnectionState,
    },
    XInputSlotReserved { slot: u8 },
    RadioSignal(RadioSignalSample),
    Unrecognised,
}

fn local_device_found(input: &mut &str) -> Result<ControllerLogLine> {
    "Local Device Found"
        .value(ControllerLogLine::LocalDeviceFound)
        .parse_next(input)
}

fn device_usb_ids(input: &mut &str) -> Result<ControllerLogLine> {
    preceded(
        ("type:", space1),
        separated_pair(hex_uint, space1, hex_uint),
    )
    .map(|(vendor, product)| ControllerLogLine::DeviceUsbIds { vendor, product })
    .parse_next(input)
}

fn device_path(input: &mut &str) -> Result<ControllerLogLine> {
    preceded(("path:", space1), take_till(1.., char::is_whitespace))
        .map(|path: &str| ControllerLogLine::DevicePath(PathBuf::from(path)))
        .parse_next(input)
}

fn device_opened(input: &mut &str) -> Result<ControllerLogLine> {
    delimited(
        "!! Steam controller device opened for index ",
        dec_uint,
        '.',
    )
    .map(|device_index| ControllerLogLine::DeviceOpened { device_index })
    .parse_next(input)
}

fn connection_state(input: &mut &str) -> Result<ControllerConnectionState> {
    take_till(1.., char::is_whitespace)
        .map(|state: &str| match state {
            "wireless_established" => ControllerConnectionState::WirelessEstablished,
            "disconnected" => ControllerConnectionState::Disconnected,
            _ => ControllerConnectionState::OtherState,
        })
        .parse_next(input)
}

fn connection_state_changed(input: &mut &str) -> Result<ControllerLogLine> {
    (
        preceded("SetLocalControllerConnectionState for ", dec_uint),
        preceded(" to ", connection_state),
    )
        .map(|(device_index, state)| ControllerLogLine::ConnectionStateChanged {
            device_index,
            state,
        })
        .parse_next(input)
}

fn xinput_slot_reserved(input: &mut &str) -> Result<ControllerLogLine> {
    preceded("Steam Controller reserving XInput slot ", dec_uint)
        .map(|slot| ControllerLogLine::XInputSlotReserved { slot })
        .parse_next(input)
}

fn radio_signal(input: &mut &str) -> Result<ControllerLogLine> {
    (
        terminated(dec_uint::<_, u32, _>, (" pkts", space1)),
        terminated(dec_uint::<_, u32, _>, ('+', space0)),
        terminated(dec_uint::<_, u32, _>, (" int ", dec_uint::<_, u32, _>)),
        delimited(" RSSI ", dec_int, "dBm"),
        preceded((" chan", space1), dec_uint),
    )
        .map(|(_, _, _, rssi_dbm, channel)| {
            ControllerLogLine::RadioSignal(RadioSignalSample { rssi_dbm, channel })
        })
        .parse_next(input)
}

impl From<LogLineShape<'_>> for ControllerLogLine {
    fn from(shape: LogLineShape<'_>) -> Self {
        let mut message = match shape {
            LogLineShape::Stamped { message, .. } => message,
            LogLineShape::Continuation(line) => line.trim_start(),
        };
        let recognised = alt((
            local_device_found,
            device_usb_ids,
            device_path,
            device_opened,
            connection_state_changed,
            xinput_slot_reserved,
            radio_signal,
            rest.value(ControllerLogLine::Unrecognised),
        ))
        .parse_next(&mut message);
        match recognised {
            Ok(meaning) => meaning,
            Err(_) => ControllerLogLine::Unrecognised,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meaning(line: &str) -> ControllerLogLine {
        ControllerLogLine::from(LogLineShape::from(line))
    }

    #[test]
    fn device_enumeration_blocks_name_vendor_path_and_index() {
        assert_eq!(
            meaning("[2026-11-19 21:34:11] Local Device Found"),
            ControllerLogLine::LocalDeviceFound
        );
        assert_eq!(
            meaning("  type: 28de 1304"),
            ControllerLogLine::DeviceUsbIds {
                vendor: 0x28de,
                product: 0x1304
            }
        );
        assert_eq!(
            meaning("  path: /dev/hidraw10"),
            ControllerLogLine::DevicePath(PathBuf::from("/dev/hidraw10"))
        );
        assert_eq!(
            meaning("[2026-11-19 21:34:11] !! Steam controller device opened for index 0."),
            ControllerLogLine::DeviceOpened { device_index: 0 }
        );
    }

    #[test]
    fn connection_lines_name_the_device_index_and_its_xinput_slot() {
        assert_eq!(
            meaning(
                "[2026-11-20 11:00:17] SetLocalControllerConnectionState for 0 to \
                 wireless_established because Event from device"
            ),
            ControllerLogLine::ConnectionStateChanged {
                device_index: 0,
                state: ControllerConnectionState::WirelessEstablished
            }
        );
        assert_eq!(
            meaning("[2026-11-20 11:00:17] Steam Controller reserving XInput slot 1"),
            ControllerLogLine::XInputSlotReserved { slot: 1 }
        );
        assert_eq!(
            meaning(
                "[2026-11-19 21:34:13] SetLocalControllerConnectionState for 4 to disconnected \
                 because CGetControllerConnectionState"
            ),
            ControllerLogLine::ConnectionStateChanged {
                device_index: 4,
                state: ControllerConnectionState::Disconnected
            }
        );
    }

    #[test]
    fn radio_lines_carry_signal_strength_and_channel() {
        assert_eq!(
            meaning("[2026-11-20 11:15:17] 502 pkts 132+  0 int 4 RSSI -52dBm chan 76/ 2 "),
            ControllerLogLine::RadioSignal(RadioSignalSample {
                rssi_dbm: -52,
                channel: 76
            })
        );
        assert_eq!(
            meaning(
                "[2026-11-20 11:15:17] 694 pkts  50+126 int 3 RSSI -53dBm chan 76/ 2 \
                 Disconnect (Spontaneous)"
            ),
            ControllerLogLine::RadioSignal(RadioSignalSample {
                rssi_dbm: -53,
                channel: 76
            })
        );
    }

    #[test]
    fn other_lines_are_unrecognised() {
        assert_eq!(
            meaning("[2026-11-20 11:00:17] DEBUG: Dongle BG RSSI: Chans:  0   0   0  96"),
            ControllerLogLine::Unrecognised
        );
    }
}
