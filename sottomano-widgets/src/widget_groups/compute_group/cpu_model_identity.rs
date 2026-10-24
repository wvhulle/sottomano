use chrono::Locale;
use sottomano_sources::host_machine_identity::udev_hardware_identity::CpuModel;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
    },
};

impl BarWidget for CpuModel {
    fn visible_in_bar(&self) -> bool {
        self.identity.name.is_some()
    }

    fn name(&self) -> &'static str {
        "CPU model"
    }

    fn icon(&self) -> Glyph {
        Phosphor::IdentificationCard(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.architecture.microarchitecture.clone()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::from(self)])
    }
}
