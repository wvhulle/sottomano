use chrono::Locale;
use sottomano_sources::{
    login_session_presence::logind_session_facts::SessionIdentity,
    systemd_units_journal::graphical_session_target::GraphicalSession,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, ReadingSections, Section},
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct CompositorFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl GroupItemReading for CompositorFace<'_> {
    fn name(&self) -> &'static str {
        "Compositor"
    }

    fn icon(&self) -> IconGlyph {
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

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([
            LabeledValue::new("Protocol", self.identity.session_type.clone()),
            LabeledValue::new(
                "Wiki",
                ValueCell::BrandedLink {
                    label: "Wayland".to_owned(),
                    url: "https://wiki.archlinux.org/title/Wayland".to_owned(),
                    glyph: SimpleIcon::ArchLinux.into(),
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
                Some(LabeledValue::new("Runs", self.compositor_version.clone())),
                Some(LabeledValue::new(
                    "Wayland apps connect to",
                    self.wayland_display.clone(),
                )),
                self.x11_display
                    .clone()
                    .map(|x11_display| LabeledValue::new("X11 apps connect to", x11_display)),
                self.current_desktop
                    .clone()
                    .map(|desktop_name| LabeledValue::new("Desktop name", desktop_name)),
                distinct_session_desktop.map(|session_desktop| {
                    LabeledValue::new("Session desktop name", session_desktop)
                }),
            ]
            .into_iter()
            .flatten(),
        )]
    }
}
