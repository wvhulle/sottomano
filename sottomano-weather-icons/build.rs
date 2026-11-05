use std::{
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let generated = generate(&assets_dir("WEATHER_ICONS_SVG"));
    fs::write(out_dir.join("icons.rs"), generated).unwrap();
}

fn generate(assets_dir: &Path) -> String {
    let mut variants = String::new();
    let mut svg_arms = String::new();
    for slug in svg_slugs(assets_dir) {
        let variant = pascal_case(slug.strip_prefix("wi-").unwrap_or(&slug));
        writeln!(variants, "    {variant},").unwrap();
        writeln!(
            svg_arms,
            "            WeatherSvg::{variant} => include_bytes!(\"{}\"),",
            assets_dir.join(format!("{slug}.svg")).display()
        )
        .unwrap();
    }
    format!(
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherSvg {{
{variants}}}

impl WeatherSvg {{
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}
}}
"
    )
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
