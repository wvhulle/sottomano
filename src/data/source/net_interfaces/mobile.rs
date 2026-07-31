use std::collections::HashMap;

use iced::futures::{SinkExt, StreamExt, channel::mpsc::Sender, stream::select_all};
use zbus::{
    Connection, MatchRule, MessageStream,
    fdo::{ManagedObjects, ObjectManagerProxy},
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value,
    font_awesome::Solid,
    net_interfaces::{interface, prop, signal_level},
    row,
};

pub const NAME: &str = "mobile-broadband";

const MODEM: &str = "org.freedesktop.ModemManager1.Modem";
const MODEM_3GPP: &str = "org.freedesktop.ModemManager1.Modem.Modem3gpp";
const BEARER: &str = "org.freedesktop.ModemManager1.Bearer";
const DASH: &str = "—";

#[derive(Debug, Clone)]
pub struct Mobile {
    model: String,
    operator: String,
    technology: Vec<String>,
    signal: Option<i64>,
    registration: String,
    state: String,
    ip: Option<String>,
    failed_reason: Option<String>,
}

impl BarWidget for Mobile {
    fn icon(&self) -> Glyph {
        Solid::TowerCell.into()
    }

    fn disabled(&self) -> bool {
        self.state != "connected"
    }

    fn level(&self) -> Level {
        self.signal.map_or(Level::Ok, signal_level)
    }

    fn value(&self) -> Option<Value> {
        self.signal.map(|percent| Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Model", self.model.clone()),
            row("Operator", self.operator.clone()),
            row("Technology", self.technology.join(", ")),
        ];
        if let Some(signal) = self.signal {
            rows.push(row("Signal", format!("{signal}%")));
        }
        rows.push(row("Registration", self.registration.clone()));
        match &self.ip {
            Some(ip) if self.state == "connected" => rows.push(row("IP", ip.clone())),
            _ => rows.push(row("State", self.state.clone())),
        }
        if self.state == "failed"
            && let Some(reason) = &self.failed_reason
        {
            rows.push(row("FailedReason", reason.clone()));
        }
        TooltipBody::Table(rows)
    }
}

fn state_name(state: i32) -> &'static str {
    match state {
        -1 => "failed",
        1 => "initializing",
        2 => "locked",
        3 => "disabled",
        4 => "disabling",
        5 => "enabling",
        6 => "enabled",
        7 => "searching",
        8 => "registered",
        9 => "disconnecting",
        10 => "connecting",
        11 => "connected",
        _ => "unknown",
    }
}

fn registration_name(state: u32) -> &'static str {
    match state {
        0 => "idle",
        1 => "home",
        2 => "searching",
        3 => "denied",
        5 => "roaming",
        6 => "home-sms-only",
        7 => "roaming-sms-only",
        8 => "emergency-only",
        9 => "home-csfb-not-preferred",
        10 => "roaming-csfb-not-preferred",
        11 => "attached-rlos",
        _ => "unknown",
    }
}

fn failed_reason_name(reason: u32) -> &'static str {
    match reason {
        2 => "sim-missing",
        3 => "sim-error",
        4 => "unknown-capabilities",
        5 => "esim-without-profiles",
        _ => "unknown",
    }
}

const ACCESS_TECHNOLOGIES: [(u32, &str); 16] = [
    (1 << 0, "pots"),
    (1 << 1, "gsm"),
    (1 << 2, "gsm-compact"),
    (1 << 3, "gprs"),
    (1 << 4, "edge"),
    (1 << 5, "umts"),
    (1 << 6, "hsdpa"),
    (1 << 7, "hsupa"),
    (1 << 8, "hspa"),
    (1 << 9, "hspa-plus"),
    (1 << 10, "1xrtt"),
    (1 << 11, "evdo0"),
    (1 << 12, "evdoa"),
    (1 << 13, "evdob"),
    (1 << 14, "lte"),
    (1 << 15, "5gnr"),
];

fn technologies(bits: u32) -> Vec<String> {
    ACCESS_TECHNOLOGIES
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| (*name).to_owned())
        .collect()
}

fn clean(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty() && s != "--")
}

fn bearer_ip(objects: &ManagedObjects, path: &OwnedObjectPath) -> Option<String> {
    let bearer = interface(objects.get(path)?, BEARER)?;
    let config: HashMap<String, OwnedValue> = prop(bearer, "Ip4Config")?;
    let address = config
        .get("address")
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| String::try_from(value).ok())?;
    clean(Some(address))
}

fn reading(objects: &ManagedObjects) -> Option<Mobile> {
    let interfaces = objects
        .values()
        .find(|interfaces| interface(interfaces, MODEM).is_some())?;
    let modem = interface(interfaces, MODEM)?;
    let modem_3gpp = interface(interfaces, MODEM_3GPP);

    let state = state_name(prop::<i32>(modem, "State").unwrap_or(0)).to_owned();
    let ip = prop::<Vec<OwnedObjectPath>>(modem, "Bearers")
        .unwrap_or_default()
        .first()
        .and_then(|bearer| bearer_ip(objects, bearer));

    Some(Mobile {
        model: clean(prop::<String>(modem, "Model")).unwrap_or_else(|| DASH.to_owned()),
        operator: modem_3gpp
            .and_then(|group| clean(prop::<String>(group, "OperatorName")))
            .unwrap_or_else(|| DASH.to_owned()),
        technology: technologies(prop::<u32>(modem, "AccessTechnologies").unwrap_or(0)),
        signal: prop::<(u32, bool)>(modem, "SignalQuality").map(|(quality, _)| i64::from(quality)),
        registration: modem_3gpp
            .and_then(|group| prop::<u32>(group, "RegistrationState"))
            .map_or_else(
                || DASH.to_owned(),
                |state| registration_name(state).to_owned(),
            ),
        failed_reason: prop::<u32>(modem, "StateFailedReason")
            .filter(|reason| *reason != 0)
            .map(|reason| failed_reason_name(reason).to_owned()),
        state,
        ip,
    })
}

async fn read(objects: &ObjectManagerProxy<'_>) -> Option<Mobile> {
    reading(&objects.get_managed_objects().await.ok()?)
}

pub async fn run(mut sink: Sender<Option<Mobile>>) {
    let Ok(connection) = Connection::system().await else {
        return;
    };
    let Ok(objects) = ObjectManagerProxy::new(
        &connection,
        "org.freedesktop.ModemManager1",
        "/org/freedesktop/ModemManager1",
    )
    .await
    else {
        return;
    };

    if sink.send(read(&objects).await).await.is_err() {
        return;
    }

    let Ok(added) = objects.receive_interfaces_added().await else {
        return;
    };
    let Ok(removed) = objects.receive_interfaces_removed().await else {
        return;
    };
    let Ok(changed) = changed_properties(&connection).await else {
        return;
    };
    let mut signals = select_all([
        added.map(|_| ()).boxed(),
        removed.map(|_| ()).boxed(),
        changed.map(|_| ()).boxed(),
    ]);

    while signals.next().await.is_some() {
        if sink.send(read(&objects).await).await.is_err() {
            return;
        }
    }
}

async fn changed_properties(connection: &Connection) -> zbus::Result<MessageStream> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.ModemManager1")?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build();
    MessageStream::for_match_rule(rule, connection, Some(16)).await
}
