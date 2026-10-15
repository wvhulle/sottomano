use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, Section, TooltipBody},
};

pub struct UserServicesFace;

impl BarWidget for UserServicesFace {
    fn name(&self) -> &'static str {
        "User services"
    }

    fn icon(&self) -> Glyph {
        Phosphor::GearSix(Weight::Regular).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Session manager", "systemd --user"),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "systemd/User".to_owned(),
                    url: "https://wiki.archlinux.org/title/Systemd/User".to_owned(),
                },
            ),
        ])
        .titled((
            "User services",
            "Per-user services such as the panel and notifications, started once the compositor \
             is up.",
        ))])
    }
}
