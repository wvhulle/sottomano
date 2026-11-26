use sottomano_desktop_sources::systemd_units_journal::{
    SystemdTimestamp, graphical_session_target::GraphicalSessionTarget,
};

use super::{LoginSessionBusRequest, LoginSessionGroup, LoginSessionInteraction, identity_refresh};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct DesktopServicesIndicator;

impl DesktopServicesIndicator {
    pub fn indicator<'indicator>() -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Desktop services",
            Phosphor::ListChecks(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(|_| Self::hover_card()))
    }

    pub fn hover_card() -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Services that need the desktop",
                "The panel, notifications and other services that need a running compositor.",
            ),
            dictionary_table([LabeledValue::new("Started by", "systemd --user")]),
        )]
    }
}

pub fn graphical_session_target_sections(
    graphical_session_target: &GraphicalSessionTarget,
) -> Vec<Section> {
    let mut entries = vec![LabeledValue::new(
        "State",
        graphical_session_target.state.clone(),
    )];
    if let SystemdTimestamp::At(at) = graphical_session_target.reached_at {
        entries.push(LabeledValue::new(
            "Ready since",
            ValueCell::Time {
                at,
                format: LocalizedTimeFormat::DateAndTime,
            },
        ));
    }
    vec![Section::new(
        SectionTitle::captioned(
            "Target state",
            "graphical-session.target is reached once the compositor is ready for services.",
        ),
        dictionary_table(entries),
    )]
}

impl LoginSessionGroup {
    pub(super) fn desktop_services_row(&self) -> GroupIndicator<'_> {
        DesktopServicesIndicator::indicator()
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .identity
                    .hover_card_with(DesktopServicesIndicator::hover_card());
                sections.extend(
                    self.graphical_target.sections_described_by(|snapshot| {
                        graphical_session_target_sections(snapshot)
                    }),
                );
                sections
            }))
            .fetching_on_hover(
                LoginSessionInteraction::OnBuses(LoginSessionBusRequest::StartDesktopServicesFetch),
                LoginSessionInteraction::StopHoverFetches,
            )
            .refreshed_by(identity_refresh())
    }
}
