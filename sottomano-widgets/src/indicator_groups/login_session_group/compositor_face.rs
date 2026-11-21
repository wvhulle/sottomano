use sottomano_desktop_sources::{
    login_session_presence::logind_session_facts::SessionIdentity,
    systemd_units_journal::graphical_session_target::{DesktopName, GraphicalSession, Xwayland},
};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

pub struct CompositorFace<'a> {
    pub identity: &'a SessionIdentity,
}

impl<'a> CompositorFace<'a> {
    fn glyph(&self) -> IconGlyph {
        let DesktopName::Named(desktop_name) = &self.identity.desktop else {
            return Phosphor::AppWindow(Weight::Regular).into();
        };
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

    pub fn indicator(self) -> GroupIndicator<'a> {
        let glyph = self.glyph();
        let label = match self.identity.desktop.clone() {
            DesktopName::Named(desktop_name) => ReadoutLabel::Labeled(desktop_name),
            DesktopName::Unnamed => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only("Compositor", glyph, IndicatorState::Idle)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Compositor drawing the desktop",
                "The compositor draws every window and routes input; apps connect to it.",
            ),
            dictionary_table([
                LabeledValue::new("Protocol", self.identity.session_type.clone()),
                LabeledValue::new(
                    "Wiki",
                    ValueCell::BrandedLink {
                        label: "Wayland".to_owned(),
                        url: "https://wiki.archlinux.org/title/Wayland".to_owned(),
                        glyph: SimpleIcon::ArchLinux.into(),
                    },
                ),
            ]),
        )]
    }
}

pub fn graphical_session_sections(graphical_session: &GraphicalSession) -> Vec<Section> {
    let mut entries = vec![
        LabeledValue::new("Runs", graphical_session.compositor_version.clone()),
        LabeledValue::new(
            "Wayland apps connect to",
            graphical_session.wayland_display.clone(),
        ),
    ];
    if let Xwayland::Serving(x11_display) = &graphical_session.x11_display {
        entries.push(LabeledValue::new(
            "X11 apps connect to",
            x11_display.clone(),
        ));
    }
    if let DesktopName::Named(desktop_name) = &graphical_session.current_desktop {
        entries.push(LabeledValue::new("Desktop name", desktop_name.clone()));
    }
    if let DesktopName::Named(session_desktop) = &graphical_session.session_desktop
        && graphical_session.session_desktop != graphical_session.current_desktop
    {
        entries.push(LabeledValue::new(
            "Session desktop name",
            session_desktop.clone(),
        ));
    }
    vec![Section::new(
        SectionTitle::captioned(
            "Displays",
            "Apps find the compositor through these sockets.",
        ),
        dictionary_table(entries),
    )]
}
