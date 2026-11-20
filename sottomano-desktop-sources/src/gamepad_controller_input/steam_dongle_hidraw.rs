use std::{
    io,
    os::fd::OwnedFd,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{Stream, stream};
use rustix::{
    fs::{Mode, OFlags, open},
    io::{Errno, read},
};
use tokio::{
    io::{Interest, unix::AsyncFd},
    time::{Instant, sleep, timeout_at},
};

use super::evdev_gamepad_devices::GamepadPower;
use crate::system_protocol_clients::sysfs_attribute_reads;

pub const VALVE_USB_VENDOR: u16 = 0x28de;
pub const STEAM_VIRTUAL_PAD_PRODUCT: u16 = 0x11ff;
pub const STEAM_PUCK_PRODUCTS: [u16; 2] = [0x1304, 0x1305];

const HIDRAW_CLASS_DIRECTORY: &str = "/sys/class/hidraw";
const DEVICE_NODE_DIRECTORY: &str = "/dev";
const STEAM_CLIENT_HID_GROUP: &str = "g0103";
const PHYS_INTERFACE_MARKER: &str = "/input";
const BATTERY_REPORT_ID: u8 = 0x43;
const BATTERY_REPORT_LENGTH: usize = 15;
const INPUT_REPORT_IDS: [u8; 2] = [0x42, 0x45];
const LONGEST_REPORT: usize = 64;
const SAMPLE_WINDOW: Duration = Duration::from_secs(6);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DongleInterface {
    pub interface_number: u8,
    pub dongle_serial: String,
    pub kernel_device: String,
    pub hidraw_node: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DongleSample {
    Battery(GamepadPower),
    Streaming,
    Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DongleReport {
    Battery(GamepadPower),
    Input,
    Unrelated,
}

impl DongleReport {
    fn parse(report: &[u8]) -> Self {
        match report {
            [BATTERY_REPORT_ID, charge_state, level_percent, ..]
                if report.len() == BATTERY_REPORT_LENGTH =>
            {
                let level_percent = (*level_percent).min(100);
                DongleReport::Battery(match charge_state {
                    2 => GamepadPower::Charging(level_percent),
                    4 => GamepadPower::Full,
                    _ => GamepadPower::Discharging(level_percent),
                })
            }
            [report_id, ..] if INPUT_REPORT_IDS.contains(report_id) => DongleReport::Input,
            _ => DongleReport::Unrelated,
        }
    }
}

fn hid_device_name(hid_device: &Path) -> Option<String> {
    Some(hid_device.file_name()?.to_str()?.to_owned())
}

fn is_puck_hid_id(hid_id: &str) -> bool {
    STEAM_PUCK_PRODUCTS.iter().any(|product| {
        hid_id.eq_ignore_ascii_case(&format!("0003:0000{VALVE_USB_VENDOR:04X}:0000{product:04X}"))
    })
}

fn valve_interface_beside(client_device: &Path) -> Option<PathBuf> {
    sysfs_attribute_reads::list_dir(client_device.parent()?)
        .into_iter()
        .find(|sibling| {
            sibling != client_device
                && sysfs_attribute_reads::uevent_field(&sibling.join("uevent"), "HID_ID")
                    .is_some_and(|hid_id| is_puck_hid_id(&hid_id))
        })
}

fn dongle_interface(hidraw_class_entry: &Path) -> Option<DongleInterface> {
    let client_device = hidraw_class_entry
        .join("device")
        .canonicalize()
        .inspect_err(|resolve_failure| {
            tracing::debug!(
                "gamepad: device of {} unresolvable: {resolve_failure}",
                hidraw_class_entry.display()
            );
        })
        .ok()?;
    let uevent = client_device.join("uevent");
    let hid_id = sysfs_attribute_reads::uevent_field(&uevent, "HID_ID")?;
    let modalias = sysfs_attribute_reads::uevent_field(&uevent, "MODALIAS")?;
    if !is_puck_hid_id(&hid_id) || !modalias.contains(STEAM_CLIENT_HID_GROUP) {
        return None;
    }
    let interface_number = sysfs_attribute_reads::uevent_field(&uevent, "HID_PHYS")?
        .rsplit_once(PHYS_INTERFACE_MARKER)?
        .1
        .parse()
        .inspect_err(|parse_failure| {
            tracing::debug!("gamepad: Steam dongle interface number unreadable: {parse_failure}");
        })
        .ok()?;
    let valve_interface = valve_interface_beside(&client_device)?;
    Some(DongleInterface {
        interface_number,
        dongle_serial: sysfs_attribute_reads::uevent_field(
            &valve_interface.join("uevent"),
            "HID_UNIQ",
        )?,
        kernel_device: format!("+hid:{}", hid_device_name(&valve_interface)?),
        hidraw_node: Path::new(DEVICE_NODE_DIRECTORY).join(hidraw_class_entry.file_name()?),
    })
}

#[must_use]
pub fn dongle_interfaces() -> Vec<DongleInterface> {
    let mut interfaces: Vec<DongleInterface> =
        sysfs_attribute_reads::list_dir(HIDRAW_CLASS_DIRECTORY)
            .iter()
            .filter_map(|hidraw_class_entry| dongle_interface(hidraw_class_entry))
            .collect();
    interfaces.sort();
    interfaces
}

fn open_read_only(hidraw_node: &Path) -> io::Result<AsyncFd<OwnedFd>> {
    let descriptor = open(
        hidraw_node,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    unsafe { AsyncFd::register_with_interest(descriptor, Interest::READABLE) }
        .map_err(io::Error::from)
}

async fn next_report(reader: &AsyncFd<OwnedFd>) -> io::Result<DongleReport> {
    let mut buffer = [0_u8; LONGEST_REPORT];
    loop {
        let mut readiness = reader.readable().await?;
        match read(reader.get_ref(), &mut buffer) {
            Ok(length) => return Ok(DongleReport::parse(&buffer[..length])),
            Err(Errno::AGAIN) => readiness.clear_ready(),
            Err(read_failure) => return Err(read_failure.into()),
        }
    }
}

async fn sample_once(interface: &DongleInterface) -> io::Result<DongleSample> {
    let reader = open_read_only(&interface.hidraw_node)?;
    let window_end = Instant::now() + SAMPLE_WINDOW;
    let mut sample = DongleSample::Silent;
    while let Ok(report) = timeout_at(window_end, next_report(&reader)).await {
        match report? {
            DongleReport::Battery(power) => return Ok(DongleSample::Battery(power)),
            DongleReport::Input => sample = DongleSample::Streaming,
            DongleReport::Unrelated => {}
        }
    }
    Ok(sample)
}

pub fn dongle_samples(
    interface: DongleInterface,
) -> impl Stream<Item = (DongleInterface, DongleSample)> + Send {
    stream::unfold((interface, true), |(interface, first_sample)| async move {
        if !first_sample {
            sleep(SAMPLE_INTERVAL).await;
        }
        let sample = sample_once(&interface)
            .await
            .unwrap_or_else(|read_failure| {
                tracing::debug!(
                    "gamepad: Steam dongle {} unreadable: {read_failure}",
                    interface.hidraw_node.display()
                );
                DongleSample::Silent
            });
        Some(((interface.clone(), sample), (interface, false)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery_report(charge_state: u8, level_percent: u8) -> [u8; BATTERY_REPORT_LENGTH] {
        let mut report = [0_u8; BATTERY_REPORT_LENGTH];
        report[0] = BATTERY_REPORT_ID;
        report[1] = charge_state;
        report[2] = level_percent;
        report
    }

    #[test]
    fn a_discharging_battery_report_carries_its_level() {
        let captured_report = [
            0x43, 0x01, 0x3f, 0x33, 0x0f, 0x3c, 0x0f, 0, 0, 0, 0, 0, 0, 0x9b, 0x54,
        ];
        assert_eq!(
            DongleReport::parse(&captured_report),
            DongleReport::Battery(GamepadPower::Discharging(63))
        );
    }

    #[test]
    fn charge_states_map_to_charging_and_full() {
        assert_eq!(
            DongleReport::parse(&battery_report(2, 76)),
            DongleReport::Battery(GamepadPower::Charging(76))
        );
        assert_eq!(
            DongleReport::parse(&battery_report(4, 100)),
            DongleReport::Battery(GamepadPower::Full)
        );
    }

    #[test]
    fn truncated_battery_reports_are_ignored() {
        assert_eq!(
            DongleReport::parse(&battery_report(1, 50)[..9]),
            DongleReport::Unrelated
        );
    }

    #[test]
    fn input_reports_mark_a_streaming_controller() {
        assert_eq!(DongleReport::parse(&[0x42; 54]), DongleReport::Input);
        assert_eq!(DongleReport::parse(&[0x7b; 13]), DongleReport::Unrelated);
    }

    #[test]
    fn only_puck_hid_ids_count_as_dongles() {
        assert!(is_puck_hid_id("0003:000028DE:00001304"));
        assert!(!is_puck_hid_id("0003:000028DE:00001102"));
        assert!(!is_puck_hid_id("0003:0000054C:00000CE6"));
    }
}
