mod compute_fixture;
mod gif_recording;
mod scene_script;
mod showcase_program;
mod showcased_group;

#[test]
#[ignore = "renders the README GIFs into docs/"]
fn record_readme_showcase_gifs() {
    scene_script::SCENES.iter().for_each(gif_recording::record);
}
