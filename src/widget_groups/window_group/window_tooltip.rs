use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{
        nix_store::{Provenance, StorePackage},
        process::{self, Inspection, Launch, Process},
    },
    wayland_layers::tooltip::relative,
    widgets::tooltip_body::{
        Row, Section, Tooltip, TooltipBody, row, section, tree_section, untitled,
    },
};

pub fn tooltip(
    niri_state: &EventStreamState,
    window: u64,
    tree: Option<&Process>,
    inspected: Option<&Inspection>,
) -> Option<Tooltip> {
    let window = niri_state.windows.windows.get(&window)?;
    let mut sections = vec![untitled(identity(window))];
    if let Some(inspected) = inspected {
        sections.extend(packages(&inspected.provenance));
        sections.push(section("Command", command(inspected)));
    }
    if let Some(root) = tree {
        sections.push(tree_section("Process", root.clone()));
    }
    Some(Tooltip::of(TooltipBody::Sections(sections)))
}

fn identity(window: &niri_ipc::Window) -> Vec<Row> {
    let mut rows = vec![row(
        "Title",
        window.title.clone().unwrap_or_else(|| "—".to_owned()),
    )];
    if let Some(app_id) = &window.app_id {
        rows.push(row("App id", app_id.clone()));
    }
    rows.push(row("Window id", window.id.to_string()));
    if let Some(focused) = focus_age(window) {
        rows.push(row(
            "Focused",
            relative(SystemTime::now() - focused, SystemTime::now()),
        ));
    }
    rows
}

fn focus_age(window: &niri_ipc::Window) -> Option<Duration> {
    let focused = window.focus_timestamp?;
    let stamp = Duration::new(focused.secs, focused.nanos);
    process::monotonic()?.checked_sub(stamp)
}

fn versioned(package: &StorePackage) -> Row {
    row(
        &package.name,
        package.version.clone().unwrap_or_else(|| "—".to_owned()),
    )
}

fn packages(provenance: &Provenance) -> Vec<Section> {
    let mut sections = Vec::new();
    if let Some(package) = &provenance.package {
        sections.push(section("Package", vec![versioned(package)]));
    }
    if let Some(runtime) = &provenance.runtime {
        sections.push(section("Runtime", vec![versioned(runtime)]));
    }
    if !provenance.referenced.is_empty() {
        sections.push(section(
            "Referenced",
            provenance.referenced.iter().map(versioned).collect(),
        ));
    }
    sections
}

fn command(inspected: &Inspection) -> Vec<Row> {
    let launched = &inspected.launch;
    let mut rows = Vec::new();
    if let Some(provider) = &inspected.provenance.profile {
        rows.push(row("Profile", provider.clone()));
    }
    if let Some(binary) = &launched.binary {
        rows.push(row("Binary", binary.display().to_string()));
    }
    for (index, argument) in beyond_binary(launched).enumerate() {
        let key = if index == 0 { "Command" } else { "" };
        rows.push(row(key, argument.clone()));
    }
    if let Some(scope) = &launched.scope {
        rows.push(row("Scope", scope.clone()));
    }
    rows
}

fn beyond_binary(launched: &Launch) -> impl Iterator<Item = &String> {
    let echoes_binary = launched
        .binary
        .as_ref()
        .zip(launched.arguments.first())
        .is_some_and(|(binary, first)| binary.as_os_str() == first.as_str());
    launched.arguments.iter().skip(usize::from(echoes_binary))
}
