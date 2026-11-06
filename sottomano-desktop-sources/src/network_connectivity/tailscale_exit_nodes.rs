use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::{Stream, stream};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::tailscale_status::{
        Error, ExitNodeOffer, NodeLastSeen, TailscaleNodeJson, TailscalePrefsJson,
        TailscaleStatusJson,
    },
    stream_operators::StreamOperators,
    system_protocol_clients::shell_command_output,
};

const RUNNING_BACKEND_STATE: &str = "Running";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitNodeReachability {
    Online,
    OfflineSince(DateTime<Utc>),
    OfflineNeverSeen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitNodeOption {
    pub name: String,
    pub ip: String,
    pub reachability: ExitNodeReachability,
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitNodeChoices {
    pub options: Vec<ExitNodeOption>,
    pub offer: ExitNodeOffer,
    pub lan_access: ExitNodeLanAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitNodeLanAccess {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitNodeControl {
    Switchable(ExitRoutingSwitch),
    BlockedByOffering,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TailscaleExitNodes {
    NotRunning,
    Running(ExitNodeChoices),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitRoutingSwitch {
    UseExitNode { ip: String },
    StopUsingExitNode,
    AllowLanAccess,
    BlockLanAccess,
}

impl ExitNodeOption {
    fn from_peer(peer: &TailscaleNodeJson) -> Option<Self> {
        let ip = peer.tailscale_ips.first()?.clone();
        let reachability = if peer.online {
            ExitNodeReachability::Online
        } else {
            match peer.last_seen {
                NodeLastSeen::At(last_seen) => ExitNodeReachability::OfflineSince(last_seen),
                NodeLastSeen::Never => ExitNodeReachability::OfflineNeverSeen,
            }
        };
        Some(Self {
            name: peer.short_name(),
            ip,
            reachability,
            in_use: peer.carries_exit_traffic,
        })
    }

    #[must_use]
    pub fn is_online(&self) -> bool {
        self.reachability == ExitNodeReachability::Online
    }
}

impl ExitNodeChoices {
    #[must_use]
    pub fn control_for_option(&self, option: &ExitNodeOption) -> ExitNodeControl {
        if option.in_use {
            ExitNodeControl::Switchable(ExitRoutingSwitch::StopUsingExitNode)
        } else if !option.is_online() {
            ExitNodeControl::Offline
        } else if self.offer.is_offered() {
            ExitNodeControl::BlockedByOffering
        } else {
            ExitNodeControl::Switchable(ExitRoutingSwitch::UseExitNode {
                ip: option.ip.clone(),
            })
        }
    }

    #[must_use]
    pub fn lan_access_switch(&self) -> ExitRoutingSwitch {
        match self.lan_access {
            ExitNodeLanAccess::Allowed => ExitRoutingSwitch::BlockLanAccess,
            ExitNodeLanAccess::Blocked => ExitRoutingSwitch::AllowLanAccess,
        }
    }
}

impl SnapshotSource for TailscaleExitNodes {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Tailscale exit nodes";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("tailscale")).available_while(move || {
            PeriodicPolling::every(Duration::from_secs(10)).poll_deliveries(move || Self::poll(()))
        })
    }
}

impl TailscaleExitNodes {
    async fn poll(_key: ()) -> Result<Self, Error> {
        let status_json = shell_command_output::output("tailscale", &["status", "--json"]).await?;
        let status: TailscaleStatusJson =
            serde_json::from_str(&status_json).map_err(|_| Error::Malformed)?;
        if status.backend_state != RUNNING_BACKEND_STATE {
            return Ok(TailscaleExitNodes::NotRunning);
        }
        let prefs_json = shell_command_output::output("tailscale", &["debug", "prefs"]).await?;
        Self::from_status_and_prefs(&status, &prefs_json)
    }

    fn from_status_and_prefs(
        status: &TailscaleStatusJson,
        prefs_json: &str,
    ) -> Result<Self, Error> {
        let prefs: TailscalePrefsJson =
            serde_json::from_str(prefs_json).map_err(|_| Error::Malformed)?;
        let mut options: Vec<ExitNodeOption> = status
            .peer
            .values()
            .filter(|peer| peer.offers_exit_node || peer.carries_exit_traffic)
            .filter_map(ExitNodeOption::from_peer)
            .collect();
        options.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(TailscaleExitNodes::Running(ExitNodeChoices {
            options,
            offer: ExitNodeOffer::from_status_and_prefs(status, &prefs),
            lan_access: if prefs.lan_access_allowed {
                ExitNodeLanAccess::Allowed
            } else {
                ExitNodeLanAccess::Blocked
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS_FIXTURE: &str = r#"{
        "BackendState": "Running",
        "Self": {"DNSName": "ryzen.tail.ts.net.", "TailscaleIPs": ["100.1.1.9"], "ExitNodeOption": true},
        "Peer": {
            "a": {"DNSName": "zbook.tail.ts.net.", "TailscaleIPs": ["100.1.1.1"], "Online": true,
                  "ExitNodeOption": true, "LastSeen": "0001-01-01T00:00:00Z"},
            "b": {"DNSName": "tcm.tail.ts.net.", "TailscaleIPs": ["100.1.1.2"], "Online": false,
                  "ExitNodeOption": true, "LastSeen": "2026-10-21T08:00:00Z"},
            "c": {"DNSName": "phone.tail.ts.net.", "TailscaleIPs": ["100.1.1.3"], "Online": true}
        }
    }"#;
    const ADVERTISING_PREFS: &str =
        r#"{"AdvertiseRoutes": ["0.0.0.0/0", "::/0"], "ExitNodeAllowLANAccess": false}"#;
    const PLAIN_PREFS: &str = r#"{"AdvertiseRoutes": null, "ExitNodeAllowLANAccess": true}"#;

    fn parsed_choices(status_json: &str, prefs_json: &str) -> ExitNodeChoices {
        let status: TailscaleStatusJson = serde_json::from_str(status_json).expect("status");
        match TailscaleExitNodes::from_status_and_prefs(&status, prefs_json) {
            Ok(TailscaleExitNodes::Running(choices)) => choices,
            other => panic!("expected running choices, got {other:?}"),
        }
    }

    fn named_option<'choices>(
        choices: &'choices ExitNodeChoices,
        name: &str,
    ) -> &'choices ExitNodeOption {
        choices
            .options
            .iter()
            .find(|option| option.name == name)
            .expect("listed option")
    }

    #[test]
    fn only_peers_offering_exit_are_listed_sorted_by_name() {
        let choices = parsed_choices(STATUS_FIXTURE, PLAIN_PREFS);
        let names: Vec<_> = choices
            .options
            .iter()
            .map(|option| option.name.as_str())
            .collect();
        assert_eq!(names, ["tcm", "zbook"]);
    }

    #[test]
    fn prefs_and_status_supply_offer_and_lan_access() {
        let advertising = parsed_choices(STATUS_FIXTURE, ADVERTISING_PREFS);
        assert_eq!(advertising.offer, ExitNodeOffer::Approved);
        assert_eq!(advertising.lan_access, ExitNodeLanAccess::Blocked);
        let plain = parsed_choices(STATUS_FIXTURE, PLAIN_PREFS);
        assert_eq!(plain.offer, ExitNodeOffer::NotOffered);
        assert_eq!(plain.lan_access, ExitNodeLanAccess::Allowed);
    }

    #[test]
    fn an_unapproved_advertisement_awaits_approval() {
        let unapproved = STATUS_FIXTURE.replace(
            r#""ExitNodeOption": true},
        "Peer""#,
            r#""ExitNodeOption": false},
        "Peer""#,
        );
        assert_eq!(
            parsed_choices(&unapproved, ADVERTISING_PREFS).offer,
            ExitNodeOffer::AwaitingApproval
        );
    }

    #[test]
    fn online_option_switches_by_ip_and_offline_option_is_marked_offline() {
        let choices = parsed_choices(STATUS_FIXTURE, PLAIN_PREFS);
        assert_eq!(
            choices.control_for_option(named_option(&choices, "zbook")),
            ExitNodeControl::Switchable(ExitRoutingSwitch::UseExitNode {
                ip: "100.1.1.1".to_owned()
            })
        );
        assert_eq!(
            choices.control_for_option(named_option(&choices, "tcm")),
            ExitNodeControl::Offline
        );
    }

    #[test]
    fn advertising_blocks_using_an_exit_node() {
        let choices = parsed_choices(STATUS_FIXTURE, ADVERTISING_PREFS);
        assert_eq!(
            choices.control_for_option(named_option(&choices, "zbook")),
            ExitNodeControl::BlockedByOffering
        );
    }

    #[test]
    fn the_exit_node_in_use_can_always_be_stopped() {
        let status_in_use = STATUS_FIXTURE.replace(
            r#""LastSeen": "0001-01-01T00:00:00Z""#,
            r#""ExitNode": true"#,
        );
        let choices = parsed_choices(&status_in_use, ADVERTISING_PREFS);
        assert_eq!(
            choices.control_for_option(named_option(&choices, "zbook")),
            ExitNodeControl::Switchable(ExitRoutingSwitch::StopUsingExitNode)
        );
    }

    #[test]
    fn offline_reachability_keeps_only_real_last_seen_times() {
        let choices = parsed_choices(STATUS_FIXTURE, PLAIN_PREFS);
        assert!(matches!(
            named_option(&choices, "tcm").reachability,
            ExitNodeReachability::OfflineSince(_)
        ));
        let never_seen = STATUS_FIXTURE.replace("2026-10-21T08:00:00Z", "0001-01-01T00:00:00Z");
        let choices = parsed_choices(&never_seen, PLAIN_PREFS);
        assert_eq!(
            named_option(&choices, "tcm").reachability,
            ExitNodeReachability::OfflineNeverSeen
        );
    }
}
