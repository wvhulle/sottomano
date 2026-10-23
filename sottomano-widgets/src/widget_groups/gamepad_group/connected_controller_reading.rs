use std::path::Path;

use chrono::Locale;
use sottomano_config::{
    severity::Severity,
    widget_groups::gamepad_bindings::{
        ButtonRole, ClaimedButton, CompositorAction, GamepadBindingsConfig, StickDirection,
    },
};
use sottomano_sources::gamepad_controller_input::evdev_gamepad_devices::{
    ConnectedGamepad, GamepadPower,
};

use super::{
    ControllerOwner,
    controller_button_labels::ControllerFamily,
    diagram_anchor_mapping::{button_anchor, stick_anchor},
};
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{DiagramCallout, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const LOW_BATTERY_PERCENT: u8 = 15;
const DIRECTION_ARROWS: &[char] = &['←', '→', '↑', '↓', ' '];

pub struct ConnectedControllerReading<'a> {
    pub gamepad: &'a ConnectedGamepad,
    pub bindings: &'a GamepadBindingsConfig,
    pub owner: ControllerOwner,
    pub player_number: Option<usize>,
}

fn axis_label(
    first: Option<CompositorAction>,
    second: Option<CompositorAction>,
    axis_arrow: &str,
) -> Option<String> {
    let stem = |action: CompositorAction| action.short_label().trim_end_matches(DIRECTION_ARROWS);
    match (first, second) {
        (Some(first), Some(second)) if stem(first) == stem(second) => {
            Some(format!("{} {axis_arrow}", stem(first)))
        }
        (first, second) => {
            let labels: Vec<&str> = first
                .into_iter()
                .chain(second)
                .map(CompositorAction::short_label)
                .collect();
            (!labels.is_empty()).then(|| labels.join(" / "))
        }
    }
}

fn program_name(command: &[String]) -> String {
    command
        .first()
        .map(|program| {
            Path::new(program).file_name().map_or_else(
                || program.clone(),
                |name| name.to_string_lossy().into_owned(),
            )
        })
        .unwrap_or_default()
}

fn role_label(role: ButtonRole<'_>) -> String {
    match role {
        ButtonRole::PushToTalk => "dictate".to_owned(),
        ButtonRole::PointerButton(button) => button.short_label().to_owned(),
        ButtonRole::Compositor(action) => action.short_label().to_owned(),
        ButtonRole::TypedKey(key) => key.to_owned(),
        ButtonRole::Command(command) => program_name(command),
    }
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

    fn battery_description(&self) -> Option<String> {
        match self.gamepad.power {
            GamepadPower::NoBattery => None,
            GamepadPower::Discharging(percent) => Some(format!("{percent}%")),
            GamepadPower::Charging(percent) => Some(format!("{percent}%, charging")),
            GamepadPower::Full => Some("full".to_owned()),
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
        let controller = match self.battery_description() {
            Some(battery) => format!("{} · {battery}", self.gamepad.name),
            None => self.gamepad.name.clone(),
        };
        let shared_stick = self.bindings.sticks_conflict().then(|| {
            KeyValue::new("Sticks", "navigation and pointer share one stick")
                .level(Severity::Warning)
        });
        Section::dictionary(
            [
                KeyValue::new("Controller", controller),
                KeyValue::new("Buttons go to", self.owner_description()),
                KeyValue::new("Hand over", self.handover_chord_description()),
            ]
            .into_iter()
            .chain(shared_stick),
        )
    }

    fn stick_callouts(&self) -> impl Iterator<Item = DiagramCallout> {
        let navigation = &self.bindings.navigation;
        let navigation_label = [
            axis_label(
                navigation.action(StickDirection::Left),
                navigation.action(StickDirection::Right),
                "↔",
            ),
            axis_label(
                navigation.action(StickDirection::Up),
                navigation.action(StickDirection::Down),
                "↕",
            ),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let navigating = navigation.stick.map(|stick| DiagramCallout {
            anchor: stick_anchor(stick),
            label: navigation_label,
            level: Severity::Normal,
        });
        let pointing = self.bindings.pointer.stick.map(|stick| DiagramCallout {
            anchor: stick_anchor(stick),
            label: "pointer".to_owned(),
            level: if self.bindings.sticks_conflict() {
                Severity::Warning
            } else {
                Severity::Normal
            },
        });
        navigating.into_iter().chain(pointing)
    }

    fn button_callouts(&self) -> impl Iterator<Item = DiagramCallout> {
        let conflicting = self.bindings.conflicting_buttons();
        let claimed: Vec<ClaimedButton<'_>> = self.bindings.claimed_buttons().collect();
        let chord_only = self
            .bindings
            .handover_chord
            .iter()
            .filter(|button| !claimed.iter().any(|claim| claim.button == **button))
            .map(|&button| DiagramCallout {
                anchor: button_anchor(button),
                label: "hand over".to_owned(),
                level: Severity::Normal,
            })
            .collect::<Vec<_>>();
        claimed
            .into_iter()
            .map(move |claim| DiagramCallout {
                anchor: button_anchor(claim.button),
                label: role_label(claim.role),
                level: if conflicting.contains(&claim.button) {
                    Severity::Warning
                } else {
                    Severity::Normal
                },
            })
            .chain(chord_only)
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
        TooltipBody::sections([
            self.overview_section(),
            Section::controller_diagram(
                self.family().controller_model(),
                self.stick_callouts().chain(self.button_callouts()),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_directions_share_one_axis_label() {
        assert_eq!(
            axis_label(
                Some(CompositorAction::FocusColumnLeft),
                Some(CompositorAction::FocusColumnRight),
                "↔",
            ),
            Some("column ↔".to_owned())
        );
    }

    #[test]
    fn different_directions_are_both_named() {
        assert_eq!(
            axis_label(
                Some(CompositorAction::FocusWorkspaceUp),
                Some(CompositorAction::CloseWindow),
                "↕",
            ),
            Some("workspace ↑ / close window".to_owned())
        );
    }

    #[test]
    fn commands_are_labelled_by_their_program() {
        assert_eq!(
            program_name(&[
                "/run/current-system/sw/bin/notify-send".to_owned(),
                "hi".to_owned()
            ]),
            "notify-send"
        );
    }
}
