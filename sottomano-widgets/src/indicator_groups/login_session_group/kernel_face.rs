use sottomano_desktop_sources::host_machine_identity::hostname1_os_identity::{
    Chassis, OperatingSystemIdentity,
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
    widgets::{
        value_cells::{LabeledValue, ValueCell},
        vendor_logo::{DistributionIds, VendorLogo},
    },
};

pub struct KernelFace<'a> {
    pub system: &'a OperatingSystemIdentity,
}

impl<'a> KernelFace<'a> {
    pub fn indicator(self) -> GroupIndicator<'a> {
        let glyph = DistributionIds {
            distribution_names: &self.system.distribution_ids,
        }
        .vendor_logo()
        .map_or_else(
            || Phosphor::LinuxLogo(Weight::Regular).into(),
            IconGlyph::from,
        );
        let label_text = self.system.kernel.clone();
        let label = if label_text.is_empty() {
            ReadoutLabel::Unlabeled
        } else {
            ReadoutLabel::Labeled(label_text)
        };
        GroupIndicator::glyph_only("Kernel", glyph, IndicatorState::Idle)
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
                "Host and kernel under this session",
                "The kernel drives this host's hardware and runs every process above it; a new \
                 version takes effect after a reboot.",
            ),
            dictionary_table(
                [
                    Some(LabeledValue::new("Host", self.system.hostname.clone())),
                    Some(LabeledValue::new("Kernel", self.system.kernel.clone())),
                    Some(LabeledValue::new(
                        "OS",
                        self.system.operating_system.clone(),
                    )),
                    match self.system.chassis {
                        Chassis::Undetermined => None,
                        known_chassis => {
                            Some(LabeledValue::new("Chassis", known_chassis.stored_name()))
                        }
                    },
                    Some(LabeledValue::new(
                        "Wiki",
                        ValueCell::BrandedLink {
                            label: "Kernel".to_owned(),
                            url: "https://wiki.archlinux.org/title/Kernel".to_owned(),
                            glyph: SimpleIcon::ArchLinux.into(),
                        },
                    )),
                ]
                .into_iter()
                .flatten(),
            ),
        )]
    }
}
