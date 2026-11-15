use crate::{
    group_items::{
        group_item::{GroupItem, GroupItemValue},
        reading_display_state::ReadingDisplayState,
        source_reading::GroupItemIcon,
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
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
        preset.name(),
        GroupItemIcon::IconGlyph {
            glyph: preset.icon().into(),
            state: display_state,
        },
        GroupItemValue::Nothing,
        Box::new(|_| Vec::new()),
    )
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Description",
            SectionWidget::paragraph(preset.description()),
        )]
    }))
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
    }
}
