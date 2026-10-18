use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use niri_ipc::{
    Event, Window, WindowLayout, Workspace,
    state::{EventStreamState, EventStreamStatePart},
};
use uom::si::{f64::Ratio, ratio::percent};
use vetrata_shell::{
    data::source::{
        SourceDelivery,
        desktop_entries::DesktopIndex,
        kitty_windows::WindowProcesses,
        niri::Snapshot,
        nix_store::{Provenance, StorePackage},
        process::{Inspection, Launch, Process},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::window_group::{
        WindowGroup, WindowGroupMessage, WindowTarget, window_list::WindowListMessage,
    },
    widgets::{continuous_reading::ContinuousReading, favicon::Favicons, text::FontFallbacks},
};

use crate::showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup, ShowcasedNiriSession};

const CODING_WORKSPACE: u64 = 1;
const BROWSING_WORKSPACE: u64 = 2;
const CHAT_WORKSPACE: u64 = 3;
const TERMINAL_WINDOW: u64 = 2;
const TERMINAL_PID: i32 = 4200;

pub fn bottom_bar() -> ShowcasedGroup {
    ShowcasedGroup::BottomBar(Box::new(session()))
}

pub fn workspace_sidebar() -> ShowcasedGroup {
    ShowcasedGroup::WorkspaceSidebar(Box::new(session()))
}

fn session() -> ShowcasedNiriSession {
    let niri = [workspaces(), windows()].into_iter().fold(
        EventStreamState::default(),
        |mut state, event| {
            let _ = state.apply(event);
            state
        },
    );
    let mut snapshot = ContinuousReading::default();
    snapshot.load(Snapshot::derive(&niri, &WindowProcesses::default()));
    ShowcasedNiriSession {
        window_group: WindowGroup::default(),
        niri,
        snapshot,
        desktop_entries: DesktopIndex::default(),
        favicons: Favicons::default(),
        font_fallbacks: FontFallbacks::default(),
    }
}

pub fn on_hover(target: &HoverTarget) -> Vec<WindowGroupMessage> {
    match target {
        HoverTarget::Window(WindowTarget::Inspect(TERMINAL_WINDOW)) => vec![
            WindowGroupMessage::WindowList(WindowListMessage::ProcessTreeScanned(
                SourceDelivery::Reading(Ok(terminal_processes())),
            )),
            WindowGroupMessage::WindowList(WindowListMessage::ProcessInspected(Box::new(
                SourceDelivery::Reading(Ok(terminal_inspection())),
            ))),
        ],
        _ => Vec::new(),
    }
}

fn workspace(id: u64, name: &str, active_window: u64) -> Workspace {
    Workspace {
        id,
        idx: u8::try_from(id).expect("few fixture workspaces"),
        name: Some(name.to_owned()),
        output: Some(SHOWCASE_OUTPUT.to_owned()),
        is_urgent: false,
        is_active: id == CODING_WORKSPACE,
        is_focused: id == CODING_WORKSPACE,
        active_window_id: Some(active_window),
    }
}

fn workspaces() -> Event {
    Event::WorkspacesChanged {
        workspaces: vec![
            workspace(CODING_WORKSPACE, "code", TERMINAL_WINDOW),
            workspace(BROWSING_WORKSPACE, "web", 4),
            workspace(CHAT_WORKSPACE, "chat", 5),
        ],
    }
}

fn window(id: u64, app_id: &str, title: &str, workspace: u64, column: usize) -> Window {
    Window {
        id,
        title: Some(title.to_owned()),
        app_id: Some(app_id.to_owned()),
        pid: Some(TERMINAL_PID + i32::try_from(id).expect("few fixture windows") - 2),
        workspace_id: Some(workspace),
        is_focused: id == TERMINAL_WINDOW,
        is_floating: false,
        is_urgent: false,
        layout: WindowLayout {
            pos_in_scrolling_layout: Some((column, 1)),
            tile_size: (800.0, 900.0),
            window_size: (800, 900),
            tile_pos_in_workspace_view: None,
            window_offset_in_tile: (0.0, 0.0),
        },
        focus_timestamp: None,
    }
}

fn windows() -> Event {
    Event::WindowsChanged {
        windows: vec![
            window(1, "org.gnome.Nautilus", "Pictures", CODING_WORKSPACE, 1),
            window(
                TERMINAL_WINDOW,
                "foot",
                "~/Code/vetrata",
                CODING_WORKSPACE,
                2,
            ),
            window(3, "firefox", "vetrata — Codeberg", BROWSING_WORKSPACE, 1),
            window(4, "firefox", "iced — docs.rs", BROWSING_WORKSPACE, 2),
            window(5, "org.telegram.desktop", "Telegram", CHAT_WORKSPACE, 1),
        ],
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
