use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use iced::futures::{Stream, StreamExt, stream};
use serde::Deserialize;

use crate::{
    data::source::{
        Delivery, PolledSource, Source, command, command::blocking::run, present_or_absent,
        ticked_polls,
    },
    widgets::tooltip_body::Section,
};

const RUNNING_BACKEND_STATE: &str = "Running";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Command(command::Error),
    Malformed,
}

impl From<Error> for Section {
    fn from(error: Error) -> Self {
        Section::error(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Command(error) => write!(f, "{error}"),
            Error::Malformed => write!(f, "unexpected status JSON"),
        }
    }
}

impl From<command::Error> for Error {
    fn from(error: command::Error) -> Self {
        Error::Command(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub name: String,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tailnet {
    pub own_node: Peer,
    pub online: Vec<Peer>,
    pub total_peers: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tailscale {
    Stopped,
    Running(Tailnet),
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Status {
    backend_state: String,
    #[serde(rename = "Self")]
    own_node: Option<Node>,
    #[serde(default)]
    peer: HashMap<String, Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Node {
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
    #[serde(default)]
    online: bool,
}

impl From<&Node> for Peer {
    fn from(node: &Node) -> Self {
        Peer {
            name: node
                .dns_name
                .split('.')
                .next()
                .unwrap_or(&node.dns_name)
                .to_owned(),
            ip: node.tailscale_ips.first().cloned(),
        }
    }
}

impl Source for Tailscale {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "tailscale";
}

impl PolledSource for Tailscale {
    const INTERVAL: Duration = Duration::from_secs(10);

    fn polls(key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(stream::once(command::exists("tailscale")), move || {
            ticked_polls::<Self>(key).boxed()
        })
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let raw = command::output("tailscale", &["status", "--json"]).await?;
        let status: Status = serde_json::from_str(&raw).map_err(|_| Error::Malformed)?;
        if status.backend_state != RUNNING_BACKEND_STATE {
            return Ok(Tailscale::Stopped);
        }
        let own_node = status.own_node.as_ref().ok_or(Error::Malformed)?;
        Ok(Tailscale::Running(Tailnet {
            own_node: Peer::from(own_node),
            online: status
                .peer
                .values()
                .filter(|peer| peer.online)
                .map(Peer::from)
                .collect(),
            total_peers: status.peer.len(),
        }))
    }
}

pub fn open_admin() -> bool {
    run("xdg-open", &["https://login.tailscale.com/admin/machines"])
}
