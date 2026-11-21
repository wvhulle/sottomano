use futures::{FutureExt, Stream, StreamExt, TryStreamExt, stream};
use zbus::{Connection, MatchRule, MessageStream, fdo::ObjectManagerProxy, message::Type};

const SIGNAL_SUBSCRIPTIONS_IN_FLIGHT: usize = 8;

const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const PROPERTIES_CHANGED_MEMBER: &str = "PropertiesChanged";
const QUEUED_SIGNALS: usize = 64;

pub fn properties_changed_rule(
    sender: &'static str,
    changed_interface: &'static str,
) -> zbus::Result<MatchRule<'static>> {
    Ok(MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender)?
        .interface(PROPERTIES_INTERFACE)?
        .member(PROPERTIES_CHANGED_MEMBER)?
        .add_arg(changed_interface)?
        .build())
}

pub fn any_properties_changed_rule(sender: &'static str) -> zbus::Result<MatchRule<'static>> {
    Ok(MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender)?
        .interface(PROPERTIES_INTERFACE)?
        .member(PROPERTIES_CHANGED_MEMBER)?
        .build())
}

pub fn signal_rule(
    sender: &'static str,
    interface: &'static str,
    member: &'static str,
) -> zbus::Result<MatchRule<'static>> {
    Ok(MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(sender)?
        .interface(interface)?
        .member(member)?
        .build())
}

pub async fn signal_messages(
    connection: &Connection,
    rule: MatchRule<'static>,
) -> zbus::Result<MessageStream> {
    MessageStream::for_match_rule(rule, connection, Some(QUEUED_SIGNALS)).await
}

pub async fn change_triggers<Rules: IntoIterator<Item = MatchRule<'static>, IntoIter: Send>>(
    connection: &Connection,
    rules: Rules,
) -> zbus::Result<impl Stream<Item = zbus::Result<()>> + Send + use<Rules>> {
    let message_streams = stream::iter(rules)
        .map(|rule| signal_messages(connection, rule))
        .buffered(SIGNAL_SUBSCRIPTIONS_IN_FLIGHT)
        .try_collect::<Vec<_>>()
        .boxed()
        .await?;
    Ok(stream::select_all(message_streams).map(|message| message.map(drop)))
}

pub async fn object_manager_changes(
    connection: &Connection,
    service: &'static str,
    object_path: &'static str,
) -> zbus::Result<impl Stream<Item = zbus::Result<()>> + Send + use<>> {
    let objects = ObjectManagerProxy::new(connection, service, object_path).await?;
    let added_interfaces = objects.receive_interfaces_added().await?.map(|_| Ok(()));
    let removed_interfaces = objects.receive_interfaces_removed().await?.map(|_| Ok(()));
    let changed_properties =
        change_triggers(connection, [any_properties_changed_rule(service)?]).await?;
    Ok(stream::select_all([
        added_interfaces.boxed(),
        removed_interfaces.boxed(),
        changed_properties.boxed(),
    ]))
}
