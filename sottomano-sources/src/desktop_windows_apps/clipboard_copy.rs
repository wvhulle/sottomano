use crate::system_protocol_clients::shell_command_output;

const COPY_PROGRAM: &str = "wl-copy";

pub fn write(text: &str) {
    if text.is_empty() {
        return;
    }
    if !shell_command_output::blocking::run(COPY_PROGRAM, &["--trim-newline", "--", text]) {
        tracing::warn!(
            "clipboard: {COPY_PROGRAM} failed to copy {} bytes",
            text.len()
        );
    }
}
