use std::borrow::Cow;

use corredo::gauges::{DurationTimeline, TimelineDensity};

use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        group_indicator::{DescribeCard, GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
};

pub fn timeline_row<'a>(
    name: impl Into<Cow<'static, str>>,
    glyph: IconGlyph,
    drawn_timeline: impl Fn() -> DurationTimeline + 'a,
    card: DescribeCard<'a>,
) -> GroupIndicator<'a> {
    GroupIndicator::composed(
        name,
        IndicatorIcon::IconGlyph {
            glyph,
            state: IndicatorState::Idle,
        },
        IndicatorValue::drawn(move |_| {
            drawn_timeline()
                .density(TimelineDensity::CompactRail)
                .view()
        }),
        card,
    )
}
