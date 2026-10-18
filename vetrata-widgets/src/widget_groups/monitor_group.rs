mod backlight_display;
mod clight_auto_brightness;
mod output_details;

use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
};

use iced::{Subscription, Task, mouse::ScrollDelta};
use output_details::ConnectedOutput;
use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    background_work::blocking,
    backlight::{self, Backlight},
    bus::DesktopBuses,
    clight::Clight,
    niri::OutputInfo,
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
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
    backlights: ContinuousReading<Vec<Backlight>>,
    sliders: BTreeMap<String, SliderControl>,
    clight: ToggleControl<Clight>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Expansion(ExpansionMessage),
    Backlights(SourceDelivery<Vec<Backlight>>),
    Backlight(String, ControlMessage),
    Clight(ToggleMessage<Clight>),
}

impl MonitorGroup {
    pub fn subscription(buses: &DesktopBuses) -> Subscription<MonitorMessage> {
        Subscription::batch([
            source_subscription::<Vec<Backlight>>(buses.clone()).map(MonitorMessage::Backlights),
            ToggleControl::<Clight>::subscription(buses.clone()).map(MonitorMessage::Clight),
        ])
    }

    pub fn refresh(target: &MonitorTarget, buses: &DesktopBuses) -> Task<MonitorMessage> {
        match target {
            MonitorTarget::Backlight(_) => {
                first_delivery::<Vec<Backlight>>(buses.clone()).map(MonitorMessage::Backlights)
            }
            MonitorTarget::ClightAuto => {
                ToggleControl::<Clight>::refresh(buses.clone()).map(MonitorMessage::Clight)
            }
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

    pub fn update(&mut self, msg: MonitorMessage) -> Task<MonitorMessage> {
        match msg {
            MonitorMessage::Expansion(ExpansionMessage::HandleScrolled(delta)) => {
                self.scroll_all_backlights(delta)
            }
            MonitorMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_controls();
                }
                Task::none()
            }
            MonitorMessage::Backlights(delivery) => {
                self.backlights.receive_keeping_last_reading(delivery);
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
                            .readout(|value| format!("{}%", as_percent(value)))
                            .announces_level("Brightness")
                        })
                        .update(ControlMessage::Reading(percent_f(backlight.percent)));
                }
                Task::none()
            }
            MonitorMessage::Backlight(serial, m) => self.update_backlight(serial, m),
            MonitorMessage::Clight(m) => self.clight.update(m).map(MonitorMessage::Clight),
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

    fn update_backlight(&mut self, serial: String, m: ControlMessage) -> Task<MonitorMessage> {
        let write_settled = matches!(m, ControlMessage::DeviceAccepted { ok: true, .. });
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
                    backlight::set(&set_serial, as_percent(value))
                })
                .map(move |m| MonitorMessage::Backlight(serial.clone(), m))
            }
            Action::None | Action::Right | Action::Adjust(_) if write_settled => {
                Task::perform(blocking("backlight reread", backlight::poll_all), |list| {
                    MonitorMessage::Backlights(SourceDelivery::Reading(Ok(list)))
                })
            }
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
    pub fn tooltip_for(
        &self,
        target: &MonitorTarget,
        outputs: &HashMap<String, OutputInfo>,
    ) -> Option<Tooltip> {
        match target {
            MonitorTarget::Backlight(serial) => {
                let backlight = self
                    .backlights
                    .get()?
                    .iter()
                    .find(|b| &b.serial == serial)?;
                let brightness = self.sliders.get(serial)?.readout_text();
                self.backlights.tooltip_with(backlight_display::tooltip(
                    backlight,
                    brightness,
                    ConnectedOutput::driving(backlight, outputs).as_ref(),
                ))
            }
            MonitorTarget::ClightAuto => self.clight.tooltip(),
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, MonitorMessage> {
        let previewed_value = self
            .sliders
            .values()
            .next()
            .map(|slider| slider as &dyn BarWidget)
            .or(self.clight.reading());
        let backlight_items = self.sliders.iter().map(|(serial, slider)| {
            let serial = serial.clone();
            GroupItem::slider(slider, config, move |m| {
                MonitorMessage::Backlight(serial.clone(), m)
            })
        });
        GroupContent::new(
            &self.expansion,
            MonitorMessage::Expansion,
            backlight_items.chain(
                self.clight
                    .group_item(MonitorTarget::ClightAuto, MonitorMessage::Clight),
            ),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Lightbulb(Weight::Regular).into(),
            handle: GroupHandle::Monitor,
        })
        .preview(previewed_value.map(PreviewedValue::Plain))
        .forwards_scroll((!self.sliders.is_empty()).then_some("change every backlight"))
    }
}
