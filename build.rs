use std::{
    collections::HashSet,
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

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.lock");

    let json_path = metadata_json_path();
    println!("cargo:rerun-if-changed={}", json_path.display());

    let metadata: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&json_path).unwrap()).unwrap();

    let slugs_with_style = |style: &str| -> Vec<String> {
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
    };
    let face = |slugs: &[String], enum_name: &str| -> (String, String, String) {
        let mut variants = String::new();
        let mut name_arms = String::new();
        let mut slug_arms = String::new();
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
        }
        (variants, name_arms, slug_arms)
    };

    let (solid_variants, solid_names, solid_slugs) = face(&slugs_with_style("solid"), "Solid");
    let (brand_variants, brand_names, brand_slugs) = face(&slugs_with_style("brands"), "Brands");

    let code = format!(
        r"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Solid {{
{solid_variants}}}

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

    #[allow(clippy::too_many_lines)]
    pub fn from_slug(slug: &str) -> Option<Solid> {{
        match slug {{
{solid_slugs}            _ => None,
        }}
    }}
}}

impl Brands {{
    #[allow(clippy::too_many_lines)]
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
    );

    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("icons.rs"),
        code,
    )
    .unwrap();
}
