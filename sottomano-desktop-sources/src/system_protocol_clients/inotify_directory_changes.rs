use std::{
    ffi::OsString,
    fs,
    future::ready,
    io,
    path::{Path, PathBuf},
};

use futures::{Stream, StreamExt};
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};

use crate::stream_operators::restarting;

const EVENT_BUFFER_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchTarget {
    EntriesOf(PathBuf),
    NamesIn(PathBuf),
    File(PathBuf),
    WrittenFile(PathBuf),
    OneLevelTree(PathBuf),
}

impl WatchTarget {
    fn directory(&self) -> PathBuf {
        match self {
            WatchTarget::EntriesOf(directory)
            | WatchTarget::NamesIn(directory)
            | WatchTarget::OneLevelTree(directory) => directory.clone(),
            WatchTarget::File(file) | WatchTarget::WrittenFile(file) => {
                file.parent().unwrap_or(Path::new("/")).to_path_buf()
            }
        }
    }

    fn entry_name(&self) -> Option<OsString> {
        match self {
            WatchTarget::EntriesOf(_) | WatchTarget::NamesIn(_) | WatchTarget::OneLevelTree(_) => {
                None
            }
            WatchTarget::File(file) | WatchTarget::WrittenFile(file) => {
                file.file_name().map(OsString::from)
            }
        }
    }

    fn watched_events(&self) -> WatchMask {
        match self {
            WatchTarget::NamesIn(_) => relinking_events(),
            WatchTarget::WrittenFile(_) => {
                WatchMask::CREATE | WatchMask::MOVED_TO | WatchMask::CLOSE_WRITE
            }
            WatchTarget::EntriesOf(_) | WatchTarget::File(_) | WatchTarget::OneLevelTree(_) => {
                entry_events()
            }
        }
    }

    fn subdirectories(&self) -> Vec<PathBuf> {
        match self {
            WatchTarget::OneLevelTree(directory) => fs::read_dir(directory)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect(),
            WatchTarget::EntriesOf(_)
            | WatchTarget::NamesIn(_)
            | WatchTarget::File(_)
            | WatchTarget::WrittenFile(_) => Vec::new(),
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
    grows_subdirectories: Option<PathBuf>,
}

impl Accepted {
    fn new_subdirectory(&self, event: &inotify::Event<OsString>) -> Option<PathBuf> {
        let root = self.grows_subdirectories.as_ref()?;
        let appeared = event.mask.contains(EventMask::ISDIR)
            && event
                .mask
                .intersects(EventMask::CREATE | EventMask::MOVED_TO);
        (event.wd == self.descriptor && appeared)
            .then(|| event.name.as_ref().map(|name| root.join(name)))
            .flatten()
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
        if let Ok(descriptor) = watches.add(&directory, target.watched_events()) {
            accepted.push(Accepted {
                descriptor,
                name: target.entry_name(),
                grows_subdirectories: matches!(target, WatchTarget::OneLevelTree(_))
                    .then(|| directory.clone()),
            });
        }
        for subdirectory in target.subdirectories() {
            if let Ok(descriptor) = watches.add(&subdirectory, entry_events()) {
                accepted.push(Accepted {
                    descriptor,
                    name: None,
                    grows_subdirectories: None,
                });
            }
        }
    }
    for symlink in symlinks {
        if let Ok(descriptor) = watches.add(&symlink.directory, relinking_events()) {
            accepted.push(Accepted {
                descriptor,
                name: Some(symlink.name),
                grows_subdirectories: None,
            });
        }
    }
    Ok(inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .filter_map(move |event| {
            let Ok(event) = event else {
                return ready(None);
            };
            let appeared: Vec<PathBuf> = accepted
                .iter()
                .filter_map(|watched| watched.new_subdirectory(&event))
                .collect();
            for subdirectory in appeared {
                if let Ok(descriptor) = watches.add(&subdirectory, entry_events()) {
                    accepted.push(Accepted {
                        descriptor,
                        name: None,
                        grows_subdirectories: None,
                    });
                }
            }
            ready(
                accepted
                    .iter()
                    .any(|watched| {
                        event.wd == watched.descriptor
                            && watched
                                .name
                                .as_ref()
                                .is_none_or(|name| event.name.as_ref() == Some(name))
                    })
                    .then_some(()),
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

#[cfg(test)]
mod tests {
    use std::{env, fs, os::unix::fs::symlink, path::PathBuf, pin::pin, process, time::Duration};

    use futures::StreamExt;
    use tokio::time::timeout;

    use super::{WatchTarget, watch};

    fn scratch_directory(test: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("sottomano-{test}-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[tokio::test]
    async fn swapping_the_symlink_like_umbra_is_seen() {
        let directory = scratch_directory("symlink-swap");
        fs::write(directory.join("dark-sottomano.toml"), "").unwrap();
        fs::write(directory.join("light-sottomano.toml"), "").unwrap();
        let config = directory.join("sottomano.toml");
        symlink("dark-sottomano.toml", &config).unwrap();
        let mut changes = pin!(watch(&[WatchTarget::WrittenFile(config.clone())]).unwrap());

        let staged = directory.join("sottomano.toml.staged");
        symlink("light-sottomano.toml", &staged).unwrap();
        fs::rename(&staged, &config).unwrap();

        let change = timeout(Duration::from_secs(2), changes.next()).await;
        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(change, Ok(Some(())));
    }

    #[tokio::test]
    async fn editing_a_sibling_file_is_ignored() {
        let directory = scratch_directory("sibling-edit");
        let config = directory.join("sottomano.toml");
        let mut changes = pin!(watch(&[WatchTarget::WrittenFile(config.clone())]).unwrap());

        fs::write(directory.join("other.toml"), "").unwrap();
        let after_sibling = timeout(Duration::from_millis(200), changes.next()).await;
        fs::write(&config, "").unwrap();
        let after_config = timeout(Duration::from_secs(2), changes.next()).await;

        fs::remove_dir_all(&directory).unwrap();
        assert!(after_sibling.is_err());
        assert_eq!(after_config, Ok(Some(())));
    }
}
