use std::convert::Infallible;

use futures::{Sink, sink};
use sottomano_desktop_sources::desktop_notifications::{
    freedesktop_notification_server::NotificationSignals, notification_data::Outbound,
};

pub fn notification_signal_sink(
    signals: NotificationSignals,
) -> impl Sink<Outbound, Error = Infallible> + Send {
    sink::unfold(signals, |signals, outbound| async move {
        signals.emit_outbound_signal(outbound).await;
        Ok(signals)
    })
}
