use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::steam_client_files::steam_app_manifests::InstalledSteamApp;
use uom::si::{f64::Ratio, ratio::ratio};

use super::opens_downloads;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

fn downloaded_share(installed_app: &InstalledSteamApp) -> Ratio {
    let downloaded = installed_app.bytes_downloaded as f64;
    let total = installed_app.bytes_to_download.max(1) as f64;
    Ratio::new::<ratio>((downloaded / total).clamp(0.0, 1.0))
}

pub(super) fn downloading_indicator(
    installed_app: &InstalledSteamApp,
    locale: Locale,
) -> GroupIndicator<'_> {
    let share = downloaded_share(installed_app);
    let percent = (share.get::<ratio>() * 100.0).round();
    opens_downloads(
        GroupIndicator::glyph_only(
            installed_app.name.clone(),
            Phosphor::DownloadSimple(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(format!("{percent}%")),
            gauge: ReadoutGauge::Gauged {
                fill: share,
                live: share,
            },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| {
            vec![Section::new(
                "Downloading",
                dictionary_table([
                    LabeledValue::new(
                        "Downloaded",
                        Quantity::size(ByteSize::b(installed_app.bytes_downloaded)),
                    ),
                    LabeledValue::new(
                        "To download",
                        Quantity::size(ByteSize::b(installed_app.bytes_to_download)),
                    ),
                    LabeledValue::new("Progress", Quantity::share(share).localized_text(locale)),
                ]),
            )]
        })),
    )
}
