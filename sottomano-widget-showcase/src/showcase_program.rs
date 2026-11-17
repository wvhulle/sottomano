use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use corredo::theming::text_color;
use iced::{
    Color, Element, Length, Rectangle, Settings, Subscription, Task, Theme,
    advanced::widget::operate,
    alignment::Vertical,
    executor,
    theme::Style,
    time::every,
    widget::{container, stack},
    window,
};
use iced_controller_diagram::ControllerArtwork;
use iced_exwlshell::SurfaceRegionsCollector;
use iced_test::program::{Preset, Program};
use sottomano_desktop_sources::screen_brightness_color::solar_switch_schedule::ColorMode;
use sottomano_toml_config::{Config, Palette, screen_edges::Position};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    group_menus::group_menu_state::{GroupMenuMessage, GroupMenuPin, GroupMenuState},
    hover_cards::{HoverCardHost, HoverTarget, PopupMessage},
    indicator_groups::{
        appearance_presets_group::AppearancePresetsMessage,
        indicator_group_interaction::IndicatorGroupInteraction, notification_group::AlertSound,
    },
    notification_toasts::{NotificationLayer, NotificationSettings},
    toast_messages::NotificationMessage,
    widgets::{
        appearance_presets::ShellAppearance,
        configured_theme::configured_theme,
        favicon::Favicons,
        text::{font_faces_of, point_generic_font_families_at},
    },
};

use crate::{
    animation_recording::{CANVAS, FRAME_INTERVAL},
    backdrop_photo::BackdropPhotos,
    demo_script::{DEMOS, Demo, DemoBackdrop, GroupShown},
    fixtures::{sample, showcase_epoch},
    frosted_backdrop::{FrostedBackdrop, frosted_rows_of},
    scripted_frames::scripted_frames,
    showcase_message::{ShowcaseInteraction, ShowcaseMessage},
    showcased_group::{ShowcasedGroup, recorded_look},
};

mod showcase_hover_cards;
mod showcase_surfaces;

pub(crate) const SHOWCASE_FLOAT_OFFSET_PX: f32 = 0.0;
pub(crate) const HOVER_FETCH_LATENCY: Duration = Duration::from_millis(700);

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
    backdrop_photos: BackdropPhotos,
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
        let backdrop_photos = BackdropPhotos::decoded();
        let presets = DEMOS
            .iter()
            .map(|demo| {
                let config = config.clone();
                let backdrop_photos = backdrop_photos.clone();
                Preset::new(demo.name, move || {
                    ShowcaseState::boot(demo, &config, &backdrop_photos, start)
                })
            })
            .collect();
        ShowcaseProgram {
            config: config.clone(),
            presets,
            pacing,
            start,
            backdrop_photos,
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
    notifications: NotificationLayer<ShowcaseInteraction>,
    clock: Clock,
    booted_at: Clock,
    delivered_sample: u32,
    pending_hover_replies: Vec<PendingHoverReply>,
    backdrop_photos: BackdropPhotos,
    frosted_rows: Vec<Rectangle>,
}

struct PendingHoverReply {
    due: Instant,
    reply: ShowcaseMessage,
}

impl ShowcaseState {
    fn boot(
        demo: &'static Demo,
        config: &Config,
        backdrop_photos: &BackdropPhotos,
        clock: Clock,
    ) -> (Self, Task<ShowcaseMessage>) {
        let appearance = recorded_look();
        point_generic_font_families_at(&config.theme);
        let hover_card = HoverCardHost::default();
        let notification_settings = NotificationSettings {
            alert_sound: AlertSound::Silent,
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
            backdrop_photos: backdrop_photos.clone(),
            frosted_rows: Vec::new(),
        };
        (state, opening_toasts)
    }

    fn route_message(&mut self, message: ShowcaseMessage) -> Task<ShowcaseMessage> {
        match message {
            ShowcaseMessage::Interaction(interaction) => self.interact(interaction),
            ShowcaseMessage::GroupMenuState(message) => self.update_group_menus(message),
            ShowcaseMessage::HoverCardHideElapsed => {
                let departing = self
                    .hover_card
                    .departure(|target| self.hover_content(target));
                self.hover_card.leave_elapsed(self.clock.instant, departing);
                Task::none()
            }
            ShowcaseMessage::Notification(message) => self.update_notification(message),
            delivery => {
                let _delivery_for_another_group = self.group.update(delivery, self.clock);
                Task::none()
            }
        }
    }

    fn interact(&mut self, interaction: ShowcaseInteraction) -> Task<ShowcaseMessage> {
        match interaction {
            ShowcaseInteraction::Group(IndicatorGroupInteraction::GroupMenus(message)) => {
                self.update_group_menus(message)
            }
            ShowcaseInteraction::Group(group_interaction) => {
                if let IndicatorGroupInteraction::AppearancePresets(
                    AppearancePresetsMessage::Apply(preset),
                ) = &group_interaction
                {
                    self.appearance = preset.applied_to(self.appearance);
                    self.theme =
                        configured_theme(&scheme_palette(&self.config, self.appearance.scheme));
                }
                let _interaction_for_another_group =
                    self.group
                        .interact(group_interaction, &self.config, self.clock);
                Task::none()
            }
            ShowcaseInteraction::Popup(popup_message) => self.update_popup(popup_message),
            ShowcaseInteraction::Notification(message) => self.update_notification(message),
            ShowcaseInteraction::OutsideShowcase => Task::none(),
        }
    }

    fn update_popup(&mut self, popup_message: PopupMessage) -> Task<ShowcaseMessage> {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => self.update_hover_card(hover_card),
            PopupMessage::WindowGroup(window_interaction) => {
                let _interaction_for_another_group = self.group.interact(
                    IndicatorGroupInteraction::WindowGroup(window_interaction),
                    &self.config,
                    self.clock,
                );
                Task::none()
            }
            PopupMessage::CopyText(_)
            | PopupMessage::OpenUrl(_)
            | PopupMessage::FocusWindow(_)
            | PopupMessage::RefreshHoveredIndicator
            | PopupMessage::ReleaseFromPanel(_) => Task::none(),
        }
    }

    fn update_group_menus(&mut self, message: GroupMenuMessage) -> Task<ShowcaseMessage> {
        if let GroupMenuMessage::InspectRow { indicator, .. } = &message {
            self.schedule_hover_replies(&HoverTarget::Indicator(indicator.clone()));
        }
        let _menu_task = self.group_menus.update(message);
        Task::none()
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
            default_font: font_faces_of(&self.config.theme.font).regular,
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
        ShowcaseState::boot(&DEMOS[0], &self.config, &self.backdrop_photos, self.start)
    }

    fn presets(&self) -> &[Preset<ShowcaseState, ShowcaseMessage>] {
        &self.presets
    }

    fn update(&self, state: &mut ShowcaseState, message: ShowcaseMessage) -> Task<ShowcaseMessage> {
        let message_task = match message {
            ShowcaseMessage::SurfaceRegionsCollected(surface_regions) => {
                state.frosted_rows = frosted_rows_of(&surface_regions);
                return Task::none();
            }
            ShowcaseMessage::Frame(clock) => state.advance_frame(clock),
            routed => state.route_message(routed),
        };
        match state.demo.backdrop {
            DemoBackdrop::FrostablePhoto => message_task.chain(
                operate(SurfaceRegionsCollector::default())
                    .map(ShowcaseMessage::SurfaceRegionsCollected),
            ),
            DemoBackdrop::Transparent => message_task,
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
        let photo_backdrop = (state.demo.backdrop == DemoBackdrop::FrostablePhoto)
            .then(|| FrostedBackdrop::new(&state.backdrop_photos, &state.frosted_rows).into());
        let layers = stack(
            photo_backdrop
                .into_iter()
                .chain(state.group.backdrop())
                .chain([strip, state.toast_layer()])
                .chain([state.hover_card_layer()]),
        )
        .width(Length::Fill)
        .height(Length::Fill);
        scripted_frames(state.clock.instant, layers).map(ShowcaseMessage::from)
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
