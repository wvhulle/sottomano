use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Write,
    fs,
    path::Path,
};

use serde_json::Value;

use crate::{capitalized, pascal_case, svg_slugs};

const SYMBOL_WORDS: [(char, &str); 4] = [('+', "Plus"), ('.', "Dot"), ('&', "And"), ('#', "Sharp")];

fn svg_title(svg: &str) -> &str {
    let (_, after_open) = svg.split_once("<title>").unwrap();
    let (title, _) = after_open.split_once("</title>").unwrap();
    title
}

fn decoded_xml_text(text: &str) -> String {
    let mut decoded = String::new();
    let mut rest = text;
    while let Some((before, after_ampersand)) = rest.split_once('&') {
        decoded.push_str(before);
        let (entity, after_entity) = after_ampersand.split_once(';').unwrap();
        decoded.push(match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => entity
                .strip_prefix("#x")
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32)
                .unwrap_or_else(|| panic!("unknown XML entity &{entity};")),
        });
        rest = after_entity;
    }
    decoded.push_str(rest);
    decoded
}

fn spelled_title_character(character: char) -> Option<Cow<'static, str>> {
    SYMBOL_WORDS
        .iter()
        .find(|(symbol, _)| *symbol == character)
        .map(|(_, word)| Cow::Borrowed(*word))
        .or_else(|| {
            character
                .is_alphanumeric()
                .then(|| Cow::Owned(character.to_string()))
        })
}

fn title_variant_name(title: &str, slug: &str) -> String {
    let mut name: String = title.chars().filter_map(spelled_title_character).collect();
    if let Some((_, variant_suffix)) = slug.split_once('_') {
        name.push_str(&pascal_case(variant_suffix));
    }
    if name.starts_with(|first: char| first.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

fn titled_arms(
    data_path: &Path,
    variant_of_slug: &HashMap<String, String>,
    variants_of_title: &HashMap<String, Vec<String>>,
) -> String {
    println!("cargo:rerun-if-changed={}", data_path.display());
    let icons: Value = serde_json::from_str(&fs::read_to_string(data_path).unwrap()).unwrap();
    let mut variants_of_name: BTreeMap<&str, BTreeSet<Option<&str>>> = BTreeMap::new();
    for icon in icons.as_array().unwrap() {
        let variant = match icon["slug"].as_str() {
            Some(slug) => variant_of_slug.get(slug),
            None => icon["title"]
                .as_str()
                .and_then(|title| variants_of_title.get(title))
                .filter(|variants| variants.len() == 1)
                .and_then(|variants| variants.first()),
        };
        if let Some(title) = icon["title"].as_str() {
            variants_of_name
                .entry(title)
                .or_default()
                .insert(variant.map(String::as_str));
        }
    }
    variants_of_name
        .into_iter()
        .filter(|(_, variants)| variants.len() == 1)
        .filter_map(|(name, variants)| {
            let variant = (*variants.first()?)?;
            Some(format!(
                "            {name:?} => Some(SimpleIcon::{variant}),\n"
            ))
        })
        .collect()
}

pub fn generate(icons_dir: &Path) -> String {
    let mut taken_names = HashSet::new();
    let mut variants = String::new();
    let mut svg_arms = String::new();
    let mut variant_of_slug = HashMap::new();
    let mut variants_of_title: HashMap<String, Vec<String>> = HashMap::new();
    for slug in svg_slugs(icons_dir) {
        let svg_path = icons_dir.join(format!("{slug}.svg"));
        let title = decoded_xml_text(svg_title(&fs::read_to_string(&svg_path).unwrap()));
        let mut variant = title_variant_name(&title, &slug);
        if !taken_names.insert(variant.clone()) {
            variant = capitalized(&slug);
            assert!(
                taken_names.insert(variant.clone()),
                "simple icon {slug} has no unique variant name"
            );
        }
        variant_of_slug.insert(slug.clone(), variant.clone());
        variants_of_title
            .entry(title.clone())
            .or_default()
            .push(variant.clone());
        writeln!(variants, "    {variant},").unwrap();
        writeln!(
            svg_arms,
            "            SimpleIcon::{variant} => include_bytes!(\"{}\"),",
            svg_path.display()
        )
        .unwrap();
    }
    let data_path = icons_dir.parent().unwrap().join("data/simple-icons.json");
    let titled_arms = titled_arms(&data_path, &variant_of_slug, &variants_of_title);
    format!(
        r"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum SimpleIcon {{
{variants}}}

impl SimpleIcon {{
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn svg_data(self) -> &'static [u8] {{
        match self {{
{svg_arms}        }}
    }}

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn titled(name: &str) -> Option<Self> {{
        match name {{
{titled_arms}            _ => None,
        }}
    }}
}}
"
    )
}
