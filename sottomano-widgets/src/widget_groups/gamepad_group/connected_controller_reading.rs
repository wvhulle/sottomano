use std::{iter::once, path::Path};

use chrono::Locale;
use sottomano_desktop_sources::gamepad_controller_input::{
    evdev_gamepad_devices::{
        ConnectedGamepad, ControllerIdentity, GamepadOrigin, PhysicalGamepadLink,
    },
    player_indicator_leds::LitPlayer,
};
use sottomano_toml_config::{
    severity::Severity,
    widget_groups::gamepad_bindings::{
        ButtonRole, ClaimedButton, CompositorAction, GamepadBindingsConfig, StickDirection,
        StickSharing,
    },
};

use super::{
    ControllerCustody,
    controller_button_labels::ControllerFamily,
    diagram_anchor_mapping::{button_anchor, stick_anchor},
};
use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{
        HoverCardBody, Section, hover_card_figure_payloads::DiagramCallout,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const DIRECTION_ARROWS: &[char] = &['←', '→', '↑', '↓', ' '];

#[derive(Debug, Clone, Copy)]
pub enum ControllerMarking<'a> {
    PlayerLights(&'a LitPlayer),
    PairedAlias(&'a str),
    IconOnly,
}

impl ControllerMarking<'_> {
    fn identity_ordinal(self) -> Option<u8> {
        match self {
            ControllerMarking::PlayerLights(lit_player) => Some(lit_player.number),
            ControllerMarking::PairedAlias(_) | ControllerMarking::IconOnly => None,
        }
    }

    fn alias_label(self, model_name: &str) -> Option<String> {
        match self {
            ControllerMarking::PairedAlias(alias) => Some(distinguishing_name(alias, model_name)),
            ControllerMarking::PlayerLights(_) | ControllerMarking::IconOnly => None,
        }
    }

    fn player_rows(self) -> impl Iterator<Item = LabeledValue> {
        let lit_player = match self {
            ControllerMarking::PlayerLights(lit_player) => Some(lit_player),
            ControllerMarking::PairedAlias(_) | ControllerMarking::IconOnly => None,
        };
        lit_player.into_iter().flat_map(|lit_player| {
            [
                LabeledValue::new("Player", lit_player.number.to_string()),
                LabeledValue::new("Lights", Self::led_pattern(lit_player)),
            ]
        })
    }

    fn numbering_note(self) -> Option<Section> {
        match self {
            ControllerMarking::PlayerLights(_) => Some(Section::detailed_note(
                "Player numbered by the kernel",
                "Assigned in the order the controllers connected, and shown on the controller's \
                 lights. Games may number players differently, for example by who presses a \
                 button first."
                    .to_owned(),
            )),
            ControllerMarking::PairedAlias(_) | ControllerMarking::IconOnly => None,
        }
    }

    fn led_pattern(lit_player: &LitPlayer) -> String {
        lit_player
            .leds_lit
            .iter()
            .map(|lit| if *lit { '●' } else { '○' })
            .collect()
    }
}

pub struct ConnectedControllerReading<'a> {
    pub gamepad: &'a ConnectedGamepad,
    pub bindings: &'a GamepadBindingsConfig,
    pub custody: ControllerCustody,
    pub marking: ControllerMarking<'a>,
}

fn axis_label(
    first: Option<CompositorAction>,
    second: Option<CompositorAction>,
    axis_arrow: &str,
) -> Option<String> {
    let stem = |action: CompositorAction| action.short_label().trim_end_matches(DIRECTION_ARROWS);
    if let Some(first_action) = first
        && let Some(second_action) = second
        && stem(first_action) == stem(second_action)
    {
        return Some(format!("{} {axis_arrow}", stem(first_action)));
    }
    let labels: Vec<&str> = first
        .into_iter()
        .chain(second)
        .map(CompositorAction::short_label)
        .collect();
    (!labels.is_empty()).then(|| labels.join(" / "))
}

fn distinguishing_name(alias: &str, model_name: &str) -> String {
    let alias_words: Vec<&str> = alias.split_whitespace().collect();
    let model_words: Vec<&str> = model_name.split_whitespace().collect();
    let shared_trailing_words = alias_words
        .iter()
        .rev()
        .zip(model_words.iter().rev())
        .take_while(|(alias_word, model_word)| alias_word == model_word)
        .count();
    let shared_leading_words = alias_words
        .iter()
        .zip(model_words.iter())
        .take_while(|(alias_word, model_word)| alias_word == model_word)
        .count();
    let distinguishing_words = alias_words[..alias_words.len() - shared_trailing_words]
        .iter()
        .skip(shared_leading_words);
    let distinguishing = distinguishing_words.copied().collect::<Vec<_>>().join(" ");
    if distinguishing.is_empty() {
        alias.to_owned()
    } else {
        distinguishing
    }
}

fn program_name(command: &[String]) -> String {
    command
        .first()
        .map(|program| {
            Path::new(program)
                .file_name()
                .map_or_else(|| program.clone(), |name| name.display().to_string())
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
        ControllerFamily::from(self.gamepad.vendor_id)
    }

    fn control_description(&self) -> String {
        match self.custody {
            ControllerCustody::Compositor => "the compositor, through the bindings below",
            ControllerCustody::HandedOver => {
                "the focused window; switch on compositor control to steer windows"
            }
        }
        .to_owned()
    }

    fn paired_alias(&self) -> Option<&str> {
        match &self.gamepad.origin {
            GamepadOrigin::Physical(PhysicalGamepadLink {
                identity: ControllerIdentity::Addressed { alias, .. },
                ..
            }) if *alias != self.gamepad.name => Some(alias),
            GamepadOrigin::Physical(_) | GamepadOrigin::Virtual => None,
        }
    }

    fn identity_section(&self) -> Section {
        let named = self.paired_alias().map_or_else(
            || vec![LabeledValue::new("Controller", self.gamepad.name.clone())],
            |alias| {
                vec![
                    LabeledValue::new("Controller", alias),
                    LabeledValue::new("Model", self.gamepad.name.clone()),
                ]
            },
        );
        Section::dictionary(named.into_iter().chain(self.marking.player_rows()))
    }

    fn bindings_section(&self) -> Section {
        let shared_stick = (self.bindings.stick_sharing() == StickSharing::Shared).then(|| {
            LabeledValue::new("Sticks", "navigation and pointer share one stick")
                .level(Severity::Warning)
        });
        Section::dictionary(
            [LabeledValue::new(
                "Buttons go to",
                self.control_description(),
            )]
            .into_iter()
            .chain(shared_stick),
        )
        .titled("Bindings")
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
            level: match self.bindings.stick_sharing() {
                StickSharing::Shared => Severity::Warning,
                StickSharing::Separate => Severity::Normal,
            },
        });
        navigating.into_iter().chain(pointing)
    }

    fn button_callouts(&self) -> impl Iterator<Item = DiagramCallout> {
        let conflicting = self.bindings.conflicting_buttons();
        let claimed: Vec<ClaimedButton<'_>> = self.bindings.claimed_buttons().collect();
        claimed.into_iter().map(move |claim| DiagramCallout {
            anchor: button_anchor(claim.button),
            label: role_label(claim.role),
            level: if conflicting.contains(&claim.button) {
                Severity::Warning
            } else {
                Severity::Normal
            },
        })
    }
}

impl GroupItemReading for ConnectedControllerReading<'_> {
    fn name(&self) -> &'static str {
        "Game controller"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::GameController(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.marking.alias_label(&self.gamepad.name)
    }

    fn identity_ordinal(&self) -> Option<u8> {
        self.marking.identity_ordinal()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.custody {
            ControllerCustody::Compositor => ReadingDisplayState::On,
            ControllerCustody::HandedOver => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(
            once(self.identity_section())
                .chain(self.marking.numbering_note())
                .chain([
                    self.bindings_section(),
                    Section::controller_diagram(
                        self.family().controller_model(),
                        self.stick_callouts().chain(self.button_callouts()),
                    ),
                ]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_model_name_is_dropped_from_the_alias() {
        let model = "DualSense Wireless Controller";
        assert_eq!(
            distinguishing_name("Blue DualSense Wireless Controller", model),
            "Blue"
        );
        assert_eq!(
            distinguishing_name(
                "Blue DualSense Wireless Controller",
                "Sony Interactive Entertainment DualSense Wireless Controller"
            ),
            "Blue"
        );
        assert_eq!(distinguishing_name(model, model), model);
        assert_eq!(distinguishing_name("Couch pad", model), "Couch pad");
    }

    #[test]
    fn matching_directions_share_one_axis_label() {
        assert_eq!(
            axis_label(
                Some(CompositorAction::FocusLeft),
                Some(CompositorAction::FocusRight),
                "↔",
            ),
            Some("focus ↔".to_owned())
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
