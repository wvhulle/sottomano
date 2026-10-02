pub mod backlight_display;
pub mod clight_auto_brightness;

use std::{collections::BTreeMap, ops::RangeInclusive};

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        backlight::{self, Backlight},
        clight::Clight,
    },
    message_subscriptions::blocking,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 5.0;

#[derive(Default)]
pub struct MonitorGroup {
    backlights: Vec<Backlight>,
    sliders: BTreeMap<String, SliderControl>,
    clight: ToggleControl<Clight>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Expansion(ExpansionMessage),
    Backlights(Vec<Backlight>),
    Backlight(String, ControlMessage),
    Clight(ToggleMessage<Clight>),
}

impl MonitorGroup {
    pub fn probe() -> Task<MonitorMessage> {
        ToggleControl::<Clight>::probe(()).map(MonitorMessage::Clight)
    }

    pub fn subscription(&self) -> Subscription<MonitorMessage> {
        Subscription::batch([
            backlight::subscription().map(MonitorMessage::Backlights),
            self.clight.subscription(()).map(MonitorMessage::Clight),
        ])
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
            MonitorMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_controls();
                }
                Task::none()
            }
            MonitorMessage::Backlights(list) => {
                self.sliders
                    .retain(|serial, _| list.iter().any(|b| &b.serial == serial));
                for backlight in &list {
                    self.sliders
                        .entry(backlight.serial.clone())
                        .or_insert_with(|| {
                            SliderControl::new(
                                PERCENT,
                                HoverTarget::Backlight(backlight.serial.clone()),
                                backlight_display::face(backlight),
                            )
                            .scroll_step(STEP)
                            .readout(|value| format!("{}%", as_percent(value)))
                        })
                        .update(ControlMessage::Reading(percent_f(backlight.percent)));
                }
                self.backlights = list;
                Task::none()
            }
            MonitorMessage::Backlight(serial, m) => self.update_backlight(serial, m),
            MonitorMessage::Clight(m) => self.clight.update(m).map(MonitorMessage::Clight),
        }
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
                blocking("backlight reread", backlight::poll_all).map(MonitorMessage::Backlights)
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Backlight(serial) => {
                let backlight = self.backlights.iter().find(|b| &b.serial == serial)?;
                let brightness = self.sliders.get(serial)?.readout_text();
                Some(Tooltip::of(backlight_display::tooltip(
                    backlight, brightness,
                )))
            }
            HoverTarget::ClightAuto => self.clight.tooltip(),
            _ => None,
        }
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, MonitorMessage> {
        let backlight_sliders: Vec<&'a SliderControl> = self
            .backlights
            .iter()
            .filter_map(|backlight| self.sliders.get(&backlight.serial))
            .collect();
        let previewed_value = backlight_sliders
            .first()
            .map(|slider| *slider as &dyn BarWidget)
            .or(self.clight.reading());
        WidgetGroup::new(&self.expansion, MonitorMessage::Expansion)
            .icon(Phosphor::Lightbulb(Weight::Regular))
            .previewed_value(previewed_value)
            .items(self.backlights.iter().filter_map(|backlight| {
                let slider = self.sliders.get(&backlight.serial)?;
                let serial = backlight.serial.clone();
                Some(GroupItem::slider(slider, config, move |m| {
                    MonitorMessage::Backlight(serial.clone(), m)
                }))
            }))
            .item(GroupItem::toggle(
                &self.clight,
                HoverTarget::ClightAuto,
                MonitorMessage::Clight,
            ))
    }
}
