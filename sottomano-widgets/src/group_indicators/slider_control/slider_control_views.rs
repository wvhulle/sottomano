use corredo::{
    changing_text::{MeasuredReadout, ReadoutLabels, steady_width_text},
    value_controls::ExpandingSliderParts,
};
use iced::{
    Alignment, Element, Length,
    alignment::Horizontal,
    widget::{Row, text, text::Wrapping},
};
use sottomano_toml_config::Config;

use super::{
    ControlMessage, SliderCaption, SliderControl, SliderLabelFormat, SliderShownIndicator,
};
use crate::{
    group_indicators::indicator_drawing::{indicator_drawing, labeled_indicator},
    widgets::{
        slider_look::slider_look,
        text::{panel_text, resolved_text_face, row_key_label},
    },
};

const CAPTION_WIDTH_IN_TEXT_SIZES: f32 = 7.0;

impl SliderControl {
    #[must_use]
    pub fn view<'view>(
        &'view self,
        config: &'view Config,
        shown_indicator: SliderShownIndicator,
        reveal: f32,
    ) -> Element<'view, ControlMessage> {
        let SliderShownIndicator { face, readout } = shown_indicator;
        self.expanding_slider.view(
            ExpandingSliderParts {
                collapsed: Box::new(move || match self.label_format {
                    SliderLabelFormat::Labeled(_) => {
                        labeled_indicator(config, &face, &readout, reveal)
                    }
                    SliderLabelFormat::Unlabeled => {
                        indicator_drawing(config, &face, &readout, reveal)
                    }
                }),
                face: Box::new(move || self.state_glyph(config)),
                readout: Box::new(move || self.readout_view(config)),
                look: Box::new(move || slider_look(config, self.severity())),
                decorate: Box::new(|content, _| content),
                gap_px: config.theme.spacing.item_gap,
                expansion_duration: config.group_items.slider_expansion_duration,
            },
            ControlMessage::from,
        )
    }

    #[must_use]
    pub fn menu_view<'view>(&self, config: &Config) -> Element<'view, ControlMessage> {
        let open_row = self.expanding_slider.open_row(
            slider_look(config, self.severity()),
            self.readout_view(config),
            config.theme.spacing.item_gap,
            ControlMessage::from,
        );
        match &self.caption {
            SliderCaption::Uncaptioned => open_row,
            SliderCaption::Captioned(caption_text) => Row::with_children([
                row_key_label(&config.theme.font, caption_text.clone().into_owned())
                    .wrapping(Wrapping::None)
                    .width(Length::Fixed(
                        config.theme.font.text_size_px * CAPTION_WIDTH_IN_TEXT_SIZES,
                    ))
                    .into(),
                open_row,
            ])
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into(),
        }
    }

    fn state_glyph<'view>(&self, config: &Config) -> Element<'view, ControlMessage> {
        let state = self.shown_state();
        self.effective_icon()
            .with_weight(state.weight())
            .view(config.theme.font.icon_size_px, move |theme| {
                state.color(theme)
            })
    }

    fn readout_view<'view>(&self, config: &Config) -> Element<'view, ControlMessage> {
        steady_width_text(
            MeasuredReadout::new(
                ReadoutLabels {
                    owner: self.name.clone(),
                    range: self.expanding_slider.range.clone(),
                    step: self.expanding_slider.slider_step,
                    format: self.readout,
                    context: config.calendar.locale,
                },
                resolved_text_face(&config.theme.font),
            ),
            panel_text(
                &config.theme.font,
                self.readout_text(config.calendar.locale),
            )
            .wrapping(Wrapping::None)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .style(text::base),
        )
        .into()
    }
}
