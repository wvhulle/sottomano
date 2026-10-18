use std::{
    ffi::OsString,
    fs,
    future::ready,
    io,
    path::{Path, PathBuf},
};

use futures::{Stream, StreamExt};
use inotify::{Inotify, WatchDescriptor, WatchMask};

use crate::stream_operators::restarting;

const EVENT_BUFFER_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchTarget {
    EntriesOf(PathBuf),
    File(PathBuf),
}

impl WatchTarget {
    fn directory(&self) -> PathBuf {
        match self {
            WatchTarget::EntriesOf(directory) => directory.clone(),
            WatchTarget::File(file) => file.parent().unwrap_or(Path::new("/")).to_path_buf(),
        }
    }

    fn entry_name(&self) -> Option<OsString> {
        match self {
            WatchTarget::EntriesOf(_) => None,
            WatchTarget::File(file) => file.file_name().map(OsString::from),
        }
    }
}

pub fn changes(targets: Vec<WatchTarget>) -> impl Stream<Item = ()> + Send {
    restarting("directory watch", move || ready(watch(&targets)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedEntry {
    directory: PathBuf,
    name: OsString,
}

struct Accepted {
    descriptor: WatchDescriptor,
    name: Option<OsString>,
}

impl Accepted {
    fn matches(&self, event: &inotify::Event<OsString>) -> bool {
        event.wd == self.descriptor
            && self
                .name
                .as_ref()
                .is_none_or(|name| event.name.as_ref() == Some(name))
    }
}

fn watch(targets: &[WatchTarget]) -> io::Result<impl Stream<Item = ()> + Send + use<>> {
    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    let mut accepted = Vec::new();
    let mut symlinks = Vec::new();
    for target in targets {
        let directory = target.directory();
        symlink_steps(&directory, &mut symlinks);
        if let Ok(descriptor) = watches.add(&directory, entry_events()) {
            accepted.push(Accepted {
                descriptor,
                name: target.entry_name(),
            });
        }
    }
    for symlink in symlinks {
        if let Ok(descriptor) = watches.add(&symlink.directory, relinking_events()) {
            accepted.push(Accepted {
                descriptor,
                name: Some(symlink.name),
            });
        }
    }
    Ok(inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .filter_map(move |event| {
            ready(
                event
                    .ok()
                    .filter(|event| accepted.iter().any(|watched| watched.matches(event)))
                    .map(drop),
            )
        }))
}

fn entry_events() -> WatchMask {
    WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::MOVED_FROM
        | WatchMask::MOVED_TO
        | WatchMask::CLOSE_WRITE
        | WatchMask::MODIFY
}

fn relinking_events() -> WatchMask {
    WatchMask::CREATE | WatchMask::DELETE | WatchMask::MOVED_FROM | WatchMask::MOVED_TO
}

fn symlink_steps(path: &Path, steps: &mut Vec<NamedEntry>) {
    for ancestor in path.ancestors() {
        if !fs::symlink_metadata(ancestor).is_ok_and(|metadata| metadata.is_symlink()) {
            continue;
        }
        let Some(parent) = ancestor.parent() else {
            continue;
        };
        let Some(name) = ancestor.file_name() else {
            continue;
        };
        let step = NamedEntry {
            directory: parent.to_path_buf(),
            name: name.to_os_string(),
        };
        if steps.contains(&step) {
            continue;
        }
        steps.push(step);
        if let Ok(target) = fs::read_link(ancestor) {
            symlink_steps(&parent.join(target), steps);
        }
    }
}
