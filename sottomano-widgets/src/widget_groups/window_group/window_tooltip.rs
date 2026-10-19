use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;
use sottomano_config::severity::Severity;
use sottomano_sources::desktop_windows_apps::{
    nix_package_facts::{self, Generation, NixPackageFacts, Origin, Upstream, UpstreamState},
    nix_store_provenance::StorePackage,
    process_tree_inspection::{self, Inspection, Launch},
};

use crate::{
    clock::Clock,
    hover_tooltips::relative,
    widgets::tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
};

pub fn pick_body(
    niri_state: &EventStreamState,
    window: u64,
    now: SystemTime,
) -> Option<TooltipBody> {
    let window = niri_state.windows.windows.get(&window)?;
    let window_entries = [
        window
            .title
            .as_deref()
            .map(|title| KeyValue::new("Title", title)),
        window
            .app_id
            .as_deref()
            .map(|app_id| KeyValue::new("App id", app_id)),
        Some(KeyValue::new("Window id", window.id.to_string())),
        focus_age(window).map(|focused| KeyValue::new("Focused", relative(now - focused, now))),
    ];
    Some(TooltipBody::sections([Section::dictionary(
        window_entries.into_iter().flatten(),
    )]))
}

impl From<process_tree_inspection::Error> for Section {
    fn from(error: process_tree_inspection::Error) -> Self {
        Section::error(error)
    }
}

impl From<nix_package_facts::Error> for Section {
    fn from(error: nix_package_facts::Error) -> Self {
        Section::error(error)
    }
}

impl ReadingSections for Inspection {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let provenance = &self.provenance;
        let profile = provenance
            .profile
            .as_deref()
            .map(|profile| KeyValue::new("Profile", profile));
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
        vec![Section::dictionary(packages.chain(command(self))).titled("Package")]
    }
}

impl ReadingSections for NixPackageFacts {
    fn sections(&self, clock: Clock) -> Vec<Section> {
        let nixpkgs = self.nixpkgs.as_ref();
        let closure = &self.closure;
        let rows = [
            nixpkgs
                .and_then(|entry| entry.homepage.clone())
                .map(|homepage| KeyValue::new("Homepage", homepage_link(homepage))),
            nixpkgs.map(|entry| {
                KeyValue::new(
                    "Nix file",
                    Cell::Link {
                        label: entry.definition.file.clone(),
                        url: entry.definition.url.clone(),
                    },
                )
            }),
            nixpkgs
                .and_then(|entry| entry.license.as_deref())
                .map(|license| KeyValue::new("License", license)),
            Some(KeyValue::new(
                "Closure size",
                format!(
                    "{} in {} paths",
                    closure.closure_size.display().iec(),
                    closure.path_count
                ),
            )),
            Some(KeyValue::new(
                "Own size",
                closure.own_size.display().iec().to_string(),
            )),
            Some(KeyValue::new(
                "Added",
                relative(self.registered, clock.wall),
            )),
            origin_entry(&self.origin),
            nixpkgs.and_then(|entry| generation_entry(&entry.generation)),
            self.upstream.as_ref().and_then(upstream_entry),
        ];
        vec![Section::dictionary(rows.into_iter().flatten()).titled("Nix")]
    }
}

fn homepage_link(url: String) -> Cell {
    let label = ["https://", "http://"]
        .into_iter()
        .find_map(|scheme| url.strip_prefix(scheme))
        .unwrap_or(&url)
        .to_owned();
    Cell::Link { label, url }
}

fn origin_entry(origin: &Origin) -> Option<KeyValue> {
    match origin {
        Origin::Signed { .. } => None,
        Origin::BuiltLocally => Some(KeyValue::new("Obtained", "built locally")),
        Origin::Unsigned => Some(KeyValue::new("Obtained", "unsigned").level(Severity::Warning)),
    }
}

fn generation_entry(generation: &Generation) -> Option<KeyValue> {
    match generation {
        Generation::Current => None,
        Generation::Rebuilt { version } => Some(
            KeyValue::new(
                "System generation",
                format!("has {version}, restart to use it"),
            )
            .level(Severity::Warning),
        ),
    }
}

fn upstream_entry(upstream: &Upstream) -> Option<KeyValue> {
    match &upstream.state {
        UpstreamState::UpToDate => None,
        UpstreamState::Newer { version } => Some(
            KeyValue::new(upstream.branch.as_str(), format!("has {version}"))
                .level(Severity::Warning),
        ),
    }
}

fn focus_age(window: &niri_ipc::Window) -> Option<Duration> {
    let focused = window.focus_timestamp?;
    let stamp = Duration::new(focused.secs, focused.nanos);
    process_tree_inspection::monotonic()?.checked_sub(stamp)
}

fn versioned(key: &'static str, package: &StorePackage) -> KeyValue {
    let named = match &package.version {
        Some(version) => format!("{} {version}", package.name),
        None => package.name.clone(),
    };
    KeyValue::new(key, named)
}

fn command(inspected: &Inspection) -> Vec<KeyValue> {
    let launched = &inspected.launch;
    let arguments = beyond_binary(launched)
        .enumerate()
        .map(|(index, argument)| {
            let key = if index == 0 { "Command" } else { "" };
            KeyValue::new(key, argument.as_str())
        });
    let binary = launched
        .binary
        .as_ref()
        .map(|binary| KeyValue::new("Binary", Cell::Path(binary.clone())));
    let scope = launched
        .scope
        .as_deref()
        .map(|scope| KeyValue::new("Scope", scope));
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
