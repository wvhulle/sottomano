use bytesize::ByteSize;
use chrono::{DateTime, TimeDelta, Utc};
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::tailscale_status::{
    ADMIN_CONSOLE_URL, ExitRouting, KeyExpiry, Peer, PeerPath, Tailnet, Tailscale,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_quantities::Quantity,
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(super) const TAILSCALE_ITEM_NAME: &str = "Tailscale";

const KEY_EXPIRY_WARNING_WINDOW: TimeDelta = TimeDelta::days(14);

fn key_expires_soon(key_expiry: KeyExpiry, now: DateTime<Utc>) -> bool {
    match key_expiry {
        KeyExpiry::Disabled => false,
        KeyExpiry::At(expires_at) => expires_at - now < KEY_EXPIRY_WARNING_WINDOW,
    }
}

fn tailnet_needs_attention(tailnet: &Tailnet, now: DateTime<Utc>) -> bool {
    !tailnet.health_warnings.is_empty() || key_expires_soon(tailnet.key_expiry, now)
}

fn key_expiry_entry(key_expiry: KeyExpiry, now: DateTime<Utc>) -> LabeledValue {
    match key_expiry {
        KeyExpiry::Disabled => LabeledValue::new("Key expires", "never"),
        KeyExpiry::At(expires_at) => {
            let expiry_severity = if key_expires_soon(key_expiry, now) {
                Severity::Warning
            } else {
                Severity::Normal
            };
            LabeledValue::new("Key expires", ValueCell::RelativeMoment(expires_at))
                .level(expiry_severity)
        }
    }
}

fn exit_node_entry(exit_routing: &ExitRouting) -> Option<LabeledValue> {
    match exit_routing {
        ExitRouting::Local => None,
        ExitRouting::ThroughPeer { peer_name } => {
            Some(LabeledValue::new("Exit node", peer_name.as_str()))
        }
    }
}

fn peer_path_text(path: &PeerPath) -> &'static str {
    match path {
        PeerPath::Direct => "direct",
        PeerPath::Relayed { .. } => "relayed",
        PeerPath::Idle => "idle",
    }
}

fn relay_region_cell(path: &PeerPath) -> ValueCell {
    match path {
        PeerPath::Relayed { region } => region.as_str().into(),
        PeerPath::Direct | PeerPath::Idle => ValueCell::Blank,
    }
}

fn transferred_bytes_cell(byte_count: u64) -> ValueCell {
    if byte_count == 0 {
        return ValueCell::Blank;
    }
    Quantity::size(ByteSize::b(byte_count)).into()
}

impl IntoTableRecord<6> for &Peer {
    fn into_table_record(self) -> TableRecord<6> {
        TableRecord::new(
            self.name.as_str(),
            [
                self.ip.as_str().into(),
                self.os.as_str().into(),
                peer_path_text(&self.path).into(),
                relay_region_cell(&self.path),
                transferred_bytes_cell(self.received_bytes),
                transferred_bytes_cell(self.sent_bytes),
            ],
        )
    }
}

impl GroupItemReading for Tailscale {
    fn name(&self) -> &'static str {
        TAILSCALE_ITEM_NAME
    }

    fn icon(&self) -> IconGlyph {
        SimpleIcon::Tailscale.into()
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        match self {
            Tailscale::Stopped => ReadingDisplayState::Off,
            Tailscale::Starting => ReadingDisplayState::Pending,
            Tailscale::Running(tailnet) if tailnet_needs_attention(tailnet, clock.utc()) => {
                ReadingDisplayState::Warning
            }
            Tailscale::Running(_) => ReadingDisplayState::On,
        }
    }

    fn badge(&self) -> Option<u32> {
        let Tailscale::Running(tailnet) = self else {
            return None;
        };
        u32::try_from(tailnet.online.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let tailnet = match self {
            Tailscale::Running(tailnet) => tailnet,
            Tailscale::Starting => {
                return HoverCardBody::dictionary([LabeledValue::new("Tailscale", "starting")]);
            }
            Tailscale::Stopped => {
                return HoverCardBody::dictionary([LabeledValue::new("Tailscale", "disconnected")]);
            }
        };
        let health_entries = tailnet
            .health_warnings
            .iter()
            .map(|warning| LabeledValue::new("Health", warning.as_str()).level(Severity::Warning));
        let summary_section = Section::dictionary(
            [
                LabeledValue::new("Tailscale", "connected"),
                LabeledValue::new("Tailnet", tailnet.tailnet_name.as_str()),
                LabeledValue::new(
                    "Dashboard",
                    ValueCell::BrandedLink {
                        label: "Admin console".to_owned(),
                        url: ADMIN_CONSOLE_URL.to_owned(),
                        glyph: SimpleIcon::Tailscale.into(),
                    },
                ),
                LabeledValue::new("Self", tailnet.own_node.name.as_str()),
                LabeledValue::new("IP", tailnet.own_node.ip.as_str()),
                LabeledValue::new(
                    "Peers",
                    format!("{}/{} online", tailnet.online.len(), tailnet.total_peers),
                ),
            ]
            .into_iter()
            .chain(exit_node_entry(&tailnet.exit_routing))
            .chain([key_expiry_entry(tailnet.key_expiry, clock.utc())])
            .chain(health_entries),
        );
        let peer_table = Section::column_table(
            [
                ColumnHeading::titled("IP").align_x(Horizontal::Left),
                ColumnHeading::titled("OS").align_x(Horizontal::Left),
                ColumnHeading::titled("Path").align_x(Horizontal::Left),
                ColumnHeading::titled("Relay").align_x(Horizontal::Left),
                ColumnHeading::titled("Received"),
                ColumnHeading::titled("Sent"),
            ],
            &tailnet.online,
        );
        HoverCardBody::sections([summary_section, peer_table])
    }
}
