use std::{
    io,
    os::fd::{AsFd, AsRawFd, RawFd},
    panic::resume_unwind,
};

use tokio::{io::unix::AsyncFd, task::spawn_blocking};
use wayland_client::{
    ConnectError, Connection, Dispatch, DispatchError, EventQueue, QueueHandle,
    backend::WaylandError,
    globals::{BindError, GlobalError, GlobalList, GlobalListContents, registry_queue_init},
    protocol::wl_registry::WlRegistry,
};

#[derive(Debug, thiserror::Error)]
pub enum WaylandProtocolError {
    #[error("could not connect to wayland: {0}")]
    Connect(ConnectError),
    #[error("could not list wayland globals: {0}")]
    Globals(GlobalError),
    #[error("compositor offers no {protocol}: {source}")]
    ProtocolUnavailable {
        protocol: &'static str,
        source: BindError,
    },
    #[error("compositor exposes no wl_seat")]
    NoSeat,
    #[error("wayland dispatch: {0}")]
    Dispatch(DispatchError),
    #[error("wayland backend: {0}")]
    Backend(WaylandError),
    #[error("wayland io: {0}")]
    Io(io::Error),
}

struct WaylandEventSocket {
    connection: Connection,
}

impl AsRawFd for WaylandEventSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.connection.as_fd().as_raw_fd()
    }
}

pub struct AsyncWaylandConnection<State> {
    pub event_queue: EventQueue<State>,
    pub queue_handle: QueueHandle<State>,
    pub globals: GlobalList,
    event_socket: AsyncFd<WaylandEventSocket>,
}

impl<State> AsyncWaylandConnection<State>
where
    State: Dispatch<WlRegistry, GlobalListContents> + Send + 'static,
{
    pub async fn connect() -> Result<Self, WaylandProtocolError> {
        let (connection, globals, event_queue) = spawn_blocking(|| {
            let connection = Connection::connect_to_env().map_err(WaylandProtocolError::Connect)?;
            let (globals, event_queue) =
                registry_queue_init::<State>(&connection).map_err(WaylandProtocolError::Globals)?;
            Ok::<_, WaylandProtocolError>((connection, globals, event_queue))
        })
        .await
        .unwrap_or_else(|connect_panic| resume_unwind(connect_panic.into_panic()))?;
        let event_socket = unsafe { AsyncFd::register(WaylandEventSocket { connection }) }
            .map_err(|register_failure| WaylandProtocolError::Io(register_failure.into()))?;
        Ok(Self {
            queue_handle: event_queue.handle(),
            event_queue,
            globals,
            event_socket,
        })
    }

    pub async fn flushed(&self) -> Result<(), WaylandProtocolError> {
        loop {
            match self.event_socket.get_ref().connection.flush() {
                Ok(()) => return Ok(()),
                Err(WaylandError::Io(io_failure))
                    if io_failure.kind() == io::ErrorKind::WouldBlock =>
                {
                    self.event_socket
                        .writable()
                        .await
                        .map_err(WaylandProtocolError::Io)?
                        .clear_ready();
                }
                Err(backend_failure) => return Err(WaylandProtocolError::Backend(backend_failure)),
            }
        }
    }

    pub async fn dispatched_events(
        &mut self,
        state: &mut State,
    ) -> Result<(), WaylandProtocolError> {
        loop {
            let dispatched_count = self
                .event_queue
                .dispatch_pending(state)
                .map_err(WaylandProtocolError::Dispatch)?;
            if dispatched_count > 0 {
                return Ok(());
            }
            self.flushed().await?;
            let mut readable_socket = self
                .event_socket
                .readable()
                .await
                .map_err(WaylandProtocolError::Io)?;
            let Some(events_read) = self.event_queue.prepare_read() else {
                continue;
            };
            match events_read.read() {
                Ok(_) => {}
                Err(WaylandError::Io(io_failure))
                    if io_failure.kind() == io::ErrorKind::WouldBlock =>
                {
                    readable_socket.clear_ready();
                }
                Err(backend_failure) => return Err(WaylandProtocolError::Backend(backend_failure)),
            }
        }
    }
}
