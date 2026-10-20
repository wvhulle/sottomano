use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_config::widget_groups::gamepad_bindings::{
    ButtonBinding, GamepadBindingsConfig, GamepadStick, StickDirection, StickRole,
};
use sottomano_sources::gamepad_controller_input::gilrs_gamepad_events::{
    ConnectedGamepad, GamepadPower,
};

use super::{ControllerOwner, controller_button_labels::ControllerFamily};
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

const LOW_BATTERY_PERCENT: u8 = 15;

pub struct ConnectedControllerReading<'a> {
    pub gamepad: &'a ConnectedGamepad,
    pub bindings: &'a GamepadBindingsConfig,
    pub owner: ControllerOwner,
}

impl ConnectedControllerReading<'_> {
    fn family(&self) -> ControllerFamily {
        ControllerFamily::of_vendor(self.gamepad.vendor_id)
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

    fn battery_description(&self) -> String {
        match self.gamepad.power {
            GamepadPower::Unknown => "unknown".to_owned(),
            GamepadPower::Wired => "wired".to_owned(),
            GamepadPower::Discharging(percent) => format!("{percent}%"),
            GamepadPower::Charging(percent) => format!("{percent}%, charging"),
            GamepadPower::Charged => "charged".to_owned(),
        }
    }

    fn binding_records(&self) -> impl Iterator<Item = TableRecord<2>> {
        let family = self.family();
        let hint = |binding: &Option<ButtonBinding>| {
            binding
                .as_ref()
                .map(|binding| Cell::from(binding.press_hint()))
        };
        let buttons = self.bindings.buttons.iter().map(move |(button, bound)| {
            TableRecord::new(
                family.button_label(*button),
                [hint(&bound.press), hint(&bound.release)],
            )
        });
        self.stick_records().into_iter().chain(buttons)
    }

    fn stick_records(&self) -> Vec<TableRecord<2>> {
        [GamepadStick::Left, GamepadStick::Right]
            .into_iter()
            .flat_map(|stick| {
                let bound = self.bindings.sticks.of(stick);
                let pointer = (bound.role == StickRole::Pointer).then(|| {
                    TableRecord::new(
                        stick_label(stick),
                        [Some(Cell::from("move the pointer")), None],
                    )
                });
                let flicks = [
                    StickDirection::Up,
                    StickDirection::Down,
                    StickDirection::Left,
                    StickDirection::Right,
                ]
                .into_iter()
                .filter_map(move |direction| {
                    let binding = bound.flick_binding(direction)?;
                    Some(TableRecord::new(
                        format!("{} {}", stick_label(stick), direction_label(direction)),
                        [Some(Cell::from(binding.press_hint())), None],
                    ))
                });
                pointer.into_iter().chain(flicks)
            })
            .collect()
    }
}

const fn stick_label(stick: GamepadStick) -> &'static str {
    match stick {
        GamepadStick::Left => "Left stick",
        GamepadStick::Right => "Right stick",
    }
}

const fn direction_label(direction: StickDirection) -> &'static str {
    match direction {
        StickDirection::Up => "up",
        StickDirection::Down => "down",
        StickDirection::Left => "left",
        StickDirection::Right => "right",
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
        match self.gamepad.power {
            GamepadPower::Discharging(percent) | GamepadPower::Charging(percent) => {
                Some(format!("{percent}%"))
            }
            GamepadPower::Unknown | GamepadPower::Wired | GamepadPower::Charged => None,
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
        TooltipBody::sections([
            Section::dictionary([
                KeyValue::new("Controller", self.gamepad.name.as_str()),
                KeyValue::new("Button names", self.family().button_layout_name()),
                KeyValue::new("Battery", self.battery_description()),
                KeyValue::new("Buttons go to", self.owner_description()),
                KeyValue::new("Hand over", self.handover_chord_description()),
            ]),
            Section::column_table(
                [
                    ColumnHeading::new("Press").align_x(Horizontal::Left),
                    ColumnHeading::new("Release").align_x(Horizontal::Left),
                ],
                self.binding_records(),
            )
            .titled("Buttons"),
        ])
    }
}
