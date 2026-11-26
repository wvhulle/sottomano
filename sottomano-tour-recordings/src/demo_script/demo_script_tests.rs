use std::panic::{AssertUnwindSafe, catch_unwind};

use sottomano_toml_config::Config;
use sottomano_widget_scenes::{
    script_steps::Step, scripted_session::ScriptedSession, showcase_program::booting_clock,
};
use sottomano_widgets::{
    clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent,
    hover_cards::HoverTarget,
};

use crate::demo_script::DEMOS;

#[test]
fn every_scripted_row_is_a_row_of_its_demo_group() {
    let config = Config::default();
    let clock = Clock::real();
    let missing: Vec<(String, String, Vec<String>)> = DEMOS
        .iter()
        .flat_map(|demo| {
            let showcased_group = (demo.scene.group)(clock);
            let contents = showcased_group.group_contents(&config, clock);
            let known_rows: Vec<String> = contents
                .iter()
                .flat_map(IndicatorGroupContent::indicator_keys)
                .map(|indicator_key| format!("{indicator_key:?}"))
                .collect();
            demo.script
                .iter()
                .filter_map(|step| match step {
                    Step::Hover(HoverTarget::Indicator(indicator_key))
                    | Step::Press(HoverTarget::Indicator(indicator_key)) => Some(indicator_key),
                    _ => None,
                })
                .filter(|indicator_key| {
                    !contents
                        .iter()
                        .any(|group| group.indicator(indicator_key).is_some())
                })
                .map(|indicator_key| {
                    (
                        demo.name.to_owned(),
                        format!("{indicator_key:?}"),
                        known_rows.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(missing, Vec::new());
}

#[test]
fn every_demo_script_reaches_each_target_on_screen() {
    let start = booting_clock();
    let unreachable_demos: Vec<&str> = DEMOS
        .iter()
        .filter(|demo| {
            catch_unwind(AssertUnwindSafe(|| {
                let mut session = ScriptedSession::start(demo.scene, start);
                session.rest();
                demo.script.iter().for_each(|step| session.perform(step));
            }))
            .is_err()
        })
        .map(|demo| demo.name)
        .collect();
    assert_eq!(unreachable_demos, Vec::<&str>::new());
}
