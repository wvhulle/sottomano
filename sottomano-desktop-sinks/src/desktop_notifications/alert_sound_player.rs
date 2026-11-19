use std::{path::PathBuf, process::Command};

use sottomano_desktop_sources::blocking_background_work::blocking;

use crate::desktop_notifications::alert_sound_policy::SoundChoice;

const PLAYER: &str = "pw-play";

#[derive(Debug, Clone, Default)]
pub enum SoundTheme {
    #[default]
    Unconfigured,
    Named(String),
}

impl SoundTheme {
    #[must_use]
    pub fn read_session() -> Self {
        let configured = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.sound", "theme-name"])
            .output()
            .inspect_err(|spawn_failure| {
                tracing::debug!("alert sounds: running gsettings failed: {spawn_failure}");
            })
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .inspect_err(|decode_failure| {
                        tracing::warn!(
                            "alert sounds: gsettings sound theme is not utf-8: {decode_failure}"
                        );
                    })
                    .ok()
            });
        match configured
            .as_deref()
            .map(|value| value.trim().trim_matches('\''))
        {
            Some(theme) if !theme.is_empty() => Self::Named(theme.to_owned()),
            Some(_) | None => Self::Unconfigured,
        }
    }

    fn find(&self, name: &str) -> Option<PathBuf> {
        let lookup = freedesktop_sound::lookup(name).with_cache();
        match self {
            Self::Named(theme) => lookup.with_theme(theme).find(),
            Self::Unconfigured => lookup.find(),
        }
    }
}

const FALLBACK_SOUND_NAME: &str = "message";

pub fn play(sound_choice: SoundChoice, theme: &SoundTheme) -> impl Future<Output = ()> + use<> {
    let theme = theme.clone();
    blocking("notification sound", move || {
        let Some(sound_file) = resolve(sound_choice, &theme) else {
            return;
        };
        match Command::new(PLAYER).arg(&sound_file).status() {
            Ok(exit_status) if exit_status.success() => {}
            Ok(exit_status) => tracing::warn!(
                path = %sound_file.display(),
                status = %exit_status,
                "alert sounds: player failed"
            ),
            Err(spawn_failure) => tracing::warn!(
                path = %sound_file.display(),
                error = %spawn_failure,
                "alert sounds: cannot start the player"
            ),
        }
    })
}

fn resolve(sound_choice: SoundChoice, theme: &SoundTheme) -> Option<PathBuf> {
    match sound_choice {
        SoundChoice::Silent => None,
        SoundChoice::RequestedFile(sound_file) if sound_file.exists() => Some(sound_file),
        SoundChoice::RequestedFile(_) => theme.find(FALLBACK_SOUND_NAME),
        SoundChoice::RequestedThemeSound(sound_name) => theme
            .find(&sound_name)
            .or_else(|| theme.find(FALLBACK_SOUND_NAME)),
        SoundChoice::PolicyThemeSounds(sound_names) => sound_names
            .iter()
            .find_map(|sound_name| theme.find(sound_name))
            .or_else(|| theme.find(FALLBACK_SOUND_NAME)),
    }
}
