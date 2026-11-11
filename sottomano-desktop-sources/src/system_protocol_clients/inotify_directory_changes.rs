use std::{
    ffi::OsString,
    fs,
    future::ready,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{Stream, StreamExt};
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};

use crate::stream_operators::resubscribed;

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
    resubscribed(Duration::from_secs(1), move || {
        let targets = targets.clone();
        async move { armed_watch(&targets).map(|events| events.map(Ok::<_, io::Error>)) }
    })
    .filter_map(|event| ready(event.ok()))
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeferredArm {
    descriptor: WatchDescriptor,
    name: OsString,
    arms_target: WatchTarget,
}

impl DeferredArm {
    fn appears_in(&self, event: &inotify::Event<OsString>) -> bool {
        event.wd == self.descriptor && event.name.as_ref() == Some(&self.name)
    }
}

fn nearest_existing_ancestor(path: &Path) -> Option<(PathBuf, OsString)> {
    let components: Vec<_> = path.components().collect();
    for missing_prefix in (1..=components.len()).rev() {
        let existing_ancestor: PathBuf = components[..missing_prefix].iter().copied().collect();
        if !existing_ancestor.exists() {
            continue;
        }
        return components
            .get(missing_prefix)
            .map(|component| (existing_ancestor, component.as_os_str().to_os_string()));
    }
    None
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

pub fn armed_watch(targets: &[WatchTarget]) -> io::Result<impl Stream<Item = ()> + Send + use<>> {
    let inotify = Inotify::init()?;
    let mut watches = inotify.watches();
    let mut accepted = Vec::new();
    let mut deferred = Vec::new();
    let mut symlinks = Vec::new();
    for target in targets {
        arm(
            target,
            &mut watches,
            &mut accepted,
            &mut deferred,
            &mut symlinks,
        );
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
            let mut fired = false;
            let appeared: Vec<PathBuf> = accepted
                .iter()
                .filter_map(|watched| watched.new_subdirectory(&event))
                .collect();
            let readied: Vec<(WatchTarget, DeferredArm)> = deferred
                .iter()
                .filter(|unread| unread.appears_in(&event))
                .map(|unread| (unread.arms_target.clone(), unread.clone()))
                .collect();
            let mut followup_symlinks = Vec::new();
            for (target, unread) in readied {
                deferred.retain(|pending| pending != &unread);
                arm(
                    &target,
                    &mut watches,
                    &mut accepted,
                    &mut deferred,
                    &mut followup_symlinks,
                );
                fired = true;
            }
            for symlink in followup_symlinks {
                if let Ok(descriptor) = watches.add(&symlink.directory, relinking_events()) {
                    accepted.push(Accepted {
                        descriptor,
                        name: Some(symlink.name),
                        grows_subdirectories: None,
                    });
                }
            }
            for subdirectory in appeared {
                if let Ok(descriptor) = watches.add(&subdirectory, entry_events()) {
                    accepted.push(Accepted {
                        descriptor,
                        name: None,
                        grows_subdirectories: None,
                    });
                }
            }
            let matched = accepted.iter().any(|watched| {
                event.wd == watched.descriptor
                    && watched
                        .name
                        .as_ref()
                        .is_none_or(|name| event.name.as_ref() == Some(name))
            });
            ready((fired || matched).then_some(()))
        }))
}

fn arm(
    target: &WatchTarget,
    watches: &mut inotify::Watches,
    accepted: &mut Vec<Accepted>,
    deferred: &mut Vec<DeferredArm>,
    symlinks: &mut Vec<NamedEntry>,
) {
    let directory = target.directory();
    symlink_steps(&directory, symlinks);
    if let Ok(descriptor) = watches.add(&directory, target.watched_events()) {
        accepted.push(Accepted {
            descriptor,
            name: target.entry_name(),
            grows_subdirectories: matches!(target, WatchTarget::OneLevelTree(_))
                .then(|| directory.clone()),
        });
    } else if let Some((existing_ancestor, first_missing)) = nearest_existing_ancestor(&directory)
        && let Ok(descriptor) = watches.add(&existing_ancestor, relinking_events())
    {
        deferred.push(DeferredArm {
            descriptor,
            name: first_missing,
            arms_target: target.clone(),
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

    use super::{WatchTarget, armed_watch};

    fn scratch_directory(test: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("sottomano-{test}-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[tokio::test]
    async fn a_target_in_a_missing_directory_fires_when_the_directory_appears() {
        let root = scratch_directory("deferred-arm");
        let config = root.join("sottomano.toml");
        fs::create_dir(root.join("elsewhere")).unwrap();
        let mut changes = pin!(armed_watch(&[WatchTarget::WrittenFile(config.clone())]).unwrap());

        fs::create_dir(root.join("sottomano")).unwrap();
        fs::write(&config, "").unwrap();
        let change = timeout(Duration::from_secs(2), changes.next()).await;
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(change, Ok(Some(())));
    }

    #[tokio::test]
    async fn swapping_the_symlink_like_umbra_is_seen() {
        let directory = scratch_directory("symlink-swap");
        fs::write(directory.join("dark-sottomano.toml"), "").unwrap();
        fs::write(directory.join("light-sottomano.toml"), "").unwrap();
        let config = directory.join("sottomano.toml");
        symlink("dark-sottomano.toml", &config).unwrap();
        let mut changes = pin!(armed_watch(&[WatchTarget::WrittenFile(config.clone())]).unwrap());

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
        let mut changes = pin!(armed_watch(&[WatchTarget::WrittenFile(config.clone())]).unwrap());

        fs::write(directory.join("other.toml"), "").unwrap();
        let after_sibling = timeout(Duration::from_millis(200), changes.next()).await;
        fs::write(&config, "").unwrap();
        let after_config = timeout(Duration::from_secs(2), changes.next()).await;

        fs::remove_dir_all(&directory).unwrap();
        assert!(after_sibling.is_err());
        assert_eq!(after_config, Ok(Some(())));
    }
}
