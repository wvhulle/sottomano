use std::time::Duration;

use sottomano_config::widget_groups::gamepad_bindings::GamepadBindingsConfig;
use sottomano_desktop_sources::gamepad_controller_input::{
    accelerometer_report_timing::{DeviceClock, ReportTiming, SentReports},
    evdev_gamepad_devices::{
        ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower, GamepadTransport,
        PhysicalGamepadLink, UsbCableState,
    },
    gamepad_input_machine::GamepadInput,
    player_indicator_leds::{LitPlayer, PlayerIndicator},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::gamepad_group::{GamepadGroup, GamepadMessage, GamepadWindowClaim},
};
use uom::si::{
    f64::{Frequency, Ratio},
    frequency::hertz,
    ratio::percent,
};

use crate::showcased_group::{ShowcasedGamepad, ShowcasedGroup};

pub const DUALSENSE_SLOT: usize = 31;
const SECOND_DUALSENSE_SLOT: usize = 258;
const STEAM_VIRTUAL_SLOT: usize = 261;

fn dualsense(
    slot: usize,
    hardware_address: &str,
    alias: &str,
    player_number: u8,
    power: GamepadPower,
) -> ConnectedGamepad {
    ConnectedGamepad {
        slot,
        name: "DualSense Wireless Controller".to_owned(),
        vendor_id: 0x054c,
        power,
        origin: GamepadOrigin::Physical(PhysicalGamepadLink {
            kernel_device: format!("+hid:0005:054C:0CE6.{slot:04X}"),
            transport: GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::Absent,
            },
            identity: ControllerIdentity::Addressed {
                hardware_address: hardware_address.to_owned(),
                alias: alias.to_owned(),
            },
            player_indicator: PlayerIndicator::Lit(LitPlayer {
                number: player_number,
                leds_lit: (1..=5)
                    .map(|position| match player_number {
                        1 => position == 3,
                        _ => position == 2 || position == 4,
                    })
                    .collect(),
            }),
        }),
    }
}

pub fn group(_clock: Clock) -> ShowcasedGroup {
    let bindings = GamepadBindingsConfig::default();
    let mut group = GamepadGroup::default();
    let messages = [
        GamepadMessage::Input(GamepadInput::Roster(vec![
            dualsense(
                DUALSENSE_SLOT,
                "d4:2f:4b:69:6b:2f",
                "Blue DualSense Wireless Controller",
                1,
                GamepadPower::Discharging(72),
            ),
            dualsense(
                SECOND_DUALSENSE_SLOT,
                "d4:2f:4b:ae:2f:50",
                "Red DualSense Wireless Controller",
                2,
                GamepadPower::Charging(40),
            ),
            ConnectedGamepad {
                slot: STEAM_VIRTUAL_SLOT,
                name: "Microsoft X-Box 360 pad 0".to_owned(),
                vendor_id: 0x28de,
                power: GamepadPower::NoBattery,
                origin: GamepadOrigin::Virtual,
            },
        ])),
        GamepadMessage::Input(GamepadInput::ReportTiming {
            slot: DUALSENSE_SLOT,
            timing: ReportTiming {
                arrival_rate: Frequency::new::<hertz>(250.0),
                stalls: 0,
                longest_gap: Duration::from_millis(4),
                device_clock: DeviceClock::Present(SentReports {
                    sent_rate: Frequency::new::<hertz>(250.0),
                    lost_share: Ratio::new::<percent>(0.0),
                    most_delayed: Duration::from_millis(1),
                }),
            },
        }),
        GamepadMessage::Input(GamepadInput::ReportTiming {
            slot: SECOND_DUALSENSE_SLOT,
            timing: ReportTiming {
                arrival_rate: Frequency::new::<hertz>(465.0),
                stalls: 22,
                longest_gap: Duration::from_millis(27),
                device_clock: DeviceClock::Present(SentReports {
                    sent_rate: Frequency::new::<hertz>(790.0),
                    lost_share: Ratio::new::<percent>(41.0),
                    most_delayed: Duration::from_millis(24),
                }),
            },
        }),
    ];
    for message in messages {
        let _ = group.update(message, &bindings, GamepadWindowClaim::Unclaimed);
    }
    ShowcasedGroup::Gamepad(Box::new(ShowcasedGamepad { group, bindings }))
}
