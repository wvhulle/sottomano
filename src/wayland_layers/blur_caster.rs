use std::collections::{BTreeMap, HashMap, HashSet};

use iced::{Element, Length, Rectangle, Task, widget::Space, window::Id};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};

use crate::{
    app::Message,
    config::Config,
    data::sink::niri_rules::StyledSurface,
    state::PanelLayout,
    wayland_layers::surface::{self, Margin, Position, Surface},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    TopStart,
    TopCenter,
    TopEnd,
    BottomStart,
    BottomMiddle,
    BottomEnd,
    Sidebar,
}

pub fn fill<'a>() -> Element<'a, Message> {
    Space::new().width(Length::Fill).height(Length::Fill).into()
}

#[derive(Default)]
pub struct BlurCasters {
    ids: HashSet<Id>,
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
        layout: &PanelLayout,
        bars: &BTreeMap<Id, Surface>,
    ) -> Task<Message> {
        let mut tasks: Vec<Task<Message>> = self
            .ids
            .drain()
            .map(|id| Task::done(Message::RemoveWindow(id)))
            .collect();
        self.pills.clear();
        self.rects
            .retain(|(window, _), _| bars.contains_key(window));
        if casts_pills(layout) {
            let rects = self.rects.clone();
            for ((window, zone), list) in rects {
                if let Some(surface) = bars.get(&window) {
                    tasks.push(self.open_pills(window, zone, &list, surface, config, layout));
                }
            }
        }
        Task::batch(tasks)
    }

    pub fn pill_rects(
        &mut self,
        window: Id,
        zone: Zone,
        new: &[Rectangle],
        config: &Config,
        layout: &PanelLayout,
        bars: &BTreeMap<Id, Surface>,
    ) -> Task<Message> {
        self.rects.insert((window, zone), new.to_vec());
        let Some(surface) = bars
            .get(&window)
            .filter(|surface| !surface.position.is_vertical() && casts_pills(layout))
        else {
            return Task::none();
        };
        let position = surface.position;
        let output = surface.output.clone();
        let float = active_float_margin(layout, position);
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
                    tasks.push(Task::done(Message::LayoutChange {
                        id: *id,
                        anchor: pill_anchor(position),
                        size: pill_size(position, *rect, config),
                    }));
                }
                Some(_) => {}
                None => {
                    let id = Id::unique();
                    self.ids.insert(id);
                    current.push((id, *rect));
                    tasks.push(Task::done(Message::NewLayerShell {
                        settings: pill_settings(&output, position, *rect, config, layout),
                        id,
                    }));
                }
            }
        }
        self.pills.insert(key, current);
        Task::batch(tasks)
    }

    pub fn reposition(&self, layout: &PanelLayout, bars: &BTreeMap<Id, Surface>) -> Task<Message> {
        let pills = self.pills.iter().flat_map(|((window, _), list)| {
            let surface = bars.get(window);
            list.iter().filter_map(move |&(caster, rect)| {
                let surface = surface?;
                let float = active_float_margin(layout, surface.position);
                Some(Task::done(Message::MarginChange {
                    id: caster,
                    margin: pill_margin(surface.position, float, rect).edges(),
                }))
            })
        });
        Task::batch(pills.collect::<Vec<_>>())
    }

    pub fn forget(&mut self, window: Id) -> Task<Message> {
        self.ids.remove(&window);
        let mut closed = Vec::new();
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

    fn open_pills(
        &mut self,
        window: Id,
        zone: Zone,
        rects: &[Rectangle],
        surface: &Surface,
        config: &Config,
        layout: &PanelLayout,
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
                        layout,
                    ),
                    id,
                })
            })
            .collect();
        self.pills.insert((window, zone), casters);
        Task::batch(tasks)
    }
}

fn casts_pills(layout: &PanelLayout) -> bool {
    layout.disconnected_groups
}

fn active_float_margin(layout: &PanelLayout, position: Position) -> Margin {
    position.float_margin(layout.margin_offset_px())
}

fn pill_anchor(position: Position) -> Anchor {
    match position {
        Position::Top | Position::Left => Anchor::Top | Anchor::Left,
        Position::Bottom => Anchor::Bottom | Anchor::Left,
    }
}

fn pill_settings(
    output: &str,
    position: Position,
    rect: Rectangle,
    config: &Config,
    layout: &PanelLayout,
) -> NewLayerShellSettings {
    let float = active_float_margin(layout, position);
    NewLayerShellSettings {
        size: pill_size(position, rect, config),
        exclusive_zone: Some(-1),
        anchor: pill_anchor(position),
        layer: Layer::Bottom,
        margin: Some(pill_margin(position, float, rect).edges()),
        events_transparent: true,
        ..surface::base_settings(Some(output), StyledSurface::Pill.namespace())
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
        Position::Left => Margin {
            top: float.top + rect.y.round() as i32,
            right: 0,
            bottom: 0,
            left,
        },
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pill_size(position: Position, rect: Rectangle, config: &Config) -> LayerSize {
    let height = match position {
        Position::Top | Position::Bottom => config.bar.height_px,
        Position::Left => rect.height.round().max(1.0),
    };
    LayerSize::px(rect.width.round().max(1.0) as u32, height as u32)
}
