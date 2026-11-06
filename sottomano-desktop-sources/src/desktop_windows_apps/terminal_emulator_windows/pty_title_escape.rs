use std::{
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

fn title_sequence(title: &str) -> String {
    let printable_title: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    format!("\x1b]2;{printable_title}\x07")
}

pub fn write_title(pty_path: &Path, title: &str) -> io::Result<()> {
    let mut pty = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(pty_path)?;
    pty.write_all(title_sequence(title).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::title_sequence;

    #[test]
    fn a_title_is_framed_as_one_operating_system_command() {
        assert_eq!(title_sequence("~/proj"), "\x1b]2;~/proj\x07");
    }

    #[test]
    fn control_characters_cannot_escape_the_title() {
        assert_eq!(
            title_sequence("evil\x07\x1b[2Jtitle"),
            "\x1b]2;evil[2Jtitle\x07"
        );
    }
}
