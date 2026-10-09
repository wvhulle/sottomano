use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{
        nix_store::StorePackage,
        process::{self, Inspection, Launch, Process},
    },
    desktop_shell::hover_tooltips::relative,
    widget_groups::window_group::process_tree::Folding,
    widgets::tooltip_body::{Row, Section, Tooltip, TooltipBody},
};

pub fn tooltip(
    niri_state: &EventStreamState,
    window: u64,
    tree: Option<&Process>,
    folding: &Folding,
    inspected: Option<&Inspection>,
) -> Option<Tooltip> {
    let window = niri_state.windows.windows.get(&window)?;
    let provenance = inspected.map(|inspected| &inspected.provenance);
    let referenced: Vec<Row> = provenance
        .into_iter()
        .flat_map(|provenance| provenance.referenced.iter().map(versioned))
        .collect();
    Some(Tooltip::of(TooltipBody::sections(&[
        Section::untitled(&[
            (
                "Title",
                Some(window.title.clone().unwrap_or_else(|| "—".to_owned())),
            ),
            ("App id", window.app_id.clone()),
            ("Window id", Some(window.id.to_string())),
            (
                "Focused",
                focus_age(window)
                    .map(|focused| relative(SystemTime::now() - focused, SystemTime::now())),
            ),
        ]),
        Section::titled(
            "Package",
            provenance
                .and_then(|provenance| provenance.package.as_ref())
                .map(versioned)
                .as_slice(),
        ),
        Section::titled(
            "Runtime",
            provenance
                .and_then(|provenance| provenance.runtime.as_ref())
                .map(versioned)
                .as_slice(),
        ),
        Section::titled("Referenced", &referenced),
        Section::titled("Command", &inspected.map(command).unwrap_or_default()),
        Section::tree("Process", tree, folding),
    ])))
}

fn focus_age(window: &niri_ipc::Window) -> Option<Duration> {
    let focused = window.focus_timestamp?;
    let stamp = Duration::new(focused.secs, focused.nanos);
    process::monotonic()?.checked_sub(stamp)
}

fn versioned(package: &StorePackage) -> Row {
    (
        package.name.as_str(),
        package.version.clone().unwrap_or_else(|| "—".to_owned()),
    )
        .into()
}

fn command(inspected: &Inspection) -> Vec<Row> {
    let launched = &inspected.launch;
    let arguments = beyond_binary(launched)
        .enumerate()
        .map(|(index, argument)| {
            let key = if index == 0 { "Command" } else { "" };
            Row::from((key, argument))
        });
    [
        ("Profile", inspected.provenance.profile.clone()),
        (
            "Binary",
            launched
                .binary
                .as_ref()
                .map(|binary| binary.display().to_string()),
        ),
    ]
    .map(Row::from)
    .into_iter()
    .chain(arguments)
    .chain([Row::from(("Scope", &launched.scope))])
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
