use std::{iter, time::SystemTime};

use sottomano_desktop_sources::gamepad_controller_input::{
    evdev_gamepad_devices::{ConnectedGamepad, ControllerIdentity, PhysicalGamepadLink},
    player_indicator_leds::PlayerIndicator,
};
use sottomano_toml_config::panel_menus::gaming_menu::gamepad_bindings::GamepadBindingsConfig;

use super::{
    GamepadGroup, GamepadInteraction, connected_controller_indicator::ControllerMarking,
    controller_battery_indicator::ControllerBatteryIndicator, controller_custody_key,
    steam_firmware_indicator::SteamFirmwareIndicator, steam_radio_indicator::SteamRadioIndicator,
    usb_cable_indicator::UsbCableIndicator,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
};

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
        let headline_indicator = self
            .connected_controller_row(gamepad, link, bindings)
            .with_headline_glyph(Phosphor::Joystick(Weight::Regular).into());
        let rows = iter::once(self.compositor_driving_row(slot))
            .chain(self.controller_link_rows(slot, link, now))
            .chain(SteamRadioIndicator::radio_rows(link))
            .chain(ControllerBatteryIndicator::battery_rows(gamepad, link))
            .chain(SteamFirmwareIndicator::firmware_rows(link))
            .chain(UsbCableIndicator::cable_rows(slot, link));
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
                iter::once(self.connected_controller_row(gamepad, link, bindings))
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
