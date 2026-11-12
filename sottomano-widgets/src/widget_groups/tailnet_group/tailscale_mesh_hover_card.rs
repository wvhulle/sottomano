use bytesize::ByteSize;
use chrono::{DateTime, TimeDelta, Utc};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::network_connectivity::tailscale_status::{
    ADMIN_CONSOLE_URL, KeyExpiry, Peer, PeerPath, Tailscale,
};
use sottomano_toml_config::severity::Severity;

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

impl IntoTableRecord<7> for &Peer {
    fn into_table_record(self) -> TableRecord<7> {
        TableRecord::new(
            self.name.as_str(),
            [
                self.ip.as_str().into(),
                self.os.as_str().into(),
                self.tags.join(", ").into(),
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
        "Tailscale"
    }

    fn icon(&self) -> IconGlyph {
        SimpleIcon::Tailscale.into()
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        match self {
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
        }
    }

    fn badge(&self) -> Option<u32> {
        let Tailscale::Running(tailnet) = self else {
            return None;
        };
        Some(u32::try_from(tailnet.online.len()).unwrap_or(u32::MAX)).filter(|count| *count > 0)
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
            .chain([key_expiry_entry(tailnet.key_expiry, clock.utc())])
            .chain(health_entries),
        );
        let peer_table = Section::column_table(
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
        .without_blank_columns();
        HoverCardBody::sections([summary_section, peer_table])
    }
}
