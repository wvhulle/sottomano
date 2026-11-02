use iced::{Element, border::Radius};
use sottomano_config::Config;

use crate::{
    hover_cards::HoverTarget,
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        configured_theme::text_color,
        context_menu::{ContextMenuLine, ContextMenuRow, Trailing, context_menu},
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        widget_group::{WidgetGroup, WidgetGroupContent},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearancePresetsTarget {
    Menu,
}

impl From<AppearancePresetsTarget> for HoverTarget {
    fn from(target: AppearancePresetsTarget) -> Self {
        HoverTarget::AppearancePresets(target)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AppearancePresetsMessage {
    Apply(AppearancePreset),
}

fn preset_menu_row<'a>(
    preset: AppearancePreset,
    current: &ShellAppearance,
    config: &Config,
) -> ContextMenuLine<'a, AppearancePresetsMessage> {
    ContextMenuLine::Row(ContextMenuRow {
        leading: Some(
            IconGlyph::from(preset.icon()).view(config.theme.font.icon_size_px, text_color),
        ),
        label: Some(preset.name().to_owned()),
        trailing: if preset.is_shown_by(current) {
            Trailing::Note("current".to_owned())
        } else {
            Trailing::Nothing
        },
        enabled: true,
        depth: 0,
        on_press: Some(AppearancePresetsMessage::Apply(preset)),
    })
}

#[derive(Default)]
pub struct AppearancePresetsGroup;

impl WidgetGroup for AppearancePresetsGroup {
    const TITLE: &'static str = "Looks";
}

impl AppearancePresetsGroup {
    #[must_use]
    pub fn hover_card_for(&self, target: AppearancePresetsTarget) -> HoverCard {
        match target {
            AppearancePresetsTarget::Menu => {
                HoverCard::sourced(HoverCardBody::sections([]), DeliveryTiming::LivePush)
            }
        }
    }

    #[must_use]
    pub fn menu_panel<'a>(
        &self,
        config: &'a Config,
        popup_corners: Radius,
        current: &ShellAppearance,
    ) -> Element<'a, AppearancePresetsMessage> {
        context_menu(
            config,
            popup_corners,
            AppearancePreset::ALL
                .into_iter()
                .map(|preset| preset_menu_row(preset, current, config))
                .collect::<Vec<_>>(),
        )
    }

    #[must_use]
    pub fn group_content<'a>(&self) -> WidgetGroupContent<'a, AppearancePresetsMessage> {
        Self::fixed_group(Some(GroupItem::composed(
            ComposedGroupItemFace {
                name: Self::TITLE.into(),
                icon: GroupItemIcon::IconGlyph {
                    glyph: Phosphor::Palette(Weight::Regular).into(),
                    state: ReadingDisplayState::Idle,
                },
            },
            GroupItemHoverArea::WholeItem(AppearancePresetsTarget::Menu.into()),
            GroupItemValue::Nothing,
        )))
    }
}
