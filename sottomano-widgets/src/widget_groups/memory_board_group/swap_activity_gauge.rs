use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::cpu_memory_compute::swap_paging_activity::{Swap, SwapActivity};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::byte_per_second,
    ratio::ratio,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const SWAP_WARNING_SHARE: f64 = 0.9;
const NOTICEABLE_PAGING: ByteSize = ByteSize::mib(1);

fn paging_rate(bytes_per_second: ByteSize) -> Quantity {
    Quantity::transfer_rate(InformationRate::new::<byte_per_second>(
        bytes_per_second.as_u64() as f64,
    ))
}

fn swap_share(activity: &SwapActivity) -> Option<Ratio> {
    match activity.swap {
        Swap::Configured { used, total } if total.as_u64() > 0 => Some(Ratio::new::<ratio>(
            used.as_u64() as f64 / total.as_u64() as f64,
        )),
        Swap::Configured { .. } | Swap::NotConfigured => None,
    }
}

impl GroupItemReading for SwapActivity {
    fn name(&self) -> &'static str {
        "Swap"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowsClockwise(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        let (direction, busiest) = if self.paged_in_per_second >= self.paged_out_per_second {
            ("in", self.paged_in_per_second)
        } else {
            ("out", self.paged_out_per_second)
        };
        (busiest >= NOTICEABLE_PAGING).then(|| {
            format!(
                "{direction} {}",
                paging_rate(busiest).localized_text(locale)
            )
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match swap_share(self) {
            Some(share) if share.get::<ratio>() > SWAP_WARNING_SHARE => Severity::Warning,
            _ => Severity::Normal,
        }
        .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        swap_share(self)
    }

    fn panel_visibility(&self) -> PanelVisibility {
        match self.swap {
            Swap::NotConfigured => PanelVisibility::Hidden,
            Swap::Configured { .. } => PanelVisibility::Shown,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let occupancy_rows = match self.swap {
            Swap::NotConfigured => vec![LabeledValue::new("Swap", "none")],
            Swap::Configured { used, total } => vec![
                LabeledValue::new("Used", Quantity::size(used)),
                LabeledValue::new("Total", Quantity::size(total)),
            ],
        };
        HoverCardBody::dictionary(occupancy_rows.into_iter().chain([
            LabeledValue::new("Swapped in", paging_rate(self.paged_in_per_second)),
            LabeledValue::new("Swapped out", paging_rate(self.paged_out_per_second)),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::ongoing_toast::reading_display_state_alert_severity;

    fn swap_with(used_gibibytes: u64, paged_in_per_second: ByteSize) -> SwapActivity {
        SwapActivity {
            swap: Swap::Configured {
                used: ByteSize::gib(used_gibibytes),
                total: ByteSize::gib(32),
            },
            paged_in_per_second,
            paged_out_per_second: ByteSize::b(0),
        }
    }

    #[test]
    fn a_missing_swap_is_hidden() {
        let without_swap = SwapActivity {
            swap: Swap::NotConfigured,
            paged_in_per_second: ByteSize::b(0),
            paged_out_per_second: ByteSize::b(0),
        };
        assert_eq!(without_swap.panel_visibility(), PanelVisibility::Hidden);
    }

    #[test]
    fn a_nearly_full_swap_warns_without_a_toast() {
        let nearly_full = swap_with(31, ByteSize::b(0));
        assert_eq!(
            nearly_full.state(Clock::real()),
            ReadingDisplayState::Warning
        );
        assert_eq!(
            reading_display_state_alert_severity(&nearly_full, Clock::real()),
            None
        );
    }

    #[test]
    fn only_noticeable_paging_shows_a_label() {
        assert_eq!(
            swap_with(20, ByteSize::kib(4)).label(Clock::real(), Locale::en_US),
            None
        );
        let label = swap_with(20, ByteSize::mib(120))
            .label(Clock::real(), Locale::en_US)
            .expect("busy paging has a label");
        assert!(label.starts_with("in "), "{label}");
    }
}
