//! Reactive output (monitor) connect/disconnect detection straight from the
//! Wayland protocol (`wl_registry` global add/remove for `wl_output`), driven
//! off the tokio reactor via `AsyncFd` rather than a blocking thread. niri's
//! own IPC event stream has no output-added/-removed event, so this talks to
//! the compositor at the Wayland level instead — compositor-agnostic, and it
//! doubles as the initial enumeration: already-connected outputs are
//! replayed as `Added` the moment the registry is bound.

use std::{
    collections::HashMap,
    io,
    os::fd::{AsFd, AsRawFd, RawFd},
};

use iced::futures::{SinkExt, channel::mpsc::Sender};
use tokio::{io::unix::AsyncFd, task};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    backend::WaylandError,
    protocol::{wl_output, wl_registry},
};

#[derive(Debug, Clone)]
pub enum OutputEvent {
    Added(String),
    Removed(String),
}

/// `Connection` only exposes its fd through `AsFd`; `AsyncFd` needs `AsRawFd`.
struct ConnectionFd(Connection);

impl AsRawFd for ConnectionFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_fd().as_raw_fd()
    }
}

/// Per-output state keyed by the registry's numeric global id — the only
/// identity `wl_registry::Event::GlobalRemove` gives us, so it's what we use
/// to look the output back up when it disappears.
#[derive(Default)]
struct State {
    outputs: HashMap<u32, wl_output::WlOutput>,
    names: HashMap<u32, String>,
    events: Vec<OutputEvent>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == wl_output::WlOutput::interface().name => {
                let output = registry.bind::<wl_output::WlOutput, _, _>(
                    name,
                    version.min(wl_output::WlOutput::interface().version),
                    qh,
                    name,
                );
                state.outputs.insert(name, output);
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.outputs.remove(&name);
                if let Some(output_name) = state.names.remove(&name) {
                    state.events.push(OutputEvent::Removed(output_name));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _proxy: &wl_output::WlOutput,
        event: wl_output::Event,
        registry_name: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            state.names.insert(*registry_name, name.clone());
            state.events.push(OutputEvent::Added(name));
        }
    }
}

/// Run the listener loop, forwarding `OutputEvent`s (translated to `M` by
/// `into_message`) as they arrive. Returns once the connection or channel
/// breaks — the caller decides whether/how to retry.
pub async fn listen<M>(sender: &mut Sender<M>, into_message: impl Fn(OutputEvent) -> M) {
    let connection = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("wayland: output listener connect failed: {e}");
            return;
        }
    };
    let mut event_queue: EventQueue<State> = connection.new_event_queue();
    let qh = event_queue.handle();
    connection.display().get_registry(&qh, ());
    let mut state = State::default();

    let async_fd = match AsyncFd::new(ConnectionFd(connection.clone())) {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("wayland: output listener fd registration failed: {e}");
            return;
        }
    };

    loop {
        // `iced::stream::channel` wraps this future in `stream::select(receiver,
        // runner)`: the messages we `send` below only reach the app when that
        // `select` gets to poll the *receiver* half — which it can't while this
        // half (the `runner`) keeps returning `Ready`. When the compositor
        // floods events, the read/dispatch loop can stay runnable across many
        // iterations and starve the receiver, so nothing renders. This explicit
        // yield hands control back each iteration so queued messages drain. It
        // costs nothing at idle: the loop still parks on `readable()` below.
        task::yield_now().await;
        if let Err(e) = event_queue.dispatch_pending(&mut state) {
            eprintln!("wayland: output listener dispatch failed: {e}");
            return;
        }
        for event in state.events.drain(..) {
            if sender.send(into_message(event)).await.is_err() {
                return; // receiver dropped: app is shutting down
            }
        }
        if let Err(e) = connection.flush() {
            eprintln!("wayland: output listener flush failed: {e}");
            return;
        }
        let Ok(mut readable) = async_fd.readable().await else {
            return;
        };
        // `ReadEventsGuard` holds a raw pointer and isn't `Send`, so it can't
        // be held across the `.await` above — `try_io`'s closure runs
        // synchronously, keeping prepare+read together without that gap.
        //
        // Crucially, a read with nothing to read yields `WouldBlock`, and that
        // kind must reach `try_io` intact: `try_io` clears the fd's readiness
        // only when it sees `WouldBlock`, and without that the next
        // `readable()` returns immediately and the loop busy-spins a core.
        // So map niri's `WaylandError::Io` back to its bare `io::Error`
        // (preserving the kind) rather than wrapping it in `Error::other`.
        let read = readable.try_io(|_| match event_queue.prepare_read() {
            Some(guard) => guard.read().map_err(|e| match e {
                WaylandError::Io(io) => io,
                protocol @ WaylandError::Protocol(_) => io::Error::other(protocol),
            }),
            // Another read already queued events for us.
            None => Ok(0),
        });
        if let Ok(Err(e)) = read {
            eprintln!("wayland: output listener read failed: {e}");
            return;
        }
    }
}
