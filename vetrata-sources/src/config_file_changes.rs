use std::{ffi::OsStr, io, path::Path};

use futures::{Stream, StreamExt, future::ready};
use inotify::{Inotify, WatchMask};
use vetrata_config::ConfigFile;

use crate::stream_operators::restarting;

const EVENT_BUFFER_BYTES: usize = 1024;

pub fn file_changes(file: &ConfigFile) -> impl Stream<Item = ()> + use<> {
    let path = file.path().to_path_buf();
    restarting("config watch", move || ready(watch_parent_directory(&path)))
}

fn watch_parent_directory(path: &Path) -> io::Result<impl Stream<Item = ()> + use<>> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", path.display()),
        )
    })?;
    let name = path.file_name().map(OsStr::to_os_string);
    let inotify = Inotify::init()?;
    inotify.watches().add(
        directory,
        WatchMask::CREATE | WatchMask::MOVED_TO | WatchMask::CLOSE_WRITE,
    )?;
    Ok(inotify
        .into_event_stream([0u8; EVENT_BUFFER_BYTES])?
        .filter_map(move |event| ready(event.ok().filter(|event| event.name == name).map(drop))))
}

#[cfg(test)]
mod tests {
    use std::{env, fs, os::unix::fs::symlink, path::PathBuf, pin::pin, process, time::Duration};

    use futures::StreamExt;
    use tokio::time::timeout;

    use super::watch_parent_directory;

    fn scratch_directory(test: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("vetrata-{test}-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[tokio::test]
    async fn swapping_the_symlink_like_umbra_is_seen() {
        let directory = scratch_directory("symlink-swap");
        fs::write(directory.join("dark-vetrata.toml"), "").unwrap();
        fs::write(directory.join("light-vetrata.toml"), "").unwrap();
        let config = directory.join("vetrata.toml");
        symlink("dark-vetrata.toml", &config).unwrap();
        let mut changes = pin!(watch_parent_directory(&config).unwrap());

        let staged = directory.join("vetrata.toml.staged");
        symlink("light-vetrata.toml", &staged).unwrap();
        fs::rename(&staged, &config).unwrap();

        let change = timeout(Duration::from_secs(2), changes.next()).await;
        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(change, Ok(Some(())));
    }

    #[tokio::test]
    async fn editing_a_sibling_file_is_ignored() {
        let directory = scratch_directory("sibling-edit");
        let config = directory.join("vetrata.toml");
        let mut changes = pin!(watch_parent_directory(&config).unwrap());

        fs::write(directory.join("other.toml"), "").unwrap();
        let after_sibling = timeout(Duration::from_millis(200), changes.next()).await;
        fs::write(&config, "").unwrap();
        let after_config = timeout(Duration::from_secs(2), changes.next()).await;

        fs::remove_dir_all(&directory).unwrap();
        assert!(after_sibling.is_err());
        assert_eq!(after_config, Ok(Some(())));
    }
}
