use std::collections::HashMap;

use niri_ipc::{Request, Response, socket::Socket};

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

#[must_use]
pub fn niri_version() -> Option<String> {
    let reply = Socket::connect()
        .and_then(|mut socket| socket.send(Request::Version))
        .ok()?;
    match reply {
        Ok(Response::Version(version)) => Some(version),
        _ => None,
    }
}

#[must_use]
pub fn niri_outputs() -> HashMap<String, OutputInfo> {
    let Ok(reply) = Socket::connect().and_then(|mut socket| socket.send(Request::Outputs)) else {
        return HashMap::new();
    };
    match reply {
        Ok(Response::Outputs(outputs)) => outputs
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
            .collect(),
        _ => HashMap::new(),
    }
}

#[must_use]
pub fn niri_window_pids() -> Option<Vec<i32>> {
    let reply = Socket::connect()
        .and_then(|mut socket| socket.send(Request::Windows))
        .ok()?;
    let Ok(Response::Windows(windows)) = reply else {
        return None;
    };
    Some(windows.iter().filter_map(|window| window.pid).collect())
}
