use super::super::source::command;

const COPY_PROGRAM: &str = "wl-copy";

pub fn write(text: &str) {
    if text.is_empty() {
        return;
    }
    if !command::blocking::run(COPY_PROGRAM, &["--trim-newline", "--", text]) {
        eprintln!(
            "clipboard: {COPY_PROGRAM} failed to copy {} bytes",
            text.len()
        );
    }
}
