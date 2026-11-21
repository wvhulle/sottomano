use std::time::{Duration, SystemTime};

use sottomano_compositor_model::{
    compositor_window_facts::{FocusTimestamp, WindowAppId, WindowTitle},
    compositor_window_state::{CompositorWindow, CompositorWindowState},
    monotonic_clock::monotonic_since_boot,
};
use sottomano_desktop_sources::desktop_windows_apps::{
    nix_flake_pin::FlakePin,
    nix_package_facts::{
        Generation, NixPackageFacts, NixpkgsListing, Origin, PackageHomepage, PackageLicense,
        UpstreamState,
    },
    nix_store_provenance::{
        LaunchPackaging, PackageRuntime, PackageVersion, ProfileProvider, StorePackage,
    },
    process_tree_inspection::{CgroupScope, Inspection, Launch, LaunchBinary},
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
        match &window.title {
            WindowTitle::Titled(title) => Some(LabeledValue::new("Title", title.as_str())),
            WindowTitle::Untitled => None,
        },
        match &window.app_id {
            WindowAppId::Reported(app_id) => Some(LabeledValue::new("App id", app_id.as_str())),
            WindowAppId::Unreported => None,
        },
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
    let mut provenance_rows = Vec::new();
    if let LaunchPackaging::Packaged { package, runtime } = &provenance.packaging {
        provenance_rows.push(LabeledValue::new("Package", package.name.as_str()));
        if let PackageRuntime::Separate(runtime) = runtime {
            provenance_rows.push(versioned("Runtime", runtime));
        }
    }
    provenance_rows.extend(
        provenance
            .referenced
            .iter()
            .map(|referenced| versioned("Referenced", referenced)),
    );
    if let ProfileProvider::Provided(profile) = &provenance.profile {
        provenance_rows.push(LabeledValue::new("Profile", profile.as_str()));
    }
    provenance_rows.extend(command(inspection));
    vec![Section::new(
        "Provenance",
        dictionary_table(provenance_rows),
    )]
}

pub fn nix_package_facts_sections(nix_package_facts: &NixPackageFacts) -> Vec<Section> {
    let closure = &nix_package_facts.closure;
    let mut derivation_rows = Vec::new();
    if let NixpkgsListing::Listed(entry) = &nix_package_facts.nixpkgs {
        if let PackageHomepage::Listed(homepage) = &entry.homepage {
            derivation_rows.push(LabeledValue::new(
                "Homepage",
                homepage_link(homepage.clone()),
            ));
        }
        derivation_rows.push(LabeledValue::new(
            "Nix file",
            ValueCell::BrandedLink {
                label: entry.definition.file.clone(),
                url: entry.definition.url.clone(),
                glyph: SimpleIcon::NixOS.into(),
            },
        ));
        if let PackageLicense::Spdx(license) = &entry.license {
            derivation_rows.push(LabeledValue::new("License", license.as_str()));
        }
    }
    let rows = [
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
    derivation_rows.extend(rows.into_iter().flatten());
    derivation_rows.extend(version_entries(nix_package_facts));
    vec![Section::new(
        "Derivation",
        dictionary_table(derivation_rows),
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
    let mut shown_versions: Vec<&str> = match &nix_package_facts.running.version {
        PackageVersion::Versioned(running_version) => vec![running_version.as_str()],
        PackageVersion::Unversioned => Vec::new(),
    };
    let mut entries: Vec<LabeledValue> = shown_versions
        .iter()
        .map(|running_version| LabeledValue::new("Running", *running_version))
        .collect();
    let NixpkgsListing::Listed(nixpkgs) = &nix_package_facts.nixpkgs else {
        return upstream_entries(nix_package_facts, &shown_versions, entries);
    };
    if let Generation::Rebuilt { version } = &nixpkgs.generation {
        let installed_value = if shown_versions.contains(&version.as_str()) {
            "rebuilt, restart to use it".to_owned()
        } else {
            shown_versions.push(version);
            format!("{version}, restart to use it")
        };
        entries.push(LabeledValue::new("Installed", installed_value).level(Severity::Warning));
    }
    if let FlakePin::Moved { version } = &nixpkgs.pin
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
    upstream_entries(nix_package_facts, &shown_versions, entries)
}

fn upstream_entries(
    nix_package_facts: &NixPackageFacts,
    shown_versions: &[&str],
    mut entries: Vec<LabeledValue>,
) -> Vec<LabeledValue> {
    let upstream = &nix_package_facts.upstream;
    if let UpstreamState::Newer { version } = &upstream.state
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
    match window.focused_at_monotonic {
        FocusTimestamp::FocusedAt(focused_at) => monotonic_since_boot().checked_sub(focused_at),
        FocusTimestamp::NeverFocused => None,
    }
}

fn versioned(key: &'static str, package: &StorePackage) -> LabeledValue {
    let named = match &package.version {
        PackageVersion::Versioned(version) => format!("{} {version}", package.name),
        PackageVersion::Unversioned => package.name.clone(),
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
    let mut command_rows = Vec::new();
    if let LaunchBinary::Resolved(binary) = &launched.binary {
        command_rows.push(LabeledValue::new("Binary", ValueCell::Path(binary.clone())));
    }
    command_rows.extend(arguments);
    if let CgroupScope::Unit(scope) = &launched.scope {
        command_rows.push(LabeledValue::new("Scope", scope.as_str()));
    }
    command_rows
}

fn beyond_binary(launched: &Launch) -> impl Iterator<Item = &String> {
    let echoes_binary = match (&launched.binary, launched.arguments.first()) {
        (LaunchBinary::Resolved(binary), Some(first)) => binary.as_os_str() == first.as_str(),
        (LaunchBinary::Resolved(_) | LaunchBinary::Unreadable, _) => false,
    };
    launched.arguments.iter().skip(usize::from(echoes_binary))
}
