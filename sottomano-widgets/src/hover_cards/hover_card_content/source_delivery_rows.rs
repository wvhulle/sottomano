use std::time::{Duration, Instant};

use corredo::{
    gauges::CountdownDial,
    press_actions::flash_on_press_text,
    theming::{danger_color, secondary_text_color, success_shades},
};
use iced::{
    Element, Length,
    alignment::Vertical,
    mouse::Interaction,
    widget::{Space, Text, container, row, text},
};
use sottomano_desktop_sources::delivery_cadence_drivers::DeliveryCadence;

use super::HoverCardContent;
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    hover_cards::{
        PopupMessage,
        hover_card_body::hover_card_source_delivery::{DeliveryStamp, SourceDelivery},
    },
    widgets::localized_durations::duration_text,
};

impl HoverCardContent<'_> {
    pub(super) fn source_row(
        &self,
        source_name: &'static str,
        delivery: SourceDelivery,
    ) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        container(
            row([
                IconGlyph::from(Phosphor::Database(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
                self.small_dim_text(source_name.to_owned()).into(),
                Space::new().width(Length::Fill).into(),
                self.delivery_view(delivery),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center),
        )
        .padding(self.title_padding())
        .into()
    }

    fn delivery_view(&self, delivery: SourceDelivery) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>,
                        glyph: Phosphor,
                        glyph_color: fn(&iced::Theme) -> iced::Color| {
            row([
                label.into(),
                IconGlyph::from(glyph).view(glyph_size, glyph_color),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center)
            .into()
        };
        match delivery {
            SourceDelivery::Delivered(delivery_stamp) => self.delivery_stamp(delivery_stamp),
            SourceDelivery::Loading => labelled(
                self.small_dim_text("loading".to_owned()),
                Phosphor::HourglassMedium(Weight::Regular),
                secondary_text_color,
            ),
            SourceDelivery::Failed => labelled(
                self.small_text("failed".to_owned())
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(danger_color(theme)),
                    }),
                Phosphor::Warning(Weight::Regular),
                danger_color,
            ),
        }
    }

    fn delivery_stamp(&self, delivery_stamp: DeliveryStamp) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        let labelled = |label: Text<'static>, glyph: Element<'static, PopupMessage>| {
            row([label.into(), glyph])
                .spacing(self.config.theme.spacing.item_gap)
                .align_y(Vertical::Center)
                .into()
        };
        let locale = self.config.calendar.locale;
        match delivery_stamp.cadence {
            DeliveryCadence::PeriodicPolling(polling) => self.refreshable_dial(
                duration_text(polling.poll_period, locale),
                polling.poll_period,
                delivery_stamp.delivered_at,
            ),
            DeliveryCadence::LivePushWithRecheck(polling) => self.refreshable_dial(
                format!(
                    "live, recheck {}",
                    duration_text(polling.poll_period, locale)
                ),
                polling.poll_period,
                delivery_stamp.delivered_at,
            ),
            DeliveryCadence::LivePush => labelled(
                self.small_dim_text("live".to_owned()),
                IconGlyph::from(Phosphor::Pulse(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
            DeliveryCadence::SingleRead => labelled(
                self.small_dim_text("read once".to_owned()),
                IconGlyph::from(Phosphor::PushPin(Weight::Regular))
                    .view(glyph_size, secondary_text_color),
            ),
        }
    }

    fn refreshable_dial(
        &self,
        label_text: String,
        refresh_period: Duration,
        delivered_at: Instant,
    ) -> Element<'static, PopupMessage> {
        let glyph_size = self.config.theme.font.small_text_size_px;
        Element::from(
            flash_on_press_text(
                row([
                    self.small_dim_text(label_text).into(),
                    CountdownDial::new(
                        self.clock
                            .instant
                            .checked_duration_since(delivered_at)
                            .expect("a delivery is stamped no later than the clock that reads it"),
                        refresh_period,
                        self.clock.instant,
                        glyph_size,
                    )
                    .view(),
                ])
                .spacing(self.config.theme.spacing.item_gap)
                .align_y(Vertical::Center),
                (),
                |()| PopupMessage::RefreshHoveredIndicator,
            )
            .accent(success_shades)
            .hold(self.config.hover_cards.copy_flash_duration)
            .interaction(Interaction::Pointer),
        )
    }
}
