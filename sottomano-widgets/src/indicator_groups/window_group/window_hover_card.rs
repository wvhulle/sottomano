use std::time::{Duration, SystemTime};

use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_window_state::{CompositorWindow, CompositorWindowState},
    desktop_windows_apps::{
        nix_flake_pin::FlakePin,
        nix_package_facts::{Generation, NixPackageFacts, Origin, UpstreamState},
        nix_store_provenance::StorePackage,
        process_tree_inspection::{self, Inspection, Launch},
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::simple_icons::SimpleIcon,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub fn window_identity_body(
    compositor: &CompositorWindowState,
    window: u64,
    now: SystemTime,
) -> Option<Vec<Section>> {
    let window = compositor.windows.get(&window)?;
    let window_entries = [
        window
            .title
            .as_deref()
            .map(|title| LabeledValue::new("Title", title)),
        window
            .app_id
            .as_deref()
            .map(|app_id| LabeledValue::new("App id", app_id)),
        Some(LabeledValue::new("Window id", window.id.to_string())),
        focus_age(window).map(|focused| {
            LabeledValue::new("Focused", ValueCell::RelativeMoment((now - focused).into()))
        }),
    ];
    Some(vec![Section::new(
        "Window",
        dictionary_table(window_entries.into_iter().flatten()),
    )])
}

pub fn inspection_sections(inspection: &Inspection) -> Vec<Section> {
    let provenance = &inspection.provenance;
    let profile = provenance
        .profile
        .as_deref()
        .map(|profile| LabeledValue::new("Profile", profile));
    let packages = provenance
        .package
        .as_ref()
        .map(|package| LabeledValue::new("Package", package.name.as_str()))
        .into_iter()
        .chain(
            provenance
                .runtime
                .as_ref()
                .map(|runtime| versioned("Runtime", runtime)),
        )
        .chain(
            provenance
                .referenced
                .iter()
                .map(|referenced| versioned("Referenced", referenced)),
        )
        .chain(profile);
    vec![Section::new(
        "Provenance",
        dictionary_table(packages.chain(command(inspection))),
    )]
}

pub fn nix_package_facts_sections(nix_package_facts: &NixPackageFacts) -> Vec<Section> {
    let nixpkgs = nix_package_facts.nixpkgs.as_ref();
    let closure = &nix_package_facts.closure;
    let rows = [
        nixpkgs
            .and_then(|entry| entry.homepage.clone())
            .map(|homepage| LabeledValue::new("Homepage", homepage_link(homepage))),
        nixpkgs.map(|entry| {
            LabeledValue::new(
                "Nix file",
                ValueCell::BrandedLink {
                    label: entry.definition.file.clone(),
                    url: entry.definition.url.clone(),
                    glyph: SimpleIcon::NixOS.into(),
                },
            )
        }),
        nixpkgs
            .and_then(|entry| entry.license.as_deref())
            .map(|license| LabeledValue::new("License", license)),
        Some(LabeledValue::new(
            "Closure size",
            format!(
                "{} in {} paths",
                closure.closure_size.display().iec(),
                closure.path_count
            ),
        )),
        Some(LabeledValue::new(
            "Own size",
            Quantity::size(closure.own_size),
        )),
        Some(LabeledValue::new(
            "Added",
            ValueCell::RelativeMoment(nix_package_facts.registered.into()),
        )),
        origin_entry(&nix_package_facts.origin),
    ];
    vec![Section::new(
        "Derivation",
        dictionary_table(
            rows.into_iter()
                .flatten()
                .chain(version_entries(nix_package_facts)),
        ),
    )]
}

fn homepage_link(url: String) -> ValueCell {
    let label = ["https://", "http://"]
        .into_iter()
        .find_map(|scheme| url.strip_prefix(scheme))
        .unwrap_or(&url)
        .to_owned();
    ValueCell::Link { label, url }
}

fn origin_entry(origin: &Origin) -> Option<LabeledValue> {
    match origin {
        Origin::Signed { .. } => None,
        Origin::BuiltLocally => Some(LabeledValue::new("Obtained", "built locally")),
        Origin::Unsigned => {
            Some(LabeledValue::new("Obtained", "unsigned").level(Severity::Warning))
        }
    }
}

fn version_entries(nix_package_facts: &NixPackageFacts) -> Vec<LabeledValue> {
    let mut shown_versions: Vec<&str> = nix_package_facts
        .running
        .version
        .as_deref()
        .into_iter()
        .collect();
    let mut entries: Vec<LabeledValue> = shown_versions
        .iter()
        .map(|running_version| LabeledValue::new("Running", *running_version))
        .collect();
    let nixpkgs = nix_package_facts.nixpkgs.as_ref();
    if let Some(Generation::Rebuilt { version }) = nixpkgs.map(|entry| &entry.generation) {
        let installed_value = if shown_versions.contains(&version.as_str()) {
            "rebuilt, restart to use it".to_owned()
        } else {
            shown_versions.push(version);
            format!("{version}, restart to use it")
        };
        entries.push(LabeledValue::new("Installed", installed_value).level(Severity::Warning));
    }
    if let Some(FlakePin::Moved { version }) = nixpkgs.map(|entry| &entry.pin)
        && !shown_versions.contains(&version.as_str())
    {
        shown_versions.push(version);
        entries.push(
            LabeledValue::new(
                "Pinned in flake.lock",
                format!("{version}, rebuild to install it"),
            )
            .level(Severity::Warning),
        );
    }
    if let Some(upstream) = &nix_package_facts.upstream
        && let UpstreamState::Newer { version } = &upstream.state
        && !shown_versions.contains(&version.as_str())
    {
        entries.push(
            LabeledValue::new(format!("Latest in {}", upstream.branch), version.as_str())
                .level(Severity::Warning),
        );
    }
    entries
}

fn focus_age(window: &CompositorWindow) -> Option<Duration> {
    process_tree_inspection::monotonic().checked_sub(window.focused_at_monotonic?)
}

fn versioned(key: &'static str, package: &StorePackage) -> LabeledValue {
    let named = match &package.version {
        Some(version) => format!("{} {version}", package.name),
        None => package.name.clone(),
    };
    LabeledValue::new(key, named)
}

fn command(inspected: &Inspection) -> Vec<LabeledValue> {
    let launched = &inspected.launch;
    let arguments = beyond_binary(launched)
        .enumerate()
        .map(|(index, argument)| {
            let key = if index == 0 { "Command" } else { "" };
            LabeledValue::new(key, argument.as_str())
        });
    let binary = launched
        .binary
        .as_ref()
        .map(|binary| LabeledValue::new("Binary", ValueCell::Path(binary.clone())));
    let scope = launched
        .scope
        .as_deref()
        .map(|scope| LabeledValue::new("Scope", scope));
    binary.into_iter().chain(arguments).chain(scope).collect()
}

fn beyond_binary(launched: &Launch) -> impl Iterator<Item = &String> {
    let echoes_binary = launched
        .binary
        .as_ref()
        .zip(launched.arguments.first())
        .is_some_and(|(binary, first)| binary.as_os_str() == first.as_str());
    launched.arguments.iter().skip(usize::from(echoes_binary))
}
