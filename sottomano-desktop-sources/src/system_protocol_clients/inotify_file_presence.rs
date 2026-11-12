use std::path::{Path, PathBuf};

use futures::{Stream, StreamExt, stream};

use crate::{
    backing_service_availability::Availability,
    system_protocol_clients::inotify_directory_changes::{WatchTarget, armed_watch},
};

pub fn path_present(path: PathBuf) -> impl Stream<Item = Availability> + Send {
    let initial = path_availability(&path);
    let watched_path = path.clone();
    let armed = armed_watch(&[WatchTarget::File(path)]);
    match armed {
        Ok(event_stream) => stream::iter([initial])
            .chain(event_stream.map(move |()| path_availability(&watched_path)))
            .boxed(),
        Err(failure) => {
            tracing::warn!("path presence: {failure}");
            stream::iter([initial])
                .chain(stream::pending::<Availability>())
                .boxed()
        }
    }
}

fn path_availability(path: &Path) -> Availability {
    if path.exists() {
        Availability::Available
    } else {
        Availability::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        os::unix::fs::symlink,
        path::{Path, PathBuf},
        pin::pin,
        process,
        time::Duration,
    };

    use futures::StreamExt;
    use tokio::time::timeout;

    use super::{Availability, path_present};

    fn scratch_directory(test: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!("sottomano-{test}-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[tokio::test]
    async fn a_missing_path_is_unavailable() {
        let root = scratch_directory("missing-path");
        let absent = root.join("daemons/incus/unix.socket");
        let mut presence = pin!(path_present(absent.clone()));
        let initial = presence.next().await;
        let later = timeout(Duration::from_millis(200), presence.next()).await;
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(initial, Some(Availability::Unavailable));
        assert!(later.is_err());
    }

    #[tokio::test]
    async fn an_appearing_path_becomes_available() {
        let root = scratch_directory("appearing-path");
        let socket_path = root.join("daemons/incus/unix.socket");
        let mut presence = pin!(path_present(socket_path.clone()));
        let initial = presence.next().await;
        fs::create_dir_all(root.join("daemons/incus")).unwrap();
        fs::write(&socket_path, "").unwrap();
        let later = timeout(Duration::from_secs(2), presence.next()).await;
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(initial, Some(Availability::Unavailable));
        assert_eq!(later, Ok(Some(Availability::Available)));
    }

    #[tokio::test]
    async fn a_present_path_is_available_and_follows_removal() {
        let root = scratch_directory("present-path");
        let watched = root.join("token");
        fs::write(&watched, "").unwrap();
        let mut presence = pin!(path_present(watched.clone()));
        let initial = presence.next().await;
        fs::remove_file(&watched).unwrap();
        let after_removal = timeout(Duration::from_secs(2), presence.next()).await;
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(initial, Some(Availability::Available));
        assert_eq!(after_removal, Ok(Some(Availability::Unavailable)));
    }

    #[tokio::test]
    async fn a_swapped_symlink_like_credentials_is_rechecked() {
        let root = scratch_directory("swapped-symlink");
        fs::write(root.join("first-credentials"), "").unwrap();
        fs::write(root.join("second-credentials"), "").unwrap();
        let credentials = root.join("credentials");
        symlink("first-credentials", &credentials).unwrap();
        let mut presence = pin!(path_present(credentials.clone()));
        let initial = presence.next().await;
        let staged = root.join("credentials.staged");
        symlink("second-credentials", &staged).unwrap();
        fs::rename(&staged, &credentials).unwrap();
        let after_swap = timeout(Duration::from_secs(2), presence.next()).await;
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(initial, Some(Availability::Available));
        assert_eq!(after_swap, Ok(Some(Availability::Available)));
    }

    fn availability_of(path: &Path) -> Availability {
        if path.exists() {
            Availability::Available
        } else {
            Availability::Unavailable
        }
    }

    #[test]
    fn availability_reflects_existence() {
        let root = scratch_directory("availability-check");
        assert_eq!(
            availability_of(&root.join("absent")),
            Availability::Unavailable
        );
        assert_eq!(availability_of(&root), Availability::Available);
        fs::remove_dir_all(&root).unwrap();
    }
}
