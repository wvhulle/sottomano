use std::time::{Duration, SystemTime};

use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_window_state::{CompositorWindow, CompositorWindowState},
    desktop_windows_apps::{
        nix_package_facts::{Generation, NixPackageFacts, Origin, Upstream, UpstreamState},
        nix_store_provenance::StorePackage,
        process_tree_inspection::{self, Inspection, Launch},
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        localized_quantities::Quantity,
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub fn pick_body(
    compositor: &CompositorWindowState,
    window: u64,
    now: SystemTime,
) -> Option<HoverCardBody> {
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
    Some(HoverCardBody::sections([Section::dictionary(
        window_entries.into_iter().flatten(),
    )]))
}

impl ReadingSections for Inspection {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let provenance = &self.provenance;
        let profile = provenance
            .profile
            .as_deref()
            .map(|profile| LabeledValue::new("Profile", profile));
        let packages = provenance
            .package
            .as_ref()
            .map(|package| versioned("Package", package))
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
        vec![Section::dictionary(packages.chain(command(self)))]
    }
}

impl ReadingSections for NixPackageFacts {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let nixpkgs = self.nixpkgs.as_ref();
        let closure = &self.closure;
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
                ValueCell::RelativeMoment(self.registered.into()),
            )),
            origin_entry(&self.origin),
            nixpkgs.and_then(|entry| generation_entry(&entry.generation)),
            self.upstream.as_ref().and_then(upstream_entry),
        ];
        vec![Section::dictionary(rows.into_iter().flatten())]
    }
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

fn generation_entry(generation: &Generation) -> Option<LabeledValue> {
    match generation {
        Generation::Current => None,
        Generation::Rebuilt { version } => Some(
            LabeledValue::new(
                "System generation",
                format!("has {version}, restart to use it"),
            )
            .level(Severity::Warning),
        ),
    }
}

fn upstream_entry(upstream: &Upstream) -> Option<LabeledValue> {
    match &upstream.state {
        UpstreamState::UpToDate => None,
        UpstreamState::Newer { version } => Some(
            LabeledValue::new(upstream.branch.as_str(), format!("has {version}"))
                .level(Severity::Warning),
        ),
    }
}

fn focus_age(window: &CompositorWindow) -> Option<Duration> {
    process_tree_inspection::monotonic()?.checked_sub(window.focused_at_monotonic?)
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
