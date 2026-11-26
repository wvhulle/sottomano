use sottomano_desktop_sources::SnapshotSource;

use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, hover_card_source_delivery::SourceDelivery},
};

pub enum Staleness<S: SnapshotSource> {
    Current,
    StaleAfterFailure(S::Error),
}

pub enum AbsenceCause<S: SnapshotSource> {
    Awaiting,
    SourceAbsent,
    FetchFailed(S::Error),
}

impl<S: SnapshotSource> AbsenceCause<S> {
    #[must_use]
    pub fn indicator(&self, absent_glyph: IconGlyph) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(S::NAME, absent_glyph, self.shown_state())
            .with_hover_card(Box::new(move |_| vec![self.source_state()]))
    }

    #[must_use]
    pub fn shown_state(&self) -> IndicatorState {
        match self {
            AbsenceCause::Awaiting => IndicatorState::Pending,
            AbsenceCause::SourceAbsent => IndicatorState::Off,
            AbsenceCause::FetchFailed(_) => IndicatorState::Failed,
        }
    }

    #[must_use]
    pub fn source_state(&self) -> Section {
        match self {
            AbsenceCause::Awaiting => Section::source_state::<S>(SourceDelivery::Loading, ""),
            AbsenceCause::SourceAbsent => {
                Section::source_state::<S>(SourceDelivery::Failed, "not present on this system")
            }
            AbsenceCause::FetchFailed(fetch_failure) => {
                Section::source_state::<S>(SourceDelivery::Failed, fetch_failure.to_string())
            }
        }
    }
}

pub fn failed_source_indicator<S: SnapshotSource>(
    fetch_failure: &S::Error,
    absent_glyph: IconGlyph,
) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(S::NAME, absent_glyph, IndicatorState::Failed).with_hover_card(
        Box::new(move |_| {
            vec![Section::source_state::<S>(
                SourceDelivery::Failed,
                fetch_failure.to_string(),
            )]
        }),
    )
}

#[must_use]
pub fn awaiting_source_indicator<'view, S: SnapshotSource>(
    absent_glyph: IconGlyph,
) -> GroupIndicator<'view> {
    GroupIndicator::glyph_only(S::NAME, absent_glyph, IndicatorState::Pending).with_hover_card(
        Box::new(|_| vec![Section::source_state::<S>(SourceDelivery::Loading, "")]),
    )
}
