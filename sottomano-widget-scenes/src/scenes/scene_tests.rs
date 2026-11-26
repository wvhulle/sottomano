use sottomano_toml_config::Config;
use sottomano_widgets::clock::Clock;

use crate::scenes::SCENES;

#[test]
fn every_scene_group_names_its_pinnable_items_apart_from_each_other_and_the_group() {
    let config = Config::default();
    let clock = Clock::real();
    let clashes: Vec<(String, Vec<String>)> = SCENES
        .iter()
        .flat_map(|scene| {
            let showcased_group = (scene.group)(clock);
            showcased_group
                .group_contents(&config, clock)
                .iter()
                .map(|group| (scene.name.to_owned(), group.clashing_indicator_names()))
                .filter(|(_, clashing_names)| !clashing_names.is_empty())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(clashes, Vec::new());
}

#[test]
fn every_scene_card_titles_its_sections_apart_from_the_item_group_and_source() {
    let config = Config::default();
    let clock = Clock::real();
    let clashes: Vec<(String, String, String)> = SCENES
        .iter()
        .flat_map(|scene| {
            let showcased_group = (scene.group)(clock);
            showcased_group
                .group_contents(&config, clock)
                .iter()
                .flat_map(|group| group.card_title_clashes(clock))
                .map(|clash| {
                    (
                        scene.name.to_owned(),
                        clash.indicator_name,
                        clash.clashing_title,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(clashes, Vec::new());
}
