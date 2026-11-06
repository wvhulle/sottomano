use futures::Stream;

use crate::desktop_windows_apps::{
    compositor_window_state::CompositorUpdate, niri_event_stream::niri_compositor_updates,
};

pub fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
    niri_compositor_updates()
}
