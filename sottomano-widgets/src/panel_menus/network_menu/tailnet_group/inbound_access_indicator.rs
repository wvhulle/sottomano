use std::collections::{BTreeMap, BTreeSet};

use iced::alignment::Horizontal;
use sottomano_desktop_sources::network_connectivity::tailscale_inbound_access::{
    InboundAccessRule, InboundDestination, InboundPorts, TailscaleInboundAccess,
};

use super::{TailnetGroup, TailnetInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, SectionWidget, headed_table},
    widgets::keyed_record_table::{ColumnHeading, TableRecord},
};

pub(super) struct InboundAccessIndicator;

impl InboundAccessIndicator {
    pub(super) fn indicator<'indicator>() -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Reachable from",
            Phosphor::ArrowsInSimple(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(|_| Vec::new()))
    }
}

fn inbound_ports_text(ports: &InboundPorts) -> String {
    match ports {
        InboundPorts::All => "all".to_owned(),
        InboundPorts::Single(port) => port.to_string(),
        InboundPorts::Range { first, last } => format!("{first}-{last}"),
    }
}

#[derive(Default)]
struct GrantedAccess {
    ports: BTreeSet<InboundPorts>,
    sources: BTreeSet<String>,
}

impl GrantedAccess {
    fn ports_text(&self) -> String {
        if self.ports.contains(&InboundPorts::All) {
            return inbound_ports_text(&InboundPorts::All);
        }
        self.ports
            .iter()
            .map(inbound_ports_text)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn sources_text(&self) -> String {
        self.sources.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

struct GrantedAccessRow<'access> {
    row_name: &'access str,
    access: &'access GrantedAccess,
}

impl From<GrantedAccessRow<'_>> for TableRecord<1> {
    fn from(granted_access_row: GrantedAccessRow<'_>) -> Self {
        TableRecord::with_cells(
            granted_access_row.row_name,
            [granted_access_row.access.ports_text().into()],
        )
    }
}

impl From<GrantedAccessRow<'_>> for TableRecord<2> {
    fn from(granted_access_row: GrantedAccessRow<'_>) -> Self {
        TableRecord::with_cells(
            granted_access_row.row_name,
            [
                granted_access_row.access.ports_text().into(),
                granted_access_row.access.sources_text().into(),
            ],
        )
    }
}

fn access_grouped_by(
    rules: &[InboundAccessRule],
    row_name_of: impl Fn(&InboundAccessRule) -> Option<String>,
) -> BTreeMap<String, GrantedAccess> {
    rules
        .iter()
        .filter_map(|rule| row_name_of(rule).map(|row_name| (row_name, rule)))
        .fold(BTreeMap::new(), |mut grouped_access, (row_name, rule)| {
            let access: &mut GrantedAccess = grouped_access.entry(row_name).or_default();
            access.ports.insert(rule.ports.clone());
            access.sources.insert(rule.source_name.clone());
            grouped_access
        })
}

fn granted_access_rows(
    grouped_access: &BTreeMap<String, GrantedAccess>,
) -> impl Iterator<Item = GrantedAccessRow<'_>> {
    grouped_access
        .iter()
        .map(|(row_name, access)| GrantedAccessRow { row_name, access })
}

fn this_machine_section(rules: &[InboundAccessRule]) -> Option<Section> {
    let access_by_source = access_grouped_by(rules, |rule| {
        (rule.destination == InboundDestination::ThisMachine).then(|| rule.source_name.clone())
    });
    (!access_by_source.is_empty()).then(|| {
        Section::new(
            "This machine",
            headed_table(
                [ColumnHeading::titled("Ports").align_x(Horizontal::Left)],
                granted_access_rows(&access_by_source),
            ),
        )
    })
}

fn destination_section(
    title: &'static str,
    rules: &[InboundAccessRule],
    destination_name_of: impl Fn(&InboundDestination) -> Option<&String>,
) -> Option<Section> {
    let access_by_destination = access_grouped_by(rules, |rule| {
        destination_name_of(&rule.destination).cloned()
    });
    (!access_by_destination.is_empty()).then(|| {
        Section::new(
            title,
            headed_table(
                [
                    ColumnHeading::titled("Ports").align_x(Horizontal::Left),
                    ColumnHeading::titled("From").align_x(Horizontal::Left),
                ],
                granted_access_rows(&access_by_destination),
            ),
        )
    })
}

fn exit_node_section(rules: &[InboundAccessRule]) -> Option<Section> {
    let exit_node_users: BTreeSet<&str> = rules
        .iter()
        .filter(|rule| rule.destination == InboundDestination::ExitNodeInternet)
        .map(|rule| rule.source_name.as_str())
        .collect();
    (!exit_node_users.is_empty()).then(|| {
        Section::new(
            "Exit node used by",
            SectionWidget::paragraph(exit_node_users.into_iter().collect::<Vec<_>>().join(", ")),
        )
    })
}

fn tailscale_inbound_access_sections(
    tailscale_inbound_access: &TailscaleInboundAccess,
) -> Vec<Section> {
    [
        this_machine_section(&tailscale_inbound_access.rules),
        destination_section(
            "Services hosted here",
            &tailscale_inbound_access.rules,
            |destination| match destination {
                InboundDestination::HostedService { service_name } => Some(service_name),
                InboundDestination::ThisMachine
                | InboundDestination::SubnetRoute { .. }
                | InboundDestination::ExitNodeInternet => None,
            },
        ),
        destination_section(
            "Subnet routes",
            &tailscale_inbound_access.rules,
            |destination| match destination {
                InboundDestination::SubnetRoute { network } => Some(network),
                InboundDestination::ThisMachine
                | InboundDestination::HostedService { .. }
                | InboundDestination::ExitNodeInternet => None,
            },
        ),
        exit_node_section(&tailscale_inbound_access.rules),
    ]
    .into_iter()
    .flatten()
    .collect()
}

impl TailnetGroup {
    pub(super) fn inbound_access_row(&self) -> GroupIndicator<'_> {
        InboundAccessIndicator::indicator()
            .with_hover_card(Box::new(move |_| {
                self.inbound_access
                    .sections_described_by(tailscale_inbound_access_sections)
            }))
            .fetching_on_hover(
                TailnetInteraction::StartInboundAccessFetch,
                TailnetInteraction::StopHoverFetches,
            )
    }
}
