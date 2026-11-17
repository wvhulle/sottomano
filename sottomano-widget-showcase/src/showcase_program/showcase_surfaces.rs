use corredo::popovers::CascadeOrientation;
use iced::{
    Element, Length, Padding, Task,
    alignment::Vertical,
    widget::{Column, Row, container},
};
use sottomano_desktop_sources::compositor_window_ipc::compositor_window_state::CompositorWindowState;
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    clock::Clock,
    desktop_panels::panel_frame::{outlined_sides_inward_of, panel_frame},
    group_menus::{
        cascading_layout::popup_growth::growth_away_from,
        group_menu_state::GroupMenuTopic,
        group_menu_view::{GroupMenuCascadeSurfaces, GroupMenuView, InspectionCard},
    },
    hover_cards::{
        HoverTarget,
        anchored_popup::PopupAppearances,
        hover_card_surfaces::{HoverCardSurfaceStyle, HoverContent},
    },
    indicator_groups::quiet_schedule_group::quiet_window::QuietPhase,
    layer_surfaces::PanelEdge,
    notification_toasts::{ToastContext, ToastSurfaceStyle},
    toast_messages::NotificationMessage,
    widgets::{
        shell_layer_appearance::ShellLayer, typing_under_cards::CardBackgroundFade,
        window_icon::WindowAssets,
    },
};

use super::{SHOWCASE_FLOAT_OFFSET_PX, ShowcaseState};
use crate::{
    demo_script::GroupShown,
    fixtures::sample,
    showcase_message::{ShowcaseInteraction, ShowcaseMessage},
    showcased_group::SHOWCASE_OUTPUT,
};

impl ShowcaseState {
    pub(super) fn group_menu(&self, kind: GroupMenuTopic) -> GroupMenuView<'_> {
        GroupMenuView::new(
            kind,
            &self.group_menus,
            self.group.group_contents(&self.config, self.clock),
        )
    }

    pub(super) fn strip_groups(&self) -> Vec<Element<'_, ShowcaseInteraction>> {
        match self.demo.shown {
            GroupShown::InMenu(kind)
            | GroupShown::InMenuPresented { menu: kind, .. }
            | GroupShown::PinnedFrom { menu: kind, .. } => self
                .group_menu(kind)
                .panel_views(
                    &self.config,
                    self.group.position(),
                    self.hover_card
                        .handle_indicator(&HoverTarget::GroupMenu(kind), SHOWCASE_OUTPUT),
                    self.clock,
                )
                .map(|element| element.map(ShowcaseInteraction::from))
                .collect(),
            GroupShown::OnPanel => self.group.panel_groups(&self.config, self.clock),
        }
    }

    pub(super) fn menu_cascade(
        &self,
        kind: GroupMenuTopic,
    ) -> Option<HoverContent<'_, ShowcaseInteraction>> {
        let group_menu = self.group_menu(kind);
        let inspected_sections = self
            .group_menus
            .inspection
            .inspected_indicator(kind)
            .map(|indicator_key| group_menu.indicator_hover_card(indicator_key, self.clock))
            .unwrap_or_default();
        let anchor = self.hover_card.shown_anchor().unwrap_or_default();
        let orientation = CascadeOrientation::toward_room(
            anchor,
            growth_away_from(self.group.position()),
            self.demo.backdrop.canvas().width,
        );
        let surfaces = group_menu.surfaces(
            &self.config,
            &self.appearance.menus,
            &InspectionCard {
                sections: &inspected_sections,
                card_appearance: self.appearance.overlays,
                assets: self.window_assets(),
                clock: self.clock,
            },
            orientation,
        );
        let GroupMenuCascadeSurfaces {
            primary,
            satellites,
        } = surfaces?;
        Some(HoverContent::Cascading {
            primary: primary.map(ShowcaseInteraction::from),
            satellites: satellites
                .into_iter()
                .map(|satellite| satellite.map(ShowcaseInteraction::from))
                .collect(),
            orientation,
        })
    }

    pub(super) fn window_assets(&self) -> WindowAssets<'_> {
        WindowAssets {
            entries: &self.desktop_entries,
            favicons: &self.favicons,
            controller_artwork: &self.controller_artwork,
        }
    }

    pub(super) fn advance_frame(&mut self, frame: Clock) -> Task<ShowcaseMessage> {
        let elapsed = frame
            .instant
            .checked_duration_since(self.booted_at.instant)
            .expect("animation frames start after the showcase booted");
        let clock = self.booted_at.advanced(elapsed);
        self.clock = clock;
        let mut tasks = Vec::new();
        if sample(clock) > self.delivered_sample {
            self.delivered_sample = sample(clock);
            for fixture_message in (self.demo.live)(clock) {
                tasks.push(self.route_message(fixture_message));
            }
        }
        let (due_replies, waiting_replies) = self
            .pending_hover_replies
            .drain(..)
            .partition(|pending| pending.due <= clock.instant);
        self.pending_hover_replies = waiting_replies;
        for pending in due_replies {
            tasks.push(self.route_message(pending.reply));
        }
        if self.hover_card.hide_schedule.elapsed_by(clock.instant) {
            tasks.push(self.route_message(ShowcaseMessage::HoverCardHideElapsed));
        }
        if self
            .notifications
            .next_expiry()
            .is_some_and(|due| due <= clock.instant)
        {
            tasks.push(self.route_message(ShowcaseMessage::Notification(
                NotificationMessage::ExpiryDue,
            )));
        }
        Task::batch(tasks)
    }

    pub(super) fn toast_layer(&self) -> Element<'_, ShowcaseInteraction> {
        Element::from(self.notifications.anchored_toasts(
            SHOWCASE_OUTPUT,
            &self.config,
            &self.theme,
            ToastSurfaceStyle {
                overlay_appearance: self.appearance.overlays,
                panel_clearance: Padding::ZERO,
                background_fade: CardBackgroundFade::Resting,
            },
            self.clock,
        ))
        .map(ShowcaseInteraction::Notification)
    }

    pub(super) fn update_notification(
        &mut self,
        message: NotificationMessage,
    ) -> Task<ShowcaseMessage> {
        let unplaced = CompositorWindowState::default();
        let context = ToastContext::new(
            self.group.compositor().unwrap_or(&unplaced),
            self.desktop_entries.entries.as_deref(),
            &self.config,
            self.notification_settings,
            QuietPhase::Inactive,
            self.clock,
        );
        self.notifications
            .update(message, &context, &|_| None)
            .map(ShowcaseMessage::from)
    }

    pub(super) fn hover_content(
        &self,
        target: &HoverTarget,
    ) -> Option<HoverContent<'_, ShowcaseInteraction>> {
        if let HoverTarget::GroupMenu(kind) = target {
            return self.menu_cascade(*kind);
        }
        self.group
            .described_hover_card(target, &self.config, self.clock)
            .map(HoverContent::Described)
    }

    pub(super) fn panel_edge(&self) -> PanelEdge {
        PanelEdge {
            output: SHOWCASE_OUTPUT.to_owned(),
            position: self.group.position(),
        }
    }

    pub(super) fn group_strip(&self) -> Element<'_, ShowcaseInteraction> {
        let panel = &self.config.desktop_panels;
        let position = self.group.position();
        let layer = self.appearance.layer(ShellLayer::from(position));
        let frame_style = layer.frame;
        let outlined_sides =
            outlined_sides_inward_of(position).all_when_floating(SHOWCASE_FLOAT_OFFSET_PX);
        let framed = self.strip_groups().into_iter().map(|group| {
            let content = match position {
                Position::Left => container(group),
                Position::Top | Position::Bottom => {
                    container(container(group).padding([0.0, self.config.theme.spacing.item_gap]))
                        .height(Length::Fixed(panel.height))
                        .align_y(Vertical::Center)
                        .padding([0.0, layer.frame_corner_inset()])
                }
            };
            panel_frame(content, frame_style)
                .outlined_sides(outlined_sides)
                .backdrop_blur(layer.backdrop_blur)
                .into()
        });
        match position {
            Position::Left => Column::with_children(framed)
                .spacing(self.config.theme.spacing.item_gap)
                .into(),
            Position::Top | Position::Bottom => Row::with_children(framed)
                .spacing(self.config.theme.spacing.item_gap)
                .into(),
        }
    }

    pub(super) fn hover_card_layer(&self) -> Element<'_, ShowcaseInteraction> {
        self.hover_card.view(
            SHOWCASE_OUTPUT,
            &self.config,
            HoverCardSurfaceStyle {
                popup: PopupAppearances {
                    overlay: self.appearance.overlays,
                    menu: self.appearance.menus,
                    background_fade: CardBackgroundFade::Resting,
                },
                float_offset_px: SHOWCASE_FLOAT_OFFSET_PX,
            },
            self.window_assets(),
            |target: &HoverTarget| self.hover_content(target),
            self.clock,
        )
    }
}
