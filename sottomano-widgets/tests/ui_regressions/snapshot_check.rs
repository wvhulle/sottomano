use std::{fs, io, path::PathBuf};

use iced_test::core::renderer::Headless;
use sha2::{Digest, Sha256};
use sottomano_widget_scenes::scripted_session::{FrameWatcher, ScriptedSession};

const SNAPSHOTS_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/ui_regressions/snapshots"
);

fn snapshot_path(renderer_name: &str, snapshot_name: &str) -> PathBuf {
    let renderer_directory: String = renderer_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    PathBuf::from(SNAPSHOTS_DIRECTORY)
        .join(renderer_directory)
        .join(format!("{snapshot_name}.sha256"))
}

pub(crate) fn assert_matches_snapshot<Watcher: FrameWatcher>(
    session: &mut ScriptedSession<Watcher>,
    snapshot_name: &str,
) {
    let screenshot_hash = format!("{:x}", Sha256::digest(session.screenshot()));
    let path = snapshot_path(&session.layout_renderer.name(), snapshot_name);
    let saved_hash = match fs::read_to_string(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(
                path.parent()
                    .expect("snapshots sit in a renderer directory"),
            )
            .unwrap();
            fs::write(&path, &screenshot_hash).unwrap();
            return;
        }
        read => read.unwrap(),
    };
    assert_eq!(
        saved_hash,
        screenshot_hash,
        "{snapshot_name} looks different; render its demo to see the change, and delete {} to \
         accept it",
        path.display()
    );
}
