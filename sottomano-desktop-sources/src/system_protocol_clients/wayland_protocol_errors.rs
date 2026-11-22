use std::io;

use wayrs_client::global::BindError;

#[derive(Debug, thiserror::Error)]
pub enum WaylandProtocolError {
    #[error("could not connect to wayland: {0}")]
    Connect(wayrs_client::ConnectError),
    #[error("compositor offers no {protocol}: {source}")]
    ProtocolUnavailable {
        protocol: &'static str,
        source: BindError,
    },
    #[error("compositor exposes no wl_seat")]
    NoSeat,
    #[error("wayland io: {0}")]
    Io(io::Error),
}
