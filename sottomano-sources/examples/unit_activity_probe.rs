use futures::StreamExt;
use sottomano_sources::{
    ReadingSource,
    bus::DesktopBuses,
    systemd_manager::{
        ScopedSystemdUnit, SystemdUnitOnBus, SystemdUnitScope, unit_activity::UnitActiveState,
    },
};

#[tokio::main]
async fn main() {
    let unit_name = std::env::args().nth(1).expect("unit name");
    let buses = DesktopBuses::connect().await.expect("buses");
    let inputs = SystemdUnitOnBus {
        buses,
        unit: ScopedSystemdUnit {
            scope: SystemdUnitScope::UserSession,
            unit_name,
        },
    };
    let mut stream = std::pin::pin!(UnitActiveState::delivery_stream(inputs));
    while let Some(delivery) = stream.next().await {
        println!(
            "{} {delivery:?}",
            chrono::Local::now().format("%H:%M:%S%.3f")
        );
    }
    println!("stream ended");
}
