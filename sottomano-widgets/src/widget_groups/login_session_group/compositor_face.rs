use chrono::Locale;
use sottomano_sources::{
    login_session_presence::logind_session_facts::SessionIdentity,
    systemd_units_journal::graphical_session_target::GraphicalSession,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
    },
};

pub struct CompositorFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl BarWidget for CompositorFace<'_> {
    fn name(&self) -> &'static str {
        "Compositor"
    }

    fn icon(&self) -> Glyph {
        let desktop_name = self.identity.desktop.as_deref().unwrap_or_default();
        match desktop_name.to_ascii_lowercase().as_str() {
            "niri" => SimpleIcon::niri.into(),
            "hyprland" => SimpleIcon::Hyprland.into(),
            "sway" => SimpleIcon::Sway.into(),
            "i3" => SimpleIcon::i3.into(),
            "gnome" => SimpleIcon::GNOME.into(),
            "kde" => SimpleIcon::KDEPlasma.into(),
            _ => Phosphor::AppWindow(Weight::Regular).into(),
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.identity.desktop.clone()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Protocol", self.identity.session_type.clone()),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Wayland".to_owned(),
                    url: "https://wiki.archlinux.org/title/Wayland".to_owned(),
                    glyph: Some(SimpleIcon::ArchLinux.into()),
                },
            ),
        ])
        .titled((
            "Compositor drawing the desktop",
            "The compositor draws every window and routes input; apps connect to it.",
        ))])
    }
}

impl ReadingSections for GraphicalSession {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let distinct_session_desktop = self
            .session_desktop
            .clone()
            .filter(|session| Some(session) != self.current_desktop.as_ref());
        vec![Section::dictionary(
            [
                Some(KeyValue::new("Runs", self.compositor_version.clone())),
                Some(KeyValue::new(
                    "Wayland apps connect to",
                    self.wayland_display.clone(),
                )),
                self.x11_display
                    .clone()
                    .map(|x11_display| KeyValue::new("X11 apps connect to", x11_display)),
                self.current_desktop
                    .clone()
                    .map(|desktop_name| KeyValue::new("Desktop name", desktop_name)),
                distinct_session_desktop
                    .map(|session_desktop| KeyValue::new("Session desktop name", session_desktop)),
            ]
            .into_iter()
            .flatten(),
        )]
    }
}
