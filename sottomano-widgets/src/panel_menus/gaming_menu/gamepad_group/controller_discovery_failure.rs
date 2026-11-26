use sottomano_desktop_sources::delivery_cadence_drivers::ChangeWatchFailure;

use super::{GamepadGroup, GamepadInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
};

const CONTROLLER_DISCOVERY_GROUP: &str = "controller_discovery";

#[derive(Debug, Default)]
pub(super) enum ControllerDiscovery {
    #[default]
    Watching,
    Failed(ChangeWatchFailure),
}

struct ControllerDiscoveryFailureIndicator<'failure> {
    discovery_failure: &'failure ChangeWatchFailure,
}

impl<'failure> ControllerDiscoveryFailureIndicator<'failure> {
    fn indicator(self) -> GroupIndicator<'failure> {
        GroupIndicator::glyph_only(
            "Controller discovery",
            Phosphor::GameController(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("failed".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Error",
                SectionWidget::paragraph(self.discovery_failure.to_string()),
            ),
            Section::new(
                "Hotplug",
                SectionWidget::paragraph(
                    "New or removed controllers are not noticed until the panel restarts.",
                ),
            ),
        ]
    }
}

impl GamepadGroup {
    fn controller_discovery_failure_indicator(
        &self,
    ) -> Option<ControllerDiscoveryFailureIndicator<'_>> {
        match &self.controller_discovery {
            ControllerDiscovery::Failed(discovery_failure) => {
                Some(ControllerDiscoveryFailureIndicator { discovery_failure })
            }
            ControllerDiscovery::Watching => None,
        }
    }

    pub(super) fn controller_discovery_group(&self) -> Option<IndicatorGroupContent<'_>> {
        let listed_failure = self.controller_discovery_failure_indicator()?.indicator();
        let group_title = IndicatorGroupTitle::Named(listed_failure.indicator_name());
        let headline_indicator = self
            .controller_discovery_failure_indicator()?
            .indicator()
            .with_headline_glyph(Phosphor::Joystick(Weight::Regular).into());
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::Gamepad,
                CONTROLLER_DISCOVERY_GROUP,
            ),
            group_title,
            headline_indicator,
            &self.controller_discovery_expansion,
            GamepadInteraction::ControllerDiscoveryExpansion,
            IndicatorListing::Listed(Vec::from([listed_failure])),
        ))
    }
}
