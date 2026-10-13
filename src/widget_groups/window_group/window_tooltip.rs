use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;

use crate::{
    data::{
        severity::Severity,
        source::{
            nix_package_facts::{Generation, NixPackageFacts, Origin, Upstream, UpstreamState},
            nix_store::StorePackage,
            process::{self, Inspection, Launch},
        },
    },
    desktop_shell::hover_tooltips::relative,
    widgets::tooltip_body::{Cell, KeyValue, Section, TooltipBody},
};

pub fn body(niri_state: &EventStreamState, window: u64) -> Option<TooltipBody> {
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
        focus_age(window).map(|focused| {
            KeyValue::new(
                "Focused",
                relative(SystemTime::now() - focused, SystemTime::now()),
            )
        }),
    ];
    Some(TooltipBody::sections([Section::dictionary(
        window_entries.into_iter().flatten(),
    )]))
}

pub fn inspection_sections(inspected: &Inspection) -> Vec<Section> {
    let provenance = &inspected.provenance;
    let profile = provenance
        .profile
        .as_deref()
        .map(|profile| KeyValue::new("Profile", profile));
    vec![
        Section::dictionary(
            provenance
                .package
                .as_ref()
                .map(versioned)
                .into_iter()
                .chain(profile),
        )
        .titled("Package"),
        Section::dictionary(provenance.runtime.as_ref().map(versioned)).titled("Runtime"),
        Section::dictionary(provenance.referenced.iter().map(versioned)).titled("Referenced"),
        Section::dictionary(command(inspected)).titled("Command"),
    ]
}

pub fn package_facts_sections(facts: &NixPackageFacts) -> Vec<Section> {
    let nixpkgs = facts.nixpkgs.as_ref();
    let closure = &facts.closure;
    let identity = [
        nixpkgs
            .and_then(|entry| entry.description.as_deref())
            .map(|description| KeyValue::new("", description)),
        nixpkgs
            .and_then(|entry| entry.license.as_deref())
            .map(|license| KeyValue::new("License", license)),
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
    ];
    let store_path = [
        KeyValue::new("Alone", closure.own_size.display().iec().to_string()),
        KeyValue::new(
            "Closure",
            format!(
                "{} in {} paths",
                closure.closure_size.display().iec(),
                closure.path_count
            ),
        ),
        origin_entry(&facts.origin),
        KeyValue::new("Added", relative(facts.registered, SystemTime::now())),
    ];
    let freshness = [
        nixpkgs.map(|entry| generation_entry(&entry.generation)),
        facts.upstream.as_ref().map(upstream_entry),
    ];
    vec![
        Section::dictionary(identity.into_iter().flatten()).titled("Nixpkgs"),
        Section::dictionary(store_path).titled((
            "Store path",
            "This package alone, and with every path it needs at runtime.",
        )),
        Section::dictionary(freshness.into_iter().flatten()).titled("Freshness"),
    ]
}

fn homepage_link(url: String) -> Cell {
    let label = ["https://", "http://"]
        .into_iter()
        .find_map(|scheme| url.strip_prefix(scheme))
        .unwrap_or(&url)
        .to_owned();
    Cell::Link { label, url }
}

fn origin_entry(origin: &Origin) -> KeyValue {
    match origin {
        Origin::Signed { by } => KeyValue::new("Obtained", format!("signed by {}", by.join(", "))),
        Origin::BuiltLocally => KeyValue::new("Obtained", "built locally"),
        Origin::Unsigned => KeyValue::new("Obtained", "unsigned"),
    }
}

fn generation_entry(generation: &Generation) -> KeyValue {
    match generation {
        Generation::Current => KeyValue::new("System generation", "running"),
        Generation::Rebuilt { version } => KeyValue::new(
            "System generation",
            format!("has {version}, restart to use it"),
        )
        .level(Severity::Warning),
    }
}

fn upstream_entry(upstream: &Upstream) -> KeyValue {
    let branch = upstream.branch.as_str();
    match &upstream.state {
        UpstreamState::UpToDate => KeyValue::new(branch, "same version"),
        UpstreamState::Newer { version } => {
            KeyValue::new(branch, format!("has {version}")).level(Severity::Warning)
        }
    }
}

fn focus_age(window: &niri_ipc::Window) -> Option<Duration> {
    let focused = window.focus_timestamp?;
    let stamp = Duration::new(focused.secs, focused.nanos);
    process::monotonic()?.checked_sub(stamp)
}

fn versioned(package: &StorePackage) -> KeyValue {
    let named = match &package.version {
        Some(version) => format!("{} {version}", package.name),
        None => package.name.clone(),
    };
    KeyValue::new("", named)
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
