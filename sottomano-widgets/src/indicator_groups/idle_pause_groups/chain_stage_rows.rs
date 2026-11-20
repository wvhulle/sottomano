use std::time::Duration;

use chrono::Locale;
use sottomano_desktop_sources::login_session_presence::system_hibernate_delay::HibernateDelay;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    widgets::localized_durations::duration_text,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HibernateDelayApply {
    Settled,
    Applying,
    Failed(String),
}

fn labeled_readout(label: String) -> IndicatorReadout {
    IndicatorReadout {
        label: ReadoutLabel::Labeled(label),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    }
}

pub(super) fn system_hibernate_delay_row<'row>(
    wanted_delay: Duration,
    system_delay: HibernateDelay,
    apply_progress: &HibernateDelayApply,
    locale: Locale,
) -> GroupIndicator<'row> {
    let (readout_text, display_state) = match (system_delay, apply_progress) {
        (_, HibernateDelayApply::Applying) => ("applying…".to_owned(), IndicatorState::Pending),
        (HibernateDelay::Configured(delay), _) if delay == wanted_delay => {
            (duration_text(delay, locale), IndicatorState::Idle)
        }
        (HibernateDelay::Configured(delay), _) => {
            (duration_text(delay, locale), IndicatorState::Warning)
        }
        (HibernateDelay::BatteryEstimated, _) => {
            ("battery estimate".to_owned(), IndicatorState::Warning)
        }
    };
    GroupIndicator::glyph_only(
        "System hibernate delay",
        Phosphor::Snowflake(Weight::Regular).into(),
        display_state,
    )
    .with_readout(labeled_readout(readout_text))
}

pub(super) fn hibernate_delay_failure_row<'row>(failure_text: &str) -> GroupIndicator<'row> {
    GroupIndicator::glyph_only(
        "Hibernate delay not changed",
        Phosphor::Warning(Weight::Regular).into(),
        IndicatorState::Failed,
    )
    .with_readout(labeled_readout(failure_text.to_owned()))
}
