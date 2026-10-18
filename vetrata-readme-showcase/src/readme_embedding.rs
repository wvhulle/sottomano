use std::{fs, path::Path};

use crate::{RECORDINGS_DIRECTORY, WORKSPACE_ROOT};

pub struct RecordedGif {
    pub name: &'static str,
    pub logical_width: u32,
}

impl RecordedGif {
    fn embedding(&self) -> String {
        format!(
            r#"<img src="{RECORDINGS_DIRECTORY}/{}.gif" width="{}""#,
            self.name, self.logical_width
        )
    }
}

pub fn assert_embedded(recorded: &[RecordedGif]) {
    let readme = fs::read_to_string(Path::new(WORKSPACE_ROOT).join("README.md"))
        .expect("README.md is readable");
    let missing: Vec<String> = recorded
        .iter()
        .map(RecordedGif::embedding)
        .filter(|embedding| !readme.contains(embedding.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "README.md does not embed these GIFs at their logical width:\n{}",
        missing.join("\n")
    );
}
