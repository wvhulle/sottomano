use vetrata_config::Config;
use vetrata_widget_showcase::showcase_program::ShowcaseProgram;

#[test]
fn toast_scenarios_play_through() {
    let scenarios = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ice");
    if let Err(failure) = iced_test::run(ShowcaseProgram::new(&Config::fallback()), scenarios) {
        panic!("{failure:?}");
    }
}
