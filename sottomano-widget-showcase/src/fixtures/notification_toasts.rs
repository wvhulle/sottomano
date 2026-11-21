use std::time::Duration;

use sottomano_compositor_model::{
    compositor_test_windows::unplaced_compositor_window,
    compositor_window_facts::{
        ScrollingPosition, TileViewPosition, Urgency, WindowAppId, WindowFocus, WindowPid,
        WindowTitle, WindowWorkspace, WorkspaceActiveWindow, WorkspaceActivity, WorkspaceName,
        WorkspaceOutput,
    },
    compositor_window_state::{
        CompositorWindow, CompositorWindowLayout, CompositorWindowState, CompositorWorkspace,
    },
};
use sottomano_desktop_sources::desktop_notifications::{
    notification_data::{
        ActionTint, ExpireTimeout, NotifAction, NotificationBody, NotificationData,
        NotificationProgress, NotifyingAppName, SenderWindowHint,
    },
    spec_hints::SoundRequest,
};
use sottomano_toml_config::severity::Severity;
use sottomano_widgets::{clock::Clock, toast_messages::NotificationMessage};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    fixtures::sample,
    showcase_message::ShowcaseMessage,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup, ShowcasedToasts, TerminalLine},
};

const DESK_WORKSPACE: u64 = 1;
const EDITOR_WINDOW: u64 = 1;
const EDITOR_PID: i32 = 5100;
const EDITOR_POSITION_IN_VIEW: TileViewPosition = TileViewPosition::InView {
    horizontal: 24.0,
    vertical: 120.0,
};
const EDITOR_WINDOW_SIZE: (i32, i32) = (660, 300);
const PROJECT_DIRECTORY: &str = "~/projects/weather-station";
const BUILD_SCROLLBACK: &[TerminalLine] = &[
    TerminalLine::Prompt {
        directory: PROJECT_DIRECTORY,
        command: " $ cargo build --release",
    },
    TerminalLine::Status {
        verb: "   Compiling",
        detail: " serde v1.0.228",
    },
    TerminalLine::Status {
        verb: "   Compiling",
        detail: " tokio v1.48.0",
    },
    TerminalLine::Status {
        verb: "   Compiling",
        detail: " rumqttc v0.25.0",
    },
    TerminalLine::Status {
        verb: "   Compiling",
        detail: " weather-station v0.4.1",
    },
    TerminalLine::Status {
        verb: "    Finished",
        detail: " `release` profile [optimized] in 42.06s",
    },
    TerminalLine::Prompt {
        directory: PROJECT_DIRECTORY,
        command: " $",
    },
];
const ARRIVAL_TOAST: u32 = 1;
const PROGRESS_TOAST: u32 = 2;
const ACTIONS_TOAST: u32 = 3;
const WINDOW_TOAST: u32 = 5;
const PROGRESS_STEP_PERCENT: f64 = 15.0;

pub const ARRIVAL_SUMMARY: &str = "Standup in 5 minutes";
pub const ACTIONS_SUMMARY: &str = "Meeting invitation";
pub const WINDOW_SUMMARY: &str = "Build finished";

pub fn arrival() -> ShowcasedGroup {
    desk(NotificationData {
        id: ARRIVAL_TOAST,
        app_name: NotifyingAppName::Named("Calendar".to_owned()),
        summary: ARRIVAL_SUMMARY.to_owned(),
        body: NotificationBody::Text("Room 2 with the platform team".to_owned()),
        expire_timeout: ExpireTimeout::After(Duration::from_secs(3)),
        ..quiet_toast()
    })
}

pub fn progress() -> ShowcasedGroup {
    desk(download(0))
}

pub fn actions() -> ShowcasedGroup {
    desk(NotificationData {
        id: ACTIONS_TOAST,
        app_name: NotifyingAppName::Named("Calendar".to_owned()),
        summary: ACTIONS_SUMMARY.to_owned(),
        body: NotificationBody::Text("Design review, Thursday 14:00".to_owned()),
        actions: vec![
            action("accept", "Accept"),
            action("later", "Later"),
            action("deny", "Decline"),
        ],
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    })
}

pub fn next_to_window() -> ShowcasedGroup {
    desk(NotificationData {
        id: WINDOW_TOAST,
        app_name: NotifyingAppName::Named("foot".to_owned()),
        summary: WINDOW_SUMMARY.to_owned(),
        body: NotificationBody::Text("cargo build finished in 42 s".to_owned()),
        window_id_hint: SenderWindowHint::Window(EDITOR_WINDOW),
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    })
}

pub fn downloading(clock: Clock) -> Vec<ShowcaseMessage> {
    let tick = sample(clock);
    vec![ShowcaseMessage::Notification(NotificationMessage::Served(
        Box::new(download(tick)),
    ))]
}

fn download(tick: u32) -> NotificationData {
    let done = (f64::from(tick) * PROGRESS_STEP_PERCENT).min(100.0);
    NotificationData {
        id: PROGRESS_TOAST,
        app_name: NotifyingAppName::Named("Downloads".to_owned()),
        summary: "nixos-25.11.iso".to_owned(),
        body: NotificationBody::Text("Downloading".to_owned()),
        progress: NotificationProgress::Measured(Ratio::new::<percent>(done)),
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    }
}

fn quiet_toast() -> NotificationData {
    NotificationData {
        severity: Severity::Normal,
        sound_request: SoundRequest::Suppressed,
        ..NotificationData::default()
    }
}

fn action(key: &str, label: &str) -> NotifAction {
    NotifAction {
        key: key.to_owned(),
        label: label.to_owned(),
        tint: ActionTint::from(key),
    }
}

fn desk(opening: NotificationData) -> ShowcasedGroup {
    let editor_view = match opening.window_id_hint {
        SenderWindowHint::Window(_) => EDITOR_POSITION_IN_VIEW,
        SenderWindowHint::Unhinted => TileViewPosition::OutsideView,
    };
    let compositor =
        CompositorWindowState::indexed_by_id([desk_workspace()], [editor_window(editor_view)]);
    ShowcasedGroup::Toasts(Box::new(ShowcasedToasts {
        compositor,
        opening: vec![opening],
        terminal_scrollback: BUILD_SCROLLBACK,
    }))
}

fn desk_workspace() -> CompositorWorkspace {
    CompositorWorkspace {
        id: DESK_WORKSPACE,
        index: 1,
        name: WorkspaceName::Named("code".to_owned()),
        output: WorkspaceOutput::Assigned(SHOWCASE_OUTPUT.to_owned()),
        urgency: Urgency::NotUrgent,
        activity: WorkspaceActivity::Focused,
        active_window: WorkspaceActiveWindow::Window(EDITOR_WINDOW),
    }
}

fn editor_window(editor_view: TileViewPosition) -> CompositorWindow {
    CompositorWindow {
        title: WindowTitle::Titled(PROJECT_DIRECTORY.to_owned()),
        app_id: WindowAppId::Reported("foot".to_owned()),
        pid: WindowPid::Reported(EDITOR_PID),
        workspace: WindowWorkspace::OnWorkspace(DESK_WORKSPACE),
        focus: WindowFocus::Focused,
        layout: CompositorWindowLayout {
            scrolling_position: ScrollingPosition::Tile { column: 1, tile: 1 },
            tile_size: (
                f64::from(EDITOR_WINDOW_SIZE.0),
                f64::from(EDITOR_WINDOW_SIZE.1),
            ),
            window_size: EDITOR_WINDOW_SIZE,
            tile_position_in_view: editor_view,
            window_offset_in_tile: (0.0, 0.0),
        },
        ..unplaced_compositor_window(EDITOR_WINDOW)
    }
}
