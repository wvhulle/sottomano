use std::{borrow::Cow, rc::Rc};

use corredo::{orientation::Axis, transitions::reveal};
use iced::{
    Alignment, Element, Padding,
    mouse::ScrollDelta,
    widget::{Row, container},
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        aligned_quantity::IndicatorAlignment,
        indicator_drawing::{
            IdentityOrdinal, IndicatorFace, IndicatorIcon, IndicatorReadout, ReadoutBadge,
            ReadoutGauge, ReadoutLabel, badged_face_icon, readout_view,
        },
        indicator_gestures::IndicatorGestures,
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::{CardHeader, PressAction, hover_card_body::Section},
    indicator_groups::{
        indicator_group_content::GroupEvent, indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::ongoing_toast::{
        IndicatorAlerts, LevelNotice, OngoingToast, OngoingToastActionButton, StateToast,
    },
};

#[cfg(test)]
mod group_indicator_tests;
#[cfg(test)]
use sottomano_toml_config::severity::Severity;

mod indicator_controls;
mod indicator_toasts;
mod indicator_views;

pub type DescribeCard<'view> = Box<dyn Fn(Clock) -> Vec<Section> + 'view>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelVisibility {
    Shown,
    Hidden,
}

type DrawItem<'view> = Box<dyn Fn(f32) -> Element<'view, GroupEvent> + 'view>;
type DrawControl<'view> =
    Box<dyn Fn(f32, Clock, IndicatorPlacement) -> Element<'view, GroupEvent> + 'view>;

type AdjustByScroll<'view> = Rc<dyn Fn(ScrollDelta) -> IndicatorGroupInteraction + 'view>;

pub(crate) enum RowScroll<'view> {
    Inert,
    AdjustsValue(AdjustByScroll<'view>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndicatorPlacement {
    Panel,
    Table,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndicatorPowerControl {
    Absent,
    Switch(SwitchPosition),
    InertSwitch(SwitchPosition),
    StopGlyph,
}

#[derive(Debug, Clone)]
pub enum HoverFetch {
    NothingToFetch,
    Fetch {
        start: Box<IndicatorGroupInteraction>,
        stop: Box<IndicatorGroupInteraction>,
    },
}

#[derive(Debug, Clone)]
pub enum IndicatorRefresh {
    Unrefreshable,
    Refreshes(IndicatorGroupInteraction),
}

pub struct RowHoverCard {
    pub sections: Vec<Section>,
    pub header: CardHeader,
}

pub enum IndicatorValue<'view> {
    Nothing,
    Drawn(DrawItem<'view>),
}

impl<'view> IndicatorValue<'view> {
    pub fn drawn(draw: impl Fn(f32) -> Element<'view, GroupEvent> + 'view) -> Self {
        IndicatorValue::Drawn(Box::new(draw))
    }

    fn view(&self, reveal: f32) -> Option<Element<'view, GroupEvent>> {
        match self {
            IndicatorValue::Nothing => None,
            IndicatorValue::Drawn(draw) => Some(draw(reveal)),
        }
    }
}

enum RowValueControl<'view> {
    IndicatorReadout(IndicatorReadout),
    Slider {
        draw: DrawControl<'view>,
        adjust_by_scroll: AdjustByScroll<'view>,
    },
    Drawn(IndicatorValue<'view>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndicatorPinning {
    Pinnable,
    Unpinnable,
}

#[cfg(test)]
pub(crate) fn indicator_toast_severity(indicator: &GroupIndicator<'_>) -> Option<Severity> {
    use crate::widgets::ongoing_toast::state_alert_severity;

    state_alert_severity(indicator.shown_state(), &indicator.alerts.state_toast)
}

#[cfg(test)]
pub(crate) fn named_test_indicator<'indicator>(name: &'static str) -> GroupIndicator<'indicator> {
    GroupIndicator::glyph_only(name, IconGlyph::default(), IndicatorState::Idle)
        .with_hover_card(Box::new(|_| Vec::new()))
}

pub struct GroupIndicator<'view> {
    pub(crate) face: IndicatorFace,
    value: RowValueControl<'view>,
    pub(crate) pinning: IndicatorPinning,
    pub card: DescribeCard<'view>,
    pub(crate) alerts: IndicatorAlerts,
    pub(crate) gestures: IndicatorGestures,
    control_actions: Vec<PressAction>,
    button_actions: Vec<PressAction>,
    pub hover_fetch: HoverFetch,
    pub refresh: IndicatorRefresh,
    power_control: IndicatorPowerControl,
    ongoing_toast_action_buttons: Vec<OngoingToastActionButton<IndicatorGroupInteraction>>,
    ongoing_toasts: Vec<OngoingToast<IndicatorGroupInteraction>>,
    pub(crate) alignment: IndicatorAlignment,
    pub panel_visibility: PanelVisibility,
}

impl<'view> GroupIndicator<'view> {
    fn assembled_row(
        face: IndicatorFace,
        value: RowValueControl<'view>,
        pinning: IndicatorPinning,
        card: DescribeCard<'view>,
        alerts: IndicatorAlerts,
    ) -> Self {
        Self {
            face,
            value,
            pinning,
            card,
            alerts,
            gestures: IndicatorGestures::inert(),
            control_actions: Vec::new(),
            button_actions: Vec::new(),
            hover_fetch: HoverFetch::NothingToFetch,
            refresh: IndicatorRefresh::Unrefreshable,
            power_control: IndicatorPowerControl::Absent,
            ongoing_toast_action_buttons: Vec::new(),
            ongoing_toasts: Vec::new(),
            alignment: IndicatorAlignment::Unaligned,
            panel_visibility: PanelVisibility::Shown,
        }
    }

    #[must_use]
    pub fn glyph_only(
        name: impl Into<Cow<'static, str>>,
        glyph: IconGlyph,
        state: IndicatorState,
    ) -> Self {
        Self::icon_only(name, IndicatorIcon::IconGlyph { glyph, state })
    }

    #[must_use]
    pub fn icon_only(name: impl Into<Cow<'static, str>>, icon: IndicatorIcon) -> Self {
        Self::assembled_row(
            IndicatorFace {
                name: name.into(),
                icon,
                ordinal: IdentityOrdinal::Unnumbered,
            },
            RowValueControl::IndicatorReadout(IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            }),
            IndicatorPinning::Pinnable,
            Box::new(|_| Vec::new()),
            IndicatorAlerts::default(),
        )
    }

    #[must_use]
    pub fn with_headline_glyph(self, glyph: IconGlyph) -> Self {
        let state = self.shown_state();
        self.with_icon(IndicatorIcon::IconGlyph { glyph, state })
    }

    pub(crate) fn handle_view(
        &self,
        config: &'view Config,
        reveal_fraction: f32,
    ) -> Element<'view, GroupEvent> {
        let (badge, value) = match &self.value {
            RowValueControl::IndicatorReadout(readout) => (
                readout.badge,
                readout_view(config, &self.face, readout, 1.0),
            ),
            RowValueControl::Drawn(value) => (ReadoutBadge::Unbadged, value.view(1.0)),
            RowValueControl::Slider { .. } => (ReadoutBadge::Unbadged, None),
        };
        let hidden = 1.0 - reveal_fraction;
        let collapsing_value: Element<'view, GroupEvent> = match value {
            Some(value) if hidden > 0.0 => reveal(
                container(value).padding(Padding::ZERO.left(config.theme.spacing.item_gap)),
                hidden,
                Axis::Horizontal,
            )
            .into(),
            Some(_) | None => Row::new().into(),
        };
        Row::with_children([
            badged_face_icon(config, &self.face, badge),
            collapsing_value,
        ])
        .align_y(Alignment::Center)
        .into()
    }

    #[must_use]
    pub fn with_icon(self, icon: IndicatorIcon) -> Self {
        Self {
            face: IndicatorFace { icon, ..self.face },
            ..self
        }
    }

    #[must_use]
    pub fn with_readout(self, readout: IndicatorReadout) -> Self {
        Self {
            value: RowValueControl::IndicatorReadout(readout),
            ..self
        }
    }

    #[must_use]
    pub fn with_label(self, label: ReadoutLabel) -> Self {
        self.with_readout_part(|readout| IndicatorReadout { label, ..readout })
    }

    #[must_use]
    pub fn with_gauge(self, gauge: ReadoutGauge) -> Self {
        self.with_readout_part(|readout| IndicatorReadout { gauge, ..readout })
    }

    #[must_use]
    pub fn with_badge(self, badge: ReadoutBadge) -> Self {
        self.with_readout_part(|readout| IndicatorReadout { badge, ..readout })
    }

    fn with_readout_part(self, change: impl FnOnce(IndicatorReadout) -> IndicatorReadout) -> Self {
        let value = match self.value {
            RowValueControl::IndicatorReadout(readout) => {
                RowValueControl::IndicatorReadout(change(readout))
            }
            unchanged_value @ (RowValueControl::Slider { .. } | RowValueControl::Drawn(_)) => {
                unchanged_value
            }
        };
        Self { value, ..self }
    }

    #[must_use]
    pub fn with_ordinal(self, ordinal: IdentityOrdinal) -> Self {
        Self {
            face: IndicatorFace {
                ordinal,
                ..self.face
            },
            ..self
        }
    }

    #[must_use]
    pub fn with_state_toast(self, state_toast: StateToast) -> Self {
        Self {
            alerts: IndicatorAlerts {
                state_toast,
                ..self.alerts
            },
            ..self
        }
    }

    #[must_use]
    pub fn with_alerts(self, alerts: IndicatorAlerts) -> Self {
        Self { alerts, ..self }
    }

    #[must_use]
    pub fn with_panel_visibility(self, panel_visibility: PanelVisibility) -> Self {
        Self {
            panel_visibility,
            ..self
        }
    }

    #[must_use]
    pub fn when_shown(self) -> Vec<Self> {
        match self.panel_visibility {
            PanelVisibility::Shown => vec![self],
            PanelVisibility::Hidden => Vec::new(),
        }
    }

    #[must_use]
    pub fn in_state(self, state: IndicatorState) -> Self {
        let icon = match self.face.icon {
            IndicatorIcon::IconGlyph { glyph, .. } => IndicatorIcon::IconGlyph { glyph, state },
            unchanged_icon @ (IndicatorIcon::Image(_) | IndicatorIcon::Absent) => unchanged_icon,
        };
        let level = match self.alerts.level {
            LevelNotice::Announced { level, glyph } => LevelNotice::Announced {
                level,
                glyph: glyph.with_weight(state.weight()),
            },
            LevelNotice::Unannounced => LevelNotice::Unannounced,
        };
        Self {
            face: IndicatorFace { icon, ..self.face },
            alerts: IndicatorAlerts {
                level,
                ..self.alerts
            },
            ..self
        }
    }

    #[cfg(test)]
    pub(crate) fn shown_readout(&self) -> Option<&IndicatorReadout> {
        match &self.value {
            RowValueControl::IndicatorReadout(readout) => Some(readout),
            RowValueControl::Slider { .. } | RowValueControl::Drawn(_) => None,
        }
    }
}
