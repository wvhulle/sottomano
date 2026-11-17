use corredo::popovers::{MenuItem, MenuRow, MenuRowContent, MenuRowEmphasis, anchor_marker};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, RowScroll},
        indicator_gestures::IndicatorGesture,
    },
    group_menus::{
        group_menu_state::{
            GroupMenuPin, GroupMenuState, IndicatorPin, PanelPin, PanelPinDuration,
        },
        group_menu_view::GroupMenuMessenger,
    },
    hover_cards::marked_by_menu_pin,
    indicator_groups::{
        indicator_group_content::GroupEvent,
        indicator_group_identity::{IndicatorGroupIdentity, IndicatorGroupTitle},
    },
    widgets::{
        glyph_button,
        phosphor::{Phosphor, Weight},
    },
};

pub(crate) struct GroupMenuGroupRow<'a> {
    pub group_identity: IndicatorGroupIdentity,
    pub group_title: IndicatorGroupTitle,
    pub headline_indicator: GroupIndicator<'a>,
    pub indicators: Vec<GroupIndicator<'a>>,
}

pub(crate) fn one_row_per_identity<'a>(
    rows: impl IntoIterator<Item = GroupMenuGroupRow<'a>>,
) -> Vec<GroupMenuGroupRow<'a>> {
    rows.into_iter().fold(Vec::new(), |mut merged, row| {
        match merged
            .iter_mut()
            .find(|existing: &&mut GroupMenuGroupRow<'a>| {
                existing.group_identity == row.group_identity
            }) {
            Some(existing) => existing.indicators.extend(row.indicators),
            None => merged.push(row),
        }
        merged
    })
}

pub(crate) struct GroupMenuRowContext<'p> {
    pub config: &'p Config,
    pub menu: &'p GroupMenuMessenger,
    pub group_menus: &'p GroupMenuState,
    pub clock: Clock,
}

pub(crate) fn group_header<'a>(
    layout: &GroupMenuRowContext<'_>,
    entry_identity: &IndicatorGroupIdentity,
    group_title: &IndicatorGroupTitle,
    headline_indicator: &GroupIndicator<'a>,
    emphasis: MenuRowEmphasis,
) -> MenuRowContent<'a, GroupEvent> {
    let pinned_entry = GroupMenuPin::Group(entry_identity.clone());
    let pin = marked_by_menu_pin(
        glyph_button(
            Phosphor::PushPin(Weight::Regular),
            layout.config.theme.font.icon_size_px,
            emphasis.secondary_glyph_color(),
            layout.menu.toggle_pin(pinned_entry.clone()),
        ),
        pinned_entry,
    );
    MenuRowContent::new(group_title.to_string())
        .icon(headline_indicator.menu_icon(layout.config))
        .value(headline_indicator.menu_value(layout.config, layout.clock))
        .trailing([pin])
}

pub(crate) fn indicator_entry<'a>(
    layout: &GroupMenuRowContext<'_>,
    group_identity: &IndicatorGroupIdentity,
    indicator: &GroupIndicator<'a>,
) -> MenuItem<'a, GroupEvent> {
    let emphasis = indicator.row_emphasis(
        group_identity,
        layout
            .group_menus
            .inspection
            .inspected_indicator(layout.menu.topic),
    );
    let indicator_key = indicator.indicator_key(group_identity);
    let indicator_pin = layout.group_menus.indicator_pin(
        layout.menu.topic,
        group_identity,
        indicator,
        layout.config.group_menus.alert_promotion_severity,
    );
    let pin_button = |entry, pin_weight, pin_color: fn(&iced::Theme) -> iced::Color| {
        glyph_button(
            Phosphor::PushPin(pin_weight),
            layout.config.theme.font.icon_size_px,
            pin_color,
            layout.menu.toggle_pin(entry),
        )
    };
    let indicator_pins = match indicator_pin {
        IndicatorPin::Unpinnable => Vec::new(),
        IndicatorPin::Pinned(PanelPin {
            entry,
            duration: PanelPinDuration::Permanent,
            ..
        }) => vec![pin_button(entry, Weight::Fill, emphasis.text_color())],
        IndicatorPin::Unpinned(entry)
        | IndicatorPin::Pinned(PanelPin {
            entry,
            duration: PanelPinDuration::UntilAlertEnds,
            ..
        }) => vec![pin_button(
            entry,
            Weight::Regular,
            emphasis.secondary_glyph_color(),
        )],
    };
    let content = MenuRowContent::new(indicator.indicator_name())
        .icon(indicator.menu_icon(layout.config))
        .value(indicator.menu_row_value(layout.config, layout.clock))
        .trailing(indicator_pins);
    let gestures = indicator.gestures.clone();
    let row = MenuRow::new(content)
        .emphasis(emphasis)
        .on_enter(layout.menu.inspect_row(indicator_key.clone()));
    let row = match gestures.left {
        IndicatorGesture::Performs { message, .. } => {
            row.on_press(GroupEvent::Interaction(message))
        }
        IndicatorGesture::Inert => row,
    };
    let row = match gestures.right {
        IndicatorGesture::Performs { message, .. } => {
            row.on_right_press(GroupEvent::Interaction(message))
        }
        IndicatorGesture::Inert => row,
    };
    let row = match indicator.row_scroll() {
        RowScroll::AdjustsValue(adjust_by_scroll) => {
            row.on_scroll(move |delta| GroupEvent::Interaction(adjust_by_scroll(delta)))
        }
        RowScroll::Inert => row,
    };
    MenuItem::new(row).wrapped(move |element| anchor_marker(element, indicator_key))
}

#[cfg(test)]
mod tests {
    use super::{GroupMenuGroupRow, IndicatorGroupIdentity, one_row_per_identity};
    use crate::{
        group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
        indicator_groups::indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupTitle},
        widgets::icon_glyph::IconGlyph,
    };

    fn row_of<'row>(
        group_identity: IndicatorGroupIdentity,
        title: &str,
    ) -> GroupMenuGroupRow<'row> {
        GroupMenuGroupRow {
            group_identity,
            group_title: IndicatorGroupTitle::Named(title.to_owned().into()),
            headline_indicator: GroupIndicator::glyph_only(
                title.to_owned(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            indicators: Vec::new(),
        }
    }

    #[test]
    fn groups_of_one_identity_share_a_row() {
        let gpu = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Gpu);
        let rows = one_row_per_identity([row_of(gpu.clone(), "GPU"), row_of(gpu, "GPU")]);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn instances_of_one_family_keep_their_own_rows() {
        let rows = one_row_per_identity([
            row_of(
                IndicatorGroupIdentity::durable(IndicatorGroupFamily::PhysicalDrive, "sda"),
                "a",
            ),
            row_of(
                IndicatorGroupIdentity::durable(IndicatorGroupFamily::PhysicalDrive, "sdb"),
                "b",
            ),
        ]);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn a_player_named_audio_keeps_its_own_row() {
        let rows = one_row_per_identity([
            row_of(
                IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio),
                "Audio",
            ),
            row_of(
                IndicatorGroupIdentity::durable(IndicatorGroupFamily::Media, "Audio"),
                "Audio",
            ),
        ]);
        assert_eq!(rows.len(), 2);
    }
}
