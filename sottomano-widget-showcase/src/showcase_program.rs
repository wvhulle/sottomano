use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use iced::{
    Color, Element, Length, Padding, Rectangle, Settings, Subscription, Task, Theme,
    alignment::Vertical,
    executor,
    theme::Style,
    time::every,
    widget::{Column, Row, container, stack},
    window,
};
use iced_controller_diagram::ControllerArtwork;
use iced_test::program::{Preset, Program};
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, Palette, screen_edges::Position};
use sottomano_sources::screen_brightness_color::solar_switch_schedule::ColorMode;
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    desktop_panels::panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
    group_menus::{
        cascading_layout::cascade_row::CascadeOrientation,
        group_menu_state::{GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic},
        group_menu_view::{GroupMenuCascadeSurfaces, GroupMenuView, InspectionCard},
    },
    hover_cards::{
        HoverCardHost, HoverCardMessage, HoverTarget,
        hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_surfaces::{HoverCardSurfaceStyle, HoverContent},
    },
    layer_surfaces::PanelEdge,
    notification_toasts::{NotificationLayer, NotificationSettings, ToastContext},
    toast_messages::NotificationMessage,
    widget_groups::appearance_presets_group::AppearancePresetsMessage,
    widgets::{
        appearance_presets::ShellAppearance,
        configured_theme::{configured_theme, text_color},
        favicon::Favicons,
        text::FontFaces,
        window_icon::WindowAssets,
    },
};

use crate::{
    animation_recording::{CANVAS, FRAME_INTERVAL},
    demo_script::{DEMOS, Demo, GroupShown},
    fixtures::{sample, showcase_epoch},
    scripted_frames::scripted_frames,
    showcase_message::ShowcaseMessage,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup},
};

pub(crate) const SHOWCASE_FLOAT_OFFSET_PX: f32 = 0.0;
pub(crate) const HOVER_FETCH_LATENCY: Duration = Duration::from_millis(1400);

#[must_use]
pub fn booting_clock() -> Clock {
    Clock {
        wall: showcase_epoch(),
        ..Clock::real()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pacing {
    Realtime,
    Scripted,
}

pub struct ShowcaseProgram {
    config: Config,
    presets: Vec<Preset<ShowcaseState, ShowcaseMessage>>,
    pacing: Pacing,
    start: Clock,
}

impl ShowcaseProgram {
    #[must_use]
    pub fn interactive(config: &Config) -> Self {
        Self::with_pacing(config, Pacing::Realtime, booting_clock())
    }

    #[must_use]
    pub fn scripted(config: &Config, start: Clock) -> Self {
        Self::with_pacing(config, Pacing::Scripted, start)
    }

    fn with_pacing(config: &Config, pacing: Pacing, start: Clock) -> Self {
        let presets = DEMOS
            .iter()
            .map(|demo| {
                let config = config.clone();
                Preset::new(demo.name, move || ShowcaseState::boot(demo, &config, start))
            })
            .collect();
        ShowcaseProgram {
            config: config.clone(),
            presets,
            pacing,
            start,
        }
    }

    pub(crate) fn preset(&self, demo: &Demo) -> &Preset<ShowcaseState, ShowcaseMessage> {
        self.presets
            .iter()
            .find(|preset| preset.name() == demo.name)
            .expect("every demo has a preset")
    }
}

pub struct ShowcaseState {
    demo: &'static Demo,
    config: Config,
    appearance: ShellAppearance,
    notification_settings: NotificationSettings,
    desktop_entries: DesktopIndex,
    favicons: Favicons,
    controller_artwork: ControllerArtwork,
    theme: Theme,
    group: ShowcasedGroup,
    group_menus: GroupMenuState,
    hover_card: HoverCardHost,
    notifications: NotificationLayer<ShowcaseMessage>,
    clock: Clock,
    booted_at: Clock,
    delivered_sample: u32,
    pending_hover_replies: Vec<PendingHoverReply>,
}

struct PendingHoverReply {
    due: Instant,
    reply: ShowcaseMessage,
}

impl ShowcaseState {
    fn boot(demo: &'static Demo, config: &Config, clock: Clock) -> (Self, Task<ShowcaseMessage>) {
        let appearance = ShellAppearance::default();
        FontFaces::point_generic_families_at(&config.theme);
        let mut hover_card = HoverCardHost::default();
        let _ = hover_card.open_surface(SHOWCASE_OUTPUT);
        let notification_settings = NotificationSettings {
            sound: false,
            ..NotificationSettings::default()
        };
        let group = (demo.group)(clock);
        let opening_toasts = Task::batch(group.opening_toasts().iter().map(|toast| {
            Task::done(ShowcaseMessage::Notification(NotificationMessage::Served(
                Box::new(toast.clone()),
            )))
        }));
        let mut group_menus = GroupMenuState::default();
        if let GroupShown::InMenuPresented { presentation, .. } = &demo.shown {
            let _unsaved_presentation =
                group_menus.update(GroupMenuMessage::PresentationLoaded(*presentation));
        }
        if let GroupShown::PinnedFrom { menu, group } = &demo.shown {
            let _unsaved_pin = group_menus.update(GroupMenuMessage::TogglePin {
                menu: *menu,
                entry: GroupMenuPin::Group(group.clone()),
            });
        }
        let state = ShowcaseState {
            demo,
            config: config.clone(),
            appearance,
            notification_settings,
            desktop_entries: DesktopIndex::default(),
            favicons: Favicons::default(),
            controller_artwork: ControllerArtwork::decoded(),
            theme: configured_theme(&scheme_palette(config, appearance.scheme)),
            group,
            group_menus,
            hover_card,
            notifications: NotificationLayer::default(),
            clock,
            booted_at: clock,
            delivered_sample: sample(clock),
            pending_hover_replies: Vec::new(),
        };
        (state, opening_toasts)
    }

    fn route_message(&mut self, message: ShowcaseMessage) -> Task<ShowcaseMessage> {
        match message {
            ShowcaseMessage::HoverCard(HoverCardMessage::EnterAnchor {
                target,
                anchor,
                announcement,
            }) => {
                if self.hover_card.target() != Some(&target) {
                    let _closed_menu = self.group_menus.update(GroupMenuMessage::Closed);
                }
                self.schedule_hover_replies(&target);
                let panel_edge = self.panel_edge();
                self.hover_card.open(
                    &panel_edge,
                    target,
                    anchor,
                    announcement,
                    self.clock.instant,
                );
                Task::none()
            }
            ShowcaseMessage::HoverCard(message) => {
                let _hide_due = self.hover_card.update(message, self.clock.instant);
                Task::none()
            }
            ShowcaseMessage::GroupMenuState(message) => {
                if let GroupMenuMessage::InspectRow { target, .. }
                | GroupMenuMessage::ToggleRowCard { target, .. } = &message
                {
                    self.schedule_hover_replies(target);
                }
                let _menu_task = self.group_menus.update(message);
                Task::none()
            }
            ShowcaseMessage::HoverCardHideElapsed => {
                let departing = self.shown_hover_card().and_then(|content| {
                    self.hover_card
                        .departure::<ShowcaseMessage>(HoverContent::Described(content))
                });
                self.hover_card.leave_elapsed(self.clock.instant, departing);
                Task::none()
            }
            ShowcaseMessage::Notification(message) => self.update_notification(message),
            ShowcaseMessage::AppearancePresets(AppearancePresetsMessage::Apply(preset)) => {
                self.appearance = preset.applied_to(self.appearance);
                self.theme =
                    configured_theme(&scheme_palette(&self.config, self.appearance.scheme));
                let departing = self.shown_hover_card().and_then(|content| {
                    self.hover_card
                        .departure::<ShowcaseMessage>(HoverContent::Described(content))
                });
                self.hover_card.dismiss(departing, self.clock.instant);
                Task::none()
            }
            other => {
                self.group.update(other, self.clock);
                Task::none()
            }
        }
    }

    fn schedule_hover_replies(&mut self, target: &HoverTarget) {
        let due = self.clock.instant + HOVER_FETCH_LATENCY;
        self.pending_hover_replies = self
            .group
            .hover(target, &self.config, self.clock)
            .into_iter()
            .map(|reply| PendingHoverReply { due, reply })
            .collect();
    }

    fn group_menu(&self, kind: GroupMenuTopic) -> GroupMenuView<'_, ShowcaseMessage> {
        GroupMenuView::new(
            kind,
            &self.group_menus,
            ShowcaseMessage::GroupMenuState,
            self.group.group_contents(&self.config, self.clock),
        )
    }

    fn strip_groups(&self) -> Vec<Element<'_, ShowcaseMessage>> {
        match self.demo.shown {
            GroupShown::InMenu(kind)
            | GroupShown::InMenuPresented { menu: kind, .. }
            | GroupShown::PinnedFrom { menu: kind, .. } => self
                .group_menu(kind)
                .panel_views(&self.config, self.group.position(), self.clock)
                .collect(),
            GroupShown::OnPanel => self.group.panel_groups(&self.config, self.clock),
        }
    }

    fn menu_cascade(&self, kind: GroupMenuTopic) -> HoverContent<'_, ShowcaseMessage> {
        let group_menu = self.group_menu(kind);
        let inspected_hover_card = self
            .group_menus
            .inspected_target(kind)
            .and_then(|target| {
                self.group
                    .hover_card_for(target, &self.config, self.clock)
                    .map(|hover_card| {
                        hover_card.with_announcement(
                            group_menu.announcement_of(target).unwrap_or_default(),
                        )
                    })
            })
            .unwrap_or_else(HoverCard::empty);
        let anchor = self
            .hover_card
            .placement()
            .map_or_else(Rectangle::default, |placement| placement.anchor);
        let orientation =
            CascadeOrientation::toward_room(anchor, self.group.position(), CANVAS.width);
        let surfaces = group_menu.surfaces(
            &self.config,
            &self.appearance.overlays,
            &InspectionCard {
                hover_card: &inspected_hover_card,
                assets: self.window_assets(),
                clock: self.clock,
            },
            orientation,
        );
        match surfaces {
            Some(GroupMenuCascadeSurfaces {
                primary,
                satellites,
            }) => HoverContent::GroupMenu {
                primary,
                satellites,
                orientation,
            },
            None => HoverContent::Described(HoverCard::empty()),
        }
    }

    fn window_assets(&self) -> WindowAssets<'_> {
        WindowAssets {
            entries: &self.desktop_entries,
            favicons: &self.favicons,
            controller_artwork: &self.controller_artwork,
        }
    }

    fn advance_frame(&mut self, frame: Clock) -> Task<ShowcaseMessage> {
        let elapsed = frame
            .instant
            .saturating_duration_since(self.booted_at.instant);
        let clock = self.booted_at.advanced(elapsed);
        self.clock = clock;
        let mut tasks = Vec::new();
        if sample(clock) > self.delivered_sample {
            self.delivered_sample = sample(clock);
            for reading in (self.demo.live)(clock) {
                tasks.push(self.route_message(reading));
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
        if self
            .hover_card
            .hide_due()
            .is_some_and(|due| due <= clock.instant)
        {
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

    fn toast_layer(&self) -> Element<'_, ShowcaseMessage> {
        Element::from(self.notifications.anchored_toasts(
            SHOWCASE_OUTPUT,
            &self.config,
            &self.theme,
            self.appearance.overlays,
            Padding::ZERO,
            self.clock,
        ))
        .map(ShowcaseMessage::Notification)
    }

    fn update_notification(&mut self, message: NotificationMessage) -> Task<ShowcaseMessage> {
        let unplaced = EventStreamState::default();
        let context = ToastContext::new(
            self.group.niri().unwrap_or(&unplaced),
            self.desktop_entries.get(),
            &self.config,
            self.notification_settings,
            false,
            self.clock,
        );
        self.notifications.update(message, &context, &|_| None)
    }

    fn shown_hover_card(&self) -> Option<HoverCard> {
        self.hover_card
            .target()
            .and_then(|target| self.group.hover_card_for(target, &self.config, self.clock))
    }

    fn panel_edge(&self) -> PanelEdge {
        PanelEdge {
            output: SHOWCASE_OUTPUT.to_owned(),
            position: self.group.position(),
        }
    }

    fn group_strip(&self) -> Element<'_, ShowcaseMessage> {
        let panel = &self.config.desktop_panels;
        let position = self.group.position();
        let layer = match position {
            Position::Left => self.appearance.sidebar,
            Position::Top | Position::Bottom => self.appearance.panels,
        };
        let frame_style = LayerFrameStyle::new(&layer);
        let outlined_sides =
            OutlinedSides::inward_of(position).all_when_floating(SHOWCASE_FLOAT_OFFSET_PX);
        let framed = self.strip_groups().into_iter().map(|group| {
            let content = match position {
                Position::Left => container(group),
                Position::Top | Position::Bottom => container(group)
                    .height(Length::Fixed(panel.height))
                    .align_y(Vertical::Center)
                    .padding([
                        0.0,
                        layer.frame_corner_inset() + self.config.theme.spacing.item_gap,
                    ]),
            };
            panel_frame(content, frame_style)
                .outlined_sides(outlined_sides)
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

    fn hover_card_layer(&self) -> Option<Element<'_, ShowcaseMessage>> {
        let surface = self.hover_card.surface_on(SHOWCASE_OUTPUT)?;
        self.hover_card.view(
            surface,
            &self.config,
            HoverCardSurfaceStyle {
                overlay_appearance: self.appearance.overlays,
                float_offset_px: SHOWCASE_FLOAT_OFFSET_PX,
            },
            self.window_assets(),
            |target: &HoverTarget| {
                if let HoverTarget::GroupMenu(kind) = target {
                    return self.menu_cascade(*kind);
                }
                let described = self
                    .group
                    .hover_card_for(target, &self.config, self.clock)
                    .unwrap_or_else(HoverCard::empty);
                match self
                    .group
                    .interactive_panel(target, &self.config, &self.appearance)
                {
                    Some(panel) => HoverContent::Interactive { described, panel },
                    None => HoverContent::Described(described),
                }
            },
            self.clock,
        )
    }
}

impl Program for ShowcaseProgram {
    type State = ShowcaseState;
    type Message = ShowcaseMessage;
    type Theme = Theme;
    type Renderer = iced::Renderer;
    type Executor = executor::Default;

    fn name() -> &'static str {
        "sottomano_widget_showcase"
    }

    fn settings(&self) -> Settings {
        Settings {
            default_font: FontFaces::from(&self.config.theme.font).regular,
            fonts: vec![Cow::Owned(self.config.theme.font.data.to_vec())],
            antialiasing: true,
            ..Settings::default()
        }
    }

    fn window(&self) -> Option<window::Settings> {
        Some(window::Settings {
            size: CANVAS,
            ..window::Settings::default()
        })
    }

    fn boot(&self) -> (ShowcaseState, Task<ShowcaseMessage>) {
        ShowcaseState::boot(&DEMOS[0], &self.config, self.start)
    }

    fn presets(&self) -> &[Preset<ShowcaseState, ShowcaseMessage>] {
        &self.presets
    }

    fn update(&self, state: &mut ShowcaseState, message: ShowcaseMessage) -> Task<ShowcaseMessage> {
        match message {
            ShowcaseMessage::Frame(clock) => state.advance_frame(clock),
            routed => state.route_message(routed),
        }
    }

    fn view<'a>(
        &self,
        state: &'a ShowcaseState,
        _window: window::Id,
    ) -> Element<'a, ShowcaseMessage> {
        let gap = state.config.theme.spacing.item_gap;
        let panel_side = match state.group.position() {
            Position::Top => Vertical::Top,
            Position::Left => Vertical::Center,
            Position::Bottom => Vertical::Bottom,
        };
        let strip = container(state.group_strip())
            .padding(gap)
            .height(Length::Fill)
            .align_y(panel_side)
            .into();
        let layers = stack(
            state
                .group
                .backdrop()
                .into_iter()
                .chain([strip, state.toast_layer()])
                .chain(state.hover_card_layer()),
        )
        .width(Length::Fill)
        .height(Length::Fill);
        scripted_frames(state.clock.instant, layers)
    }

    fn subscription(&self, _state: &ShowcaseState) -> Subscription<ShowcaseMessage> {
        match self.pacing {
            Pacing::Realtime => {
                every(FRAME_INTERVAL).map(|_| ShowcaseMessage::Frame(Clock::real()))
            }
            Pacing::Scripted => Subscription::none(),
        }
    }

    fn theme(&self, state: &ShowcaseState, _window: window::Id) -> Option<Theme> {
        Some(state.theme.clone())
    }

    fn style(&self, _state: &ShowcaseState, theme: &Theme) -> Style {
        Style {
            background_color: Color::TRANSPARENT,
            text_color: text_color(theme),
        }
    }
}

fn scheme_palette(config: &Config, scheme: ColorMode) -> Palette {
    match scheme {
        ColorMode::Dark => config.theme.palette.clone(),
        ColorMode::Light => Palette::light(),
    }
}
