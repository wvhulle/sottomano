use futures::{Stream, StreamExt, future::ready, stream::select};
use zbus::{
    Connection, MatchRule, MessageStream,
    fdo::{DBusProxy, PropertiesProxy},
    message::Type,
    names::{BusName, InterfaceName},
    proxy::CacheProperties,
};

use super::{
    Capabilities, CoverArtUrl, Media, PREFIX, PlayerBusOwner, PlayerBusOwners, Progress, SeekMode,
    TrackAlbum, TrackSource, TrackTitle,
    mpris_property_decoding::{artists, controllable_setting, declared_queue_order, parsed},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    system_protocol_clients::{
        dbus_change_signals::signal_messages,
        dbus_desktop_buses::{DesktopBuses, OnBus, OwnerProcess, Properties, prop, text_prop},
        protocol_peer_reconnects::subscribed,
    },
};

const PLAYERCTLD: &str = "org.mpris.MediaPlayer2.playerctld";

const PATH: &str = "/org/mpris/MediaPlayer2";

const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";

const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";

async fn properties(
    connection: &Connection,
    bus: &str,
    interface: &'static str,
) -> zbus::Result<Properties> {
    let proxy = PropertiesProxy::builder(connection)
        .destination(bus.to_owned())?
        .path(PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    Ok(proxy
        .get_all(InterfaceName::from_static_str_unchecked(interface))
        .await?)
}

async fn owner_process(dbus: &DBusProxy<'_>, bus: &str) -> OwnerProcess {
    match BusName::try_from(bus) {
        Ok(name) => OwnerProcess::query_bus_owner(dbus, name).await,
        Err(_) => OwnerProcess::Unidentified,
    }
}

async fn read_media(
    connection: &Connection,
    dbus: &DBusProxy<'_>,
    bus: &str,
) -> zbus::Result<Media> {
    let root = properties(connection, bus, ROOT_INTERFACE).await?;
    let player = properties(connection, bus, PLAYER_INTERFACE).await?;
    let Some(status) = text_prop(&player, "PlaybackStatus") else {
        return Err(zbus::Error::Failure(
            "PlaybackStatus is not declared".to_owned(),
        ));
    };
    let status = status.parse()?;
    let metadata = prop::<Properties>(&player, "Metadata").unwrap_or_default();
    Ok(Media {
        bus: bus.to_owned(),
        owner: owner_process(dbus, bus).await,
        identity: text_prop(&root, "Identity").unwrap_or_else(|| bus.to_owned()),
        title: text_prop(&metadata, "xesam:title").map_or(TrackTitle::Untitled, TrackTitle::Titled),
        artists: artists(&metadata),
        album: text_prop(&metadata, "xesam:album").map_or(TrackAlbum::NoAlbum, TrackAlbum::OnAlbum),
        url: parsed(&metadata, "xesam:url").map_or(TrackSource::Unlocated, TrackSource::Located),
        status,
        progress: Progress::declared(&player, &metadata),
        art_url: text_prop(&metadata, "mpris:artUrl")
            .map_or(CoverArtUrl::NotAdvertised, CoverArtUrl::Advertised),
        capabilities: Capabilities::declared(&player, &root),
        queue_order: controllable_setting(&player, declared_queue_order(&player)),
        loop_mode: controllable_setting(&player, parsed(&player, "LoopStatus")),
        seek_mode: SeekMode::declared(&metadata),
    })
}

async fn listed_player_owners(dbus: &DBusProxy<'_>) -> zbus::Result<PlayerBusOwners> {
    let mut player_owners = Vec::new();
    for name in dbus.list_names().await? {
        let bus = name.to_string();
        if !bus.starts_with(PREFIX) || bus == PLAYERCTLD {
            continue;
        }
        match dbus.get_name_owner(name.into()).await {
            Ok(owner) => player_owners.push(PlayerBusOwner {
                bus,
                owner: owner.to_string(),
            }),
            Err(owner_lookup_failure) => {
                tracing::debug!(
                    "mpris {bus}: left before its owner was looked up: {owner_lookup_failure}"
                );
            }
        }
    }
    player_owners.sort();
    Ok(PlayerBusOwners {
        listed_players: player_owners,
    })
}

impl SnapshotSource for PlayerBusOwners {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Media players";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        subscribed(buses.session, connect_player_owners).map(SnapshotDelivery::live_push)
    }
}

async fn connect_player_owners(
    connection: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<PlayerBusOwners>>> {
    let dbus = DBusProxy::new(&connection).await?;
    let ownership_changes = dbus
        .receive_name_owner_changed()
        .await?
        .filter(|signal| {
            ready(
                signal
                    .args()
                    .is_ok_and(|args| args.name().as_str().starts_with(PREFIX)),
            )
        })
        .map(|_| Ok::<(), zbus::Error>(()));
    Ok(ownership_changes.triggered_rereads(move || {
        let dbus = dbus.clone();
        async move { listed_player_owners(&dbus).await }
    }))
}

impl SnapshotSource for Media {
    type Error = zbus::Error;
    type StreamInputs = OnBus<PlayerBusOwner>;

    const NAME: &'static str = "Media player";

    fn delivery_stream(
        OnBus {
            buses,
            inputs: player,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        subscribed(buses.session, move |connection| {
            connect_player(connection, player.clone())
        })
        .map(SnapshotDelivery::live_push)
    }
}

async fn connect_player(
    connection: Connection,
    player: PlayerBusOwner,
) -> zbus::Result<impl Stream<Item = zbus::Result<Media>>> {
    let dbus = DBusProxy::new(&connection).await?;
    let properties = player_signals(
        &connection,
        &player.owner,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
    )
    .await?
    .map(|message| message.map(drop));
    let seeks = player_signals(&connection, &player.owner, PLAYER_INTERFACE, "Seeked")
        .await?
        .map(|message| message.map(drop));
    Ok(select(properties, seeks).triggered_rereads(move || {
        let connection = connection.clone();
        let dbus = dbus.clone();
        let bus = player.bus.clone();
        async move { read_media(&connection, &dbus, &bus).await }
    }))
}

async fn player_signals(
    connection: &Connection,
    owner: &str,
    interface: &'static str,
    member: &'static str,
) -> zbus::Result<MessageStream> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(owner)?
        .path(PATH)?
        .interface(interface)?
        .member(member)?
        .build()
        .into_owned();
    signal_messages(connection, rule).await
}
