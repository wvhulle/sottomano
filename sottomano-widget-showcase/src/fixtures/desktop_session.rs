use iced_controller_diagram::ControllerArtwork;
use sottomano_compositor_model::{
    compositor_test_windows::{unnamed_compositor_workspace, unplaced_compositor_window},
    compositor_window_facts::{
        ScrollingPosition, WindowAppId, WindowFocus, WindowPid, WindowTitle, WindowWorkspace,
        WorkspaceActiveWindow, WorkspaceActivity, WorkspaceName, WorkspaceOutput,
    },
    compositor_window_state::{
        CompositorWindow, CompositorWindowLayout, CompositorWindowState, CompositorWorkspace,
    },
};
use sottomano_desktop_sources::desktop_windows_apps::{
    desktop_entries::DesktopEntries,
    terminal_emulator_windows::{
        WindowProcesses,
        terminal_shell_label::{
            ForegroundActivity, ForegroundProgramName, TerminalShellLabel, WorkingLocation,
        },
    },
    workspace_window_snapshot::Snapshot,
};
use sottomano_widgets::{
    icon_catalog::favicons::Favicons,
    output_windows::{OutputWindows, WindowRowLayout},
};

use crate::{
    fixtures::bundled_app_icons::bundled_app_icon,
    showcased_group::{
        SHOWCASE_OUTPUT, ShowcasedGroup, showcased_desktop_session::ShowcasedDesktopSession,
    },
};

pub mod terminal_hover_inspection;

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

const KITTY_APP_ID: &str = "kitty";
const PROJECT_DIRECTORY: &str = "weather-station";
const NOTES_DIRECTORY: &str = "~/notes";
const SECOND_PROJECT_TERMINAL: u64 = 21;
const IDLE_PROJECT_TERMINAL: u64 = 22;
const NOTES_TERMINAL: u64 = 23;

const KITTY_TERMINALS: &[PlacedWindow] = &[
    PlacedWindow {
        id: SECOND_PROJECT_TERMINAL,
        app_id: KITTY_APP_ID,
        title: "hx src/main.rs",
        workspace: CODING_WORKSPACE,
        column: 1,
    },
    PlacedWindow {
        id: TERMINAL_WINDOW,
        app_id: KITTY_APP_ID,
        title: "cargo test",
        workspace: CODING_WORKSPACE,
        column: 2,
    },
    PlacedWindow {
        id: IDLE_PROJECT_TERMINAL,
        app_id: KITTY_APP_ID,
        title: "weather-station",
        workspace: CODING_WORKSPACE,
        column: 3,
    },
    PlacedWindow {
        id: NOTES_TERMINAL,
        app_id: KITTY_APP_ID,
        title: "jj log",
        workspace: CODING_WORKSPACE,
        column: 4,
    },
    PlacedWindow {
        id: 24,
        app_id: "org.gnome.Nautilus",
        title: "Pictures",
        workspace: CODING_WORKSPACE,
        column: 5,
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

pub fn bottom_bar() -> ShowcasedGroup {
    ShowcasedGroup::BottomPanel(Box::new(session(
        &NAMED_WORKSPACES[..BOTTOM_BAR_WORKSPACE_COUNT],
        PLACED_WINDOWS,
        &WindowProcesses::default(),
        WindowRowLayout::default(),
    )))
}

pub fn terminals_grouped_by_directory() -> ShowcasedGroup {
    terminals_in_layout(WindowRowLayout::GroupedByLocation)
}

fn terminals_in_layout(window_row_layout: WindowRowLayout) -> ShowcasedGroup {
    ShowcasedGroup::BottomPanel(Box::new(session(
        &NAMED_WORKSPACES[..BOTTOM_BAR_WORKSPACE_COUNT],
        KITTY_TERMINALS,
        &kitty_terminal_labels(),
        window_row_layout,
    )))
}

fn terminal_label_in(
    directory: &str,
    foreground_activity: ForegroundActivity,
) -> TerminalShellLabel {
    TerminalShellLabel {
        working_location: WorkingLocation::LocalDirectory(directory.to_owned()),
        foreground_activity,
    }
}

fn typed_command(command_line: &str) -> ForegroundActivity {
    ForegroundActivity::ShellCommand {
        command_line: command_line.to_owned(),
        program_name: ForegroundProgramName::Unidentified,
    }
}

fn kitty_terminal_labels() -> WindowProcesses {
    WindowProcesses::default()
        .with_terminal_label(
            SECOND_PROJECT_TERMINAL,
            window_pid(SECOND_PROJECT_TERMINAL),
            terminal_label_in(PROJECT_DIRECTORY, typed_command("hx src/main.rs")),
        )
        .with_terminal_label(
            TERMINAL_WINDOW,
            window_pid(TERMINAL_WINDOW),
            terminal_label_in(PROJECT_DIRECTORY, typed_command("cargo test")),
        )
        .with_terminal_label(
            IDLE_PROJECT_TERMINAL,
            window_pid(IDLE_PROJECT_TERMINAL),
            terminal_label_in(
                PROJECT_DIRECTORY,
                ForegroundActivity::ShellPrompt {
                    shell_name: "nu".to_owned(),
                },
            ),
        )
        .with_terminal_label(
            NOTES_TERMINAL,
            window_pid(NOTES_TERMINAL),
            terminal_label_in(NOTES_DIRECTORY, typed_command("jj log")),
        )
}

fn window_pid(window: u64) -> i32 {
    TERMINAL_PID + i32::try_from(window).expect("few fixture windows") - 2
}

fn session(
    shown_workspaces: &[NamedWorkspace],
    placed_windows: &[PlacedWindow],
    window_processes: &WindowProcesses,
    window_row_layout: WindowRowLayout,
) -> ShowcasedDesktopSession {
    let shown_windows: Vec<&PlacedWindow> = placed_windows
        .iter()
        .filter(|placed| {
            shown_workspaces
                .iter()
                .any(|named| named.id == placed.workspace)
        })
        .collect();
    let compositor = CompositorWindowState::indexed_by_id(
        shown_workspaces.iter().map(workspace),
        shown_windows.iter().copied().map(window),
    );
    let snapshot = Snapshot::derive(&compositor, window_processes);
    ShowcasedDesktopSession {
        output_windows: OutputWindows::default(),
        window_row_layout,
        compositor,
        snapshot,
        icon_catalog: shown_windows
            .iter()
            .map(|placed| bundled_app_icon(placed.app_id))
            .collect(),
        desktop_entries: DesktopEntries::default(),
        favicons: Favicons::default(),
        controller_artwork: ControllerArtwork::decoded(),
    }
}

fn workspace(named: &NamedWorkspace) -> CompositorWorkspace {
    CompositorWorkspace {
        index: u8::try_from(named.id).expect("few fixture workspaces"),
        name: WorkspaceName::Named(named.name.to_owned()),
        output: WorkspaceOutput::Assigned(SHOWCASE_OUTPUT.to_owned()),
        activity: if named.id == CODING_WORKSPACE {
            WorkspaceActivity::Focused
        } else {
            WorkspaceActivity::Inactive
        },
        active_window: WorkspaceActiveWindow::Window(named.active_window),
        ..unnamed_compositor_workspace(named.id)
    }
}

fn window(placed: &PlacedWindow) -> CompositorWindow {
    let unplaced = unplaced_compositor_window(placed.id);
    CompositorWindow {
        title: WindowTitle::Titled(placed.title.to_owned()),
        app_id: WindowAppId::Reported(placed.app_id.to_owned()),
        pid: WindowPid::Reported(window_pid(placed.id)),
        workspace: WindowWorkspace::OnWorkspace(placed.workspace),
        focus: if placed.id == TERMINAL_WINDOW {
            WindowFocus::Focused
        } else {
            WindowFocus::Unfocused
        },
        layout: CompositorWindowLayout {
            scrolling_position: ScrollingPosition::Tile {
                column: placed.column,
                tile: 1,
            },
            tile_size: (800.0, 900.0),
            window_size: (800, 900),
            ..unplaced.layout
        },
        ..unplaced
    }
}
