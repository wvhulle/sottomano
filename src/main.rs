//! Odin → iced feasibility spike.
//!
//! Default: launch the iced_layershell bar (must run inside a niri session).
//! `--smoke`: run a headless backend test of the data layer (script + niri) with no
//! GUI, so (B) and (C) logic can be checked without a display.

mod app;
mod config;
mod niri;
mod script;
mod widget;

use app::Bar;
use iced_layershell::build_pattern::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|a| a == "--smoke") {
        return smoke();
    }

    daemon(Bar::new, Bar::namespace, Bar::update, Bar::view)
        .style(Bar::style)
        .subscription(app::subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                // `Background` opens no surface up-front: `Bar` enumerates niri outputs
                // and opens one `NewLayerShell` per (output, edge) itself, binding each
                // to its output by name. Per-surface size/anchor/zone come from those
                // `NewLayerShellSettings`, so nothing else is set here.
                start_mode: StartMode::Background,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()?;
    Ok(())
}

/// Headless backend smoke test — no iced, no display.
#[tokio::main]
async fn smoke() -> Result<(), Box<dyn std::error::Error>> {
    // (B) subprocess → JSON → typed data
    println!("== battery.nu ==");
    match script::run("battery", &[]).await {
        Ok(d) => println!(
            "  icon={:?} level={} available={} percent={:?}",
            d.icon,
            d.level,
            d.available,
            d.percent()
        ),
        Err(e) => println!("  ERROR: {e}"),
    }

    // (C) niri event stream → live workspace state (prints the first few events)
    println!("== niri event stream (first ~10 events) ==");
    let mut rx = niri::spawn_event_thread();
    let mut state = niri_ipc::state::EventStreamState::default();
    for _ in 0..10 {
        match rx.recv().await {
            Some(event) => {
                niri::apply(&mut state, event);
                let n = state.workspaces.workspaces.len();
                let active: Vec<String> = state
                    .workspaces
                    .workspaces
                    .values()
                    .filter(|w| w.is_active)
                    .map(|w| {
                        format!(
                            "{}:{}",
                            w.output.clone().unwrap_or_default(),
                            w.name.clone().unwrap_or_else(|| w.idx.to_string())
                        )
                    })
                    .collect();
                println!("  workspaces={n} active={active:?}");
            }
            None => break,
        }
    }
    Ok(())
}
