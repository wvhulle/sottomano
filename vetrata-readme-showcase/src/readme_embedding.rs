use std::{fs, path::Path};

use crate::{RECORDING_EXTENSION, RECORDINGS_DIRECTORY, WORKSPACE_ROOT};

pub struct RecordedAnimation {
    pub name: &'static str,
    pub logical_width: u32,
}

impl RecordedAnimation {
    fn embedding(&self) -> String {
        format!(
            r#"<img src="{RECORDINGS_DIRECTORY}/{}.{RECORDING_EXTENSION}" width="{}""#,
            self.name, self.logical_width
        )
    }
}

pub fn assert_embedded(recorded: &[RecordedAnimation]) {
    let readme = fs::read_to_string(Path::new(WORKSPACE_ROOT).join("README.md"))
        .expect("README.md is readable");
    let missing: Vec<String> = recorded
        .iter()
        .map(RecordedAnimation::embedding)
        .filter(|embedding| !readme.contains(embedding.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "README.md does not embed these recordings at their logical width:\n{}",
        missing.join("\n")
    );
}
