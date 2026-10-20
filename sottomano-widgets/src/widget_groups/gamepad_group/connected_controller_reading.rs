use chrono::Locale;
use sottomano_config::{
    severity::Severity,
    widget_groups::gamepad_bindings::{ButtonRole, GamepadBindingsConfig, StickDirection},
};
use sottomano_sources::gamepad_controller_input::evdev_gamepad_devices::{
    ConnectedGamepad, GamepadPower,
};

use super::{ControllerOwner, controller_button_labels::ControllerFamily};
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const LOW_BATTERY_PERCENT: u8 = 15;

pub struct ConnectedControllerReading<'a> {
    pub gamepad: &'a ConnectedGamepad,
    pub bindings: &'a GamepadBindingsConfig,
    pub owner: ControllerOwner,
    pub player_number: Option<usize>,
}

impl ConnectedControllerReading<'_> {
    fn family(&self) -> ControllerFamily {
        ControllerFamily::of_vendor(self.gamepad.vendor_id)
    }

    fn battery_percent(&self) -> Option<u8> {
        match self.gamepad.power {
            GamepadPower::Discharging(percent) | GamepadPower::Charging(percent) => Some(percent),
            GamepadPower::NoBattery | GamepadPower::Full => None,
        }
    }

    fn battery_description(&self) -> String {
        match self.gamepad.power {
            GamepadPower::NoBattery => "none reported".to_owned(),
            GamepadPower::Discharging(percent) => format!("{percent}%"),
            GamepadPower::Charging(percent) => format!("{percent}%, charging"),
            GamepadPower::Full => "full".to_owned(),
        }
    }

    fn owner_description(&self) -> &'static str {
        match self.owner {
            ControllerOwner::Shell => "the shell, except in Steam and games",
            ControllerOwner::HandedOver => "Steam and games",
        }
    }

    fn handover_chord_description(&self) -> String {
        let family = self.family();
        self.bindings
            .handover_chord
            .iter()
            .map(|&button| family.button_label(button))
            .collect::<Vec<_>>()
            .join(" + ")
    }

    fn overview_section(&self) -> Section {
        Section::dictionary([
            KeyValue::new("Controller", self.gamepad.name.as_str()),
            KeyValue::new("Button names", self.family().button_layout_name()),
            KeyValue::new("Battery", self.battery_description()),
            KeyValue::new("Buttons go to", self.owner_description()),
            KeyValue::new("Hand over", self.handover_chord_description()),
        ])
    }

    fn navigation_section(&self) -> Option<Section> {
        let navigation = &self.bindings.navigation;
        let stick = navigation.stick?;
        let entries: Vec<KeyValue> = StickDirection::ALL
            .into_iter()
            .filter_map(|direction| {
                let action = navigation.action(direction)?;
                Some(KeyValue::new(direction.name(), action.press_hint()))
            })
            .collect();
        Some(Section::dictionary(entries).titled(("Navigation", stick.name())))
    }

    fn pointer_section(&self) -> Option<Section> {
        let pointer = &self.bindings.pointer;
        let family = self.family();
        let tilt = pointer.stick.map(|stick| {
            KeyValue::new(
                format!("Tilt {}", stick.name().to_lowercase()),
                format!("move, up to {} px/s", pointer.pixels_per_second),
            )
        });
        let buttons = pointer.claimed_buttons().map(|claimed| {
            KeyValue::new(
                claimed.pointer_button.name(),
                family.described_button(claimed.button),
            )
        });
        let entries: Vec<KeyValue> = tilt.into_iter().chain(buttons).collect();
        (!entries.is_empty()).then(|| Section::dictionary(entries).titled("Pointer"))
    }

    fn claimed_section(
        &self,
        title: &'static str,
        describe: impl Fn(ButtonRole<'_>) -> Option<String>,
    ) -> Option<Section> {
        let family = self.family();
        let entries: Vec<KeyValue> = self
            .bindings
            .claimed_buttons()
            .filter_map(|claimed| {
                Some(KeyValue::new(
                    family.described_button(claimed.button),
                    describe(claimed.role)?,
                ))
            })
            .collect();
        (!entries.is_empty()).then(|| Section::dictionary(entries).titled(title))
    }

    fn push_to_talk_section(&self) -> Option<Section> {
        let push_to_talk = &self.bindings.push_to_talk;
        let button = push_to_talk.button?;
        Some(
            Section::dictionary([
                KeyValue::new("Hold", self.family().described_button(button)),
                KeyValue::new("Starts", push_to_talk.start.join(" ")),
                KeyValue::new("Stops", push_to_talk.stop.join(" ")),
            ])
            .titled("Dictation"),
        )
    }

    fn conflicts_section(&self) -> Option<Section> {
        let family = self.family();
        let doubled_buttons = self
            .bindings
            .conflicting_buttons()
            .into_iter()
            .map(|button| {
                KeyValue::new(
                    family.described_button(button),
                    "bound in two sections; the first one listed here wins",
                )
                .level(Severity::Warning)
            });
        let shared_stick = self.bindings.sticks_conflict().then(|| {
            KeyValue::new("Sticks", "navigation and pointer share one stick")
                .level(Severity::Warning)
        });
        let entries: Vec<KeyValue> = doubled_buttons.chain(shared_stick).collect();
        (!entries.is_empty()).then(|| Section::dictionary(entries).titled("Conflicts"))
    }
}

impl BarWidget for ConnectedControllerReading<'_> {
    fn name(&self) -> &'static str {
        "Game controller"
    }

    fn icon(&self) -> Glyph {
        Phosphor::GameController(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        let battery = self.battery_percent().map(|percent| format!("{percent}%"));
        match (self.player_number, battery) {
            (Some(number), Some(battery)) => Some(format!("{number} · {battery}")),
            (Some(number), None) => Some(number.to_string()),
            (None, battery) => battery,
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match (self.owner, self.gamepad.power) {
            (_, GamepadPower::Discharging(percent)) if percent <= LOW_BATTERY_PERCENT => {
                WidgetState::Warning
            }
            (ControllerOwner::HandedOver, _) => WidgetState::Off,
            (ControllerOwner::Shell, _) => WidgetState::On,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let purposes = [
            self.navigation_section(),
            self.pointer_section(),
            self.claimed_section("Typed keys", |role| match role {
                ButtonRole::TypedKey(key) => Some(key.to_owned()),
                _ => None,
            }),
            self.claimed_section("Window actions", |role| match role {
                ButtonRole::Compositor(action) => Some(action.press_hint().to_owned()),
                _ => None,
            }),
            self.claimed_section("Commands", |role| match role {
                ButtonRole::Command(command) => Some(command.join(" ")),
                _ => None,
            }),
            self.push_to_talk_section(),
            self.conflicts_section(),
        ];
        TooltipBody::sections(
            [self.overview_section()]
                .into_iter()
                .chain(purposes.into_iter().flatten()),
        )
    }
}
