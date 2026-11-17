use std::{borrow::Cow, rc::Rc};

use sottomano_toml_config::Config;

use super::{
    DescribeCard, GroupIndicator, HoverFetch, IndicatorPinning, IndicatorPlacement,
    IndicatorPowerControl, IndicatorRefresh, IndicatorValue, RowValueControl,
};
use crate::{
    group_indicators::{
        aligned_quantity::{AlignedQuantity, IndicatorAlignment},
        indicator_drawing::{IdentityOrdinal, IndicatorFace, IndicatorIcon},
        indicator_gestures::IndicatorGesture,
        slider_control::{ControlMessage, SliderControl, SliderShownIndicator},
        toggle_control::{IconToggle, SwitchPosition},
    },
    hover_cards::{MouseGesture, PressAction},
    indicator_groups::{
        indicator_group_content::GroupEvent, indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::ongoing_toast::IndicatorAlerts,
};

impl<'a> GroupIndicator<'a> {
    pub fn slider<Interaction: Into<IndicatorGroupInteraction>>(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> Interaction + 'a,
    ) -> Self {
        let locale = config.calendar.locale;
        let face = slider.indicator_face();
        let readout = slider.indicator_readout(locale);
        let alerts = slider.indicator_alerts(locale);
        let card: DescribeCard<'a> = Box::new(|_| slider.readout_hover_card());
        let shown_indicator = SliderShownIndicator {
            face: face.clone(),
            readout: readout.clone(),
        };
        let on_control = Rc::new(on_control);
        let scrolled_control = Rc::clone(&on_control);
        let control_actions = slider.press_actions();
        let slider_row = Self::assembled_row(
            face,
            RowValueControl::Slider {
                adjust_by_scroll: Rc::new(move |delta| {
                    scrolled_control(ControlMessage::Scroll(delta)).into()
                }),
                draw: Box::new(move |reveal, _, placement| {
                    let on_control = Rc::clone(&on_control);
                    let slider_view = match placement {
                        IndicatorPlacement::Panel => {
                            slider.view(config, shown_indicator.clone(), reveal)
                        }
                        IndicatorPlacement::Table => slider.always_open_view(config),
                        IndicatorPlacement::Menu => slider.menu_view(config),
                    };
                    slider_view
                        .map(move |message| GroupEvent::Interaction(on_control(message).into()))
                }),
            },
            IndicatorPinning::Pinnable,
            card,
            alerts,
        );
        Self {
            control_actions,
            ..slider_row
        }
    }

    pub fn composed(
        name: impl Into<Cow<'static, str>>,
        icon: IndicatorIcon,
        value: IndicatorValue<'a>,
        card: DescribeCard<'a>,
    ) -> Self {
        Self::assembled_row(
            IndicatorFace {
                name: name.into(),
                icon,
                ordinal: IdentityOrdinal::Unnumbered,
            },
            RowValueControl::Drawn(value),
            IndicatorPinning::Unpinnable,
            card,
            IndicatorAlerts::default(),
        )
    }

    pub fn composed_pinnable(
        name: impl Into<Cow<'static, str>>,
        icon: IndicatorIcon,
        value: IndicatorValue<'a>,
        card: DescribeCard<'a>,
    ) -> Self {
        Self {
            pinning: IndicatorPinning::Pinnable,
            ..Self::composed(name, icon, value, card)
        }
    }

    pub fn toggle(toggle: IconToggle, message: impl Into<IndicatorGroupInteraction>) -> Self {
        let switch_position = toggle.switch_position;
        let press_does = toggle.press_does();
        toggle
            .indicator()
            .on_switch_press(switch_position, press_does, message)
    }

    #[must_use]
    pub fn on_switch_press(
        mut self,
        switch_position: impl Into<SwitchPosition>,
        description: &'static str,
        message: impl Into<IndicatorGroupInteraction>,
    ) -> Self {
        self.power_control = IndicatorPowerControl::Switch(switch_position.into());
        self.on_press(description, message)
    }

    #[must_use]
    pub fn inert_switch(mut self, switch_position: impl Into<SwitchPosition>) -> Self {
        self.power_control = IndicatorPowerControl::InertSwitch(switch_position.into());
        self
    }

    #[must_use]
    pub fn on_stop_press(
        mut self,
        description: &'static str,
        message: impl Into<IndicatorGroupInteraction>,
    ) -> Self {
        self.power_control = IndicatorPowerControl::StopGlyph;
        self.on_press(description, message)
    }

    #[must_use]
    pub fn on_press(
        mut self,
        description: &'static str,
        message: impl Into<IndicatorGroupInteraction>,
    ) -> Self {
        self.gestures.left = IndicatorGesture::Performs {
            description,
            message: message.into(),
        };
        self
    }

    #[must_use]
    pub fn on_right_press(
        mut self,
        description: &'static str,
        message: impl Into<IndicatorGroupInteraction>,
    ) -> Self {
        self.gestures.right = IndicatorGesture::Performs {
            description,
            message: message.into(),
        };
        self
    }

    #[must_use]
    pub fn with_button_actions(
        self,
        button_actions: impl IntoIterator<Item = PressAction>,
    ) -> Self {
        Self {
            button_actions: button_actions.into_iter().collect(),
            ..self
        }
    }

    #[must_use]
    pub fn fetching_on_hover(
        self,
        start: impl Into<IndicatorGroupInteraction>,
        stop: impl Into<IndicatorGroupInteraction>,
    ) -> Self {
        Self {
            hover_fetch: HoverFetch::Fetch {
                start: Box::new(start.into()),
                stop: Box::new(stop.into()),
            },
            ..self
        }
    }

    #[must_use]
    pub fn refreshed_by(self, refresh: impl Into<IndicatorGroupInteraction>) -> Self {
        Self {
            refresh: IndicatorRefresh::Refreshes(refresh.into()),
            ..self
        }
    }

    #[must_use]
    pub fn press_actions(&self) -> Vec<PressAction> {
        let gesture_action = |gesture, indicator_gesture: &IndicatorGesture| {
            indicator_gesture
                .description()
                .map(|description| PressAction::new(gesture, description))
        };
        gesture_action(MouseGesture::LeftClick, &self.gestures.left)
            .into_iter()
            .chain(gesture_action(
                MouseGesture::RightClick,
                &self.gestures.right,
            ))
            .chain(self.control_actions.iter().cloned())
            .chain(self.button_actions.iter().cloned())
            .collect()
    }

    #[must_use]
    pub fn aligned_as(self, quantity: AlignedQuantity) -> Self {
        Self {
            alignment: IndicatorAlignment::Aligned(quantity),
            ..self
        }
    }
}
