use sottomano_desktop_sources::steam_client_files::steam_content_activity::SteamContentActivity;

use super::opens_downloads;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
};

pub(super) fn download_rate_indicator(activity: &SteamContentActivity) -> GroupIndicator<'_> {
    let rate = format!("{:.1} Mbit/s", activity.download_rate_megabits_per_second);
    opens_downloads(
        GroupIndicator::glyph_only(
            "Download rate",
            Phosphor::DownloadSimple(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_label(ReadoutLabel::Labeled(rate)),
    )
}
