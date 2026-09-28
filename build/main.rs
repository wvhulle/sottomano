mod phosphor;
mod weather_icons;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    weather_icons::fit_to_phosphor_cloud_rain(
        &icon_source("WEATHER_ICONS_SVG"),
        &out_dir.join("weather-icons"),
    );

    fs::write(
        out_dir.join("icons.rs"),
        phosphor::generate(&icon_source("PHOSPHOR_SVG")),
    )
    .unwrap();
}

fn icon_source(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} is set by the flake dev shell"))
        .into()
}
