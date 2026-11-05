use std::{
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

const WEIGHTS: [Weight; 6] = [
    Weight {
        variant: "Thin",
        directory: "thin",
        file_suffix: "-thin",
    },
    Weight {
        variant: "Light",
        directory: "light",
        file_suffix: "-light",
    },
    Weight {
        variant: "Regular",
        directory: "regular",
        file_suffix: "",
    },
    Weight {
        variant: "Bold",
        directory: "bold",
        file_suffix: "-bold",
    },
    Weight {
        variant: "Fill",
        directory: "fill",
        file_suffix: "-fill",
    },
    Weight {
        variant: "Duotone",
        directory: "duotone",
        file_suffix: "-duotone",
    },
];

struct Weight {
    variant: &'static str,
    directory: &'static str,
    file_suffix: &'static str,
}

impl Weight {
    fn svg_path(&self, assets_dir: &Path, slug: &str) -> String {
        assets_dir
            .join(self.directory)
            .join(format!("{slug}{}.svg", self.file_suffix))
            .display()
            .to_string()
    }
}

struct Icon {
    variant: String,
    slug: String,
}

impl Icon {
    fn of(slug: &str) -> Self {
        Self {
            variant: pascal_case(slug),
            slug: slug.to_owned(),
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let generated = generate(&assets_dir("PHOSPHOR_SVG"));
    fs::write(out_dir.join("icons.rs"), generated).unwrap();
}

fn generate(assets_dir: &Path) -> String {
    let icons = icons(&assets_dir.join("regular"));
    let mut variants = String::new();
    let mut svg_arms = String::new();
    let mut weight_arms = String::new();
    let mut reweight_arms = String::new();
    for Icon { variant, slug } in &icons {
        writeln!(variants, "    {variant}(Weight),").unwrap();
        writeln!(
            svg_arms,
            "            Phosphor::{variant}(weight) => match weight {{"
        )
        .unwrap();
        for weight in &WEIGHTS {
            writeln!(
                svg_arms,
                "                Weight::{} => include_bytes!(\"{}\"),",
                weight.variant,
                weight.svg_path(assets_dir, slug)
            )
            .unwrap();
        }
        writeln!(svg_arms, "            }},").unwrap();
        writeln!(
            weight_arms,
            "            Phosphor::{variant}(weight) => weight,"
        )
        .unwrap();
        writeln!(
            reweight_arms,
            "            Phosphor::{variant}(_) => Phosphor::{variant}(weight),"
        )
        .unwrap();
    }
    format!(
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names)]
pub enum Phosphor {{
{variants}}}

impl Phosphor {{
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}

    #[must_use]
    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    pub const fn weight(self) -> Weight {{
        match self {{
{weight_arms}        }}
    }}

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn with_weight(self, weight: Weight) -> Self {{
        match self {{
{reweight_arms}        }}
    }}
}}
"
    )
}

fn icons(regular_dir: &Path) -> Vec<Icon> {
    svg_slugs(regular_dir)
        .iter()
        .map(|slug| Icon::of(slug))
        .collect()
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
