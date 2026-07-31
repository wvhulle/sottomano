use iced::futures::{SinkExt, StreamExt, channel::mpsc::Sender};
use zbus::Connection;

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value,
    font_awesome::Solid,
    net_interfaces::{nm, signal_level},
    row,
};

pub const NAME: &str = "wifi";

#[derive(Debug, Clone)]
struct Link {
    ssid: String,
    signal: i64,
    frequency: String,
    security: String,
    ip: String,
    gateway: String,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    device: String,
    link: Option<Link>,
}

impl BarWidget for Wifi {
    fn icon(&self) -> Glyph {
        Solid::Wifi.into()
    }

    fn disabled(&self) -> bool {
        self.link.is_none()
    }

    fn level(&self) -> Level {
        self.link
            .as_ref()
            .map_or(Level::Ok, |link| signal_level(link.signal))
    }

    fn value(&self) -> Option<Value> {
        self.link.as_ref().map(|link| Value::Gauge {
            percent: link.signal,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        match &self.link {
            None => TooltipBody::Table(vec![row("WiFi", "disconnected")]),
            Some(link) => TooltipBody::Table(vec![
                row("SSID", link.ssid.clone()),
                row("Signal", format!("{}%", link.signal)),
                row("Frequency", link.frequency.clone()),
                row("Security", link.security.clone()),
                row("Interface", self.device.clone()),
                row("IP", link.ip.clone()),
                row("Gateway", link.gateway.clone()),
            ]),
        }
    }
}

async fn read(connection: &Connection) -> Option<Wifi> {
    let device = nm::devices(connection)
        .await
        .into_iter()
        .find(|device| device.kind == nm::TYPE_WIFI)?;
    if !device.connected {
        return Some(Wifi {
            device: device.interface,
            link: None,
        });
    }
    let link = match nm::access_point(connection, &device.path).await {
        Some(ap) => {
            let ip = nm::ip(connection, &device.path).await;
            Some(Link {
                ssid: ap.ssid,
                signal: ap.signal,
                frequency: ap.frequency,
                security: ap.security,
                ip: ip.address,
                gateway: ip.gateway,
            })
        }
        None => None,
    };
    Some(Wifi {
        device: device.interface,
        link,
    })
}

pub async fn run(mut sink: Sender<Option<Wifi>>) {
    let Ok(connection) = Connection::system().await else {
        return;
    };
    if sink.send(read(&connection).await).await.is_err() {
        return;
    }
    let Ok(mut signals) = nm::changes(&connection).await else {
        return;
    };
    while signals.next().await.is_some() {
        if sink.send(read(&connection).await).await.is_err() {
            return;
        }
    }
}
