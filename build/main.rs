mod font_awesome;
mod weather_icons;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    weather_icons::fit_to_font_awesome_cloud(
        &icon_source("WEATHER_ICONS_SVG"),
        &out_dir.join("weather-icons"),
    );

    fs::write(
        out_dir.join("icons.rs"),
        font_awesome::generate(&icon_source("FONT_AWESOME_SVG")),
    )
    .unwrap();
}

fn icon_source(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} is set by the flake dev shell"))
        .into()
}
