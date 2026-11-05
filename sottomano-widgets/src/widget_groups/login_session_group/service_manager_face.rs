use sottomano_desktop_sources::systemd_units_journal::user_service_manager::UserServiceManager;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct ServiceManagerFace;

impl GroupItemReading for ServiceManagerFace {
    fn name(&self) -> &'static str {
        "Service manager"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::GearSix(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary([
            LabeledValue::new("Service manager", "systemd --user"),
            LabeledValue::new(
                "Wiki",
                ValueCell::BrandedLink {
                    label: "systemd/User".to_owned(),
                    url: "https://wiki.archlinux.org/title/Systemd/User".to_owned(),
                    glyph: SimpleIcon::ArchLinux.into(),
                },
            ),
        ])
        .titled((
            "Per-user service manager",
            "Starts and supervises your services, the compositor included.",
        ))])
    }
}

impl ReadingSections for UserServiceManager {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(
            [
                Some(LabeledValue::new("systemd version", self.version.clone())),
                Some(LabeledValue::new("State", self.state.clone())),
                self.started_at.map(|at| {
                    LabeledValue::new(
                        "Running since",
                        ValueCell::Time {
                            at,
                            format: LocalizedTimeFormat::DateAndTime,
                        },
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        )]
    }
}
