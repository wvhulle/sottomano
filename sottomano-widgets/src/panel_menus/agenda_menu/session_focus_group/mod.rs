mod context_switch_indicator;
mod daily_totals_indicator;
mod focus_session_indicator;
mod focus_timeline_indicator;
mod focus_tracker;
mod sqlite_focus_spans;
mod window_usage;

use chrono::{DateTime, Utc};
use iced::{Subscription, Task};
use sottomano_compositor_model::{
    compositor_styling_model::HeatedWindow, compositor_window_facts::WindowTitle,
    compositor_window_state::CompositorWindowState,
};
use sottomano_desktop_sinks::login_session_presence::logind_session_commands::perform_session_action;
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::ActionTint,
    desktop_windows_apps::terminal_emulator_windows::WindowProcesses,
    login_session_presence::{
        focus_time_ledger::{FocusSignal, ScreenEdge},
        session_power_actions::SessionAction,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, panel_menus::agenda_menu::session_focus::SessionFocusConfig};

use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    hover_cards::hover_card_body::Section,
    panel_menus::agenda_menu::session_focus_group::{
        daily_totals_indicator::DailyTotalsGauge,
        focus_tracker::{FocusTracker, TrackerMessage},
        window_usage::WindowUsage,
    },
    widgets::ongoing_toast::OngoingToastActionButton,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSnooze {
    NotSnoozed,
    SnoozedUntil(DateTime<Utc>),
}

pub struct SessionFocusGroup {
    tracker: FocusTracker,
    snooze: WarningSnooze,
    expansion: IndicatorGroupExpansion,
    daily_totals: DailyTotalsGauge,
}

#[derive(Debug, Clone)]
pub enum SessionFocusMessage {
    Tracker(TrackerMessage),
    BreakStarted(SessionAction),
    WarningSnoozed,
    Expansion(IndicatorGroupExpansionMessage),
}

impl SessionFocusGroup {
    #[must_use]
    pub fn started_at(config: &SessionFocusConfig, now: DateTime<Utc>) -> Self {
        let tracker = FocusTracker::started_at(now);
        let daily_totals = DailyTotalsGauge::new(&tracker.ledger.accounted(config), now);
        Self {
            tracker,
            snooze: WarningSnooze::NotSnoozed,
            expansion: IndicatorGroupExpansion::default(),
            daily_totals,
        }
    }

    pub fn refresh_daily_totals(&mut self, config: &SessionFocusConfig, clock: Clock) {
        let now = clock.utc();
        if self.daily_totals.computed_at.timestamp() != now.timestamp() {
            self.recompute_daily_totals(config, now);
        }
    }

    fn recompute_daily_totals(&mut self, config: &SessionFocusConfig, now: DateTime<Utc>) {
        self.daily_totals = DailyTotalsGauge::new(&self.tracker.ledger.accounted(config), now);
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn record_signal(
        &mut self,
        signal: FocusSignal,
        config: &SessionFocusConfig,
        clock: Clock,
    ) -> Task<SessionFocusMessage> {
        self.update(
            SessionFocusMessage::Tracker(TrackerMessage::Signal(signal)),
            config,
            clock,
        )
    }

    pub fn load(clock: Clock) -> Task<SessionFocusMessage> {
        Task::perform(
            FocusTracker::load(clock.utc()),
            SessionFocusMessage::Tracker,
        )
    }

    pub fn subscription(config: &SessionFocusConfig) -> Subscription<SessionFocusMessage> {
        Subscription::run_with(config.idle_detection_delay, FocusTracker::idle_messages)
            .map(SessionFocusMessage::Tracker)
    }

    pub fn sleep_subscription(buses: &DesktopBuses) -> Subscription<SessionFocusMessage> {
        Subscription::run_with(buses.clone(), FocusTracker::sleep_messages)
            .map(SessionFocusMessage::Tracker)
    }

    pub fn screen_power_changed(
        &mut self,
        edge: ScreenEdge,
        config: &SessionFocusConfig,
        clock: Clock,
    ) -> Task<SessionFocusMessage> {
        self.update(
            SessionFocusMessage::Tracker(TrackerMessage::Signal(FocusSignal::ScreenPower(edge))),
            config,
            clock,
        )
    }

    pub fn refocus(
        &mut self,
        compositor: &CompositorWindowState,
        processes: &WindowProcesses,
    ) -> Task<SessionFocusMessage> {
        self.tracker
            .refocus(compositor, processes)
            .map_or_else(Task::none, |probe| {
                Task::perform(probe, SessionFocusMessage::Tracker)
            })
    }

    pub fn update(
        &mut self,
        message: SessionFocusMessage,
        config: &SessionFocusConfig,
        clock: Clock,
    ) -> Task<SessionFocusMessage> {
        match message {
            SessionFocusMessage::Tracker(message) => {
                let span_writes = self.tracker.update(message, config, clock.utc());
                self.recompute_daily_totals(config, clock.utc());
                Task::future(span_writes).discard()
            }
            SessionFocusMessage::BreakStarted(action) => logged_failure_task(
                format!("break action {action:?}"),
                perform_session_action(action),
            ),
            SessionFocusMessage::WarningSnoozed => {
                let snooze_length = config.warning_snooze;
                self.snooze = WarningSnooze::SnoozedUntil(clock.utc() + snooze_length);
                Task::none()
            }
            SessionFocusMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn heated_windows(
        &self,
        compositor: &CompositorWindowState,
        config: &Config,
        clock: Clock,
    ) -> Vec<HeatedWindow> {
        let accounting = self
            .tracker
            .ledger
            .accounted(&config.panel_menus.agenda.session_focus);
        let now = clock.utc();
        let mut windows: Vec<_> = compositor.windows.values().collect();
        windows.sort_by_key(|window| window.id);
        windows
            .into_iter()
            .filter_map(|window| {
                let WindowTitle::Titled(title) = &window.title else {
                    return None;
                };
                let color = accounting.switch_heat_color(window.id, &config.theme.palette, now)?;
                HeatedWindow::titled(
                    window.app_id.clone(),
                    title,
                    format!("{:x}", color.to_rgba8()),
                )
            })
            .collect()
    }

    #[must_use]
    pub fn window_usage_hover_card(
        &self,
        window: u64,
        config: &Config,
        clock: Clock,
    ) -> Vec<Section> {
        let usage = WindowUsage {
            accounting: self
                .tracker
                .ledger
                .accounted(&config.panel_menus.agenda.session_focus),
            window,
        };
        usage.body(clock.utc()).into_iter().collect()
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'view> {
        let locale = config.calendar.locale;
        let headline_indicator = self
            .focus_session(config)
            .indicator(clock, locale)
            .ongoing_toast_action_buttons(break_action_buttons())
            .with_headline_glyph(Phosphor::Crosshair(Weight::Regular).into());
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::SessionFocus,
            headline_indicator,
            &self.expansion,
            SessionFocusMessage::Expansion,
            IndicatorListing::Listed(Vec::from([
                self.focus_timeline_row(config, clock),
                self.daily_totals.indicator(locale),
                self.context_switch_rate(config).indicator(clock),
            ])),
        )
    }
}

pub(super) fn break_action_buttons() -> [OngoingToastActionButton<SessionFocusMessage>; 3] {
    [
        OngoingToastActionButton {
            button_label: "Lock screen".to_owned(),
            tint: ActionTint::Proceed,
            on_button_pressed: SessionFocusMessage::BreakStarted(SessionAction::Lock),
        },
        OngoingToastActionButton {
            button_label: "Screen off".to_owned(),
            tint: ActionTint::Proceed,
            on_button_pressed: SessionFocusMessage::BreakStarted(SessionAction::ScreenOff),
        },
        OngoingToastActionButton {
            button_label: "Snooze".to_owned(),
            tint: ActionTint::Hold,
            on_button_pressed: SessionFocusMessage::WarningSnoozed,
        },
    ]
}
