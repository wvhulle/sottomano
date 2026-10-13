use std::{ffi::OsString, future::ready, io, path::PathBuf};

use iced::futures::{Stream, StreamExt, stream};
use inotify::{Inotify, WatchMask};

const EVENT_BUFFER_BYTES: usize = 1024;

pub fn path_present(path: PathBuf) -> impl Stream<Item = bool> + Send {
    stream::once(ready(watch(path))).flat_map(|watched| match watched {
        Ok(presence) => presence.boxed(),
        Err(e) => {
            tracing::warn!("path presence: {e}");
            stream::iter([false]).boxed()
        }
    })
}

fn watch(path: PathBuf) -> io::Result<impl Stream<Item = bool> + Send> {
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
    let present_now = stream::iter([path.exists()]);
    let changes = inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .filter_map(move |event| {
            let about_path =
                event.is_ok_and(|event| event.name.as_deref() == Some(name.as_os_str()));
            ready(about_path.then(|| path.exists()))
        });
    Ok(present_now.chain(changes))
}
