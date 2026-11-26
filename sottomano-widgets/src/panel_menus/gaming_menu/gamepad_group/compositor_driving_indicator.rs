use super::{ControllerCustody, GamepadGroup, GamepadInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
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
    pub(super) fn compositor_driving_row(&self, slot: usize) -> GroupIndicator<'_> {
        GroupIndicator::toggle(
            compositor_driving_toggle(self.controller_custody(slot)),
            GamepadInteraction::ToggleCompositorDriving { slot },
        )
        .with_hover_card(Box::new(move |_| self.compositor_driving_hover_card(slot)))
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
}
