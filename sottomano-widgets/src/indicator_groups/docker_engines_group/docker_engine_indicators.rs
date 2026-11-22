use sottomano_desktop_sources::{
    docker_container_engine::{DockerContainerListing, DockerEngineSocket},
    podman_container_engine::libpod_running_containers::EngineReadError,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    indicator_groups::engine_container_indicators::{engine_container_name, engine_failure_label},
    widgets::value_cells::LabeledValue,
};

pub(super) struct UnreadableEngineIndicator<'view> {
    pub(super) engine: &'view DockerEngineSocket,
    pub(super) read_failure: &'view EngineReadError,
}

impl<'view> UnreadableEngineIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.engine.owner.to_string(),
            Phosphor::Package(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Error",
                SectionWidget::paragraph(self.read_failure.to_string()),
            ),
            Section::new(
                "Connection",
                dictionary_table([LabeledValue::new(
                    "Socket",
                    self.engine.socket_path.display().to_string(),
                )]),
            ),
        ]
    }
}

pub(super) struct ListedContainerIndicator<'view> {
    pub(super) engine: &'view DockerEngineSocket,
    pub(super) listed: &'view DockerContainerListing,
}

impl<'view> ListedContainerIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            engine_container_name(&self.listed.container_name, &self.engine.owner),
            Phosphor::Package(Weight::Regular).into(),
            self.listed.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.listed.run_state.label().to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("State", self.listed.run_state.label()),
                LabeledValue::new("Engine", self.engine.owner.to_string()),
            ]),
        )]
    }
}
