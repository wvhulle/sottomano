use corredo::popovers::{MenuItem, MenuRow, MenuRowContent, MenuRowEmphasis, anchor_marker};
use iced::Element;
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, RowScroll},
        indicator_gestures::IndicatorGesture,
        indicator_group_content::{GroupEvent, IndicatorListing},
        indicator_group_identity::{IndicatorGroupIdentity, IndicatorGroupTitle},
    },
    group_menus::{
        group_menu_state::{
            GroupMenuPin, GroupMenuState, IndicatorPin, PanelPin, PanelPinDuration,
        },
        group_menu_view::GroupMenuMessenger,
    },
    widgets::glyph_button,
};

pub(crate) struct GroupMenuEntry<'view> {
    pub group_identity: IndicatorGroupIdentity,
    pub group_title: IndicatorGroupTitle,
    pub headline_indicator: GroupIndicator<'view>,
    pub listing: IndicatorListing<'view>,
}

pub(crate) fn one_entry_per_identity<'view>(
    entries: impl IntoIterator<Item = GroupMenuEntry<'view>>,
) -> Vec<GroupMenuEntry<'view>> {
    entries.into_iter().fold(Vec::new(), |mut merged, entry| {
        match merged.iter().position(|existing: &GroupMenuEntry<'view>| {
            existing.group_identity == entry.group_identity
        }) {
            Some(existing_position) => {
                let existing = merged.remove(existing_position);
                merged.insert(
                    existing_position,
                    GroupMenuEntry {
                        listing: [existing.listing, entry.listing].into_iter().collect(),
                        ..existing
                    },
                );
            }
            None => merged.push(entry),
        }
        merged
    })
}

pub(crate) struct GroupMenuRowContext<'menu> {
    pub config: &'menu Config,
    pub menu: &'menu GroupMenuMessenger,
    pub group_menus: &'menu GroupMenuState,
    pub clock: Clock,
}

pub(crate) fn group_header<'view>(
    layout: &GroupMenuRowContext<'_>,
    entry: &GroupMenuEntry<'view>,
    emphasis: MenuRowEmphasis,
) -> MenuRowContent<'view, GroupEvent> {
    MenuRowContent::new(entry.group_title.to_string())
        .icon(entry.headline_indicator.menu_icon(layout.config))
        .value(
            entry
                .headline_indicator
                .menu_value(layout.config, layout.clock),
        )
        .trailing([group_pin(layout, entry, emphasis)])
}

fn group_pin<'view>(
    layout: &GroupMenuRowContext<'_>,
    entry: &GroupMenuEntry<'view>,
    emphasis: MenuRowEmphasis,
) -> Element<'view, GroupEvent> {
    let pinned_entry = GroupMenuPin::Group(entry.group_identity.clone());
    anchor_marker(
        glyph_button(
            Phosphor::PushPin(Weight::Regular),
            layout.config.theme.font.icon_size_px,
            emphasis.secondary_glyph_color(),
            layout.menu.toggle_pin(pinned_entry.clone()),
        ),
        pinned_entry,
    )
}

pub(crate) fn indicator_entry<'view>(
    layout: &GroupMenuRowContext<'_>,
    group_identity: &IndicatorGroupIdentity,
    indicator: &GroupIndicator<'view>,
) -> MenuItem<'view, GroupEvent> {
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
    use super::{GroupMenuEntry, IndicatorGroupIdentity, IndicatorListing, one_entry_per_identity};
    use crate::{
        glyph_icons::icon_glyph::IconGlyph,
        group_indicators::{
            group_indicator::GroupIndicator,
            indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupTitle},
            indicator_state::IndicatorState,
        },
    };

    fn glyph_group_entry<'row>(
        group_identity: IndicatorGroupIdentity,
        title: &str,
    ) -> GroupMenuEntry<'row> {
        GroupMenuEntry {
            group_identity,
            group_title: IndicatorGroupTitle::Named(title.to_owned().into()),
            headline_indicator: GroupIndicator::glyph_only(
                title.to_owned(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            listing: IndicatorListing::Listed(Vec::new()),
        }
    }

    #[test]
    fn groups_of_one_identity_share_a_row() {
        let gpu = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Gpu);
        let entries = one_entry_per_identity([
            glyph_group_entry(gpu.clone(), "GPU"),
            glyph_group_entry(gpu, "GPU"),
        ]);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn instances_of_one_family_keep_their_own_rows() {
        let entries = one_entry_per_identity([
            glyph_group_entry(
                IndicatorGroupIdentity::durable(IndicatorGroupFamily::PhysicalDrive, "sda"),
                "a",
            ),
            glyph_group_entry(
                IndicatorGroupIdentity::durable(IndicatorGroupFamily::PhysicalDrive, "sdb"),
                "b",
            ),
        ]);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn a_player_named_audio_keeps_its_own_row() {
        let entries = one_entry_per_identity([
            glyph_group_entry(
                IndicatorGroupIdentity::sole(IndicatorGroupFamily::Audio),
                "Audio",
            ),
            glyph_group_entry(
                IndicatorGroupIdentity::durable(IndicatorGroupFamily::Media, "Audio"),
                "Audio",
            ),
        ]);
        assert_eq!(entries.len(), 2);
    }
}
