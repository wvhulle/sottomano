use iced::{Rectangle, window::Id};
use iced_exwlshell::{
    actions::RegionActionCB,
    reexport::{KeyboardInteractivity, NewLayerShellSettings, OutputOption},
};
use sottomano_config::screen_edges::Position;

use crate::shell_message::Message;

#[derive(Debug, Clone)]
pub struct PanelEdge {
    pub output: String,
    pub position: Position,
}

#[must_use]
pub fn input_region(surface: Id, bounds: Rectangle) -> Message {
    input_regions(surface, vec![bounds])
}

#[must_use]
pub fn input_regions(surface: Id, rects: Vec<Rectangle>) -> Message {
    Message::SetInputRegion {
        id: surface,
        callback: RegionActionCB::new(move |region| {
            rects
                .iter()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                .for_each(|rect| {
                    let _ = region.add(
                        rect.x as i32,
                        rect.y as i32,
                        rect.width.ceil() as i32,
                        rect.height.ceil() as i32,
                    );
                });
        }),
    }
}

#[must_use]
pub fn base_settings(output: Option<&str>, namespace: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output.map_or(OutputOption::Active, |name| {
            OutputOption::OutputName(name.to_string())
        }),
        namespace: Some(namespace.to_string()),
        ..Default::default()
    }
}
