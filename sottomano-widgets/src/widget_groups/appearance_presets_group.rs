use iced::Element;
use sottomano_config::Config;

use crate::{
    hover_tooltips::{HoverTarget, PressHints, hoverable},
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        bar_widget::Glyph,
        configured_theme::text_color,
        context_menu::{MenuLine, MenuRow, Trailing, context_menu},
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            TooltipBody,
            tooltip_source_delivery::{DeliveryTiming, Tooltip},
        },
        widget_group::{GroupContent, GroupEvent, WidgetGroup},
        widget_state::{StateTint, WidgetState},
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
) -> MenuLine<'a, AppearancePresetsMessage> {
    MenuLine::Row(MenuRow {
        leading: Some(Glyph::from(preset.icon()).view(config.theme.font.icon_size_px, text_color)),
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
pub struct AppearancePresetsMenu;

impl WidgetGroup for AppearancePresetsMenu {
    const TITLE: &'static str = "Looks";
}

impl AppearancePresetsMenu {
    #[must_use]
    pub fn tooltip_for(&self, target: AppearancePresetsTarget) -> Tooltip {
        match target {
            AppearancePresetsTarget::Menu => {
                Tooltip::sourced(TooltipBody::sections([]), DeliveryTiming::LivePush)
            }
        }
    }

    #[must_use]
    pub fn menu_panel<'a>(
        &self,
        config: &'a Config,
        current: &ShellAppearance,
    ) -> Element<'a, AppearancePresetsMessage> {
        context_menu(
            config,
            AppearancePreset::ALL
                .into_iter()
                .map(|preset| preset_menu_row(preset, current, config))
                .collect::<Vec<_>>(),
        )
    }

    #[must_use]
    pub fn group_content<'a>(
        &self,
        config: &'a Config,
    ) -> GroupContent<'a, AppearancePresetsMessage> {
        let glyph = Glyph::from(Phosphor::Palette(Weight::Regular));
        Self::fixed_group(Some(GroupItem::element(move |reveal| {
            hoverable(
                glyph.view(config.theme.font.icon_size_px, {
                    let tint = StateTint {
                        state: WidgetState::Idle,
                        opacity: reveal,
                    };
                    move |theme| tint.color(theme)
                }),
                AppearancePresetsTarget::Menu.into(),
                PressHints::default(),
                GroupEvent::Tooltip,
            )
        })))
    }
}
