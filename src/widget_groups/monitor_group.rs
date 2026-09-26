pub mod backlight_display;
pub mod clight_auto_brightness;
pub mod umbra_dark_mode;

use std::{collections::BTreeMap, ops::RangeInclusive};

use iced::{Alignment, Element, Subscription, Task, widget::Row};

use crate::{
    config::Config,
    data::source::{
        backlight::{self, Backlight},
        clight::Clight,
        umbra::Umbra,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        bar_widget::BarWidget,
        font_awesome::Solid,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::Tooltip,
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 5.0;

#[derive(Default)]
pub struct MonitorGroup {
    backlights: Vec<Backlight>,
    sliders: BTreeMap<String, SliderControl>,
    clight: ToggleControl<Clight>,
    umbra: ToggleControl<Umbra>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Menu(MenuMessage),
    Backlights(Vec<Backlight>),
    Backlight(String, ControlMessage),
    Clight(ToggleMessage<Clight>),
    Umbra(ToggleMessage<Umbra>),
    Tooltip(TooltipMessage),
}

impl MonitorGroup {
    pub fn probe() -> Task<MonitorMessage> {
        Task::batch([
            ToggleControl::<Clight>::probe(()).map(MonitorMessage::Clight),
            ToggleControl::<Umbra>::probe(()).map(MonitorMessage::Umbra),
        ])
    }

    pub fn subscription(&self) -> Subscription<MonitorMessage> {
        Subscription::batch([
            backlight::subscription().map(MonitorMessage::Backlights),
            self.clight.subscription(()).map(MonitorMessage::Clight),
            self.umbra.subscription(()).map(MonitorMessage::Umbra),
        ])
    }

    pub fn is_animating(&self) -> bool {
        self.sliders.values().any(SliderControl::is_animating)
    }

    fn close_controls(&mut self) {
        self.close_backlight_sliders(None);
    }

    pub fn close_menu(&mut self) {
        if self.menu.close() {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: MonitorMessage) -> Task<MonitorMessage> {
        match msg {
            MonitorMessage::Menu(m) => {
                if self.menu.update(m) {
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
            MonitorMessage::Umbra(m) => self.umbra.update(m).map(MonitorMessage::Umbra),
            MonitorMessage::Tooltip(_) => Task::none(),
        }
    }

    fn update_backlight(&mut self, serial: String, m: ControlMessage) -> Task<MonitorMessage> {
        let write_settled = matches!(m, ControlMessage::Confirmed { ok: true, .. });
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
            HoverTarget::DarkMode => self.umbra.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, MonitorMessage>> {
        let children = move || {
            let mut children: Vec<Element<'a, MonitorMessage>> = Vec::new();
            for backlight in &self.backlights {
                if let Some(control) = self.sliders.get(&backlight.serial) {
                    children.push(backlight_slider(config, control, &backlight.serial));
                }
            }
            if let Some(clight) = self.clight.view(config, HoverTarget::ClightAuto, 1.0) {
                children.push(clight.map(|m| match m {
                    ToggleMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
                    m => MonitorMessage::Clight(m),
                }));
            }
            if let Some(umbra) = self.umbra.view(config, HoverTarget::DarkMode, 1.0) {
                children.push(umbra.map(|m| match m {
                    ToggleMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
                    m => MonitorMessage::Umbra(m),
                }));
            }
            children
        };

        let first_child = self
            .backlights
            .iter()
            .filter_map(|backlight| self.sliders.get(&backlight.serial))
            .map(|slider| slider as &dyn BarWidget)
            .chain(self.clight.reading())
            .chain(self.umbra.reading())
            .next()?;

        let content = move |_reveal: f32| {
            Row::with_children(children())
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };
        Some(
            self.menu
                .slot(move || Self::face(config), content, MonitorMessage::Menu)
                .previewed_value(first_child)
                .view(config, position),
        )
    }

    fn face<'a>(config: &Config) -> Element<'a, MonitorMessage> {
        Solid::Lightbulb
            .glyph()
            .size(config.font.icon_size_px)
            .color(config.palette.bar_foreground)
            .into()
    }
}

fn backlight_slider<'a>(
    config: &Config,
    control: &SliderControl,
    serial: &str,
) -> Element<'a, MonitorMessage> {
    let serial = serial.to_string();
    control.view(config, 1.0).map(move |m| match m {
        ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
        m => MonitorMessage::Backlight(serial.clone(), m),
    })
}
