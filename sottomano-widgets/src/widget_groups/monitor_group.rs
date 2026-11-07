mod backlight_display;
mod clight_auto_brightness;
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
    blocking_background_work::blocking,
    desktop_windows_apps::compositor_output_facts::OutputInfo,
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
    group_items::{
        group_item::GroupItem,
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        latest_snapshot::{CommandOutcome, HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 5.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorTarget {
    Backlight(String),
    ClightAuto,
}

impl From<MonitorTarget> for HoverTarget {
    fn from(target: MonitorTarget) -> Self {
        HoverTarget::Monitor(target)
    }
}

#[derive(Default)]
pub struct MonitorGroup {
    backlights: LatestSnapshot<Vec<Backlight>, HidesFetchFailures>,
    sliders: BTreeMap<String, SliderControl>,
    clight: ToggleControl<AutoBrightness>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Expansion(WidgetGroupExpansionMessage),
    Backlights(SnapshotDelivery<Vec<Backlight>>),
    BacklightsReread(Vec<Backlight>),
    Backlight(String, ControlMessage),
    AutoBrightness(ToggleMessage<AutoBrightness>),
}

impl MonitorGroup {
    pub fn subscription(buses: &DesktopBuses) -> Subscription<MonitorMessage> {
        Subscription::batch([
            snapshot_subscription::<Vec<Backlight>>(buses.clone()).map(MonitorMessage::Backlights),
            ToggleControl::<AutoBrightness>::subscription(buses.clone())
                .map(MonitorMessage::AutoBrightness),
        ])
    }

    pub fn refresh(target: &MonitorTarget, buses: &DesktopBuses) -> Task<MonitorMessage> {
        match target {
            MonitorTarget::Backlight(_) => {
                first_delivery::<Vec<Backlight>>(buses.clone()).map(MonitorMessage::Backlights)
            }
            MonitorTarget::ClightAuto => ToggleControl::<AutoBrightness>::refresh(buses.clone())
                .map(MonitorMessage::AutoBrightness),
        }
    }

    fn close_controls(&mut self) {
        self.close_backlight_sliders(None);
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: MonitorMessage, clock: Clock) -> Task<MonitorMessage> {
        match msg {
            MonitorMessage::Expansion(WidgetGroupExpansionMessage::HandleScrolled(delta)) => {
                self.scroll_all_backlights(delta)
            }
            MonitorMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_controls();
                }
                Task::none()
            }
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
            MonitorMessage::Backlight(serial, m) => self.update_backlight(serial, m),
            MonitorMessage::AutoBrightness(m) => self
                .clight
                .update(m, clock)
                .map(MonitorMessage::AutoBrightness),
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
                        "Backlight",
                        PERCENT,
                        MonitorTarget::Backlight(backlight.serial.clone()),
                        backlight_display::face(backlight),
                    )
                    .scroll_step(STEP)
                    .readout(|value, _| format!("{}%", as_percent(value)))
                    .announces_level("Brightness")
                })
                .update(ControlMessage::Reading(percent_f(backlight.percent)));
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
                ControlMessage::commit("backlight set", value, move || {
                    clightd_backlight_writes::set(&set_serial, as_percent(value))
                })
                .map(move |m| MonitorMessage::Backlight(serial.clone(), m))
            }
            Action::None | Action::Right | Action::Adjust(_) if write_settled => Task::perform(
                blocking("backlight reread", clightd_backlight::poll_all),
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

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: &MonitorTarget,
        outputs: &HashMap<String, OutputInfo>,
        clock: Clock,
    ) -> Option<HoverCard> {
        match target {
            MonitorTarget::Backlight(serial) => {
                let backlight = self
                    .backlights
                    .get()?
                    .iter()
                    .find(|b| &b.serial == serial)?;
                let brightness = self.sliders.get(serial)?.readout_cell();
                self.backlights
                    .hover_card_with(backlight_display::hover_card(
                        backlight,
                        brightness,
                        ConnectedOutput::driving(backlight, outputs).as_ref(),
                    ))
            }
            MonitorTarget::ClightAuto => self.clight.hover_card(clock),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, MonitorMessage> {
        let backlight_items = self.sliders.iter().map(|(serial, slider)| {
            let serial = serial.clone();
            GroupItem::slider(slider, config, move |m| {
                MonitorMessage::Backlight(serial.clone(), m)
            })
            .previewed_on_handle()
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Monitor,
            &self.expansion,
            MonitorMessage::Expansion,
            backlight_items.chain(
                self.clight
                    .group_item(MonitorTarget::ClightAuto, MonitorMessage::AutoBrightness)
                    .map(GroupItem::previewed_on_handle),
            ),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Lightbulb(Weight::Regular).into(),
        })
        .forwards_scroll((!self.sliders.is_empty()).then_some("change every backlight"))
    }
}
