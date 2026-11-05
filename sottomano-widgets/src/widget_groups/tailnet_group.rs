use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        internet_connectivity_gate::OnlineSourceInputs,
        tailscale_inbound_access::TailscaleInboundAccess,
        tailscale_policy_file::{PolicyFileError, TailscalePolicyFile},
        tailscale_status::{
            self, ExitOfferControl, ExitOfferSwitch, Tailnet, Tailscale, TailscaleSwitch,
            open_admin, switch_exit_offer, switch_tailscale,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            Section,
            hover_card_source_delivery::{HoverCard, SourcedSections},
        },
    },
    widget_groups::{
        tailnet_group::{
            tailscale_access_hover_card::{InboundAccessRow, OwnNodeRow, PolicyFileRow},
            tailscale_exit_offer_hover_card::ExitOfferRow,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, latest_snapshot::LatestSnapshot,
        simple_icons::SimpleIcon,
    },
};

mod tailscale_access_hover_card;
mod tailscale_exit_offer_hover_card;
mod tailscale_mesh_hover_card;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailnetTarget {
    Membership,
    OwnNode,
    InboundAccess,
    PolicyFile,
    ExitOffer,
}

impl From<TailnetTarget> for HoverTarget {
    fn from(target: TailnetTarget) -> Self {
        HoverTarget::Tailnet(target)
    }
}

impl From<tailscale_status::Error> for Section {
    fn from(error: tailscale_status::Error) -> Self {
        Section::error(error)
    }
}

impl From<PolicyFileError> for Section {
    fn from(error: PolicyFileError) -> Self {
        Section::error(error)
    }
}

enum PolicyFileAccess {
    NoApiToken,
    ApiToken {
        token_path: PathBuf,
        policy_file: Box<HoverFetchedSnapshot<TailscalePolicyFile>>,
    },
}

impl PolicyFileAccess {
    fn stop_hover_fetch(&mut self) {
        if let PolicyFileAccess::ApiToken { policy_file, .. } = self {
            policy_file.stop();
        }
    }

    fn receive(&mut self, delivery: SnapshotDelivery<TailscalePolicyFile>, clock: Clock) {
        if let PolicyFileAccess::ApiToken { policy_file, .. } = self {
            policy_file.receive(delivery, clock);
        }
    }

    fn sections(&self, clock: Clock) -> Option<SourcedSections> {
        match self {
            PolicyFileAccess::NoApiToken => None,
            PolicyFileAccess::ApiToken { policy_file, .. } => policy_file.sections(clock),
        }
    }
}

pub struct TailnetGroup {
    tailscale: LatestSnapshot<Tailscale>,
    inbound_access: HoverFetchedSnapshot<TailscaleInboundAccess>,
    policy_file_access: PolicyFileAccess,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum TailnetMessage {
    Tailscale(SnapshotDelivery<Tailscale>),
    InboundAccessFetched(SnapshotDelivery<TailscaleInboundAccess>),
    PolicyFileFetched(SnapshotDelivery<TailscalePolicyFile>),
    Expansion(WidgetGroupExpansionMessage),
    SwitchTailscale(TailscaleSwitch),
    SwitchExitOffer(ExitOfferSwitch),
    OpenAdminConsole,
}

impl TailnetGroup {
    #[must_use]
    pub fn without_policy_file() -> Self {
        Self::with_access(PolicyFileAccess::NoApiToken)
    }

    #[must_use]
    pub fn with_policy_file(token_path: PathBuf) -> Self {
        Self::with_access(PolicyFileAccess::ApiToken {
            token_path,
            policy_file: Box::default(),
        })
    }

    fn with_access(policy_file_access: PolicyFileAccess) -> Self {
        Self {
            tailscale: LatestSnapshot::default(),
            inbound_access: HoverFetchedSnapshot::default(),
            policy_file_access,
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: &GroupSourceDemand) -> Subscription<TailnetMessage> {
        demand
            .while_face_shown(snapshot_subscription::<Tailscale>(()).map(TailnetMessage::Tailscale))
    }

    pub fn refresh() -> Task<TailnetMessage> {
        first_delivery::<Tailscale>(()).map(TailnetMessage::Tailscale)
    }

    pub fn update(&mut self, message: TailnetMessage, clock: Clock) -> Task<TailnetMessage> {
        use sottomano_desktop_sources::blocking_background_work::blocking;
        match message {
            TailnetMessage::Tailscale(delivery) => self.tailscale.receive(delivery, clock),
            TailnetMessage::InboundAccessFetched(delivery) => {
                self.inbound_access.receive(delivery, clock);
            }
            TailnetMessage::PolicyFileFetched(delivery) => {
                self.policy_file_access.receive(delivery, clock);
            }
            TailnetMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            TailnetMessage::SwitchTailscale(tailscale_switch) => {
                return Task::future(switch_tailscale(tailscale_switch)).then(|()| Self::refresh());
            }
            TailnetMessage::SwitchExitOffer(exit_offer_switch) => {
                return Task::future(switch_exit_offer(exit_offer_switch))
                    .then(|()| Self::refresh());
            }
            TailnetMessage::OpenAdminConsole => {
                return Task::future(blocking("tailscale admin", open_admin)).discard();
            }
        }
        Task::none()
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: Option<&DesktopBuses>,
        clock: Clock,
    ) -> Task<TailnetMessage> {
        self.inbound_access.stop();
        self.policy_file_access.stop_hover_fetch();
        match hovered {
            Some(HoverTarget::Tailnet(TailnetTarget::InboundAccess)) => self
                .inbound_access
                .start_hover_fetch((), TailnetMessage::InboundAccessFetched, config, clock),
            Some(HoverTarget::Tailnet(TailnetTarget::PolicyFile)) => {
                let (
                    PolicyFileAccess::ApiToken {
                        token_path,
                        policy_file,
                    },
                    Some(buses),
                ) = (&mut self.policy_file_access, buses)
                else {
                    return Task::none();
                };
                policy_file.start_hover_fetch(
                    OnlineSourceInputs {
                        buses: buses.clone(),
                        source_inputs: token_path.clone(),
                    },
                    TailnetMessage::PolicyFileFetched,
                    config,
                    clock,
                )
            }
            _ => Task::none(),
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: TailnetTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            TailnetTarget::Membership => self.tailscale.hover_card(clock),
            TailnetTarget::ExitOffer => {
                let tailnet = self.running_tailnet()?;
                self.tailscale
                    .hover_card_with(ExitOfferRow { tailnet }.hover_card(clock))
            }
            TailnetTarget::OwnNode => {
                let own_node_row = OwnNodeRow::of(self.tailscale.get()?)?;
                self.tailscale
                    .hover_card_with(own_node_row.hover_card(clock))
            }
            TailnetTarget::InboundAccess => {
                Some(HoverCard::grouped(self.inbound_access.sections(clock)))
            }
            TailnetTarget::PolicyFile => {
                Some(HoverCard::grouped(self.policy_file_access.sections(clock)))
            }
        }
    }

    fn membership_item(&self) -> Option<GroupItem<'_, TailnetMessage>> {
        let item = GroupItem::reading(self.tailscale.display()?, TailnetTarget::Membership)
            .on_right_press(
                "open the Tailscale admin console",
                TailnetMessage::OpenAdminConsole,
            );
        Some(match self.tailscale.get().map(Tailscale::switch) {
            Some(TailscaleSwitch::Up) => item.on_switch_press(
                false,
                "turn Tailscale on",
                TailnetMessage::SwitchTailscale(TailscaleSwitch::Up),
            ),
            Some(TailscaleSwitch::Down) => item.on_switch_press(
                true,
                "turn Tailscale off",
                TailnetMessage::SwitchTailscale(TailscaleSwitch::Down),
            ),
            None => item,
        })
    }

    fn running_tailnet(&self) -> Option<&Tailnet> {
        match self.tailscale.get()? {
            Tailscale::Running(tailnet) => Some(tailnet),
            Tailscale::Stopped | Tailscale::Starting => None,
        }
    }

    fn policy_file_item(&self) -> Option<GroupItem<'_, TailnetMessage>> {
        self.running_tailnet()?;
        match self.policy_file_access {
            PolicyFileAccess::NoApiToken => None,
            PolicyFileAccess::ApiToken { .. } => {
                Some(GroupItem::reading(PolicyFileRow, TailnetTarget::PolicyFile))
            }
        }
    }

    fn exit_offer_item(tailnet: &Tailnet) -> GroupItem<'_, TailnetMessage> {
        let item = GroupItem::reading(ExitOfferRow { tailnet }, TailnetTarget::ExitOffer);
        match tailnet.offer_control() {
            ExitOfferControl::Switchable(exit_offer_switch @ ExitOfferSwitch::StopOffering) => item
                .on_switch_press(
                    true,
                    "stop offering this machine as an exit node",
                    TailnetMessage::SwitchExitOffer(exit_offer_switch),
                ),
            ExitOfferControl::Switchable(exit_offer_switch @ ExitOfferSwitch::StartOffering) => {
                item.on_switch_press(
                    false,
                    "offer this machine as an exit node",
                    TailnetMessage::SwitchExitOffer(exit_offer_switch),
                )
            }
            ExitOfferControl::BlockedByExitNodeInUse => item,
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, TailnetMessage> {
        let own_node_item = self
            .tailscale
            .get()
            .and_then(OwnNodeRow::of)
            .map(|own_node_row| GroupItem::reading(own_node_row, TailnetTarget::OwnNode));
        let inbound_access_item = self
            .running_tailnet()
            .map(|_| GroupItem::reading(InboundAccessRow, TailnetTarget::InboundAccess));
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Tailnet,
            &self.expansion,
            TailnetMessage::Expansion,
            self.membership_item()
                .into_iter()
                .chain(own_node_item)
                .chain(inbound_access_item)
                .chain(self.policy_file_item())
                .chain(self.running_tailnet().map(Self::exit_offer_item)),
        )
        .face(WidgetGroupFace::Icon {
            glyph: SimpleIcon::Tailscale.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::network_connectivity::tailscale_status::{
        ExitNodeOffer, KeyExpiry, Peer, PeerPath,
    };

    use super::*;
    use crate::group_items::group_item_gestures::GroupItemGesture;

    fn tailnet_offering(exit_offer: ExitNodeOffer, exit_node_in_use: bool) -> Tailnet {
        Tailnet {
            tailnet_name: "example.ts.net".to_owned(),
            own_node: Peer {
                name: "workstation".to_owned(),
                ip: "100.1.1.9".to_owned(),
                os: "linux".to_owned(),
                path: PeerPath::Idle,
                received_bytes: 0,
                sent_bytes: 0,
                tags: Vec::new(),
            },
            online: Vec::new(),
            total_peers: 0,
            own_capabilities: Vec::new(),
            exit_offer,
            exit_node_in_use,
            key_expiry: KeyExpiry::Disabled,
            health_warnings: Vec::new(),
        }
    }

    #[test]
    fn the_offer_row_is_inert_while_an_exit_node_is_in_use() {
        let tailnet = tailnet_offering(ExitNodeOffer::NotOffered, true);
        assert!(matches!(
            TailnetGroup::exit_offer_item(&tailnet).gestures().left,
            GroupItemGesture::Inert
        ));
    }

    fn group_running_tailnet(tailnet_group: TailnetGroup) -> TailnetGroup {
        let mut running_group = tailnet_group;
        let _ = running_group.update(
            TailnetMessage::Tailscale(SnapshotDelivery::live_push(Ok(Tailscale::Running(
                Box::new(tailnet_offering(ExitNodeOffer::NotOffered, false)),
            )))),
            Clock::real(),
        );
        running_group
    }

    #[test]
    fn the_policy_file_row_needs_an_api_token() {
        let without_token = group_running_tailnet(TailnetGroup::without_policy_file());
        assert!(without_token.policy_file_item().is_none());
        let with_token =
            group_running_tailnet(TailnetGroup::with_policy_file(PathBuf::from("token")));
        assert!(with_token.policy_file_item().is_some());
    }

    #[test]
    fn an_approved_offer_can_be_stopped() {
        let tailnet = tailnet_offering(ExitNodeOffer::Approved, false);
        assert!(matches!(
            TailnetGroup::exit_offer_item(&tailnet).gestures().left,
            GroupItemGesture::Performs {
                message: TailnetMessage::SwitchExitOffer(ExitOfferSwitch::StopOffering),
                ..
            }
        ));
    }
}
