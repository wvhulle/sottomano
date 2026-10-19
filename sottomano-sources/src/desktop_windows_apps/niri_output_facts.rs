use std::collections::HashMap;

use niri_ipc::{Request, Response, socket::Socket};

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub mode: Option<OutputMode>,
    pub scale: f64,
    pub logical_height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputMode {
    pub width: u16,
    pub height: u16,
    pub refresh_hz: f64,
}

fn describes_a_product(edid_field: &str) -> bool {
    let trimmed = edid_field.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        return false;
    }
    !trimmed
        .strip_prefix("0x")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

fn product_field(edid_field: String) -> Option<String> {
    describes_a_product(&edid_field).then_some(edid_field)
}

fn current_mode(output: &niri_ipc::Output) -> Option<OutputMode> {
    let mode = output.modes.get(output.current_mode?)?;
    Some(OutputMode {
        width: mode.width,
        height: mode.height,
        refresh_hz: f64::from(mode.refresh_rate) / 1000.0,
    })
}

#[must_use]
pub fn version() -> Option<String> {
    let reply = Socket::connect()
        .and_then(|mut socket| socket.send(Request::Version))
        .ok()?;
    match reply {
        Ok(Response::Version(version)) => Some(version),
        _ => None,
    }
}

#[must_use]
pub fn output_info() -> HashMap<String, OutputInfo> {
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
                        logical_height: logical.height,
                    },
                ))
            })
            .collect(),
        _ => HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::describes_a_product;

    #[test]
    fn edid_product_codes_are_not_product_names() {
        assert!(!describes_a_product("0x07BE"));
        assert!(!describes_a_product("0x0000ffa1"));
        assert!(!describes_a_product(""));
        assert!(!describes_a_product("   "));
        assert!(!describes_a_product("Unknown"));
    }

    #[test]
    fn real_edid_strings_are_product_names() {
        assert!(describes_a_product("LG Display"));
        assert!(describes_a_product("U2720Q"));
        assert!(describes_a_product("0xSomething"));
    }
}
