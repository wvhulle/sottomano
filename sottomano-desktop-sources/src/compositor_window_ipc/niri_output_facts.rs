use std::{collections::HashMap, io};

use niri_ipc::{Request, Response, socket::Socket};
use tokio::task::JoinError;

use crate::compositor_window_ipc::compositor_output_facts::{
    OutputInfo, OutputMode, product_field,
};

fn current_mode(output: &niri_ipc::Output) -> Option<OutputMode> {
    let mode = output.modes.get(output.current_mode?)?;
    Some(OutputMode {
        width: mode.width,
        height: mode.height,
        refresh_hz: f64::from(mode.refresh_rate) / 1000.0,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum NiriQueryError {
    #[error("niri socket: {0}")]
    Socket(#[from] io::Error),
    #[error("niri refused the request: {0}")]
    Refused(String),
    #[error("niri answered with an unexpected response")]
    UnexpectedResponse,
    #[error("niri query did not finish: {0}")]
    Unfinished(#[from] JoinError),
}

fn niri_response(request: Request) -> Result<Response, NiriQueryError> {
    Socket::connect()
        .and_then(|mut socket| socket.send(request))?
        .map_err(NiriQueryError::Refused)
}

pub fn niri_version() -> Result<String, NiriQueryError> {
    match niri_response(Request::Version)? {
        Response::Version(version) => Ok(version),
        _ => Err(NiriQueryError::UnexpectedResponse),
    }
}

pub fn niri_outputs() -> Result<HashMap<String, OutputInfo>, NiriQueryError> {
    match niri_response(Request::Outputs)? {
        Response::Outputs(outputs) => Ok(outputs
            .into_iter()
            .filter_map(|(name, output)| {
                let logical = output.logical?;
                let mode = current_mode(&output);
                Some((
                    name,
                    OutputInfo {
                        make: product_field(output.make),
                        model: product_field(output.model),
                        serial: output.serial.and_then(product_field),
                        mode,
                        scale: logical.scale,
                        logical_width: logical.width,
                        logical_height: logical.height,
                    },
                ))
            })
            .collect()),
        _ => Err(NiriQueryError::UnexpectedResponse),
    }
}

pub fn niri_window_pids() -> Result<Vec<i32>, NiriQueryError> {
    match niri_response(Request::Windows)? {
        Response::Windows(windows) => Ok(windows.iter().filter_map(|window| window.pid).collect()),
        _ => Err(NiriQueryError::UnexpectedResponse),
    }
}
