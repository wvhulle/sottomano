use chrono::Locale;
use sottomano_desktop_sources::steam_client_files::steam_content_activity::SteamContentActivity;

use super::opens_downloads;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    widgets::localized_quantities::Quantity,
};

pub(super) fn download_rate_indicator(
    activity: &SteamContentActivity,
    locale: Locale,
) -> GroupIndicator<'_> {
    opens_downloads(
        GroupIndicator::glyph_only(
            "Download rate",
            Phosphor::DownloadSimple(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_label(ReadoutLabel::Labeled(
            Quantity::transfer_rate(activity.download_rate).localized_text(locale),
        )),
    )
}

pub(super) fn download_queue_indicator<'activity>() -> GroupIndicator<'activity> {
    opens_downloads(GroupIndicator::glyph_only(
        "Download queue",
        Phosphor::Queue(Weight::Regular).into(),
        IndicatorState::Idle,
    ))
}
