use std::{collections::HashSet, fmt::Write, fs, path::Path};

const DIGIT_WORDS: [&str; 10] = [
    "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
];

const STYLES: [Style; 3] = [
    Style {
        directory: "solid",
        enum_name: "Solid",
    },
    Style {
        directory: "regular",
        enum_name: "Regular",
    },
    Style {
        directory: "brands",
        enum_name: "Brands",
    },
];

struct Style {
    directory: &'static str,
    enum_name: &'static str,
}

pub fn generate(source_dir: &Path) -> String {
    STYLES
        .iter()
        .map(|style| {
            let style_dir = source_dir.join(style.directory);
            face(style.enum_name, &svg_slugs(&style_dir), &style_dir)
        })
        .collect()
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

fn face(enum_name: &str, slugs: &[String], svg_dir: &Path) -> String {
    let mut variants = String::new();
    let mut slug_arms = String::new();
    let mut svg_arms = String::new();
    let mut seen = HashSet::new();
    for slug in slugs {
        let ident = to_ident(slug);
        assert!(
            ident.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'),
            "slug {slug:?} produced invalid Rust identifier {ident:?}",
        );
        assert!(
            seen.insert(ident.clone()),
            "slug {slug:?} produced duplicate identifier {ident:?} in {enum_name}",
        );
        let svg_path = svg_dir.join(format!("{slug}.svg"));
        writeln!(variants, "    {ident},").unwrap();
        writeln!(
            slug_arms,
            "            {slug:?} => Some({enum_name}::{ident}),"
        )
        .unwrap();
        writeln!(
            svg_arms,
            "            {enum_name}::{ident} => include_bytes!(\"{}\"),",
            svg_path.display()
        )
        .unwrap();
    }
    format!(
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum {enum_name} {{
{variants}}}

#[allow(dead_code)]
impl {enum_name} {{
    #[allow(clippy::too_many_lines)]
    pub fn from_slug(slug: &str) -> Option<{enum_name}> {{
        match slug {{
{slug_arms}            _ => None,
        }}
    }}

    #[allow(clippy::too_many_lines)]
    pub const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}
}}

",
    )
}
