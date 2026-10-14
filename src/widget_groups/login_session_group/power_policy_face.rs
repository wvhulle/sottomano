use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{KeyValue, Section, TooltipBody},
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
        TooltipBody::sections([Section::dictionary([KeyValue::new(
            "Decided by",
            "systemd-logind",
        )])
        .titled("Power policy")])
    }
}
