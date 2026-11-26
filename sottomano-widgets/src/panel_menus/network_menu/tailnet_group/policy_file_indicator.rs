use iced::alignment::Horizontal;
use sottomano_desktop_sources::network_connectivity::tailscale_policy_file::{
    ACCESS_CONTROLS_URL, PolicyRule, PolicySshAction, PolicySshRule, TailscalePolicyFile,
};

use super::{TailnetBusRequest, TailnetGroup, TailnetInteraction};
use crate::{
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        value_cells::{LabeledValue, ValueCell},
    },
};
pub(super) struct PolicyFileIndicator;

impl PolicyFileIndicator {
    pub(super) fn indicator<'indicator>() -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Policy file",
            Phosphor::Scroll(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(|_| Vec::new()))
    }
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

pub(super) fn tailscale_policy_file_sections(
    tailscale_policy_file: &TailscalePolicyFile,
) -> Vec<Section> {
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

impl TailnetGroup {
    pub(super) fn policy_file_row(&self) -> GroupIndicator<'_> {
        PolicyFileIndicator::indicator()
            .with_hover_card(Box::new(move |_| self.policy_file_access.sections()))
            .fetching_on_hover(
                TailnetInteraction::OnBuses(TailnetBusRequest::StartPolicyFileFetch),
                TailnetInteraction::StopHoverFetches,
            )
    }
}
