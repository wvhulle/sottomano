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
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody, entry},
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
        Phosphor::AppWindow(Weight::Regular).into()
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
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Runs", self.compositor_version.clone())),
                    Some(KeyValue::new(
                        "Wayland apps connect to",
                        self.wayland_display.clone(),
                    )),
                    entry("X11 apps connect to", self.x11_display.clone()),
                    entry("Desktop name", self.current_desktop.clone()),
                    entry("Session desktop name", distinct_session_desktop),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Where apps connect to it",
                "Apps find the compositor through these sockets.",
            )),
        ]
    }
}
