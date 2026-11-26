use super::IndicatorListing;
use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
};

fn named_indicator(indicator_name: &str) -> GroupIndicator<'static> {
    GroupIndicator::glyph_only(
        indicator_name.to_owned(),
        IconGlyph::default(),
        IndicatorState::Idle,
    )
}

fn listed_names(listing: &IndicatorListing<'_>) -> Vec<String> {
    listing
        .iter()
        .map(|indicator| indicator.indicator_name().into_owned())
        .collect()
}

#[test]
fn listings_that_all_read_stay_listed() {
    let combined: IndicatorListing<'_> = [
        IndicatorListing::Listed(vec![named_indicator("first")]),
        IndicatorListing::Listed(Vec::new()),
    ]
    .into_iter()
    .collect();
    assert!(matches!(combined, IndicatorListing::Listed(_)));
    assert_eq!(listed_names(&combined), ["first"]);
}

#[test]
fn one_unread_source_makes_the_combination_incomplete() {
    let combined: IndicatorListing<'_> = [
        IndicatorListing::Incomplete {
            listed_indicators: vec![named_indicator("first")],
            unread_sources: vec![named_indicator("unread")],
        },
        IndicatorListing::Listed(vec![named_indicator("second")]),
    ]
    .into_iter()
    .collect();
    let IndicatorListing::Incomplete {
        listed_indicators,
        unread_sources,
    } = &combined
    else {
        panic!("an unread source keeps the listing incomplete");
    };
    assert_eq!(listed_indicators.len(), 2);
    assert_eq!(unread_sources.len(), 1);
    assert_eq!(listed_names(&combined), ["first", "second", "unread"]);
}

#[test]
fn no_listings_combine_into_an_empty_reading() {
    let combined: IndicatorListing<'_> = [].into_iter().collect();
    assert!(matches!(combined, IndicatorListing::Listed(listed) if listed.is_empty()));
}
