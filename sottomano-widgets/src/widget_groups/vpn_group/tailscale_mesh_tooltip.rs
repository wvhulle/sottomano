use bytesize::ByteSize;
use chrono::{DateTime, TimeDelta, Utc};
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::tailscale_status::{
    ADMIN_CONSOLE_URL, ExitRouting, KeyExpiry, Peer, PeerPath, Tailnet, Tailscale,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        simple_icons::SimpleIcon,
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow, TooltipBody,
        },
        widget_state::WidgetState,
    },
};

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

fn key_expiry_entry(key_expiry: KeyExpiry, now: DateTime<Utc>) -> KeyValue {
    match key_expiry {
        KeyExpiry::Disabled => KeyValue::new("Key expires", "never"),
        KeyExpiry::At(expires_at) => {
            let expiry_severity = if key_expires_soon(key_expiry, now) {
                Severity::Warning
            } else {
                Severity::Normal
            };
            KeyValue::new("Key expires", Cell::RelativeMoment(expires_at)).level(expiry_severity)
        }
    }
}

fn exit_node_entry(exit_routing: &ExitRouting) -> Option<KeyValue> {
    match exit_routing {
        ExitRouting::Local => None,
        ExitRouting::ThroughPeer { peer_name } => {
            Some(KeyValue::new("Exit node", peer_name.as_str()))
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

fn relay_region_cell(path: &PeerPath) -> Option<Cell> {
    match path {
        PeerPath::Relayed { region } => Some(region.as_str().into()),
        PeerPath::Direct | PeerPath::Idle => None,
    }
}

fn transferred_bytes_cell(byte_count: u64) -> Option<Cell> {
    (byte_count > 0).then(|| ByteSize::b(byte_count).display().iec().to_string().into())
}

impl TableRow<6> for &Peer {
    fn table_record(self) -> TableRecord<6> {
        TableRecord::new(
            self.name.as_str(),
            [
                Some(self.ip.as_str().into()),
                Some(self.os.as_str().into()),
                Some(peer_path_text(&self.path).into()),
                relay_region_cell(&self.path),
                transferred_bytes_cell(self.received_bytes),
                transferred_bytes_cell(self.sent_bytes),
            ],
        )
    }
}

impl BarWidget for Tailscale {
    fn name(&self) -> &'static str {
        "Tailscale"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Tailscale.into()
    }

    fn state(&self, clock: Clock) -> WidgetState {
        match self {
            Tailscale::Stopped => WidgetState::Off,
            Tailscale::Running(tailnet) if tailnet_needs_attention(tailnet, clock.utc()) => {
                WidgetState::Warning
            }
            Tailscale::Running(_) => WidgetState::On,
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

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let Tailscale::Running(tailnet) = self else {
            return TooltipBody::dictionary([KeyValue::new("Tailscale", "disconnected")]);
        };
        let health_entries = tailnet
            .health_warnings
            .iter()
            .map(|warning| KeyValue::new("Health", warning.as_str()).level(Severity::Warning));
        let summary_section = Section::dictionary(
            [
                KeyValue::new("Tailscale", "connected"),
                KeyValue::new("Tailnet", tailnet.tailnet_name.as_str()),
                KeyValue::new(
                    "Dashboard",
                    Cell::Link {
                        label: "Admin console".to_owned(),
                        url: ADMIN_CONSOLE_URL.to_owned(),
                        glyph: Some(SimpleIcon::Tailscale.into()),
                    },
                ),
                KeyValue::new("Self", tailnet.own_node.name.as_str()),
                KeyValue::new("IP", tailnet.own_node.ip.as_str()),
                KeyValue::new(
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
                ColumnHeading::new("IP").align_x(Horizontal::Left),
                ColumnHeading::new("OS").align_x(Horizontal::Left),
                ColumnHeading::new("Path").align_x(Horizontal::Left),
                ColumnHeading::new("Relay").align_x(Horizontal::Left),
                ColumnHeading::new("Received"),
                ColumnHeading::new("Sent"),
            ],
            &tailnet.online,
        );
        TooltipBody::sections([summary_section, peer_table])
    }
}
