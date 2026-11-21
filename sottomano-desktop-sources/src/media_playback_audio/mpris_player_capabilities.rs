use crate::system_protocol_clients::dbus_desktop_buses::{Properties, prop};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CommandOffer {
    Offered,
    #[default]
    NotOffered,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub play: CommandOffer,
    pub pause: CommandOffer,
    pub stop: CommandOffer,
    pub seek: CommandOffer,
    pub go_next: CommandOffer,
    pub go_previous: CommandOffer,
    pub raise: CommandOffer,
    pub quit: CommandOffer,
}

fn declared_offer(properties: &Properties, key: &str) -> CommandOffer {
    match prop::<bool>(properties, key) {
        Some(true) => CommandOffer::Offered,
        Some(false) | None => CommandOffer::NotOffered,
    }
}

impl Capabilities {
    pub(super) fn declared(player: &Properties, root: &Properties) -> Self {
        let control = declared_offer(player, "CanControl");
        let controllable = |key| match control {
            CommandOffer::Offered => declared_offer(player, key),
            CommandOffer::NotOffered => CommandOffer::NotOffered,
        };
        Self {
            play: controllable("CanPlay"),
            pause: controllable("CanPause"),
            stop: control,
            seek: controllable("CanSeek"),
            go_next: controllable("CanGoNext"),
            go_previous: controllable("CanGoPrevious"),
            raise: declared_offer(root, "CanRaise"),
            quit: declared_offer(root, "CanQuit"),
        }
    }
}
