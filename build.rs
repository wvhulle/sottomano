use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const DIGIT_WORDS: [&str; 10] = [
    "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
];

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

fn metadata_json_path() -> PathBuf {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("failed to run `cargo metadata`");
    assert!(output.status.success(), "`cargo metadata` failed");

    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let manifest = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "iced_font_awesome")
        .expect("iced_font_awesome not among resolved dependencies")["manifest_path"]
        .as_str()
        .unwrap();

    Path::new(manifest)
        .parent()
        .unwrap()
        .join("assets/font-awesome-7/icons-light.json")
}

fn slugs_with_style(metadata: &serde_json::Value, style: &str) -> Vec<String> {
    let mut slugs: Vec<String> = metadata
        .as_array()
        .unwrap()
        .iter()
        .filter(|icon| {
            icon["styles"]
                .as_array()
                .is_some_and(|s| s.iter().any(|s| s == style))
        })
        .map(|icon| icon["label"].as_str().unwrap().to_owned())
        .collect();
    slugs.sort_unstable();
    slugs.dedup();
    slugs
}

fn code_points(metadata: &serde_json::Value) -> HashMap<String, char> {
    metadata
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|icon| {
            let label = icon["label"].as_str()?.to_owned();
            let code = u32::from_str_radix(icon["unicode"].as_str()?, 16).ok()?;
            Some((label, char::from_u32(code)?))
        })
        .collect()
}

struct Face {
    variants: String,
    name_arms: String,
    slug_arms: String,
    code_arms: String,
}

fn face(slugs: &[String], enum_name: &str, codes: &HashMap<String, char>) -> Face {
    let mut variants = String::new();
    let mut name_arms = String::new();
    let mut slug_arms = String::new();
    let mut code_arms = String::new();
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
        writeln!(variants, "    {ident},").unwrap();
        writeln!(name_arms, "            {enum_name}::{ident} => {slug:?},").unwrap();
        writeln!(
            slug_arms,
            "            {slug:?} => Some({enum_name}::{ident}),"
        )
        .unwrap();
        let code = codes
            .get(slug)
            .unwrap_or_else(|| panic!("slug {slug:?} has no unicode code point"));
        writeln!(code_arms, "            {enum_name}::{ident} => {code:?},").unwrap();
    }
    Face {
        variants,
        name_arms,
        slug_arms,
        code_arms,
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.lock");

    let json_path = metadata_json_path();
    println!("cargo:rerun-if-changed={}", json_path.display());

    let metadata: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();

    let codes = code_points(&metadata);
    let solid = face(&slugs_with_style(&metadata, "solid"), "Solid", &codes);
    let regular = face(&slugs_with_style(&metadata, "regular"), "Regular", &codes);
    let brands = face(&slugs_with_style(&metadata, "brands"), "Brands", &codes);

    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("icons.rs"),
        generated(&solid, &regular, &brands),
    )
    .unwrap();
}

fn generated(solid: &Face, regular: &Face, brands: &Face) -> String {
    let Face {
        variants: solid_variants,
        name_arms: solid_names,
        slug_arms: solid_slugs,
        code_arms: solid_codes,
    } = solid;
    let Face {
        variants: regular_variants,
        name_arms: regular_names,
        slug_arms: regular_slugs,
        code_arms: regular_codes,
    } = regular;
    let Face {
        variants: brand_variants,
        name_arms: brand_names,
        slug_arms: brand_slugs,
        code_arms: brand_codes,
    } = brands;

    format!(
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Solid {{
{solid_variants}}}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Regular {{
{regular_variants}}}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Brands {{
{brand_variants}}}

impl Solid {{
    #[allow(clippy::too_many_lines)]
    pub const fn name(self) -> &'static str {{
        match self {{
{solid_names}        }}
    }}

    #[allow(clippy::too_many_lines, dead_code)]
    pub const fn code(self) -> char {{
        match self {{
{solid_codes}        }}
    }}

    #[allow(clippy::too_many_lines)]
    pub fn from_slug(slug: &str) -> Option<Solid> {{
        match slug {{
{solid_slugs}            _ => None,
        }}
    }}
}}

#[allow(dead_code)]
impl Regular {{
    #[allow(clippy::too_many_lines)]
    pub const fn name(self) -> &'static str {{
        match self {{
{regular_names}        }}
    }}

    #[allow(clippy::too_many_lines)]
    pub const fn code(self) -> char {{
        match self {{
{regular_codes}        }}
    }}

    #[allow(clippy::too_many_lines)]
    pub fn from_slug(slug: &str) -> Option<Regular> {{
        match slug {{
{regular_slugs}            _ => None,
        }}
    }}
}}

impl Brands {{
    #[allow(clippy::too_many_lines, dead_code)]
    pub const fn code(self) -> char {{
        match self {{
{brand_codes}        }}
    }}

    #[allow(clippy::too_many_lines, dead_code)]
    pub const fn name(self) -> &'static str {{
        match self {{
{brand_names}        }}
    }}

    #[allow(clippy::too_many_lines)]
    pub fn from_slug(slug: &str) -> Option<Brands> {{
        match slug {{
{brand_slugs}            _ => None,
        }}
    }}
}}
",
    )
}
