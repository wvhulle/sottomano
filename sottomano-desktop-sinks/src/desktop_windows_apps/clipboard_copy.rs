use sottomano_desktop_sources::system_protocol_clients::shell_command_output;

const COPY_PROGRAM: &str = "wl-copy";

pub fn write(text: &str) -> Result<(), shell_command_output::Error> {
    if text.is_empty() {
        return Ok(());
    }
    shell_command_output::blocking::run(COPY_PROGRAM, &["--trim-newline", "--", text])
}
