use sottomano_desktop_sources::steam_client_files::{
    protondb_compatibility_summary::{
        ProtonDbCompatibility, ProtonDbSummary, ProtonDbTier, ReportConfidence,
        compatibility_page_url,
    },
    steam_log_lines::SteamAppId,
};

use super::SteamGamesInteraction;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

const fn tier_text(tier: ProtonDbTier) -> &'static str {
    match tier {
        ProtonDbTier::Platinum => "Platinum",
        ProtonDbTier::Gold => "Gold",
        ProtonDbTier::Silver => "Silver",
        ProtonDbTier::Bronze => "Bronze",
        ProtonDbTier::Borked => "Borked",
        ProtonDbTier::Pending => "Pending",
    }
}

const fn confidence_text(confidence: ReportConfidence) -> &'static str {
    match confidence {
        ReportConfidence::Strong => "strong",
        ReportConfidence::Good => "good",
        ReportConfidence::Moderate => "moderate",
        ReportConfidence::Low => "low",
        ReportConfidence::Inadequate => "inadequate",
    }
}

fn rating_rows(summary: &ProtonDbSummary) -> [LabeledValue; 4] {
    [
        LabeledValue::new(
            "Rating",
            format!(
                "{} ({} confidence)",
                tier_text(summary.tier),
                confidence_text(summary.confidence)
            ),
        ),
        LabeledValue::new("Trending", tier_text(summary.trending_tier)),
        LabeledValue::new("Best reported", tier_text(summary.best_reported_tier)),
        LabeledValue::new("Reports", Quantity::count(u64::from(summary.total))),
    ]
}

fn compatibility_sections(
    app_id: SteamAppId,
    compatibility: &ProtonDbCompatibility,
) -> Vec<Section> {
    let page_link = LabeledValue::new(
        "Reports page",
        ValueCell::Link {
            label: "ProtonDB".to_owned(),
            url: compatibility_page_url(app_id),
        },
    );
    match compatibility {
        ProtonDbCompatibility::Rated(summary) => vec![Section::new(
            "Proton compatibility",
            dictionary_table(rating_rows(summary).into_iter().chain([page_link])),
        )],
        ProtonDbCompatibility::Unreported => vec![Section::new(
            "Proton compatibility",
            dictionary_table([LabeledValue::new("Rating", "no reports yet"), page_link]),
        )],
    }
}

pub(super) fn protondb_rating_indicator(
    app_id: SteamAppId,
    compatibility_summaries: &HoverFetchedSnapshot<ProtonDbCompatibility>,
) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        "ProtonDB",
        Phosphor::LinuxLogo(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_hover_card(Box::new(move |_| {
        compatibility_summaries
            .sections_described_by(|compatibility| compatibility_sections(app_id, compatibility))
    }))
    .fetching_on_hover(
        SteamGamesInteraction::StartCompatibilityFetch(app_id),
        SteamGamesInteraction::StopCompatibilityFetch,
    )
}
