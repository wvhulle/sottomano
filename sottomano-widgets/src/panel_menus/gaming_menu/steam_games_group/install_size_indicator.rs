use bytesize::ByteSize;
use chrono::Locale;

use super::InstalledGame;
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

pub(super) fn size_text(bytes: u64, locale: Locale) -> String {
    Quantity::size(ByteSize::b(bytes)).localized_text(locale)
}

pub(super) fn install_size_indicator<'library>(
    installed_game: &InstalledGame<'library>,
    locale: Locale,
) -> GroupIndicator<'library> {
    let installed_app = installed_game.installed_app;
    let folder_usage = installed_game.folder_usage;
    let share = installed_game.library_share;
    GroupIndicator::glyph_only(
        "Install size",
        Phosphor::HardDrives(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(size_text(installed_app.size_on_disk_bytes, locale)),
        gauge: ReadoutGauge::Gauged {
            fill: share,
            live: share,
        },
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Installed",
            dictionary_table([
                LabeledValue::new(
                    "Folder",
                    installed_app.install_directory.display().to_string(),
                ),
                LabeledValue::new(
                    "Share of library",
                    Quantity::share(share).localized_text(locale),
                ),
                LabeledValue::new(
                    "Free on its drive",
                    size_text(folder_usage.free_bytes, locale),
                ),
                LabeledValue::new("App id", installed_app.app_id.number.to_string()),
            ]),
        )]
    }))
}
