use sottomano_compositor_model::compositor_window_facts::{
    Urgency, WindowFocus, WindowPid, WindowTiling,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::{
        unity_launcher_badges::{LauncherCount, LauncherProgress},
        workspace_window_snapshot,
    },
    media_playback_audio::pipewire_audio::WindowAudio,
};

use crate::hover_cards::{MouseGesture, PressAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowSound {
    Untracked,
    Tracked(WindowAudio),
}

#[derive(Clone, Copy)]
pub(super) struct Extras {
    pub(super) audio: WindowSound,
    pub(super) progress: LauncherProgress,
    pub(super) count: LauncherCount,
    pub(super) launcher_urgency: Urgency,
}

pub(super) fn entry_tile_press(window_entry: &workspace_window_snapshot::WinEntry) -> TilePress {
    match (window_entry.focus, window_entry.layout.tiling) {
        (WindowFocus::Focused, WindowTiling::Tiled) => TilePress::TogglesMaximize,
        (WindowFocus::Focused, WindowTiling::Floating) | (WindowFocus::Unfocused, _) => {
            TilePress::FocusesWindow
        }
    }
}

pub(super) fn audible_pid(
    window_entry: &workspace_window_snapshot::WinEntry,
    extras: &Extras,
) -> Option<i32> {
    match window_entry.pid {
        WindowPid::Reported(pid) if extras.audio != WindowSound::Untracked => Some(pid),
        WindowPid::Reported(_) | WindowPid::Unreported => None,
    }
}

pub(super) fn slot_press_actions(
    window_entry: &workspace_window_snapshot::WinEntry,
    extras: &Extras,
) -> Vec<PressAction> {
    let left_does = if entry_tile_press(window_entry) == TilePress::TogglesMaximize {
        "maximize or restore"
    } else {
        "focus the window"
    };
    [PressAction::new(MouseGesture::LeftClick, left_does)]
        .into_iter()
        .chain(
            audible_pid(window_entry, extras)
                .map(|_| PressAction::new(MouseGesture::MiddleClick, "mute or unmute its audio")),
        )
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TilePress {
    TogglesMaximize,
    FocusesWindow,
}
