use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::{
    GpuClockSpeed, GpuHandle, GpuMemory, GpuModel, GpuPowerDraw, GpuPowerReading, GpuRuntimePower,
    GpuSnapshot, GpuTemperatureReading,
};

use super::{gpu_clock_label::GpuClockLabel, gpu_temperature_label::GpuTemperatureLabel};
use crate::{
    group_indicators::{
        indicator_group_content::IndicatorGroupExpansion, snapshot_absence::AbsenceCause,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub(super) enum GpuDeviceAccess {
    Readable {
        handle: GpuHandle,
        snapshot: SubscribedSnapshot<GpuSnapshot>,
    },
    Unreadable {
        handle_failure: AbsenceCause<GpuSnapshot>,
    },
}

pub(super) struct GpuDevice {
    pub(super) model: GpuModel,
    pub(super) access: GpuDeviceAccess,
    pub(super) runtime_power: GpuRuntimePower,
    pub(super) expansion: IndicatorGroupExpansion,
}

impl GpuDevice {
    pub(super) fn readable_handle(&self) -> Option<&GpuHandle> {
        match &self.access {
            GpuDeviceAccess::Readable { handle, .. } => Some(handle),
            GpuDeviceAccess::Unreadable { .. } => None,
        }
    }

    pub(super) fn latest_snapshot(&self) -> Option<&GpuSnapshot> {
        match &self.access {
            GpuDeviceAccess::Readable { snapshot, .. } => snapshot.get(),
            GpuDeviceAccess::Unreadable { .. } => None,
        }
    }

    pub(super) fn clock_label(&self) -> Option<GpuClockLabel> {
        let GpuClockSpeed::Reported(current) = self.latest_snapshot()?.graphics_clock else {
            return None;
        };
        Some(GpuClockLabel { current })
    }

    pub(super) fn memory(&self) -> Option<GpuMemory> {
        Some(self.latest_snapshot()?.memory.clone())
    }

    pub(super) fn temperature_label(&self) -> Option<GpuTemperatureLabel> {
        let GpuTemperatureReading::Measured(current) = self.latest_snapshot()?.temperature else {
            return None;
        };
        Some(GpuTemperatureLabel {
            current,
            limits: self.model.temperature_limits,
        })
    }

    pub(super) fn power_label(&self) -> Option<GpuPowerDraw> {
        let GpuPowerReading::Drawing(draw) = self.latest_snapshot()?.power_draw else {
            return None;
        };
        Some(GpuPowerDraw {
            draw,
            limit: self.model.power_limit,
        })
    }
}
