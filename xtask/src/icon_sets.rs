use std::{
    env,
    error::Error,
    fmt,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    process,
    process::ExitCode,
};

use git2::{Repository, build::CheckoutBuilder};

const FLAKE: &str = "flake.nix";
const ICON_SET_CACHE: &str = "icon-sets";

#[derive(Debug)]
enum IconSetCheckoutError {
    Clone {
        url: String,
        source: git2::Error,
    },
    Checkout {
        revision: String,
        source: git2::Error,
    },
}

impl fmt::Display for IconSetCheckoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconSetCheckoutError::Clone { url, source } => {
                write!(formatter, "cloning {url} failed: {source}")
            }
            IconSetCheckoutError::Checkout { revision, source } => {
                write!(formatter, "checking out {revision} failed: {source}")
            }
        }
    }
}

impl Error for IconSetCheckoutError {}

struct IconSet {
    name: &'static str,
    url: &'static str,
    revision: String,
    svg_directory: &'static str,
    export: &'static str,
}

impl IconSet {
    fn svg_directory(&self, cache_root: &Path) -> PathBuf {
        cache_root.join(self.name).join(self.svg_directory)
    }
}

const ICON_SET_NAMES: [&str; 3] = ["phosphor-core", "simple-icons", "weather-icons"];
const ICON_SET_URLS: [&str; 3] = [
    "https://github.com/phosphor-icons/core",
    "https://github.com/simple-icons/simple-icons",
    "https://github.com/erikflowers/weather-icons",
];
const ICON_SET_SVG_DIRECTORIES: [&str; 3] = ["assets", "icons", "svg"];
const ICON_SET_EXPORTS: [&str; 3] = ["PHOSPHOR_SVG", "SIMPLE_ICONS_SVG", "WEATHER_ICONS_SVG"];

fn icon_sets_from_flake(flake_contents: &str) -> Result<Vec<IconSet>, Box<dyn Error>> {
    let mut icon_sets = Vec::new();
    for (index, name) in ICON_SET_NAMES.iter().enumerate() {
        let owned_revision = nixpkgs_package_version("weather-icons")?;
        let revision = match *name {
            "phosphor-core" => flake_fetch_rev(
                flake_contents,
                "phosphorCore = pkgs.fetchFromGitHub",
                "phosphorCore rev",
            )?,
            "simple-icons" => flake_fetch_rev(
                flake_contents,
                "simpleIcons = pkgs.fetchFromGitHub",
                "simpleIcons rev",
            )?,
            _ => owned_revision.as_str(),
        };
        icon_sets.push(IconSet {
            name,
            url: ICON_SET_URLS[index],
            revision: revision.to_string(),
            svg_directory: ICON_SET_SVG_DIRECTORIES[index],
            export: ICON_SET_EXPORTS[index],
        });
    }
    Ok(icon_sets)
}

fn flake_fetch_rev<'contents>(
    flake_contents: &'contents str,
    fetch_block_marker: &str,
    rev_line_name: &str,
) -> Result<&'contents str, Box<dyn Error>> {
    let block_offset = flake_contents
        .find(fetch_block_marker)
        .ok_or(format!("{FLAKE} has no {rev_line_name} line"))?;
    let block = &flake_contents[block_offset..];
    let rev_offset = block
        .find("rev = \"")
        .ok_or(format!("{FLAKE} has no {rev_line_name} line"))?;
    let rev_line = &block[rev_offset + "rev = \"".len()..];
    let closing_quote_offset = rev_line
        .find('"')
        .ok_or(format!("{FLAKE} has no {rev_line_name} line"))?;
    Ok(&rev_line[..closing_quote_offset])
}

fn nixpkgs_package_version(package_name: &str) -> Result<String, Box<dyn Error>> {
    let output = process::Command::new("nix")
        .args([
            "eval",
            "--raw",
            "--inputs-from",
            ".",
            &format!("nixpkgs#{package_name}.version"),
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "nix eval nixpkgs#{package_name}.version failed; the flake pins weather-icons \n             through nixpkgs, so the revision needs nix to read",
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn icon_set_cache_directory() -> Result<PathBuf, Box<dyn Error>> {
    let cache_root = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok_or("no HOME and no XDG_CACHE_HOME")?;
    Ok(cache_root.join("sottomano").join(ICON_SET_CACHE))
}

fn checkout_revision(repository: &Repository, revision: &str) -> Result<(), IconSetCheckoutError> {
    let checkout_error = |source: git2::Error| IconSetCheckoutError::Checkout {
        revision: revision.to_string(),
        source,
    };
    let commit_object = repository
        .revparse_single(revision)
        .map_err(checkout_error)?;
    let commit = commit_object.peel_to_commit().map_err(checkout_error)?;
    let commit_tree = commit.tree().map_err(checkout_error)?;
    let mut checkout_options = CheckoutBuilder::new();
    checkout_options.force();
    repository
        .checkout_tree(commit_tree.as_object(), Some(&mut checkout_options))
        .map_err(checkout_error)
}

pub fn fetch_icon_sets() -> Result<ExitCode, Box<dyn Error>> {
    let flake_contents = fs::read_to_string(FLAKE)?;
    let icon_sets = icon_sets_from_flake(&flake_contents)?;
    let cache_directory = icon_set_cache_directory()?;
    fs::create_dir_all(&cache_directory)?;
    let mut exports = String::new();
    for icon_set in icon_sets {
        let destination = cache_directory.join(icon_set.name);
        println!(
            "fetching {} at {} into {}",
            icon_set.name,
            icon_set.revision,
            destination.display()
        );
        if destination.join(".git").exists() {
            let repository = Repository::open(&destination)?;
            repository.find_remote("origin")?.fetch(
                &[
                    "+refs/heads/*:refs/remotes/origin/*",
                    "+refs/tags/*:refs/tags/*",
                ],
                None,
                None,
            )?;
        } else {
            Repository::clone(icon_set.url, &destination).map_err(|source| {
                IconSetCheckoutError::Clone {
                    url: icon_set.url.to_string(),
                    source,
                }
            })?;
        }
        let repository = Repository::open(&destination)?;
        checkout_revision(&repository, &icon_set.revision)?;
        let svg_directory = icon_set.svg_directory(&cache_directory);
        if !svg_directory.is_dir() {
            return Err(format!(
                "{} {} has no svg directory at {}",
                icon_set.name,
                icon_set.revision,
                svg_directory.display()
            )
            .into());
        }
        writeln!(
            exports,
            "export {}={}",
            icon_set.export,
            svg_directory.display()
        )?;
    }
    print!("{exports}");
    Ok(ExitCode::SUCCESS)
}
