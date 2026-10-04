use std::{fmt::Write, fs, path::Path};

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
            variant: slug.split('-').map(capitalized).collect(),
            slug: slug.to_owned(),
        }
    }
}

fn capitalized(segment: &str) -> String {
    let mut chars = segment.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

pub fn generate(assets_dir: &Path) -> String {
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
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Phosphor {{
{variants}}}

impl Phosphor {{
    #[allow(clippy::too_many_lines)]
    pub const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}

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
    let mut slugs: Vec<String> = fs::read_dir(regular_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    slugs.sort_unstable();
    slugs.iter().map(|slug| Icon::of(slug)).collect()
}
