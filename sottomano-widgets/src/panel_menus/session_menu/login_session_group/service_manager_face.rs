use sottomano_desktop_sources::systemd_units_journal::{
    SystemdTimestamp, user_service_manager::UserServiceManager,
};

use crate::{
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct ServiceManagerFace;

impl ServiceManagerFace {
    pub fn indicator<'indicator>() -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Service manager",
            Phosphor::GearSix(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(|_| Self::hover_card()))
    }

    pub fn hover_card() -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Per-user service manager",
                "Starts and supervises your services, the compositor included.",
            ),
            dictionary_table([
                LabeledValue::new("Service manager", "systemd --user"),
                LabeledValue::new(
                    "Wiki",
                    ValueCell::BrandedLink {
                        label: "systemd/User".to_owned(),
                        url: "https://wiki.archlinux.org/title/Systemd/User".to_owned(),
                        glyph: SimpleIcon::ArchLinux.into(),
                    },
                ),
            ]),
        )]
    }
}

pub fn user_service_manager_sections(user_service_manager: &UserServiceManager) -> Vec<Section> {
    let mut entries = vec![
        LabeledValue::new("systemd version", user_service_manager.version.clone()),
        LabeledValue::new("State", user_service_manager.state.clone()),
    ];
    if let SystemdTimestamp::At(at) = user_service_manager.started_at {
        entries.push(LabeledValue::new(
            "Running since",
            ValueCell::Time {
                at,
                format: LocalizedTimeFormat::DateAndTime,
            },
        ));
    }
    vec![Section::new(
        SectionTitle::captioned(
            "Status",
            "One instance per user; it outlives this login and stops at your last logout.",
        ),
        dictionary_table(entries),
    )]
}
