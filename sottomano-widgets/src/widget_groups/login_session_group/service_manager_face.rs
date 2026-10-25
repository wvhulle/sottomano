use sottomano_sources::systemd_units_journal::user_service_manager::UserServiceManager;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
    },
};

pub struct ServiceManagerFace;

impl BarWidget for ServiceManagerFace {
    fn name(&self) -> &'static str {
        "Service manager"
    }

    fn icon(&self) -> Glyph {
        Phosphor::GearSix(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Service manager", "systemd --user"),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "systemd/User".to_owned(),
                    url: "https://wiki.archlinux.org/title/Systemd/User".to_owned(),
                    glyph: Some(SimpleIcon::ArchLinux.into()),
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
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("systemd version", self.version.clone())),
                    Some(KeyValue::new("State", self.state.clone())),
                    self.started_at.map(|at| {
                        KeyValue::new(
                            "Running since",
                            Cell::Time {
                                at,
                                format: LocalizedTimeFormat::DateAndTime,
                            },
                        )
                    }),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Your systemd instance right now",
                "One instance per user; it outlives this login and stops at your last logout.",
            )),
        ]
    }
}
