use std::{future::Future, pin::Pin};

use iced::{Rectangle, window::Id};
use iced_exwlshell::reexport::{
    ActivationTokenSink, KeyboardInteractivity, NewLayerShellSettings, OutputOption,
    XdgActivationTokenV1State,
};
use sottomano_toml_config::screen_edges::Position;
use tokio::sync::mpsc;

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

#[derive(Debug, Clone)]
pub enum ActivationTokenOutcome {
    Issued(String),
    Unavailable,
}

pub struct ActivationTokenRequest {
    pub surface_request: SurfaceRequest,
    pub token_arrival: Pin<Box<dyn Future<Output = ActivationTokenOutcome> + Send>>,
}

#[must_use]
pub fn activation_token_request(surface: Id) -> ActivationTokenRequest {
    let (token_sender, mut token_receiver) = mpsc::channel(1);
    let sink = ActivationTokenSink::callback(move |token_state| {
        let token_outcome = match token_state {
            XdgActivationTokenV1State::Ready(token_wrapper) => {
                ActivationTokenOutcome::Issued(token_wrapper.token().to_owned())
            }
            XdgActivationTokenV1State::Unsupported | XdgActivationTokenV1State::NoSurface => {
                ActivationTokenOutcome::Unavailable
            }
        };
        let _ = token_sender.try_send(token_outcome);
    });
    ActivationTokenRequest {
        surface_request: SurfaceRequest::ActivationToken { surface, sink },
        token_arrival: Box::pin(async move {
            token_receiver
                .recv()
                .await
                .unwrap_or(ActivationTokenOutcome::Unavailable)
        }),
    }
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
