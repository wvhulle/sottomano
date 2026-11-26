use sottomano_toml_config::Config;
use sottomano_widget_scenes::showcase_program::ShowcaseProgram;

fn play_ice_scenarios(ui_part_directory: &str) {
    let scenarios = format!(
        "{}/tests/ice/{ui_part_directory}",
        env!("CARGO_MANIFEST_DIR")
    );
    if let Err(failure) =
        iced_test::run(ShowcaseProgram::interactive(&Config::default()), scenarios)
    {
        panic!("{failure:?}");
    }
}

#[test]
fn toast_scenarios_play_through() {
    play_ice_scenarios("toasts");
}
