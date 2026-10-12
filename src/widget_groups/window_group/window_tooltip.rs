use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{
        nix_store::StorePackage,
        process::{self, Inspection, Launch},
    },
    desktop_shell::hover_tooltips::relative,
    widgets::tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
};

pub fn tooltip(
    niri_state: &EventStreamState,
    window: u64,
    process_sections: &[Section],
) -> Option<Tooltip> {
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
    Some(Tooltip::of(TooltipBody::sections(
        [
            &[Section::dictionary(window_entries.into_iter().flatten())],
            process_sections,
        ]
        .concat(),
    )))
}

pub fn inspection_sections(inspected: &Inspection) -> Vec<Section> {
    let provenance = &inspected.provenance;
    vec![
        Section::dictionary(provenance.package.as_ref().map(versioned)).titled("Package"),
        Section::dictionary(provenance.runtime.as_ref().map(versioned)).titled("Runtime"),
        Section::dictionary(provenance.referenced.iter().map(versioned)).titled("Referenced"),
        Section::dictionary(command(inspected)).titled("Command"),
    ]
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
    let profile = inspected
        .provenance
        .profile
        .as_deref()
        .map(|profile| KeyValue::new("Profile", profile));
    let binary = launched
        .binary
        .as_ref()
        .map(|binary| KeyValue::new("Binary", binary.display().to_string()));
    let scope = launched
        .scope
        .as_deref()
        .map(|scope| KeyValue::new("Scope", scope));
    profile
        .into_iter()
        .chain(binary)
        .chain(arguments)
        .chain(scope)
        .collect()
}

fn beyond_binary(launched: &Launch) -> impl Iterator<Item = &String> {
    let echoes_binary = launched
        .binary
        .as_ref()
        .zip(launched.arguments.first())
        .is_some_and(|(binary, first)| binary.as_os_str() == first.as_str());
    launched.arguments.iter().skip(usize::from(echoes_binary))
}
