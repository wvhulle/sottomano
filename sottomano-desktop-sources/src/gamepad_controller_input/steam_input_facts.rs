use crate::steam_client_files::{
    steam_controller_facts::SteamControllerFacts,
    steam_log_lines::{
        controller_log_lines::RadioSignalSample, hardware_update_log_lines::SteamHardwareKind,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioReading {
    Unattributed,
    Measured(RadioSignalSample),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareStatus {
    Current,
    UpdatePending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingEvidence {
    SteamControllerLog,
    InputTimingCorrelation,
    SoleRemainingCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SteamInputFacts {
    pub pairing: PairingEvidence,
    pub radio: RadioReading,
    pub controller_firmware: FirmwareStatus,
    pub dongle_firmware: FirmwareStatus,
}

fn firmware_status(
    facts: &SteamControllerFacts,
    serial_number: &str,
    expected_hardware: &[SteamHardwareKind],
) -> FirmwareStatus {
    match facts.hardware_updates.pending_update_for(serial_number) {
        Some(hardware) if expected_hardware.contains(&hardware) => FirmwareStatus::UpdatePending,
        Some(_) | None => FirmwareStatus::Current,
    }
}

impl SteamInputFacts {
    #[must_use]
    pub fn gathered(
        facts: &SteamControllerFacts,
        pairing: PairingEvidence,
        controller_serial: &str,
        dongle_serial: &str,
    ) -> Self {
        SteamInputFacts {
            pairing,
            radio: facts
                .controller_log
                .sole_controller_radio_signal()
                .map_or(RadioReading::Unattributed, RadioReading::Measured),
            controller_firmware: firmware_status(
                facts,
                controller_serial,
                &[
                    SteamHardwareKind::ControllerOverRadio,
                    SteamHardwareKind::ControllerOverUsb,
                ],
            ),
            dongle_firmware: firmware_status(
                facts,
                dongle_serial,
                &[SteamHardwareKind::PuckDongle],
            ),
        }
    }
}
