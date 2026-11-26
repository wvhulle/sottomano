use std::{iter, time::SystemTime};

use sottomano_desktop_sources::{
    gamepad_controller_input::{
        evdev_gamepad_devices::{
            ConnectedGamepad, ControllerIdentity, GamepadPower, GamepadTransport, InputRoute,
            PhysicalGamepadLink, UsbCableState,
        },
        player_indicator_leds::PlayerIndicator,
        steam_input_facts::{RadioReading, SteamInputFacts},
    },
    system_protocol_clients::journal_follow::unexpired_events,
};
use sottomano_toml_config::indicator_groups::gamepad_bindings::GamepadBindingsConfig;

use super::{
    ControllerCustody, GamepadGroup, GamepadInteraction,
    connected_controller_indicator::{ConnectedControllerIndicator, ControllerMarking},
    controller_battery_indicator::ControllerBatteryIndicator,
    controller_custody_key,
    controller_link_indicator::{BandSharing, ControllerLinkIndicator},
    steam_radio_firmware_indicators::{SteamFirmwareIndicator, SteamRadioIndicator},
    usb_cable_indicator::{UsbCableIndicator, UsbCableRole},
    wifi_band_use::WifiBandUse,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
    indicator_groups::{
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::input_report_sections::MeasuredReportTiming,
};

fn compositor_driving_toggle(controller_custody: ControllerCustody) -> IconToggle {
    IconToggle::new(
        "Compositor control",
        Phosphor::AppWindow(Weight::Regular),
        controller_custody,
        SwitchPresses {
            turning_on: "let this controller drive the compositor again",
            turning_off: "hand this controller to the focused window",
        },
    )
}

impl GamepadGroup {
    pub(super) fn controllers_in_player_order(
        &self,
    ) -> Vec<(&ConnectedGamepad, &PhysicalGamepadLink)> {
        let mut ordered: Vec<_> = self.physical_controllers().collect();
        ordered.sort_by(
            |(left_controller, left_link), (right_controller, right_link)| {
                (&left_link.player_indicator, left_controller.slot)
                    .cmp(&(&right_link.player_indicator, right_controller.slot))
            },
        );
        ordered
    }

    pub(super) fn controller_marking<'view>(
        &self,
        gamepad: &ConnectedGamepad,
        link: &'view PhysicalGamepadLink,
    ) -> ControllerMarking<'view> {
        let unlit_controller_count = self
            .physical_controllers()
            .filter(|(_, other_link)| matches!(other_link.player_indicator, PlayerIndicator::Unlit))
            .count();
        match (&link.player_indicator, &link.identity) {
            (PlayerIndicator::Lit(lit_player), _) => ControllerMarking::PlayerLights(lit_player),
            (PlayerIndicator::Unlit, ControllerIdentity::Addressed { alias, .. })
                if unlit_controller_count > 1 && *alias != gamepad.name =>
            {
                ControllerMarking::PairedAlias(alias)
            }
            (PlayerIndicator::Unlit, _) => ControllerMarking::IconOnly,
        }
    }

    fn face_indicator<'view>(
        &'view self,
        gamepad: &'view ConnectedGamepad,
        link: &'view PhysicalGamepadLink,
        bindings: &'view GamepadBindingsConfig,
    ) -> ConnectedControllerIndicator<'view> {
        ConnectedControllerIndicator {
            gamepad,
            bindings,
            custody: self.controller_custody(gamepad.slot),
            marking: self.controller_marking(gamepad, link),
        }
    }

    fn link_indicator<'view>(
        &'view self,
        slot: usize,
        link: &'view PhysicalGamepadLink,
        now: SystemTime,
    ) -> ControllerLinkIndicator<'view> {
        ControllerLinkIndicator {
            transport: link.transport,
            report_timing: self.report_timings.get(&slot).copied().map_or(
                MeasuredReportTiming::Pending,
                MeasuredReportTiming::Measured,
            ),
            kernel_errors: unexpired_events(&self.kernel_errors, now)
                .filter(|error| error.kernel_device == link.kernel_device)
                .collect(),
            band_sharing: match &self.wifi_band_use {
                WifiBandUse::ClearOfBluetooth => BandSharing::NoConflict,
                WifiBandUse::SharingBluetoothBand {
                    network_name,
                    frequency,
                } => BandSharing::SharedWithWifi {
                    network_name,
                    frequency: *frequency,
                    wired_fallback: self.wired_fallback,
                },
            },
        }
    }

    fn compositor_driving_hover_card(&self, slot: usize) -> Vec<Section> {
        let (note_title, note_body) = match self.controller_custody(slot) {
            ControllerCustody::Compositor => (
                "Driving the compositor",
                "Buttons and sticks focus and move windows, whatever window has focus. Turn this \
                 off before playing a game: the controller then belongs to the focused window \
                 again.",
            ),
            ControllerCustody::HandedOver => (
                "Handed to the focused window",
                "Controllers start out belonging to the focused window, so games keep their \
                 input. Turn this on to focus and move windows with this controller; the choice \
                 is remembered for this controller.",
            ),
        };
        vec![Section::new(
            note_title,
            SectionWidget::paragraph(note_body),
        )]
    }

    fn controller_group<'view>(
        &'view self,
        gamepad: &'view ConnectedGamepad,
        link: &'view PhysicalGamepadLink,
        group_title: IndicatorGroupTitle,
        bindings: &'view GamepadBindingsConfig,
        now: SystemTime,
    ) -> Option<IndicatorGroupContent<'view>> {
        let slot = gamepad.slot;
        let expansion = self.expansions.expansions.get(&slot)?;
        let battery_indicator = ControllerBatteryIndicator {
            power: gamepad.power,
        };
        let build_controller_indicator =
            || self.face_indicator(gamepad, link, bindings).indicator();
        let headline_indicator = build_controller_indicator()
            .with_headline_glyph(Phosphor::Joystick(Weight::Regular).into());
        let shown_in_bluetooth_group = matches!(link.transport, GamepadTransport::Bluetooth { .. });
        let mut rows = vec![
            GroupIndicator::toggle(
                compositor_driving_toggle(self.controller_custody(slot)),
                GamepadInteraction::ToggleCompositorDriving { slot },
            )
            .with_hover_card(Box::new(move |_| self.compositor_driving_hover_card(slot))),
        ];
        if !shown_in_bluetooth_group {
            match link.input_route {
                InputRoute::Kernel => rows.push(self.link_indicator(slot, link, now).indicator()),
                InputRoute::SteamInput(SteamInputFacts {
                    radio: RadioReading::Measured(signal),
                    ..
                }) => rows.push(SteamRadioIndicator { signal }.indicator()),
                InputRoute::SteamInput(_) => {}
            }
            if gamepad.power != GamepadPower::NoBattery {
                rows.push(battery_indicator.indicator());
            }
        }
        if let InputRoute::SteamInput(facts) = link.input_route
            && SteamFirmwareIndicator::has_pending_update(&facts)
        {
            rows.push(SteamFirmwareIndicator { facts }.indicator());
        }
        if let Some(cable_role) = usb_cable_role(link.transport) {
            let cable_indicator = UsbCableIndicator { role: cable_role }.indicator();
            rows.push(match cable_role {
                UsbCableRole::ChargingOnly => cable_indicator.on_press(
                    "disconnect Bluetooth so the cable can take over",
                    GamepadInteraction::DisconnectBluetooth { slot },
                ),
                UsbCableRole::InputAndCharging => cable_indicator,
            });
        }
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::Gamepad,
                controller_custody_key(link).to_owned(),
            ),
            group_title,
            headline_indicator,
            expansion,
            move |expansion| GamepadInteraction::Expansion { slot, expansion },
            IndicatorListing::Listed(
                iter::once(build_controller_indicator())
                    .chain(rows)
                    .collect(),
            ),
        ))
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        bindings: &'view GamepadBindingsConfig,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let controllers = self.controllers_in_player_order();
        let titles = IndicatorGroupTitle::numbered_namesakes(
            controllers.iter().map(|(gamepad, _)| gamepad.name.clone()),
        );
        controllers
            .iter()
            .zip(titles)
            .filter_map(|((gamepad, link), group_title)| {
                self.controller_group(gamepad, link, group_title, bindings, clock.wall)
            })
            .chain(self.controller_discovery_group())
            .collect()
    }
}

fn usb_cable_role(transport: GamepadTransport) -> Option<UsbCableRole> {
    match transport {
        GamepadTransport::Usb => Some(UsbCableRole::InputAndCharging),
        GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::IgnoredByKernel,
        } => Some(UsbCableRole::ChargingOnly),
        GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::Absent,
        }
        | GamepadTransport::SteamDongle
        | GamepadTransport::Other => None,
    }
}
