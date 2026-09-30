use std::collections::{BTreeMap, HashMap};

use iced::{Rectangle, Task, window::Id};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use tokio::time::sleep;

use crate::{
    app::Message,
    config::Config,
    state::PanelLayout,
    wayland_layers::{
        niri_rules::StyledSurface,
        surface::{self, Margin, Position, Surface},
    },
    widgets::widget_group::EXPAND,
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

#[derive(Debug, Clone, Copy)]
pub struct PillPlacement {
    pub strip: Id,
    pub zone: Zone,
    pub index: usize,
    pub rect: Rectangle,
}

#[derive(Default)]
pub struct PanelPills {
    pills: HashMap<(Id, Zone), Vec<(Id, Rectangle)>>,
    rects: HashMap<(Id, Zone), Vec<Rectangle>>,
}

impl PanelPills {
    pub fn placement(&self, pill: Id) -> Option<PillPlacement> {
        self.pills.iter().find_map(|(&(strip, zone), list)| {
            list.iter()
                .position(|&(id, _)| id == pill)
                .map(|index| PillPlacement {
                    strip,
                    zone,
                    index,
                    rect: list[index].1,
                })
        })
    }

    pub fn rebuild(
        &mut self,
        config: &Config,
        layout: &PanelLayout,
        bars: &BTreeMap<Id, Surface>,
    ) -> Task<Message> {
        let closed = close(self.pills.drain().flat_map(|(_, list)| list));
        self.rects
            .retain(|(window, _), _| bars.contains_key(window));
        let reopened: Vec<Task<Message>> = self
            .rects
            .clone()
            .into_iter()
            .map(|((window, zone), _)| self.follow(window, zone, config, layout, bars))
            .collect();
        Task::batch([closed, Task::batch(reopened)])
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
        if self.shrinks(window, zone, new) {
            return Task::future(sleep(EXPAND))
                .map(move |()| Message::PillsSettled { window, zone });
        }
        self.follow(window, zone, config, layout, bars)
    }

    fn shrinks(&self, window: Id, zone: Zone, new: &[Rectangle]) -> bool {
        self.pills.get(&(window, zone)).is_some_and(|current| {
            new.len() < current.len()
                || current
                    .iter()
                    .zip(new)
                    .any(|(&(_, old), rect)| rect.width < old.width)
        })
    }

    pub fn follow(
        &mut self,
        window: Id,
        zone: Zone,
        config: &Config,
        layout: &PanelLayout,
        bars: &BTreeMap<Id, Surface>,
    ) -> Task<Message> {
        let new = self.rects.get(&(window, zone)).cloned().unwrap_or_default();
        let Some(surface) = bars
            .get(&window)
            .filter(|surface| !surface.position.is_vertical() && layout.disconnected_groups)
        else {
            return Task::none();
        };
        let position = surface.position;
        let float = position.float_margin(layout.margin_offset_px());
        let mut current = self.pills.remove(&(window, zone)).unwrap_or_default();
        let surplus = close(current.drain(new.len().min(current.len())..));
        let mut tasks = vec![surplus];
        for (index, &rect) in new.iter().enumerate() {
            match current.get_mut(index) {
                Some((id, old)) if *old != rect => {
                    *old = rect;
                    tasks.push(Task::done(Message::MarginChange {
                        id: *id,
                        margin: pill_margin(position, float, rect).edges(),
                    }));
                    tasks.push(Task::done(Message::LayoutChange {
                        id: *id,
                        anchor: pill_anchor(position),
                        size: pill_size(rect, config),
                    }));
                }
                Some(_) => {}
                None => {
                    let id = Id::unique();
                    current.push((id, rect));
                    tasks.push(Task::done(Message::NewLayerShell {
                        settings: pill_settings(&surface.output, position, float, rect, config),
                        id,
                    }));
                }
            }
        }
        self.pills.insert((window, zone), current);
        Task::batch(tasks)
    }

    pub fn reposition(&self, layout: &PanelLayout, bars: &BTreeMap<Id, Surface>) -> Task<Message> {
        let pills = self.pills.iter().flat_map(|((window, _), list)| {
            let surface = bars.get(window);
            list.iter().filter_map(move |&(pill, rect)| {
                let position = surface?.position;
                let float = position.float_margin(layout.margin_offset_px());
                Some(Task::done(Message::MarginChange {
                    id: pill,
                    margin: pill_margin(position, float, rect).edges(),
                }))
            })
        });
        Task::batch(pills.collect::<Vec<_>>())
    }

    pub fn forget(&mut self, window: Id) -> Task<Message> {
        self.rects.retain(|(bar, _), _| *bar != window);
        let strips: Vec<(Id, Zone)> = self
            .pills
            .keys()
            .filter(|(bar, _)| *bar == window)
            .copied()
            .collect();
        close(
            strips
                .into_iter()
                .filter_map(|key| self.pills.remove(&key))
                .flatten()
                .collect::<Vec<_>>(),
        )
    }
}

fn close(pills: impl IntoIterator<Item = (Id, Rectangle)>) -> Task<Message> {
    Task::batch(
        pills
            .into_iter()
            .map(|(id, _)| Task::done(Message::RemoveWindow(id)))
            .collect::<Vec<_>>(),
    )
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
    float: Margin,
    rect: Rectangle,
    config: &Config,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: pill_size(rect, config),
        exclusive_zone: Some(-1),
        anchor: pill_anchor(position),
        layer: Layer::Top,
        margin: Some(pill_margin(position, float, rect).edges()),
        ..surface::base_settings(Some(output), StyledSurface::Pill.namespace())
    }
}

#[allow(clippy::cast_possible_truncation)]
fn pill_margin(position: Position, float: Margin, rect: Rectangle) -> Margin {
    let left = float.left + rect.x.round() as i32;
    match position {
        Position::Top | Position::Left => Margin {
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
fn pill_size(rect: Rectangle, config: &Config) -> LayerSize {
    LayerSize::px(
        rect.width.ceil().max(1.0) as u32,
        config.bar.height_px as u32,
    )
}
