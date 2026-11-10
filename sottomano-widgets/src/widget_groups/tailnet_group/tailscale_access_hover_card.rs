use std::collections::{BTreeMap, BTreeSet};

use chrono::Locale;
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
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const NO_TAGS_TEXT: &str = "none";

pub(super) struct OwnNodeRow<'a> {
    tailnet: &'a Tailnet,
}

impl<'a> OwnNodeRow<'a> {
    pub(super) fn of(tailscale: &'a Tailscale) -> Option<Self> {
        match tailscale {
            Tailscale::Running(tailnet) => Some(Self { tailnet }),
            Tailscale::Stopped | Tailscale::Starting => None,
        }
    }

    fn own_tags_text(&self) -> String {
        match self.tailnet.own_node.tags.as_slice() {
            [] => NO_TAGS_TEXT.to_owned(),
            tags => tags.join(", "),
        }
    }
}

impl GroupItemReading for OwnNodeRow<'_> {
    fn name(&self) -> &'static str {
        "This machine"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::IdentificationBadge(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.own_tags_text())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Idle
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([
            Section::dictionary([
                LabeledValue::new("Machine", self.tailnet.own_node.name.as_str()),
                LabeledValue::new("Tags", self.own_tags_text()),
            ]),
            Section::detailed_note("Capabilities", self.tailnet.own_capabilities.join(", ")),
        ])
    }
}

pub(super) struct InboundAccessRow;

impl GroupItemReading for InboundAccessRow {
    fn name(&self) -> &'static str {
        "Reachable from"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowsInSimple(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub(super) struct PolicyFileRow;

impl GroupItemReading for PolicyFileRow {
    fn name(&self) -> &'static str {
        "Policy file"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Scroll(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
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

impl IntoTableRecord<1> for GrantedAccessRow<'_> {
    fn into_table_record(self) -> TableRecord<1> {
        TableRecord::new(self.row_name, [self.access.ports_text().into()])
    }
}

impl IntoTableRecord<2> for GrantedAccessRow<'_> {
    fn into_table_record(self) -> TableRecord<2> {
        TableRecord::new(
            self.row_name,
            [
                self.access.ports_text().into(),
                self.access.sources_text().into(),
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
        Section::column_table(
            [ColumnHeading::titled("Ports").align_x(Horizontal::Left)],
            granted_access_rows(&access_by_source),
        )
        .titled("This machine")
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
        Section::column_table(
            [
                ColumnHeading::titled("Ports").align_x(Horizontal::Left),
                ColumnHeading::titled("From").align_x(Horizontal::Left),
            ],
            granted_access_rows(&access_by_destination),
        )
        .titled(title)
    })
}

fn exit_node_section(rules: &[InboundAccessRule]) -> Option<Section> {
    let exit_node_users: BTreeSet<&str> = rules
        .iter()
        .filter(|rule| rule.destination == InboundDestination::ExitNodeInternet)
        .map(|rule| rule.source_name.as_str())
        .collect();
    (!exit_node_users.is_empty()).then(|| {
        Section::note(exit_node_users.into_iter().collect::<Vec<_>>().join(", "))
            .titled("Exit node used by")
    })
}

impl ReadingSections for TailscaleInboundAccess {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        [
            this_machine_section(&self.rules),
            destination_section(
                "Services hosted here",
                &self.rules,
                |destination| match destination {
                    InboundDestination::HostedService { service_name } => Some(service_name),
                    InboundDestination::ThisMachine
                    | InboundDestination::SubnetRoute { .. }
                    | InboundDestination::ExitNodeInternet => None,
                },
            ),
            destination_section(
                "Subnet routes",
                &self.rules,
                |destination| match destination {
                    InboundDestination::SubnetRoute { network } => Some(network),
                    InboundDestination::ThisMachine
                    | InboundDestination::HostedService { .. }
                    | InboundDestination::ExitNodeInternet => None,
                },
            ),
            exit_node_section(&self.rules),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl IntoTableRecord<3> for &PolicyRule {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(
            self.sources.join(", "),
            [
                self.destinations.join(", ").into(),
                self.permissions.join(", ").into(),
                self.routed_via.join(", ").into(),
            ],
        )
    }
}

impl IntoTableRecord<3> for &PolicySshRule {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(
            self.sources.join(", "),
            [
                self.destinations.join(", ").into(),
                self.login_users.join(", ").into(),
                match self.action {
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
        Section::column_table(
            [
                ColumnHeading::titled("Destination").align_x(Horizontal::Left),
                ColumnHeading::titled("Permits").align_x(Horizontal::Left),
                ColumnHeading::titled("Via").align_x(Horizontal::Left),
            ],
            rules,
        )
        .without_blank_columns()
        .titled(title)
    })
}

fn policy_ssh_section(ssh_rules: &[PolicySshRule]) -> Option<Section> {
    (!ssh_rules.is_empty()).then(|| {
        Section::column_table(
            [
                ColumnHeading::titled("Destination").align_x(Horizontal::Left),
                ColumnHeading::titled("Users").align_x(Horizontal::Left),
                ColumnHeading::titled("Action").align_x(Horizontal::Left),
            ],
            ssh_rules,
        )
        .titled("SSH")
    })
}

impl ReadingSections for TailscalePolicyFile {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let exit_node_approvers_entry = (!self.exit_node_auto_approvers.is_empty()).then(|| {
            LabeledValue::new(
                "Exit nodes auto-approved for",
                self.exit_node_auto_approvers.join(", "),
            )
        });
        let summary_section =
            Section::dictionary(
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
            );
        [
            policy_rule_section("Grants", &self.grant_rules),
            policy_rule_section("ACLs", &self.acl_rules),
            policy_ssh_section(&self.ssh_rules),
            Some(summary_section),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbound_rule(
        source_name: &str,
        destination: InboundDestination,
        ports: InboundPorts,
    ) -> InboundAccessRule {
        InboundAccessRule {
            source_name: source_name.to_owned(),
            destination,
            ports,
        }
    }

    fn nextcloud() -> InboundDestination {
        InboundDestination::HostedService {
            service_name: "nextcloud".to_owned(),
        }
    }

    fn section_titles(access: &TailscaleInboundAccess) -> Vec<String> {
        access
            .sections(Clock::real())
            .iter()
            .filter_map(Section::title)
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn inbound_rules_split_by_destination() {
        let access = TailscaleInboundAccess {
            rules: vec![
                inbound_rule("zbook", InboundDestination::ThisMachine, InboundPorts::All),
                inbound_rule("x1", nextcloud(), InboundPorts::Single(443)),
                inbound_rule(
                    "x1",
                    InboundDestination::SubnetRoute {
                        network: "192.168.1.0/24".to_owned(),
                    },
                    InboundPorts::All,
                ),
                inbound_rule(
                    "iphone",
                    InboundDestination::ExitNodeInternet,
                    InboundPorts::All,
                ),
            ],
        };
        assert_eq!(
            section_titles(&access),
            [
                "This machine",
                "Services hosted here",
                "Subnet routes",
                "Exit node used by"
            ]
        );
    }

    #[test]
    fn an_unused_destination_has_no_section() {
        let access = TailscaleInboundAccess {
            rules: vec![inbound_rule(
                "zbook",
                InboundDestination::ThisMachine,
                InboundPorts::All,
            )],
        };
        assert_eq!(section_titles(&access), ["This machine"]);
    }

    #[test]
    fn one_service_row_merges_ports_and_sources() {
        let rules = [
            inbound_rule("zbook", nextcloud(), InboundPorts::Single(587)),
            inbound_rule("x1", nextcloud(), InboundPorts::Single(143)),
            inbound_rule("zbook", nextcloud(), InboundPorts::Single(143)),
        ];
        let access_by_service =
            access_grouped_by(&rules, |rule| Some(format!("{:?}", rule.destination)));
        let merged_access: Vec<&GrantedAccess> = access_by_service.values().collect();
        assert_eq!(merged_access.len(), 1);
        assert_eq!(merged_access[0].ports_text(), "143, 587");
        assert_eq!(merged_access[0].sources_text(), "x1, zbook");
    }
}
