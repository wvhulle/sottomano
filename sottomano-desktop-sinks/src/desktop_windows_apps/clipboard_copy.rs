use sottomano_desktop_sources::system_protocol_clients::shell_command_output;

const COPY_PROGRAM: &str = "wl-copy";

pub fn write(text: &str) {
    if text.is_empty() {
        return;
    }
    if let Err(copy_failure) =
        shell_command_output::blocking::run(COPY_PROGRAM, &["--trim-newline", "--", text])
    {
        tracing::warn!("clipboard: copying {} bytes: {copy_failure}", text.len());
    }
}
