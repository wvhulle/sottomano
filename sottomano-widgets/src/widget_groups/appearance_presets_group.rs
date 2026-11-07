use crate::{
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody, Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        shell_layer_appearance::LookComparison,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearancePresetsTarget {
    Preset(AppearancePreset),
}

impl From<AppearancePresetsTarget> for HoverTarget {
    fn from(target: AppearancePresetsTarget) -> Self {
        HoverTarget::AppearancePresets(target)
    }
}

#[derive(Debug, Clone)]
pub enum AppearancePresetsMessage {
    Apply(AppearancePreset),
    Expansion(WidgetGroupExpansionMessage),
}

fn preset_group_item<'a>(
    preset: AppearancePreset,
    current: &ShellAppearance,
) -> GroupItem<'a, AppearancePresetsMessage> {
    let display_state = match preset.applied_to(*current).look_comparison(current) {
        LookComparison::Matching => ReadingDisplayState::On,
        LookComparison::Differing => ReadingDisplayState::Idle,
    };
    GroupItem::composed(
        ComposedGroupItemFace {
            name: preset.name().into(),
            icon: GroupItemIcon::IconGlyph {
                glyph: preset.icon().into(),
                state: display_state,
            },
        },
        GroupItemHoverArea::WholeItem(AppearancePresetsTarget::Preset(preset).into()),
        GroupItemValue::Nothing,
    )
    .on_press(preset.press_does(), AppearancePresetsMessage::Apply(preset))
}

#[derive(Default)]
pub struct AppearancePresetsGroup {
    expansion: WidgetGroupExpansion,
}

impl AppearancePresetsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update_expansion(&mut self, message: WidgetGroupExpansionMessage) {
        self.expansion.update(message);
    }

    #[must_use]
    pub fn hover_card_for(&self, target: AppearancePresetsTarget) -> HoverCard {
        match target {
            AppearancePresetsTarget::Preset(preset) => HoverCard::sourced(
                HoverCardBody::sections([Section::note(preset.description())]),
                DeliveryTiming::LivePush,
            ),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        current: &ShellAppearance,
    ) -> WidgetGroupContent<'a, AppearancePresetsMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::AppearancePresets,
            &self.expansion,
            AppearancePresetsMessage::Expansion,
            AppearancePreset::ALL
                .into_iter()
                .map(|preset| preset_group_item(preset, current)),
        )
        .staying_in_menu_during_alerts()
    }
}
