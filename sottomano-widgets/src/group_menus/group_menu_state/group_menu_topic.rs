use crate::glyph_icons::phosphor::{Phosphor, Weight};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMenuTopic {
    Hardware,
    Network,
    Agenda,
    Media,
    Display,
    Virtualization,
    Services,
    Session,
    Appearance,
    Behaviour,
    Applications,
    WindowMenuBar,
}

impl GroupMenuTopic {
    pub const ALL: [GroupMenuTopic; 12] = [
        GroupMenuTopic::Hardware,
        GroupMenuTopic::Network,
        GroupMenuTopic::Agenda,
        GroupMenuTopic::Media,
        GroupMenuTopic::Display,
        GroupMenuTopic::Virtualization,
        GroupMenuTopic::Services,
        GroupMenuTopic::Session,
        GroupMenuTopic::Appearance,
        GroupMenuTopic::Behaviour,
        GroupMenuTopic::Applications,
        GroupMenuTopic::WindowMenuBar,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            GroupMenuTopic::Hardware => "Hardware",
            GroupMenuTopic::Network => "Network",
            GroupMenuTopic::Agenda => "Agenda",
            GroupMenuTopic::Media => "Media",
            GroupMenuTopic::Display => "Display",
            GroupMenuTopic::Virtualization => "Virtualization",
            GroupMenuTopic::Services => "Services",
            GroupMenuTopic::Session => "Session",
            GroupMenuTopic::Appearance => "Appearance",
            GroupMenuTopic::Behaviour => "Behaviour",
            GroupMenuTopic::Applications => "Applications",
            GroupMenuTopic::WindowMenuBar => "Menu bar",
        }
    }

    #[must_use]
    pub const fn glyph(self) -> Phosphor {
        match self {
            GroupMenuTopic::Hardware => Phosphor::Cpu(Weight::Regular),
            GroupMenuTopic::Network => Phosphor::Network(Weight::Regular),
            GroupMenuTopic::Agenda => Phosphor::CalendarBlank(Weight::Regular),
            GroupMenuTopic::Media => Phosphor::MusicNotes(Weight::Regular),
            GroupMenuTopic::Display => Phosphor::Monitor(Weight::Regular),
            GroupMenuTopic::Virtualization => Phosphor::Cube(Weight::Regular),
            GroupMenuTopic::Services => Phosphor::Stack(Weight::Regular),
            GroupMenuTopic::Session => Phosphor::UserCircle(Weight::Regular),
            GroupMenuTopic::Appearance => Phosphor::SlidersHorizontal(Weight::Regular),
            GroupMenuTopic::Behaviour => Phosphor::ToggleRight(Weight::Regular),
            GroupMenuTopic::Applications => Phosphor::SquaresFour(Weight::Regular),
            GroupMenuTopic::WindowMenuBar => Phosphor::AppWindow(Weight::Regular),
        }
    }

    pub(super) const fn storage_key(self) -> &'static str {
        match self {
            GroupMenuTopic::Hardware => "hardware",
            GroupMenuTopic::Network => "network",
            GroupMenuTopic::Agenda => "agenda",
            GroupMenuTopic::Media => "media",
            GroupMenuTopic::Display => "display",
            GroupMenuTopic::Virtualization => "virtualization",
            GroupMenuTopic::Services => "services",
            GroupMenuTopic::Session => "session",
            GroupMenuTopic::Appearance => "appearance",
            GroupMenuTopic::Behaviour => "behaviour",
            GroupMenuTopic::Applications => "applications",
            GroupMenuTopic::WindowMenuBar => "application_menu",
        }
    }
}
