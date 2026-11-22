use sottomano_desktop_sources::{
    container_run_state::ContainerRunState,
    container_runtime_facts::ContainerMemoryUse,
    waydroid_container::{
        SessionLcdDensity, SessionUserName, WaydroidListing, WaydroidReadout, WaydroidState,
    },
};

use super::WAYDROID_CONTAINER_UNIT;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::process_usage_cells::approximate_memory_text,
    widgets::value_cells::LabeledValue,
};

#[derive(Clone, Copy)]
pub(super) enum ShownWaydroid<'view> {
    Detailed(&'view WaydroidReadout),
    Listed(&'view WaydroidListing),
}

impl<'view> ShownWaydroid<'view> {
    pub(super) fn state(self) -> &'view WaydroidState {
        match self {
            ShownWaydroid::Detailed(readout) => &readout.state,
            ShownWaydroid::Listed(listing) => &listing.state,
        }
    }

    pub(super) fn container(self) -> ContainerRunState {
        self.state().container_run_state()
    }
}

pub(super) struct WaydroidContainerIndicator<'view> {
    pub(super) shown: ShownWaydroid<'view>,
}

impl<'view> WaydroidContainerIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let label = match self.shown {
            ShownWaydroid::Detailed(WaydroidReadout {
                memory_in_use: ContainerMemoryUse::Measured(memory_in_use),
                ..
            }) if self.shown.container() == ContainerRunState::Running => {
                approximate_memory_text(*memory_in_use)
            }
            ShownWaydroid::Detailed(_) | ShownWaydroid::Listed(_) => {
                self.shown.container().label().to_owned()
            }
        };
        GroupIndicator::glyph_only(
            "Container",
            Phosphor::Package(Weight::Regular).into(),
            self.shown.container().into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let mut detail_entries = vec![
            LabeledValue::new("State", self.shown.container().label()),
            LabeledValue::new("Unit", WAYDROID_CONTAINER_UNIT),
        ];
        if let ShownWaydroid::Detailed(WaydroidReadout {
            memory_in_use: ContainerMemoryUse::Measured(memory_in_use),
            ..
        }) = self.shown
        {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(*memory_in_use),
            ));
        }
        vec![Section::new("Status", dictionary_table(detail_entries))]
    }
}

pub(super) struct WaydroidSessionIndicator<'view> {
    pub(super) state: &'view WaydroidState,
}

impl<'view> WaydroidSessionIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let label = match self.state {
            WaydroidState::Session { session_state, .. } => session_state.to_lowercase(),
            WaydroidState::ContainerOnly | WaydroidState::Absent => "no session".to_owned(),
        };
        let state = match self.state {
            WaydroidState::Session { .. } => IndicatorState::On,
            WaydroidState::ContainerOnly | WaydroidState::Absent => IndicatorState::Off,
        };
        GroupIndicator::glyph_only(
            "Session",
            Phosphor::DeviceMobile(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let WaydroidState::Session {
            session_state,
            user_name,
            lcd_density,
        } = self.state
        else {
            return vec![Section::new(
                "Status",
                dictionary_table([LabeledValue::new("Session", "none")]),
            )];
        };
        let mut detail_entries = vec![LabeledValue::new("Session", session_state.clone())];
        if let SessionUserName::Reported(user_name) = user_name {
            detail_entries.push(LabeledValue::new("User", user_name.clone()));
        }
        if let SessionLcdDensity::Reported(lcd_density) = lcd_density {
            detail_entries.push(LabeledValue::new("Screen density", lcd_density.clone()));
        }
        vec![Section::new("Status", dictionary_table(detail_entries))]
    }
}
