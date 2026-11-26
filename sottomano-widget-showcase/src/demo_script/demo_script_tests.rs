use std::panic::{AssertUnwindSafe, catch_unwind};

use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent,
    hover_cards::HoverTarget,
};

use crate::{
    animation_recording::RecordingSession,
    demo_script::{DEMOS, Step},
    showcase_program::booting_clock,
};

#[test]
fn every_demo_group_names_its_pinnable_items_apart_from_each_other_and_the_group() {
    let config = Config::default();
    let clock = Clock::real();
    let clashes: Vec<(String, Vec<String>)> = DEMOS
        .iter()
        .flat_map(|demo| {
            let showcased_group = (demo.group)(clock);
            showcased_group
                .group_contents(&config, clock)
                .iter()
                .map(|group| (demo.name.to_owned(), group.clashing_indicator_names()))
                .filter(|(_, clashing_names)| !clashing_names.is_empty())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(clashes, Vec::new());
}

#[test]
fn every_demo_card_titles_its_sections_apart_from_the_item_group_and_source() {
    let config = Config::default();
    let clock = Clock::real();
    let clashes: Vec<(String, String, String)> = DEMOS
        .iter()
        .flat_map(|demo| {
            let showcased_group = (demo.group)(clock);
            showcased_group
                .group_contents(&config, clock)
                .iter()
                .flat_map(|group| group.card_title_clashes(clock))
                .map(|clash| {
                    (
                        demo.name.to_owned(),
                        clash.indicator_name,
                        clash.clashing_title,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(clashes, Vec::new());
}

#[test]
fn every_scripted_row_is_a_row_of_its_demo_group() {
    let config = Config::default();
    let clock = Clock::real();
    let missing: Vec<(String, String, Vec<String>)> = DEMOS
        .iter()
        .flat_map(|demo| {
            let showcased_group = (demo.group)(clock);
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
                RecordingSession::open(start).rehearse(demo, start);
            }))
            .is_err()
        })
        .map(|demo| demo.name)
        .collect();
    assert_eq!(unreachable_demos, Vec::<&str>::new());
}
