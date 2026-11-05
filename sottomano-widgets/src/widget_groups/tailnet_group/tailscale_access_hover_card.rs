use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_sources::network_connectivity::{
    tailscale_inbound_access::{
        InboundAccessRule, InboundPorts, InboundReach, TailscaleInboundAccess,
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

impl IntoTableRecord<1> for &InboundAccessRule {
    fn into_table_record(self) -> TableRecord<1> {
        TableRecord::new(
            self.source_name.as_str(),
            [inbound_ports_text(&self.ports).into()],
        )
    }
}

fn inbound_reach_section(
    title: &'static str,
    reach: InboundReach,
    rules: &[InboundAccessRule],
) -> Option<Section> {
    let matching_rules: Vec<&InboundAccessRule> =
        rules.iter().filter(|rule| rule.reach == reach).collect();
    (!matching_rules.is_empty()).then(|| {
        Section::column_table(
            [ColumnHeading::titled("Ports").align_x(Horizontal::Left)],
            matching_rules,
        )
        .titled(title)
    })
}

impl ReadingSections for TailscaleInboundAccess {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        [
            inbound_reach_section("This machine", InboundReach::ThisMachine, &self.rules),
            inbound_reach_section(
                "Routes through this machine",
                InboundReach::RoutesThroughThisMachine,
                &self.rules,
            ),
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

    fn inbound_rule(source_name: &str, reach: InboundReach) -> InboundAccessRule {
        InboundAccessRule {
            source_name: source_name.to_owned(),
            reach,
            ports: InboundPorts::All,
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
    fn inbound_rules_split_by_what_they_reach() {
        let access = TailscaleInboundAccess {
            rules: vec![
                inbound_rule("zbook", InboundReach::ThisMachine),
                inbound_rule("iphone", InboundReach::RoutesThroughThisMachine),
            ],
        };
        assert_eq!(
            section_titles(&access),
            ["This machine", "Routes through this machine"]
        );
    }

    #[test]
    fn an_unused_reach_has_no_section() {
        let access = TailscaleInboundAccess {
            rules: vec![inbound_rule("zbook", InboundReach::ThisMachine)],
        };
        assert_eq!(section_titles(&access), ["This machine"]);
    }
}
