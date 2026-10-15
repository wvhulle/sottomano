use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, Section, TooltipBody},
};

pub struct PowerPolicyFace;

impl BarWidget for PowerPolicyFace {
    fn name(&self) -> &'static str {
        "Power policy"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Plug(Weight::Regular).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Decided by", "systemd-logind"),
            KeyValue::new(
                "Wiki",
                Cell::Link {
                    label: "Power management".to_owned(),
                    url: "https://wiki.archlinux.org/title/Power_management".to_owned(),
                },
            ),
        ])
        .titled((
            "Power policy",
            "What the power key, lid and idle timeout do, for every login on this machine.",
        ))])
    }
}
