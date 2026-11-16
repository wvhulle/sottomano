use crate::{
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::LookComparison,
    },
};

#[derive(Debug, Clone)]
pub enum AppearancePresetsMessage {
    Apply(AppearancePreset),
    Expansion(WidgetGroupExpansionMessage),
}

fn preset_group_indicator<'a>(
    preset: AppearancePreset,
    current: &ShellAppearance,
) -> GroupIndicator<'a> {
    let display_state = match preset.applied_to(*current).look_comparison(current) {
        LookComparison::Matching => IndicatorState::On,
        LookComparison::Differing => IndicatorState::Idle,
    };
    GroupIndicator::composed(
        preset.name(),
        IndicatorIcon::IconGlyph {
            glyph: preset.icon().into(),
            state: display_state,
        },
        IndicatorValue::Nothing,
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

    pub fn group_content<'a>(&'a self, current: &ShellAppearance) -> WidgetGroupContent<'a> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::AppearancePresets,
            GroupSummary::glyph(
                Phosphor::Palette(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            AppearancePresetsMessage::Expansion,
            AppearancePreset::ALL
                .into_iter()
                .map(|preset| preset_group_indicator(preset, current)),
        )
    }
}
