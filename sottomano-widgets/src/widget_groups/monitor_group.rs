mod backlight_display;
mod clight_auto_brightness;
use clight_auto_brightness::auto_brightness_indicator;
mod output_details;

use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
};

use iced::{Subscription, Task, mouse::ScrollDelta};
use output_details::ConnectedOutput;
use sottomano_desktop_sinks::screen_brightness_color::clightd_backlight_writes;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    compositor_window_ipc::compositor_output_facts::OutputInfo,
    screen_brightness_color::{
        clight_auto_brightness::AutoBrightness,
        clightd_backlight::{self, Backlight},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage, ToggleableSource},
    },
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::{CommandOutcome, SubscribedSnapshot},
    },
};

const AUTO_BRIGHTNESS_GLYPH: Phosphor = Phosphor::Eye(Weight::Regular);
const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSource {
    Backlights,
    AutoBrightness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorBusRequest {
    Refresh(MonitorSource),
}

#[derive(Default)]
pub struct MonitorGroup {
    backlights: SubscribedSnapshot<Vec<Backlight>>,
    sliders: BTreeMap<String, SliderControl>,
    clight: ToggleControl<AutoBrightness>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum MonitorMessage {
    Backlights(SnapshotDelivery<Vec<Backlight>>),
    BacklightsReread(Vec<Backlight>),
    AutoBrightness(ToggleMessage<AutoBrightness>),
    BacklightCommitted(String, ControlMessage),
}

#[derive(Debug, Clone)]
pub enum MonitorInteraction {
    Expansion(WidgetGroupExpansionMessage),
    Backlight(String, ControlMessage),
    AutoBrightnessPressed,
    OnBuses(MonitorBusRequest),
}

impl MonitorGroup {
    pub fn subscription(buses: &DesktopBuses) -> Subscription<MonitorMessage> {
        Subscription::batch([
            snapshot_subscription::<Vec<Backlight>>(buses.clone()).map(MonitorMessage::Backlights),
            ToggleControl::<AutoBrightness>::subscription(buses.clone())
                .map(MonitorMessage::AutoBrightness),
        ])
    }

    pub fn on_buses(request: MonitorBusRequest, buses: &DesktopBuses) -> Task<MonitorMessage> {
        let MonitorBusRequest::Refresh(source) = request;
        match source {
            MonitorSource::Backlights => {
                first_delivery::<Vec<Backlight>>(buses.clone()).map(MonitorMessage::Backlights)
            }
            MonitorSource::AutoBrightness => {
                ToggleControl::<AutoBrightness>::refresh(buses.clone())
                    .map(MonitorMessage::AutoBrightness)
            }
        }
    }

    fn close_controls(&mut self) {
        self.close_backlight_sliders(None);
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: MonitorMessage, clock: Clock) -> Task<MonitorMessage> {
        match msg {
            MonitorMessage::Backlights(delivery) => {
                self.backlights.receive(delivery, clock);
                self.sync_backlight_sliders();
                Task::none()
            }
            MonitorMessage::BacklightsReread(reread_backlights) => {
                self.backlights.apply_command(CommandOutcome::Reread {
                    reread_fetch: Ok(reread_backlights),
                });
                self.sync_backlight_sliders();
                Task::none()
            }
            MonitorMessage::AutoBrightness(m) => self
                .clight
                .update(m, clock)
                .map(MonitorMessage::AutoBrightness),
            MonitorMessage::BacklightCommitted(serial, control_message) => {
                self.update_backlight(serial, control_message)
            }
        }
    }

    pub fn interact(&mut self, interaction: MonitorInteraction) -> Task<MonitorMessage> {
        match interaction {
            MonitorInteraction::Expansion(WidgetGroupExpansionMessage::HandleScrolled(delta)) => {
                self.scroll_all_backlights(delta)
            }
            MonitorInteraction::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.close_controls();
                }
                Task::none()
            }
            MonitorInteraction::Backlight(serial, control_message) => {
                self.update_backlight(serial, control_message)
            }
            MonitorInteraction::AutoBrightnessPressed => {
                self.clight.press().map(MonitorMessage::AutoBrightness)
            }
            MonitorInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn scroll_all_backlights(&mut self, delta: ScrollDelta) -> Task<MonitorMessage> {
        let serials: Vec<String> = self.sliders.keys().cloned().collect();
        Task::batch(
            serials
                .into_iter()
                .map(|serial| self.update_backlight(serial, ControlMessage::Scroll(delta))),
        )
    }

    fn sync_backlight_sliders(&mut self) {
        let list = self.backlights.get().into_iter().flatten();
        self.sliders
            .retain(|serial, _| list.clone().any(|b| &b.serial == serial));
        for backlight in list {
            self.sliders
                .entry(backlight.serial.clone())
                .or_insert_with(|| {
                    SliderControl::new(
                        format!("Backlight {}", backlight.serial),
                        PERCENT,
                        backlight_display::face(backlight),
                    )
                    .scroll_step(STEP)
                    .readout(|value, _| format!("{}%", as_percent(value)))
                    .announces_level("Brightness")
                })
                .update(ControlMessage::Sample(percent_f(backlight.percent)));
        }
    }

    fn update_backlight(&mut self, serial: String, m: ControlMessage) -> Task<MonitorMessage> {
        let write_settled = matches!(
            m,
            ControlMessage::DeviceAccepted {
                write_outcome: Ok(()),
                ..
            }
        );
        let action = match self.sliders.get_mut(&serial) {
            Some(control) => control.update(m),
            None => return Task::none(),
        };
        match action {
            Action::Opened => {
                self.close_backlight_sliders(Some(&serial));
                Task::none()
            }
            Action::Commit(value) => {
                let set_serial = serial.clone();
                ControlMessage::commit(
                    "backlight set",
                    value,
                    clightd_backlight_writes::set(set_serial, as_percent(value)),
                )
                .map(move |control_message| {
                    MonitorMessage::BacklightCommitted(serial.clone(), control_message)
                })
            }
            Action::None | Action::Right | Action::Adjust(_) if write_settled => Task::perform(
                clightd_backlight::poll_all(),
                MonitorMessage::BacklightsReread,
            ),
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn close_backlight_sliders(&mut self, keep: Option<&str>) {
        for (serial, control) in &mut self.sliders {
            if Some(serial.as_str()) != keep {
                control.close();
            }
        }
    }

    fn backlight_hover_card(
        &self,
        serial: &str,
        brightness_slider: &SliderControl,
        outputs: &HashMap<String, OutputInfo>,
    ) -> Vec<Section> {
        let Some(backlight) = self.backlights.get().and_then(|backlights| {
            backlights
                .iter()
                .find(|known_backlight| known_backlight.serial == serial)
        }) else {
            return Vec::new();
        };
        self.backlights
            .hover_card_with(backlight_display::hover_card(
                backlight,
                brightness_slider.readout_cell(),
                ConnectedOutput::driving(backlight, outputs).as_ref(),
            ))
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        outputs: &'a HashMap<String, OutputInfo>,
        clock: Clock,
    ) -> WidgetGroupContent<'a> {
        let mut rows: Vec<_> = self
            .sliders
            .iter()
            .map(|(backlight_serial, slider)| {
                let message_serial = backlight_serial.clone();
                GroupIndicator::slider(slider, config, move |control_message| {
                    MonitorInteraction::Backlight(message_serial.clone(), control_message)
                })
                .with_hover_card(Box::new(move |_| {
                    self.backlight_hover_card(backlight_serial, slider, outputs)
                }))
                .refreshed_by(MonitorInteraction::OnBuses(
                    MonitorBusRequest::Refresh(MonitorSource::Backlights),
                ))
            })
            .collect();
        match self.clight.subscribed_indicator() {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let brightness_indicator = auto_brightness_indicator(held_snapshot);
                if brightness_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(
                        self.wired_auto_brightness_indicator(brightness_indicator)
                            .on_switch_press(
                                held_snapshot.switch_position(),
                                held_snapshot.press_action_text(),
                                MonitorInteraction::AutoBrightnessPressed,
                            ),
                    );
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let held_snapshot = &troubled_snapshot.held_snapshot;
                let brightness_indicator =
                    troubled_snapshot.shown_over(auto_brightness_indicator(held_snapshot), clock);
                if brightness_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(
                        self.wired_auto_brightness_indicator(brightness_indicator)
                            .on_switch_press(
                                held_snapshot.switch_position(),
                                held_snapshot.press_action_text(),
                                MonitorInteraction::AutoBrightnessPressed,
                            ),
                    );
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.wired_auto_brightness_indicator(
                    placeholder.indicator(AUTO_BRIGHTNESS_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Monitor,
            GroupSummary::glyph(
                Phosphor::Lightbulb(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            MonitorInteraction::Expansion,
            rows,
        )
        .forwards_scroll((!self.sliders.is_empty()).then_some("change every backlight"))
    }

    fn wired_auto_brightness_indicator<'a>(
        &'a self,
        brightness_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        brightness_indicator
            .with_hover_card(Box::new(move |_| self.clight.hover_card()))
            .refreshed_by(MonitorInteraction::OnBuses(MonitorBusRequest::Refresh(
                MonitorSource::AutoBrightness,
            )))
    }
}
