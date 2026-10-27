use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bytesize::ByteSize;
use iced_controller_diagram::ControllerArtwork;
use niri_ipc::{
    Event, Window, WindowLayout, Workspace,
    state::{EventStreamState, EventStreamStatePart},
};
use sottomano_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    desktop_windows_apps::{
        kitty_terminal_windows::WindowProcesses,
        niri_window_snapshot::Snapshot,
        nix_package_facts::{
            ClosureFacts, Definition, Generation, NixPackageFacts, NixpkgsEntry, Origin, Upstream,
            UpstreamState,
        },
        nix_store_provenance::{Provenance, StorePackage},
        process_tree_inspection::{Inspection, Launch, Process},
    },
};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::HoverTarget,
    widget_groups::window_group::{
        WindowGroup, WindowGroupMessage, WindowTarget, window_list::WindowListMessage,
    },
    widgets::favicon::Favicons,
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    fixtures::showcase_epoch,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup, ShowcasedNiriSession},
};

const CODING_WORKSPACE: u64 = 1;
const BROWSING_WORKSPACE: u64 = 2;
const CHAT_WORKSPACE: u64 = 3;
const MAIL_WORKSPACE: u64 = 4;
const NOTES_WORKSPACE: u64 = 5;
const DESIGN_WORKSPACE: u64 = 6;
const WRITING_WORKSPACE: u64 = 7;
const VIDEO_WORKSPACE: u64 = 8;
const GAMES_WORKSPACE: u64 = 9;
const BOTTOM_BAR_WORKSPACE_COUNT: usize = 3;
const TERMINAL_WINDOW: u64 = 2;
const TERMINAL_PID: i32 = 4200;
const SHOWCASE_APP_ICONS: &str = env!("SHOWCASE_APP_ICONS");

const NAMED_WORKSPACES: &[NamedWorkspace] = &[
    NamedWorkspace {
        id: CODING_WORKSPACE,
        name: "code",
        active_window: TERMINAL_WINDOW,
    },
    NamedWorkspace {
        id: BROWSING_WORKSPACE,
        name: "web",
        active_window: 4,
    },
    NamedWorkspace {
        id: CHAT_WORKSPACE,
        name: "chat",
        active_window: 5,
    },
    NamedWorkspace {
        id: MAIL_WORKSPACE,
        name: "mail",
        active_window: 6,
    },
    NamedWorkspace {
        id: NOTES_WORKSPACE,
        name: "notes",
        active_window: 7,
    },
    NamedWorkspace {
        id: DESIGN_WORKSPACE,
        name: "design",
        active_window: 8,
    },
    NamedWorkspace {
        id: WRITING_WORKSPACE,
        name: "docs",
        active_window: 10,
    },
    NamedWorkspace {
        id: VIDEO_WORKSPACE,
        name: "video",
        active_window: 11,
    },
    NamedWorkspace {
        id: GAMES_WORKSPACE,
        name: "games",
        active_window: 12,
    },
];

const PLACED_WINDOWS: &[PlacedWindow] = &[
    PlacedWindow {
        id: 1,
        app_id: "org.gnome.Nautilus",
        title: "Pictures",
        workspace: CODING_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: TERMINAL_WINDOW,
        app_id: "foot",
        title: "~/projects/weather-station",
        workspace: CODING_WORKSPACE,
        column: 2,
    },
    PlacedWindow {
        id: 3,
        app_id: "firefox",
        title: "weather-station — Codeberg",
        workspace: BROWSING_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 4,
        app_id: "firefox",
        title: "iced — docs.rs",
        workspace: BROWSING_WORKSPACE,
        column: 2,
    },
    PlacedWindow {
        id: 5,
        app_id: "org.telegram.desktop",
        title: "Telegram",
        workspace: CHAT_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 6,
        app_id: "thunderbird",
        title: "Inbox — Thunderbird",
        workspace: MAIL_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 7,
        app_id: "obsidian",
        title: "Reading list — Obsidian",
        workspace: NOTES_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 8,
        app_id: "org.inkscape.Inkscape",
        title: "logo.svg — Inkscape",
        workspace: DESIGN_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 9,
        app_id: "gimp",
        title: "poster.xcf — GIMP",
        workspace: DESIGN_WORKSPACE,
        column: 2,
    },
    PlacedWindow {
        id: 10,
        app_id: "libreoffice-writer",
        title: "report.odt — LibreOffice Writer",
        workspace: WRITING_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 11,
        app_id: "vlc",
        title: "VLC media player",
        workspace: VIDEO_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: 12,
        app_id: "steam",
        title: "Steam",
        workspace: GAMES_WORKSPACE,
        column: 1,
    },
];

struct NamedWorkspace {
    id: u64,
    name: &'static str,
    active_window: u64,
}

struct PlacedWindow {
    id: u64,
    app_id: &'static str,
    title: &'static str,
    workspace: u64,
    column: usize,
}

pub fn bottom_bar(_clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::BottomBar(Box::new(session(
        &NAMED_WORKSPACES[..BOTTOM_BAR_WORKSPACE_COUNT],
    )))
}

pub fn workspace_sidebar(_clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::WorkspaceSidebar(Box::new(session(NAMED_WORKSPACES)))
}

fn session(shown_workspaces: &[NamedWorkspace]) -> ShowcasedNiriSession {
    let shown_windows: Vec<&PlacedWindow> = PLACED_WINDOWS
        .iter()
        .filter(|placed| {
            shown_workspaces
                .iter()
                .any(|named| named.id == placed.workspace)
        })
        .collect();
    let niri = [workspaces(shown_workspaces), windows(&shown_windows)]
        .into_iter()
        .fold(EventStreamState::default(), |mut state, event| {
            let _ = state.apply(event);
            state
        });
    let snapshot = Snapshot::derive(&niri, &WindowProcesses::default());
    ShowcasedNiriSession {
        window_group: WindowGroup::default(),
        niri,
        snapshot,
        desktop_entries: bundled_app_icons(&shown_windows),
        favicons: Favicons::default(),
        controller_artwork: ControllerArtwork::decoded(),
    }
}

fn bundled_app_icons(shown_windows: &[&PlacedWindow]) -> DesktopIndex {
    DesktopIndex::with_app_icons(shown_windows.iter().map(|placed| {
        (
            placed.app_id.to_owned(),
            Path::new(SHOWCASE_APP_ICONS).join(format!("{}.svg", placed.app_id)),
        )
    }))
}

pub fn on_hover(target: &HoverTarget) -> Vec<WindowGroupMessage> {
    match target {
        HoverTarget::Window(WindowTarget::Inspect(TERMINAL_WINDOW)) => vec![
            WindowGroupMessage::WindowList(WindowListMessage::ProcessTreeScanned(
                SnapshotDelivery::fixture_delivered(
                    Ok(terminal_processes()),
                    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(
                        1,
                    ))),
                ),
            )),
            WindowGroupMessage::WindowList(WindowListMessage::ProcessInspected(Box::new(
                SnapshotDelivery::single_read(Ok(terminal_inspection())),
            ))),
            WindowGroupMessage::WindowList(WindowListMessage::PackageFactsRead(Box::new(
                SnapshotDelivery::single_read(Ok(terminal_package_facts())),
            ))),
        ],
        _ => Vec::new(),
    }
}

fn workspace(named: &NamedWorkspace) -> Workspace {
    Workspace {
        id: named.id,
        idx: u8::try_from(named.id).expect("few fixture workspaces"),
        name: Some(named.name.to_owned()),
        output: Some(SHOWCASE_OUTPUT.to_owned()),
        is_urgent: false,
        is_active: named.id == CODING_WORKSPACE,
        is_focused: named.id == CODING_WORKSPACE,
        active_window_id: Some(named.active_window),
    }
}

fn workspaces(shown_workspaces: &[NamedWorkspace]) -> Event {
    Event::WorkspacesChanged {
        workspaces: shown_workspaces.iter().map(workspace).collect(),
    }
}

fn window(placed: &PlacedWindow) -> Window {
    Window {
        id: placed.id,
        title: Some(placed.title.to_owned()),
        app_id: Some(placed.app_id.to_owned()),
        pid: Some(TERMINAL_PID + i32::try_from(placed.id).expect("few fixture windows") - 2),
        workspace_id: Some(placed.workspace),
        is_focused: placed.id == TERMINAL_WINDOW,
        is_floating: false,
        is_urgent: false,
        layout: WindowLayout {
            pos_in_scrolling_layout: Some((placed.column, 1)),
            tile_size: (800.0, 900.0),
            window_size: (800, 900),
            tile_pos_in_workspace_view: None,
            window_offset_in_tile: (0.0, 0.0),
        },
        focus_timestamp: None,
    }
}

fn windows(shown_windows: &[&PlacedWindow]) -> Event {
    Event::WindowsChanged {
        windows: shown_windows.iter().copied().map(window).collect(),
    }
}

fn process(pid: i32, parent: i32, name: &str, mebibytes: u64, children: Vec<Process>) -> Process {
    Process {
        pid,
        parent,
        name: name.to_owned(),
        memory: ByteSize::mib(mebibytes),
        cpu: Ratio::new::<percent>(0.4),
        threads: 4,
        uptime: Duration::from_hours(3),
        children,
    }
}

fn terminal_processes() -> Process {
    process(
        TERMINAL_PID,
        1,
        "foot",
        42,
        vec![process(
            TERMINAL_PID + 10,
            TERMINAL_PID,
            "nu",
            64,
            vec![process(
                TERMINAL_PID + 11,
                TERMINAL_PID + 10,
                "cargo",
                380,
                vec![],
            )],
        )],
    )
}

fn terminal_inspection() -> Inspection {
    let foot = StorePackage {
        root: PathBuf::from("/nix/store/7rj0kcq2w9j8foot-1.23.1"),
        name: "foot".to_owned(),
        version: Some("1.23.1".to_owned()),
    };
    Inspection {
        pid: TERMINAL_PID,
        launch: Launch {
            binary: Some(foot.root.join("bin/foot")),
            arguments: Vec::new(),
            scope: Some("niri.service".to_owned()),
        },
        provenance: Provenance {
            package: Some(foot),
            runtime: None,
            referenced: Vec::new(),
            profile: Some("home-manager-path".to_owned()),
        },
    }
}

fn terminal_package_facts() -> NixPackageFacts {
    NixPackageFacts {
        closure: ClosureFacts {
            closure_size: ByteSize::mib(96),
            path_count: 41,
            own_size: ByteSize::kib(1_380),
        },
        origin: Origin::Signed {
            by: vec!["cache.nixos.org-1".to_owned()],
        },
        registered: showcase_epoch() - Duration::from_hours(9 * 24),
        nixpkgs: Some(NixpkgsEntry {
            definition: Definition {
                file: "pkgs/by-name/fo/foot/package.nix".to_owned(),
                url: "https://github.com/NixOS/nixpkgs/blob/nixos-unstable/pkgs/by-name/fo/foot/package.nix".to_owned(),
            },
            homepage: Some("https://codeberg.org/dnkl/foot".to_owned()),
            description: Some("Fast, lightweight and minimalistic Wayland terminal emulator".to_owned()),
            license: Some("MIT".to_owned()),
            generation: Generation::Current,
        }),
        upstream: Some(Upstream {
            branch: "nixos-unstable".to_owned(),
            state: UpstreamState::Newer {
                version: "1.24.0".to_owned(),
            },
        }),
    }
}
