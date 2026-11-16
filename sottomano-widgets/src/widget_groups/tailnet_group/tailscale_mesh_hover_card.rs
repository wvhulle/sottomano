use bytesize::ByteSize;
use chrono::{DateTime, TimeDelta, Utc};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    network_connectivity::tailscale_status::{
        ADMIN_CONSOLE_URL, KeyExpiry, Peer, PeerPath, Tailscale,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const KEY_EXPIRY_WARNING_WINDOW: TimeDelta = TimeDelta::days(14);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyExpiryUrgency {
    ExpiresSoon,
    Distant,
}

fn key_expiry_urgency(key_expiry: KeyExpiry, now: DateTime<Utc>) -> KeyExpiryUrgency {
    match key_expiry {
        KeyExpiry::At(expires_at) if expires_at - now < KEY_EXPIRY_WARNING_WINDOW => {
            KeyExpiryUrgency::ExpiresSoon
        }
        KeyExpiry::At(_) | KeyExpiry::Disabled => KeyExpiryUrgency::Distant,
    }
}

fn key_expiry_entry(key_expiry: KeyExpiry, now: DateTime<Utc>) -> LabeledValue {
    match key_expiry {
        KeyExpiry::Disabled => LabeledValue::new("Key expires", "never"),
        KeyExpiry::At(expires_at) => {
            let expiry_severity = match key_expiry_urgency(key_expiry, now) {
                KeyExpiryUrgency::ExpiresSoon => Severity::Warning,
                KeyExpiryUrgency::Distant => Severity::Normal,
            };
            LabeledValue::new("Key expires", ValueCell::RelativeMoment(expires_at))
                .level(expiry_severity)
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

impl From<&Peer> for TableRecord<7> {
    fn from(peer: &Peer) -> Self {
        TableRecord::with_cells(
            peer.name.as_str(),
            [
                peer.ip.as_str().into(),
                peer.os.as_str().into(),
                peer.tags.join(", ").into(),
                peer_path_text(&peer.path).into(),
                relay_region_cell(&peer.path),
                transferred_bytes_cell(peer.received_bytes),
                transferred_bytes_cell(peer.sent_bytes),
            ],
        )
    }
}

pub(super) fn tailscale_reading_row(tailscale: &Tailscale, clock: Clock) -> ReadingRow<'_> {
    let state = match tailscale {
        Tailscale::Stopped => ReadingDisplayState::Off,
        Tailscale::Starting => ReadingDisplayState::Pending,
        Tailscale::Running(tailnet)
            if !tailnet.health_warnings.is_empty()
                || key_expiry_urgency(tailnet.key_expiry, clock.utc())
                    == KeyExpiryUrgency::ExpiresSoon =>
        {
            ReadingDisplayState::Warning
        }
        Tailscale::Running(_) => ReadingDisplayState::On,
    };
    let badge = match tailscale {
        Tailscale::Running(tailnet) if !tailnet.online.is_empty() => {
            ReadoutBadge::Counted(tailnet.online.len())
        }
        Tailscale::Running(_) | Tailscale::Stopped | Tailscale::Starting => ReadoutBadge::Unbadged,
    };
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Ungauged,
            badge,
        },
        ..ReadingRow::glyph_only(
            Tailscale::NAME,
            SimpleIcon::Tailscale.into(),
            state,
            Box::new(move |card_clock| tailscale_hover_card(tailscale, card_clock)),
        )
    }
}

pub(super) fn tailscale_hover_card(tailscale: &Tailscale, clock: Clock) -> Vec<Section> {
    let tailnet = match tailscale {
        Tailscale::Running(tailnet) => tailnet,
        Tailscale::Starting => {
            return vec![Section::new(
                "Status",
                dictionary_table([LabeledValue::new("Tailscale", "starting")]),
            )];
        }
        Tailscale::Stopped => {
            return vec![Section::new(
                "Status",
                dictionary_table([LabeledValue::new("Tailscale", "disconnected")]),
            )];
        }
    };
    let health_entries = tailnet
        .health_warnings
        .iter()
        .map(|warning| LabeledValue::new("Health", warning.as_str()).level(Severity::Warning));
    let summary_section = Section::new(
        "Status",
        dictionary_table(
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
            .chain([key_expiry_entry(tailnet.key_expiry, clock.utc())])
            .chain(health_entries),
        ),
    );
    let peer_table = Section::new(
        "Online peers",
        headed_table(
            [
                ColumnHeading::titled("IP").align_x(Horizontal::Left),
                ColumnHeading::titled("OS").align_x(Horizontal::Left),
                ColumnHeading::titled("Tags").align_x(Horizontal::Left),
                ColumnHeading::titled("Path").align_x(Horizontal::Left),
                ColumnHeading::titled("Relay").align_x(Horizontal::Left),
                ColumnHeading::titled("Received"),
                ColumnHeading::titled("Sent"),
            ],
            &tailnet.online,
        )
        .without_blank_columns(ValueCell::is_blank),
    );
    vec![summary_section, peer_table]
}
