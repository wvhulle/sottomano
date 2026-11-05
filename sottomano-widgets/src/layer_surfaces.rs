use iced::{Rectangle, window::Id};
use iced_exwlshell::reexport::{
    ActivationTokenSink, KeyboardInteractivity, NewLayerShellSettings, OutputOption,
};
use sottomano_toml_config::screen_edges::Position;

#[derive(Debug, Clone)]
pub struct PanelEdge {
    pub output: String,
    pub position: Position,
}

#[derive(Debug, Clone)]
pub enum SurfaceRequest {
    Open {
        settings: NewLayerShellSettings,
        surface: Id,
    },
    Close {
        surface: Id,
    },
    InputRegions {
        surface: Id,
        regions: Vec<Rectangle>,
    },
    KeyboardInteractivity {
        surface: Id,
        interactivity: KeyboardInteractivity,
    },
    ActivationToken {
        surface: Id,
        sink: ActivationTokenSink,
    },
}

#[must_use]
pub fn input_regions(surface: Id, regions: Vec<Rectangle>) -> SurfaceRequest {
    SurfaceRequest::InputRegions { surface, regions }
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
