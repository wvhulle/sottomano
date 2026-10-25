mod phosphor;
mod simple_icons;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=build");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let generated = [
        phosphor::generate(&assets_dir("PHOSPHOR_SVG")),
        simple_icons::generate(&assets_dir("SIMPLE_ICONS_SVG")),
    ]
    .concat();
    fs::write(out_dir.join("icons.rs"), generated).unwrap();
}

fn assets_dir(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} is set by the flake dev shell"))
        .into()
}

fn svg_slugs(directory: &Path) -> Vec<String> {
    let mut slugs: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    slugs.sort_unstable();
    slugs
}

fn pascal_case(slug: &str) -> String {
    slug.split(['-', '_']).map(capitalized).collect()
}

fn capitalized(segment: &str) -> String {
    let mut chars = segment.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}
