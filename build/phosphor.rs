use std::{collections::HashSet, fmt::Write, fs, path::Path};

const DIGIT_WORDS: [&str; 10] = [
    "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
];

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

pub fn generate(assets_dir: &Path) -> String {
    let slugs = svg_slugs(&assets_dir.join("regular"));
    let mut icon_enums = String::new();
    let mut union_variants = String::new();
    let mut slug_arms = String::new();
    let mut svg_arms = String::new();
    let mut seen = HashSet::new();
    for slug in &slugs {
        let ident = to_ident(slug);
        assert!(
            ident.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'),
            "slug {slug:?} produced invalid Rust identifier {ident:?}",
        );
        assert!(
            seen.insert(ident.clone()),
            "slug {slug:?} produced duplicate identifier {ident:?}",
        );
        icon_enums.push_str(&icon_enum(&ident, slug, assets_dir));
        writeln!(union_variants, "    {ident}({ident}),").unwrap();
        writeln!(
            slug_arms,
            "            {slug:?} => \
             ::core::option::Option::Some(Phosphor::{ident}({ident}::Regular)),"
        )
        .unwrap();
        writeln!(
            svg_arms,
            "            Phosphor::{ident}(icon) => icon.svg_data(),"
        )
        .unwrap();
    }
    format!(
        r"{icon_enums}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phosphor {{
{union_variants}}}

#[allow(dead_code)]
impl Phosphor {{
    #[allow(clippy::too_many_lines)]
    pub fn from_slug(slug: &str) -> ::core::option::Option<Phosphor> {{
        match slug {{
{slug_arms}            _ => ::core::option::Option::None,
        }}
    }}

    #[allow(clippy::too_many_lines)]
    const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}

    pub fn view<'a, Message: 'a>(
        self,
        size: f32,
        color: ::iced::Color,
    ) -> ::iced::Element<'a, Message> {{
        crate::widgets::svg_icon::view(self.svg_data(), size, color)
    }}
}}
",
    )
}

fn icon_enum(ident: &str, slug: &str, assets_dir: &Path) -> String {
    let mut weight_variants = String::new();
    let mut svg_arms = String::new();
    for weight in &WEIGHTS {
        let variant = weight.variant;
        writeln!(weight_variants, "    {variant},").unwrap();
        writeln!(
            svg_arms,
            "            {ident}::{variant} => include_bytes!(\"{}\"),",
            weight.svg_path(assets_dir, slug)
        )
        .unwrap();
    }
    format!(
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum {ident} {{
{weight_variants}}}

#[allow(dead_code)]
impl {ident} {{
    const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}

    pub fn view<'a, Message: 'a>(
        self,
        size: f32,
        color: ::iced::Color,
    ) -> ::iced::Element<'a, Message> {{
        Phosphor::{ident}(self).view(size, color)
    }}
}}

impl ::core::convert::From<{ident}> for Phosphor {{
    fn from(icon: {ident}) -> Self {{
        Phosphor::{ident}(icon)
    }}
}}

impl ::core::convert::From<{ident}> for crate::widgets::bar_widget::Glyph {{
    fn from(icon: {ident}) -> Self {{
        crate::widgets::bar_widget::Glyph::Phosphor(Phosphor::{ident}(icon))
    }}
}}

impl ::core::convert::From<{ident}> for crate::widgets::tooltip_body::Cell {{
    fn from(icon: {ident}) -> Self {{
        crate::widgets::tooltip_body::Cell::Glyph(crate::widgets::bar_widget::Glyph::Phosphor(
            Phosphor::{ident}(icon),
        ))
    }}
}}

"
    )
}

fn svg_slugs(dir: &Path) -> Vec<String> {
    let mut slugs: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    slugs.sort_unstable();
    slugs
}

fn to_ident(slug: &str) -> String {
    if slug.len() == 1
        && let Some(d) = slug.chars().next().and_then(|c| c.to_digit(10))
    {
        return DIGIT_WORDS[d as usize].to_owned();
    }
    let ident: String = slug
        .split('-')
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if ident.starts_with(|c: char| c.is_ascii_digit()) {
        format!("Icon{ident}")
    } else {
        ident
    }
}
