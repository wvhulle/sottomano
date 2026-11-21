use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_runtime_facts::ContainerMemoryUse,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    waydroid_container::{SessionLcdDensity, SessionUserName, WaydroidReadout, WaydroidState},
};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::waydroid_container_group::{
        WaydroidContainerGroup, WaydroidContainerMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = WaydroidContainerGroup::default();
    let _ = group.update(
        WaydroidContainerMessage::Readout(SnapshotDelivery::fixture_delivered(
            Ok(WaydroidReadout {
                state: WaydroidState::Session {
                    session_state: "RUNNING".to_owned(),
                    user_name: SessionUserName::Reported("wvhulle".to_owned()),
                    lcd_density: SessionLcdDensity::Reported("180".to_owned()),
                },
                memory_in_use: ContainerMemoryUse::Measured(ByteSize::mib(1_420)),
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::WaydroidContainer(Box::new(group))
}
