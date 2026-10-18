use std::time::Duration;

use niri_ipc::{
    Event, Window, WindowLayout, Workspace,
    state::{EventStreamState, EventStreamStatePart},
};
use uom::si::{f64::Ratio, ratio::percent};
use vetrata_config::severity::Severity;
use vetrata_sources::notifications::model::{
    ActionTint, ExpireTimeout, NotifAction, NotificationData, ReplyPrompt,
};
use vetrata_widgets::{shell_message::Message, toast_messages::NotificationMessage};

use crate::showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup, ShowcasedToasts};

const DESK_WORKSPACE: u64 = 1;
const EDITOR_WINDOW: u64 = 1;
const EDITOR_PID: i32 = 5100;
const EDITOR_POSITION_IN_VIEW: (f64, f64) = (24.0, 120.0);
const ARRIVAL_TOAST: u32 = 1;
const PROGRESS_TOAST: u32 = 2;
const ACTIONS_TOAST: u32 = 3;
const REPLY_TOAST: u32 = 4;
const WINDOW_TOAST: u32 = 5;
const PROGRESS_STEP_PERCENT: f64 = 15.0;

pub const ARRIVAL_SUMMARY: &str = "Standup in 5 minutes";
pub const ACTIONS_SUMMARY: &str = "Meeting invitation";
pub const WINDOW_SUMMARY: &str = "Build finished";

pub fn arrival() -> ShowcasedGroup {
    desk(NotificationData {
        id: ARRIVAL_TOAST,
        app_name: Some("Calendar".to_owned()),
        summary: ARRIVAL_SUMMARY.to_owned(),
        body: Some("Room 2 with the platform team".to_owned()),
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
        app_name: Some("Calendar".to_owned()),
        summary: ACTIONS_SUMMARY.to_owned(),
        body: Some("Design review, Thursday 14:00".to_owned()),
        actions: vec![
            action("accept", "Accept"),
            action("later", "Later"),
            action("deny", "Decline"),
        ],
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    })
}

pub fn reply() -> ShowcasedGroup {
    desk(NotificationData {
        id: REPLY_TOAST,
        app_name: Some("Chat".to_owned()),
        summary: "Alex".to_owned(),
        body: Some("Lunch at noon?".to_owned()),
        reply: ReplyPrompt::Offered {
            placeholder: "Reply".to_owned(),
            submit: "Send".to_owned(),
        },
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    })
}

pub fn next_to_window() -> ShowcasedGroup {
    desk(NotificationData {
        id: WINDOW_TOAST,
        app_name: Some("foot".to_owned()),
        summary: WINDOW_SUMMARY.to_owned(),
        body: Some("cargo build finished in 42 s".to_owned()),
        window_id_hint: Some(EDITOR_WINDOW),
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    })
}

pub fn downloading(tick: u32) -> Vec<Message> {
    vec![Message::Notification(NotificationMessage::Served(
        Box::new(download(tick)),
    ))]
}

fn download(tick: u32) -> NotificationData {
    let done = (f64::from(tick) * PROGRESS_STEP_PERCENT).min(100.0);
    NotificationData {
        id: PROGRESS_TOAST,
        app_name: Some("Downloads".to_owned()),
        summary: "nixos-25.11.iso".to_owned(),
        body: Some("Downloading".to_owned()),
        progress: Some(Ratio::new::<percent>(done)),
        expire_timeout: ExpireTimeout::Never,
        ..quiet_toast()
    }
}

fn quiet_toast() -> NotificationData {
    NotificationData {
        severity: Severity::Normal,
        suppress_sound: true,
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
    let editor_view = opening
        .window_id_hint
        .map(|_| EDITOR_POSITION_IN_VIEW);
    let niri = [workspaces(), windows(editor_view)].into_iter().fold(
        EventStreamState::default(),
        |mut state, event| {
            let _ = state.apply(event);
            state
        },
    );
    ShowcasedGroup::Toasts(Box::new(ShowcasedToasts {
        niri,
        opening: vec![opening],
    }))
}

fn workspaces() -> Event {
    Event::WorkspacesChanged {
        workspaces: vec![Workspace {
            id: DESK_WORKSPACE,
            idx: 1,
            name: Some("code".to_owned()),
            output: Some(SHOWCASE_OUTPUT.to_owned()),
            is_urgent: false,
            is_active: true,
            is_focused: true,
            active_window_id: Some(EDITOR_WINDOW),
        }],
    }
}

fn windows(editor_view: Option<(f64, f64)>) -> Event {
    Event::WindowsChanged {
        windows: vec![Window {
            id: EDITOR_WINDOW,
            title: Some("~/Code/vetrata".to_owned()),
            app_id: Some("foot".to_owned()),
            pid: Some(EDITOR_PID),
            workspace_id: Some(DESK_WORKSPACE),
            is_focused: true,
            is_floating: false,
            is_urgent: false,
            layout: WindowLayout {
                pos_in_scrolling_layout: Some((1, 1)),
                tile_size: (360.0, 420.0),
                window_size: (360, 420),
                tile_pos_in_workspace_view: editor_view,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }],
    }
}
