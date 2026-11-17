use std::{borrow::Cow, convert::identity, rc::Rc};

use corredo::{
    popovers::MenuRowEmphasis,
    press_actions::flat_button,
    theming::{secondary_text_color, text_color},
    value_controls::SwitchResponse,
};
use iced::{
    Alignment, Element,
    widget::{Row, Space, button, mouse_area, row},
};
use sottomano_toml_config::Config;

use super::{
    GroupIndicator, IndicatorPinning, IndicatorPlacement, IndicatorPowerControl, RowScroll,
    RowValueControl,
};
use crate::{
    clock::Clock,
    group_indicators::{
        indicator_drawing::{face_icon, indicator_drawing, readout_view},
        indicator_gestures::IndicatorGesture,
        indicator_toggle_switch::indicator_toggle_switch,
    },
    group_menus::group_menu_state::GroupMenuPin,
    hover_cards::{HoverTarget, IndicatorKey, hoverable},
    indicator_groups::{
        indicator_group_content::GroupEvent, indicator_group_identity::IndicatorGroupIdentity,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

impl<'a> GroupIndicator<'a> {
    pub(crate) fn view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        hover_target: HoverTarget,
    ) -> Element<'a, GroupEvent> {
        let opening = self.card_opening_trigger;
        self.placed_view(
            config,
            reveal,
            clock,
            IndicatorPlacement::Panel,
            move |content| hoverable(opening, content, hover_target, GroupEvent::from),
        )
    }

    pub(crate) fn table_cell_view(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        self.placed_view(config, 1.0, clock, IndicatorPlacement::Table, identity)
    }

    pub(crate) fn menu_icon(&self, config: &Config) -> Element<'a, GroupEvent> {
        face_icon(config, &self.face, 1.0).unwrap_or_else(|| Space::new().into())
    }

    pub(crate) fn menu_value(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        let empty = || -> Element<'a, GroupEvent> { Space::new().into() };
        match &self.value {
            RowValueControl::IndicatorReadout(readout) => {
                readout_view(config, &self.face, readout, 1.0).unwrap_or_else(empty)
            }
            RowValueControl::Slider { draw, .. } => draw(1.0, clock, IndicatorPlacement::Menu),
            RowValueControl::Drawn(value) => value.view(1.0).unwrap_or_else(empty),
        }
    }

    fn with_switch_indicator(
        &self,
        config: &Config,
        switched_content: Element<'a, GroupEvent>,
        opacity: f32,
    ) -> Element<'a, GroupEvent> {
        let power_indicator = match self.power_control {
            IndicatorPowerControl::Absent => return switched_content,
            IndicatorPowerControl::Switch(switch_position) => {
                indicator_toggle_switch(config, switch_position, SwitchResponse::Pressable, opacity)
            }
            IndicatorPowerControl::InertSwitch(switch_position) => {
                indicator_toggle_switch(config, switch_position, SwitchResponse::Inert, opacity)
            }
            IndicatorPowerControl::StopGlyph => IconGlyph::from(Phosphor::Stop(Weight::Fill))
                .view(config.theme.font.icon_size_px, move |theme| {
                    secondary_text_color(theme).scale_alpha(opacity)
                }),
        };
        row![switched_content, power_indicator]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    pub(crate) fn menu_row_value(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        self.with_switch_indicator(config, self.menu_value(config, clock), 1.0)
    }

    pub(crate) fn row_emphasis(
        &self,
        group: &IndicatorGroupIdentity,
        inspected_indicator: Option<&IndicatorKey>,
    ) -> MenuRowEmphasis {
        if inspected_indicator == Some(&self.indicator_key(group)) {
            MenuRowEmphasis::Selected
        } else {
            MenuRowEmphasis::Plain
        }
    }

    pub(crate) fn row_scroll(&self) -> RowScroll<'a> {
        match &self.value {
            RowValueControl::Slider {
                adjust_by_scroll, ..
            } => RowScroll::AdjustsValue(Rc::clone(adjust_by_scroll)),
            RowValueControl::IndicatorReadout(_) | RowValueControl::Drawn(_) => RowScroll::Inert,
        }
    }

    pub(crate) fn menu_pin(&self, group: &IndicatorGroupIdentity) -> Option<GroupMenuPin> {
        match self.pinning {
            IndicatorPinning::Pinnable => Some(GroupMenuPin::Item(self.indicator_key(group))),
            IndicatorPinning::Unpinnable => None,
        }
    }

    #[must_use]
    pub fn indicator_key(&self, group: &IndicatorGroupIdentity) -> IndicatorKey {
        IndicatorKey {
            group: group.clone(),
            name: self.face.name.clone(),
        }
    }

    pub(crate) fn indicator_name(&self) -> Cow<'static, str> {
        self.face.name.clone()
    }

    pub(crate) fn placed_view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        placement: IndicatorPlacement,
        hover_area: impl FnOnce(Element<'a, GroupEvent>) -> Element<'a, GroupEvent>,
    ) -> Element<'a, GroupEvent> {
        let content = match &self.value {
            RowValueControl::IndicatorReadout(readout) => self.with_switch_indicator(
                config,
                indicator_drawing(config, &self.face, readout, reveal),
                reveal,
            ),
            RowValueControl::Slider { draw, .. } => draw(reveal, clock, placement),
            RowValueControl::Drawn(value) => {
                let composed_row = Row::with_children(
                    face_icon(config, &self.face, reveal)
                        .into_iter()
                        .chain(value.view(reveal)),
                )
                .spacing(config.theme.spacing.item_gap)
                .align_y(Alignment::Center)
                .into();
                self.with_switch_indicator(config, composed_row, reveal)
            }
        };
        let mut element = hover_area(content);
        if let IndicatorGesture::Performs { message, .. } = self.gestures.left.clone() {
            element = button(element)
                .padding(0)
                .style(flat_button(text_color))
                .on_press(GroupEvent::Interaction(message))
                .into();
        }
        if let IndicatorGesture::Performs { message, .. } = self.gestures.right.clone() {
            element = mouse_area(element)
                .on_right_press(GroupEvent::Interaction(message))
                .into();
        }
        element
    }
}
