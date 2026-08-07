use std::collections::{BTreeMap, HashMap, HashSet};

use iced::{Element, Length, Rectangle, Task, widget::Space, window::Id};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    app::Message,
    config::Config,
    state::ChromeState,
    wayland_layers::{
        bar::float_margin,
        surface::{Margin, Position, Surface},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    TopStart,
    TopCenter,
    TopEnd,
    BottomStart,
    BottomMiddle,
    BottomEnd,
}

pub fn strip_namespace() -> &'static str {
    "vetrata-blur"
}

pub fn pill_namespace() -> &'static str {
    "vetrata-blur-pill"
}

pub fn fill<'a>() -> Element<'a, Message> {
    Space::new().width(Length::Fill).height(Length::Fill).into()
}

#[derive(Default)]
pub struct BlurCasters {
    ids: HashSet<Id>,
    strips: HashMap<Id, Id>,
    pills: HashMap<(Id, Zone), Vec<(Id, Rectangle)>>,
    rects: HashMap<(Id, Zone), Vec<Rectangle>>,
}

impl BlurCasters {
    pub fn contains(&self, id: Id) -> bool {
        self.ids.contains(&id)
    }

    pub fn rebuild(
        &mut self,
        config: &Config,
        chrome: &ChromeState,
        bars: &BTreeMap<Id, Surface>,
    ) -> Task<Message> {
        let mut tasks: Vec<Task<Message>> = self
            .ids
            .drain()
            .map(|id| Task::done(Message::RemoveWindow(id)))
            .collect();
        self.strips.clear();
        self.pills.clear();
        self.rects
            .retain(|(window, _), _| bars.contains_key(window));
        if chrome.blur_background {
            if chrome.disconnected_groups {
                let rects = self.rects.clone();
                for ((window, zone), list) in rects {
                    if let Some(surface) = bars.get(&window) {
                        tasks.push(self.open_pills(window, zone, &list, surface, config, chrome));
                    }
                }
            } else {
                for (&window, surface) in bars {
                    tasks.push(self.open_strip(window, surface, config, chrome));
                }
            }
        }
        Task::batch(tasks)
    }

    pub fn open_strip_for_new_bar(
        &mut self,
        bar: Id,
        surface: &Surface,
        config: &Config,
        chrome: &ChromeState,
    ) -> Task<Message> {
        if !chrome.blur_background || chrome.disconnected_groups {
            return Task::none();
        }
        self.open_strip(bar, surface, config, chrome)
    }

    pub fn pill_rects(
        &mut self,
        window: Id,
        zone: Zone,
        new: &[Rectangle],
        config: &Config,
        chrome: &ChromeState,
        bars: &BTreeMap<Id, Surface>,
    ) -> Task<Message> {
        self.rects.insert((window, zone), new.to_vec());
        if !(chrome.blur_background && chrome.disconnected_groups) {
            return Task::none();
        }
        let Some(surface) = bars.get(&window) else {
            return Task::none();
        };
        let position = surface.position;
        let output = surface.output.clone();
        let float = active_float_margin(chrome, position);
        let key = (window, zone);
        let mut current = self.pills.remove(&key).unwrap_or_default();
        let mut tasks = Vec::new();
        while current.len() > new.len() {
            let (id, _) = current.pop().expect("length checked above");
            self.ids.remove(&id);
            tasks.push(Task::done(Message::RemoveWindow(id)));
        }
        for (index, rect) in new.iter().enumerate() {
            match current.get_mut(index) {
                Some((id, old)) if old != rect => {
                    *old = *rect;
                    tasks.push(Task::done(Message::MarginChange {
                        id: *id,
                        margin: pill_margin(position, float, *rect).edges(),
                    }));
                    tasks.push(Task::done(Message::SizeChange {
                        id: *id,
                        size: pill_size(*rect, config),
                    }));
                }
                Some(_) => {}
                None => {
                    let id = Id::unique();
                    self.ids.insert(id);
                    current.push((id, *rect));
                    tasks.push(Task::done(Message::NewLayerShell {
                        settings: pill_settings(&output, position, *rect, config, chrome),
                        id,
                    }));
                }
            }
        }
        self.pills.insert(key, current);
        Task::batch(tasks)
    }

    pub fn reposition(&self, chrome: &ChromeState, bars: &BTreeMap<Id, Surface>) -> Task<Message> {
        let strips = self.strips.iter().filter_map(|(bar, &caster)| {
            let surface = bars.get(bar)?;
            Some(Task::done(Message::MarginChange {
                id: caster,
                margin: active_float_margin(chrome, surface.position).edges(),
            }))
        });
        let pills = self.pills.iter().flat_map(|((window, _), list)| {
            let surface = bars.get(window);
            list.iter().filter_map(move |&(caster, rect)| {
                let surface = surface?;
                let float = active_float_margin(chrome, surface.position);
                Some(Task::done(Message::MarginChange {
                    id: caster,
                    margin: pill_margin(surface.position, float, rect).edges(),
                }))
            })
        });
        Task::batch(strips.chain(pills).collect::<Vec<_>>())
    }

    pub fn forget(&mut self, window: Id) -> Task<Message> {
        self.ids.remove(&window);
        let mut closed = Vec::new();
        if let Some(caster) = self.strips.remove(&window) {
            closed.push(caster);
        }
        let zones: Vec<_> = self
            .pills
            .keys()
            .filter(|(bar, _)| *bar == window)
            .copied()
            .collect();
        for key in zones {
            if let Some(list) = self.pills.remove(&key) {
                closed.extend(list.into_iter().map(|(id, _)| id));
            }
        }
        self.rects.retain(|(bar, _), _| *bar != window);
        for id in &closed {
            self.ids.remove(id);
        }
        Task::batch(
            closed
                .into_iter()
                .map(|id| Task::done(Message::RemoveWindow(id))),
        )
    }

    fn open_strip(
        &mut self,
        bar: Id,
        surface: &Surface,
        config: &Config,
        chrome: &ChromeState,
    ) -> Task<Message> {
        let id = Id::unique();
        self.ids.insert(id);
        self.strips.insert(bar, id);
        Task::done(Message::NewLayerShell {
            settings: strip_settings(&surface.output, surface.position, config, chrome),
            id,
        })
    }

    fn open_pills(
        &mut self,
        window: Id,
        zone: Zone,
        rects: &[Rectangle],
        surface: &Surface,
        config: &Config,
        chrome: &ChromeState,
    ) -> Task<Message> {
        let casters: Vec<(Id, Rectangle)> =
            rects.iter().map(|&rect| (Id::unique(), rect)).collect();
        let tasks: Vec<Task<Message>> = casters
            .iter()
            .map(|&(id, rect)| {
                self.ids.insert(id);
                Task::done(Message::NewLayerShell {
                    settings: pill_settings(
                        &surface.output,
                        surface.position,
                        rect,
                        config,
                        chrome,
                    ),
                    id,
                })
            })
            .collect();
        self.pills.insert((window, zone), casters);
        Task::batch(tasks)
    }
}

fn active_float_margin(chrome: &ChromeState, position: Position) -> Margin {
    float_margin(position, chrome.margin_offset_px())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn strip_settings(
    output: &str,
    position: Position,
    config: &Config,
    chrome: &ChromeState,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, config.chrome.outer_height_px as u32)),
        exclusive_zone: Some(-1),
        anchor: position.anchor(),
        layer: Layer::Bottom,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(active_float_margin(chrome, position).edges()),
        events_transparent: true,
        namespace: Some(strip_namespace().to_string()),
    }
}

fn pill_settings(
    output: &str,
    position: Position,
    rect: Rectangle,
    config: &Config,
    chrome: &ChromeState,
) -> NewLayerShellSettings {
    let float = active_float_margin(chrome, position);
    let edge = match position {
        Position::Top => Anchor::Top,
        Position::Bottom => Anchor::Bottom,
    };
    NewLayerShellSettings {
        size: Some(pill_size(rect, config)),
        exclusive_zone: Some(-1),
        anchor: edge | Anchor::Left,
        layer: Layer::Bottom,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(pill_margin(position, float, rect).edges()),
        events_transparent: true,
        namespace: Some(pill_namespace().to_string()),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn pill_margin(position: Position, float: Margin, rect: Rectangle) -> Margin {
    let left = float.left + rect.x.round() as i32;
    match position {
        Position::Top => Margin {
            top: float.top,
            right: 0,
            bottom: 0,
            left,
        },
        Position::Bottom => Margin {
            top: 0,
            right: 0,
            bottom: float.bottom,
            left,
        },
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pill_size(rect: Rectangle, config: &Config) -> (u32, u32) {
    (
        rect.width.round().max(1.0) as u32,
        config.chrome.outer_height_px as u32,
    )
}
