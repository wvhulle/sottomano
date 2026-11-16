use std::num::NonZeroUsize;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::swap_paging_activity::{Swap, SwapActivity, SwappedProcesses},
};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::byte_per_second,
    ratio::ratio,
};

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::hover_card_body::{Section, SectionTable, SectionTitle, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const SWAP_WARNING_SHARE: f64 = 0.9;
const NOTICEABLE_PAGING: ByteSize = ByteSize::mib(1);
const LISTED_PROCESSES: NonZeroUsize = NonZeroUsize::new(10).unwrap();

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

fn swapped_process_table(swapped: &SwappedProcesses) -> SectionTable {
    dictionary_table(
        swapped.processes.iter().map(|process| {
            LabeledValue::new(process.name.clone(), Quantity::size(process.swapped))
        }),
    )
    .truncated_to(LISTED_PROCESSES)
}

pub fn swapped_processes_sections(swapped_processes: &SwappedProcesses) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Swap use",
            "Private memory of each process that sits in swap; swapped shared memory is not \
             counted.",
        ),
        swapped_process_table(swapped_processes),
    )]
}

pub fn swap_activity_indicator(activity: &SwapActivity, locale: Locale) -> ReadingRow<'_> {
    let state = match swap_share(activity) {
        Some(share) if share.get::<ratio>() > SWAP_WARNING_SHARE => Severity::Warning,
        _ => Severity::Normal,
    }
    .into();
    ReadingRow {
        readout: IndicatorReadout {
            label: paging_label(activity, locale),
            gauge: match swap_share(activity) {
                Some(fill) => ReadoutGauge::Gauged { fill, live: fill },
                None => ReadoutGauge::Ungauged,
            },
            badge: ReadoutBadge::Unbadged,
        },
        panel_visibility: match activity.swap {
            Swap::NotConfigured => PanelVisibility::Hidden,
            Swap::Configured { .. } => PanelVisibility::Shown,
        },
        ..ReadingRow::glyph_only(
            SwapActivity::NAME,
            Phosphor::ArrowsClockwise(Weight::Regular).into(),
            state,
            Box::new(move |_| swap_activity_hover_card(activity)),
        )
    }
}

fn paging_label(activity: &SwapActivity, locale: Locale) -> ReadoutLabel {
    let (direction, busiest) = if activity.paged_in_per_second >= activity.paged_out_per_second {
        ("in", activity.paged_in_per_second)
    } else {
        ("out", activity.paged_out_per_second)
    };
    if busiest >= NOTICEABLE_PAGING {
        ReadoutLabel::Labeled(format!(
            "{direction} {}",
            paging_rate(busiest).localized_text(locale)
        ))
    } else {
        ReadoutLabel::Unlabeled
    }
}

pub fn swap_activity_hover_card(activity: &SwapActivity) -> Vec<Section> {
    let occupancy_rows = match activity.swap {
        Swap::NotConfigured => vec![LabeledValue::new("Swap", "none")],
        Swap::Configured { used, total } => vec![
            LabeledValue::new("Used", Quantity::size(used)),
            LabeledValue::new("Total", Quantity::size(total)),
        ],
    };
    vec![Section::new(
        "Usage",
        dictionary_table(occupancy_rows.into_iter().chain([
            LabeledValue::new("Swapped in", paging_rate(activity.paged_in_per_second)),
            LabeledValue::new("Swapped out", paging_rate(activity.paged_out_per_second)),
        ])),
    )]
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::cpu_memory_compute::swap_paging_activity::SwappedProcess;

    use super::*;
    use crate::{
        group_indicators::{
            group_indicator::indicator_toast_severity, indicator_state::IndicatorState,
        },
        widgets::keyed_record_table::KeyedTableRow,
    };

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
        assert_eq!(
            swap_activity_indicator(&without_swap, Locale::default()).panel_visibility,
            PanelVisibility::Hidden
        );
    }

    #[test]
    fn a_nearly_full_swap_warns_without_a_toast() {
        let nearly_full = swap_with(31, ByteSize::b(0));
        let nearly_full_row = swap_activity_indicator(&nearly_full, Locale::default());
        assert_eq!(nearly_full_row.shown_state(), IndicatorState::Warning);
        assert_eq!(indicator_toast_severity(&nearly_full_row), None);
    }

    #[test]
    fn swapped_processes_beyond_the_listed_count_end_in_an_omitted_row() {
        let swapped = SwappedProcesses {
            processes: (0..12)
                .map(|rank| SwappedProcess {
                    pid: 100 + rank,
                    name: format!("process-{rank}"),
                    swapped: ByteSize::mib(u64::try_from(100 - rank).unwrap()),
                })
                .collect(),
        };
        let table = swapped_process_table(&swapped);
        assert_eq!(table.rows().len(), 11);
        assert_eq!(
            table.rows().last(),
            Some(&KeyedTableRow::OmittedRecordCount(
                NonZeroUsize::new(2).unwrap()
            ))
        );
    }

    #[test]
    fn only_noticeable_paging_shows_a_label() {
        assert_eq!(
            paging_label(&swap_with(20, ByteSize::kib(4)), Locale::en_US),
            ReadoutLabel::Unlabeled
        );
        let ReadoutLabel::Labeled(label) =
            paging_label(&swap_with(20, ByteSize::mib(120)), Locale::en_US)
        else {
            panic!("busy paging has a label");
        };
        assert!(label.starts_with("in "), "{label}");
    }
}
