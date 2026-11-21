use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        shell_layer_appearance::LookComparison,
    },
};

#[derive(Debug, Clone)]
pub enum AppearancePresetsMessage {
    Apply(AppearancePreset),
    Expansion(IndicatorGroupExpansionMessage),
}

fn preset_group_indicator<'view>(
    preset: AppearancePreset,
    current: &ShellAppearance,
) -> GroupIndicator<'view> {
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
    expansion: IndicatorGroupExpansion,
}

impl AppearancePresetsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update_expansion(&mut self, message: IndicatorGroupExpansionMessage) {
        self.expansion.update(message);
    }

    pub fn group_content<'view>(
        &'view self,
        current: &ShellAppearance,
    ) -> IndicatorGroupContent<'view> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::AppearancePresets,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::AppearancePresets.title(),
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
