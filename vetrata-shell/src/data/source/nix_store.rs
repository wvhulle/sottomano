use std::{
    env::var,
    fs::{canonicalize, read_link},
    iter::successors,
    path::{Path, PathBuf},
};

const STORE: &str = "/nix/store/";
const HASH_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePackage {
    pub root: PathBuf,
    pub name: String,
    pub version: Option<String>,
}

impl StorePackage {
    fn parse(derivation: &str) -> Option<Self> {
        let named = derivation.get(HASH_LEN + 1..).filter(|_| {
            derivation.len() > HASH_LEN
                && derivation.as_bytes()[HASH_LEN] == b'-'
                && derivation[..HASH_LEN]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric())
        })?;
        let root = Path::new(STORE).join(derivation);
        Some(match version_start(named) {
            Some(start) => Self {
                root,
                name: named[..start].to_owned(),
                version: Some(named[start + 1..].to_owned()),
            },
            None => Self {
                root,
                name: named.to_owned(),
                version: None,
            },
        })
    }

    fn shares_lineage(&self, other: &Self) -> bool {
        self.name.starts_with(&other.name) || other.name.starts_with(&self.name)
    }

    fn identifies(&self, slug: &str) -> bool {
        let name = self.name.to_lowercase();
        name.starts_with(slug) || slug.starts_with(&name)
    }
}

fn version_start(named: &str) -> Option<usize> {
    named
        .bytes()
        .zip(named.bytes().skip(1))
        .position(|(dash, next)| dash == b'-' && !next.is_ascii_alphabetic())
}

fn derivation(path: &str) -> Option<&str> {
    path.strip_prefix(STORE)?
        .split('/')
        .next()
        .filter(|root| !root.is_empty())
}

#[must_use]
pub fn below_derivation(path: &str) -> Option<&str> {
    path.strip_prefix(STORE)?
        .split_once('/')
        .map(|(_, below)| below)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provenance {
    pub package: Option<StorePackage>,
    pub runtime: Option<StorePackage>,
    pub referenced: Vec<StorePackage>,
    pub profile: Option<String>,
}

impl Provenance {
    pub fn of(binary: &Path, arguments: &[String], app_id: Option<&str>) -> Self {
        let executing = package(binary);
        let candidates = candidates(executing.clone(), arguments);
        let identified = app_id.map(app_slug).and_then(|slug| {
            candidates
                .iter()
                .find(|found| found.identifies(&slug))
                .cloned()
        });
        let package = identified.or_else(|| executing.clone());
        let runtime = executing.filter(|exe| {
            package
                .as_ref()
                .is_none_or(|primary| primary.name != exe.name)
        });
        Self {
            referenced: candidates
                .into_iter()
                .filter(|found| {
                    ![package.as_ref(), runtime.as_ref()]
                        .into_iter()
                        .flatten()
                        .any(|shown| shown.name == found.name)
                })
                .collect(),
            profile: profile(binary),
            package,
            runtime,
        }
    }
}

fn candidates(executing: Option<StorePackage>, arguments: &[String]) -> Vec<StorePackage> {
    let mut found: Vec<StorePackage> = executing.into_iter().collect();
    for candidate in arguments.iter().flat_map(|argument| store_paths(argument)) {
        if !found.iter().any(|seen| seen.name == candidate.name) {
            found.push(candidate);
        }
    }
    found
}

fn app_slug(app_id: &str) -> String {
    app_id.rsplit('.').next().unwrap_or(app_id).to_lowercase()
}

fn package(path: &Path) -> Option<StorePackage> {
    derivation(path.to_str()?).and_then(StorePackage::parse)
}

fn store_paths(argument: &str) -> impl Iterator<Item = StorePackage> + '_ {
    argument
        .match_indices(STORE)
        .filter_map(|(at, _)| derivation(&argument[at..]))
        .filter_map(StorePackage::parse)
}

fn profile(exe: &Path) -> Option<String> {
    let installed = package(exe)?;
    let name = command_name(exe)?;
    profile_roots()
        .filter_map(|root| canonicalize(root).ok())
        .find_map(|root| provider(&root, &name, &installed))
}

fn profile_roots() -> impl Iterator<Item = PathBuf> {
    let user = var("USER").ok();
    let home = var("HOME").ok();
    [
        user.map(|user| PathBuf::from(format!("/etc/profiles/per-user/{user}"))),
        home.map(|home| PathBuf::from(home).join(".nix-profile")),
        Some(PathBuf::from("/run/current-system/sw")),
    ]
    .into_iter()
    .flatten()
}

fn provider(root: &Path, name: &str, installed: &StorePackage) -> Option<String> {
    let mut chain = vec![package(root)?];
    for hop in hops(root.join("bin").join(name)).filter_map(|hop| package(&hop)) {
        if chain.last() != Some(&hop) {
            chain.push(hop);
        }
    }
    let terminal = chain.last()?;
    if !terminal.shares_lineage(installed) {
        return None;
    }
    chain
        .get(chain.len().checked_sub(2)?)
        .map(|provider| provider.name.clone())
}

fn hops(start: PathBuf) -> impl Iterator<Item = PathBuf> {
    successors(Some(start), |current| {
        let target = read_link(current).ok()?;
        if target.is_absolute() {
            return Some(target);
        }
        Some(current.parent()?.join(target))
    })
    .skip(1)
}

fn command_name(exe: &Path) -> Option<String> {
    let file = exe.file_name()?.to_str()?;
    let bare = file.strip_prefix('.').unwrap_or(file);
    Some(bare.strip_suffix("-wrapped").unwrap_or(bare).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(path: &str) -> Option<(String, Option<String>)> {
        package(Path::new(path)).map(|found| (found.name, found.version))
    }

    fn store(name: &str, version: Option<&str>) -> (String, Option<String>) {
        (name.to_owned(), version.map(str::to_owned))
    }

    #[test]
    fn package_root_is_the_store_entry_without_the_inner_path() {
        let found = package(Path::new(
            "/nix/store/pbsm8dv4d5nhdmdr494ibwsq393cvka2-kitty-0.48.2/bin/.kitty-wrapped",
        ));
        assert_eq!(
            found.map(|found| found.root),
            Some(PathBuf::from(
                "/nix/store/pbsm8dv4d5nhdmdr494ibwsq393cvka2-kitty-0.48.2"
            ))
        );
        assert_eq!(
            below_derivation("/nix/store/abc-source/pkgs/by-name/ki/kitty/package.nix"),
            Some("pkgs/by-name/ki/kitty/package.nix")
        );
    }

    #[test]
    fn version_splits_at_first_dash_not_followed_by_a_letter() {
        assert_eq!(
            found("/nix/store/pbsm8dv4d5nhdmdr494ibwsq393cvka2-kitty-0.48.2/bin/.kitty-wrapped"),
            Some(store("kitty", Some("0.48.2")))
        );
        assert_eq!(
            found("/nix/store/hgc5zar9zz8lzsxwhyc0m7yw8i68a97z-electron-unwrapped-43.2.0/libexec"),
            Some(store("electron-unwrapped", Some("43.2.0")))
        );
        assert_eq!(
            found("/nix/store/bfqsxlviikq7vlp36kasy5hhamlxlkd2-nodejs-slim-24.19.0/bin/node"),
            Some(store("nodejs-slim", Some("24.19.0")))
        );
        assert_eq!(
            found("/nix/store/65pq3m921w1d0ks0d79i7is78w6pv2r0-nu-inline-diagnostics-0.114.2/bin"),
            Some(store("nu-inline-diagnostics", Some("0.114.2")))
        );
        assert_eq!(
            found("/nix/store/v8llyqw71lygr2llhmcc8ya5bdlzq45v-bash-5.3p9/bin/bash"),
            Some(store("bash", Some("5.3p9")))
        );
        assert_eq!(
            found("/nix/store/0gcdf9qcnxgjbq88wf8grvh217z9iqfh-spotify-1.2.92.147.g5b8f9367/bin"),
            Some(store("spotify", Some("1.2.92.147.g5b8f9367")))
        );
    }

    #[test]
    fn unversioned_derivation_keeps_whole_name() {
        assert_eq!(
            found("/nix/store/r0q93kwqisbgskg5h9iyzkrncz7gb5lm-claude-code/bin/claude"),
            Some(store("claude-code", None))
        );
    }

    #[test]
    fn paths_outside_the_store_have_no_package() {
        assert_eq!(
            found("/home/wvhulle/Code/vetrata/target/debug/vetrata"),
            None
        );
        assert_eq!(found("/nix/store"), None);
    }

    #[test]
    fn wrapper_decoration_is_stripped_from_the_command_name() {
        assert_eq!(
            command_name(Path::new("/nix/store/x/bin/.kitty-wrapped")).as_deref(),
            Some("kitty")
        );
        assert_eq!(
            command_name(Path::new("/nix/store/x/bin/mpd")).as_deref(),
            Some("mpd")
        );
    }
}
