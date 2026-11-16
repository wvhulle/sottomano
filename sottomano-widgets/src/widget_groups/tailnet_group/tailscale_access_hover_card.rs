use std::collections::{BTreeMap, BTreeSet};

use iced::alignment::Horizontal;
use sottomano_desktop_sources::network_connectivity::{
    tailscale_inbound_access::{
        InboundAccessRule, InboundDestination, InboundPorts, TailscaleInboundAccess,
    },
    tailscale_policy_file::{
        ACCESS_CONTROLS_URL, PolicyRule, PolicySshAction, PolicySshRule, TailscalePolicyFile,
    },
    tailscale_status::{Tailnet, Tailscale},
};

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const NO_TAGS_TEXT: &str = "none";

#[derive(Clone, Copy)]
pub(super) struct OwnNodeRow<'a> {
    tailnet: &'a Tailnet,
}

impl<'a> OwnNodeRow<'a> {
    pub(super) fn running_own_node(tailscale: &'a Tailscale) -> Option<Self> {
        match tailscale {
            Tailscale::Running(tailnet) => Some(Self { tailnet }),
            Tailscale::Stopped | Tailscale::Starting => None,
        }
    }

    fn own_tags_text(self) -> String {
        match self.tailnet.own_node.tags.as_slice() {
            [] => NO_TAGS_TEXT.to_owned(),
            tags => tags.join(", "),
        }
    }
}

impl<'a> OwnNodeRow<'a> {
    pub(super) fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(self.own_tags_text()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "This machine",
                Phosphor::IdentificationBadge(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(self) -> Vec<Section> {
        vec![
            Section::new(
                "Identity",
                dictionary_table([
                    LabeledValue::new("Machine", self.tailnet.own_node.name.as_str()),
                    LabeledValue::new("Tags", self.own_tags_text()),
                ]),
            ),
            Section::new(
                "Capabilities",
                SectionWidget::paragraph(self.tailnet.own_capabilities.join(", ")),
            ),
        ]
    }
}

pub(super) struct InboundAccessRow;

impl InboundAccessRow {
    pub(super) fn reading_row() -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            "Reachable from",
            Phosphor::ArrowsInSimple(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(|_| Vec::new()),
        )
    }
}

pub(super) struct PolicyFileRow;

impl PolicyFileRow {
    pub(super) fn reading_row() -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            "Policy file",
            Phosphor::Scroll(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(|_| Vec::new()),
        )
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

pub fn tailscale_inbound_access_sections(
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

impl From<&PolicyRule> for TableRecord<3> {
    fn from(policy_rule: &PolicyRule) -> Self {
        TableRecord::with_cells(
            policy_rule.sources.join(", "),
            [
                policy_rule.destinations.join(", ").into(),
                policy_rule.permissions.join(", ").into(),
                policy_rule.routed_via.join(", ").into(),
            ],
        )
    }
}

impl From<&PolicySshRule> for TableRecord<3> {
    fn from(policy_ssh_rule: &PolicySshRule) -> Self {
        TableRecord::with_cells(
            policy_ssh_rule.sources.join(", "),
            [
                policy_ssh_rule.destinations.join(", ").into(),
                policy_ssh_rule.login_users.join(", ").into(),
                match policy_ssh_rule.action {
                    PolicySshAction::Accept => "accept",
                    PolicySshAction::Check => "check",
                    PolicySshAction::Unrecognized => "unrecognized",
                }
                .into(),
            ],
        )
    }
}

fn policy_rule_section(title: &'static str, rules: &[PolicyRule]) -> Option<Section> {
    (!rules.is_empty()).then(|| {
        Section::new(
            title,
            headed_table(
                [
                    ColumnHeading::titled("Destination").align_x(Horizontal::Left),
                    ColumnHeading::titled("Permits").align_x(Horizontal::Left),
                    ColumnHeading::titled("Via").align_x(Horizontal::Left),
                ],
                rules,
            )
            .without_blank_columns(ValueCell::is_blank),
        )
    })
}

fn policy_ssh_section(ssh_rules: &[PolicySshRule]) -> Option<Section> {
    (!ssh_rules.is_empty()).then(|| {
        Section::new(
            "SSH",
            headed_table(
                [
                    ColumnHeading::titled("Destination").align_x(Horizontal::Left),
                    ColumnHeading::titled("Users").align_x(Horizontal::Left),
                    ColumnHeading::titled("Action").align_x(Horizontal::Left),
                ],
                ssh_rules,
            ),
        )
    })
}

pub fn tailscale_policy_file_sections(tailscale_policy_file: &TailscalePolicyFile) -> Vec<Section> {
    let exit_node_approvers_entry = (!tailscale_policy_file.exit_node_auto_approvers.is_empty())
        .then(|| {
            LabeledValue::new(
                "Exit nodes auto-approved for",
                tailscale_policy_file.exit_node_auto_approvers.join(", "),
            )
        });
    let summary_section = Section::new(
        "Administration",
        dictionary_table(
            exit_node_approvers_entry
                .into_iter()
                .chain([LabeledValue::new(
                    "Edit",
                    ValueCell::BrandedLink {
                        label: "Access controls".to_owned(),
                        url: ACCESS_CONTROLS_URL.to_owned(),
                        glyph: SimpleIcon::Tailscale.into(),
                    },
                )]),
        ),
    );
    [
        policy_rule_section("Grants", &tailscale_policy_file.grant_rules),
        policy_rule_section("ACLs", &tailscale_policy_file.acl_rules),
        policy_ssh_section(&tailscale_policy_file.ssh_rules),
        Some(summary_section),
    ]
    .into_iter()
    .flatten()
    .collect()
}
