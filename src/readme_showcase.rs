use std::env;

mod demo_script;
mod fixtures;
mod gif_recording;
mod hovered_widget;
mod readme_embedding;
mod showcase_program;
mod showcased_group;

const SINGLE_DEMO_VARIABLE: &str = "README_DEMO";

#[test]
#[ignore = "renders the README GIFs into docs/"]
fn record_readme_demo_gifs() {
    let only = env::var(SINGLE_DEMO_VARIABLE).ok();
    let recorded: Vec<_> = demo_script::DEMOS
        .iter()
        .filter(|demo| only.as_deref().is_none_or(|name| name == demo.name))
        .map(gif_recording::record)
        .collect();
    if only.is_none() {
        readme_embedding::assert_embedded(&recorded);
    }
}
