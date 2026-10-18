use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, Section, TooltipBody},
};

pub struct ServiceManagerFace;

impl BarWidget for ServiceManagerFace {
    fn name(&self) -> &'static str {
        "Service manager"
    }

    fn icon(&self) -> Glyph {
        Phosphor::GearSix(Weight::Regular).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Service manager", "systemd --user"),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "systemd/User".to_owned(),
                    url: "https://wiki.archlinux.org/title/Systemd/User".to_owned(),
                },
            ),
        ])
        .titled((
            "Per-user service manager",
            "Starts and supervises your services, the compositor included.",
        ))])
    }
}
