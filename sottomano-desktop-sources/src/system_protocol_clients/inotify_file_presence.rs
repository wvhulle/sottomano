use std::{
    ffi::OsString,
    future::ready,
    io,
    path::{Path, PathBuf},
};

use futures::{Stream, StreamExt, stream};
use inotify::{Inotify, WatchMask};

use crate::stream_operators::{Availability, connected_until_failure};

const EVENT_BUFFER_BYTES: usize = 1024;

pub fn path_present(path: PathBuf) -> impl Stream<Item = Availability> + Send {
    connected_until_failure(ready(watch(path))).map(|present| {
        present.unwrap_or_else(|e| {
            tracing::warn!("path presence: {e}");
            Availability::Unavailable
        })
    })
}

fn watch(path: PathBuf) -> io::Result<impl Stream<Item = io::Result<Availability>> + Send> {
    let name: OsString = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?
        .to_owned();
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no directory"))?
        .to_owned();
    let inotify = Inotify::init()?;
    inotify.watches().add(
        &directory,
        WatchMask::CREATE
            | WatchMask::DELETE
            | WatchMask::MOVED_FROM
            | WatchMask::MOVED_TO
            | WatchMask::CLOSE_WRITE,
    )?;
    let present_now = stream::iter([path_availability(&path)]);
    let changes = inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .filter_map(move |event| {
            ready(match event {
                Ok(event) => (event.name.as_deref() == Some(name.as_os_str()))
                    .then(|| Ok(path_availability(&path))),
                Err(failure) => Some(Err(failure)),
            })
        });
    Ok(present_now.map(Ok).chain(changes))
}

fn path_availability(path: &Path) -> Availability {
    if path.exists() {
        Availability::Available
    } else {
        Availability::Unavailable
    }
}
