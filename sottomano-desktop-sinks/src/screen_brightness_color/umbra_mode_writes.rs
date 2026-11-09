use sottomano_desktop_sources::{
    screen_brightness_color::solar_switch_schedule::ColorMode,
    system_protocol_clients::shell_command_output,
};

pub async fn toggle_mode() -> Result<(), shell_command_output::Error> {
    shell_command_output::output("umbra", &["toggle"])
        .await
        .map(drop)
}

pub fn set_mode(mode: ColorMode) -> Result<(), shell_command_output::Error> {
    shell_command_output::blocking::run("umbra", &["set", mode.umbra_name()])
}
