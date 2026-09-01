use std::collections::{HashMap, HashSet};

use iced::{Task, window::Id};
use iced_layershell::reexport::{Anchor, Layer, NewLayerShellSettings};

use crate::{app::Message, wayland_layers::surface};

pub const NAMESPACE: &str = "vetrata-notifications";

#[derive(Default)]
pub struct ToastSurfaces {
    by_output: HashMap<Option<String>, Id>,
}

impl ToastSurfaces {
    pub fn output_of(&self, window: Id) -> Option<&Option<String>> {
        self.by_output
            .iter()
            .find(|(_, id)| **id == window)
            .map(|(output, _)| output)
    }

    pub fn window_on(&self, output: Option<&String>) -> Option<Id> {
        self.by_output
            .iter()
            .find(|(key, _)| key.as_ref() == output)
            .map(|(_, id)| *id)
    }

    pub fn forget(&mut self, window: Id) {
        self.by_output.retain(|_, id| *id != window);
    }

    pub fn reconcile(&mut self, desired: &HashSet<Option<String>>) -> Task<Message> {
        let closes: Vec<Task<Message>> = self
            .by_output
            .iter()
            .filter(|(output, _)| !desired.contains(*output))
            .map(|(_, id)| Task::done(Message::RemoveWindow(*id)))
            .collect();
        self.by_output.retain(|output, _| desired.contains(output));

        let missing: Vec<Option<String>> = desired
            .iter()
            .filter(|output| !self.by_output.contains_key(*output))
            .cloned()
            .collect();
        let opens: Vec<Task<Message>> = missing
            .into_iter()
            .map(|output| {
                let window = Id::unique();
                let settings = field_settings(output.as_deref());
                self.by_output.insert(output, window);
                Task::done(Message::NewLayerShell {
                    settings,
                    id: window,
                })
            })
            .collect();

        Task::batch(closes.into_iter().chain(opens))
    }
}

fn field_settings(output: Option<&str>) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, 0)),
        exclusive_zone: Some(0),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        ..surface::base_settings(output, NAMESPACE)
    }
}
