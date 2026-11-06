use std::{path::PathBuf, time::Duration};

use futures::Stream;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::internet_connectivity_gate::{self, OnlineSourceInputs},
    secret_file_reads::read_secret,
    system_protocol_clients::http_json_client,
};

const POLICY_FILE_URL: &str = "https://api.tailscale.com/api/v2/tailnet/-/acl";
pub const ACCESS_CONTROLS_URL: &str = "https://login.tailscale.com/admin/acls";

#[derive(Debug, Clone, thiserror::Error)]
pub enum PolicyFileError {
    #[error("API token unreadable")]
    TokenUnreadable,
    #[error("{0}")]
    Fetch(#[from] http_json_client::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRule {
    pub sources: Vec<String>,
    pub destinations: Vec<String>,
    pub permissions: Vec<String>,
    pub routed_via: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySshRule {
    pub sources: Vec<String>,
    pub destinations: Vec<String>,
    pub login_users: Vec<String>,
    pub action: PolicySshAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySshAction {
    Accept,
    Check,
    #[serde(other)]
    Unrecognized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscalePolicyFile {
    pub grant_rules: Vec<PolicyRule>,
    pub acl_rules: Vec<PolicyRule>,
    pub ssh_rules: Vec<PolicySshRule>,
    pub exit_node_auto_approvers: Vec<String>,
}

#[derive(Deserialize)]
struct PolicyFileJson {
    #[serde(default)]
    acls: Vec<AclRuleJson>,
    #[serde(default)]
    grants: Vec<GrantRuleJson>,
    #[serde(default)]
    ssh: Vec<SshRuleJson>,
    #[serde(rename = "autoApprovers", default)]
    auto_approvers: AutoApproversJson,
}

#[derive(Deserialize, Default)]
struct AutoApproversJson {
    #[serde(rename = "exitNode", default)]
    exit_node: Vec<String>,
}

#[derive(Deserialize)]
struct SshRuleJson {
    #[serde(default)]
    src: Vec<String>,
    #[serde(default)]
    dst: Vec<String>,
    #[serde(default)]
    users: Vec<String>,
    action: PolicySshAction,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum AclProtocolJson {
    Named(String),
    Numbered(u64),
    #[default]
    Unspecified,
}

impl From<AclProtocolJson> for Vec<String> {
    fn from(protocol: AclProtocolJson) -> Self {
        match protocol {
            AclProtocolJson::Named(protocol_name) => vec![protocol_name],
            AclProtocolJson::Numbered(protocol_number) => vec![protocol_number.to_string()],
            AclProtocolJson::Unspecified => Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct AclRuleJson {
    #[serde(default, alias = "users")]
    src: Vec<String>,
    #[serde(default, alias = "ports")]
    dst: Vec<String>,
    #[serde(default)]
    proto: AclProtocolJson,
}

#[derive(Deserialize)]
struct GrantRuleJson {
    #[serde(default)]
    src: Vec<String>,
    #[serde(default)]
    dst: Vec<String>,
    #[serde(default)]
    ip: Vec<String>,
    #[serde(default)]
    app: serde_json::Map<String, Value>,
    #[serde(default)]
    via: Vec<String>,
}

impl From<AclRuleJson> for PolicyRule {
    fn from(acl_rule: AclRuleJson) -> Self {
        Self {
            sources: acl_rule.src,
            destinations: acl_rule.dst,
            permissions: acl_rule.proto.into(),
            routed_via: Vec::new(),
        }
    }
}

impl From<SshRuleJson> for PolicySshRule {
    fn from(ssh_rule: SshRuleJson) -> Self {
        Self {
            sources: ssh_rule.src,
            destinations: ssh_rule.dst,
            login_users: ssh_rule.users,
            action: ssh_rule.action,
        }
    }
}

impl From<GrantRuleJson> for PolicyRule {
    fn from(grant_rule: GrantRuleJson) -> Self {
        Self {
            sources: grant_rule.src,
            destinations: grant_rule.dst,
            permissions: grant_rule
                .ip
                .into_iter()
                .chain(grant_rule.app.into_iter().map(|(capability, _)| capability))
                .collect(),
            routed_via: grant_rule.via,
        }
    }
}

impl From<PolicyFileJson> for TailscalePolicyFile {
    fn from(policy_file: PolicyFileJson) -> Self {
        Self {
            grant_rules: policy_file
                .grants
                .into_iter()
                .map(PolicyRule::from)
                .collect(),
            acl_rules: policy_file.acls.into_iter().map(PolicyRule::from).collect(),
            ssh_rules: policy_file
                .ssh
                .into_iter()
                .map(PolicySshRule::from)
                .collect(),
            exit_node_auto_approvers: policy_file.auto_approvers.exit_node,
        }
    }
}

impl SnapshotSource for TailscalePolicyFile {
    type Error = PolicyFileError;
    type StreamInputs = OnlineSourceInputs<PathBuf>;

    const NAME: &'static str = "Tailscale policy file";

    fn delivery_stream(
        OnlineSourceInputs {
            buses,
            source_inputs: token_path,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        internet_connectivity_gate::while_online(buses, move || {
            let token_path = token_path.clone();
            PeriodicPolling::every(Duration::from_mins(5))
                .poll_deliveries(move || Self::poll(token_path.clone()))
        })
    }
}

impl TailscalePolicyFile {
    async fn poll(token_path: PathBuf) -> Result<Self, PolicyFileError> {
        let token = read_secret(&token_path).ok_or(PolicyFileError::TokenUnreadable)?;
        let authorization = format!("Bearer {token}");
        let policy_file: PolicyFileJson = http_json_client::get_json(
            POLICY_FILE_URL,
            &[
                ("Authorization", &authorization),
                ("Accept", "application/json"),
            ],
        )
        .await?;
        Ok(policy_file.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY_FIXTURE: &str = r#"{
        "groups": {"group:admins": ["someone@example.com"]},
        "tagOwners": {"tag:home": ["autogroup:admin"]},
        "autoApprovers": {"routes": {"10.0.0.0/8": ["tag:home"]}, "exitNode": ["tag:work"]},
        "tests": [{"src": "tag:work", "accept": ["tag:home:22"]}],
        "acls": [
            {"action": "accept", "src": ["tag:work"], "dst": ["tag:home:22"], "proto": "tcp"},
            {"action": "accept", "users": ["autogroup:member"], "ports": ["autogroup:internet:*"]}
        ],
        "grants": [
            {"src": ["tag:home"], "dst": ["tag:work"], "ip": ["tcp:443"],
             "app": {"example.com/cap/monitoring": [{}]}},
            {"src": ["autogroup:member"], "dst": ["autogroup:internet"], "ip": ["*"],
             "via": ["tag:work"]}
        ],
        "ssh": [{"action": "accept", "src": ["autogroup:member"], "dst": ["autogroup:self"]}]
    }"#;

    fn parsed_policy() -> TailscalePolicyFile {
        let policy_file: PolicyFileJson = serde_json::from_str(POLICY_FIXTURE).expect("policy");
        TailscalePolicyFile::from(policy_file)
    }

    #[test]
    fn acl_rules_keep_source_destination_and_protocol() {
        assert_eq!(
            parsed_policy().acl_rules[0],
            PolicyRule {
                sources: vec!["tag:work".to_owned()],
                destinations: vec!["tag:home:22".to_owned()],
                permissions: vec!["tcp".to_owned()],
                routed_via: Vec::new(),
            }
        );
    }

    #[test]
    fn legacy_acl_field_names_are_read() {
        let legacy_rule = &parsed_policy().acl_rules[1];
        assert_eq!(legacy_rule.sources, ["autogroup:member"]);
        assert_eq!(legacy_rule.destinations, ["autogroup:internet:*"]);
    }

    #[test]
    fn grants_list_network_and_application_permissions() {
        let grant = &parsed_policy().grant_rules[0];
        assert_eq!(grant.permissions, ["tcp:443", "example.com/cap/monitoring"]);
    }

    #[test]
    fn grants_keep_the_nodes_they_route_through() {
        assert_eq!(parsed_policy().grant_rules[1].routed_via, ["tag:work"]);
    }

    #[test]
    fn ssh_rules_keep_their_login_users_and_action() {
        assert_eq!(
            parsed_policy().ssh_rules,
            [PolicySshRule {
                sources: vec!["autogroup:member".to_owned()],
                destinations: vec!["autogroup:self".to_owned()],
                login_users: Vec::new(),
                action: PolicySshAction::Accept,
            }]
        );
    }

    #[test]
    fn exit_node_auto_approvers_are_read_without_route_approvers() {
        assert_eq!(parsed_policy().exit_node_auto_approvers, ["tag:work"]);
    }

    #[test]
    fn an_empty_policy_file_has_no_rules() {
        let policy_file: PolicyFileJson = serde_json::from_str("{}").expect("policy");
        let empty_policy = TailscalePolicyFile::from(policy_file);
        assert!(empty_policy.grant_rules.is_empty());
        assert!(empty_policy.acl_rules.is_empty());
        assert!(empty_policy.ssh_rules.is_empty());
        assert!(empty_policy.exit_node_auto_approvers.is_empty());
    }
}
