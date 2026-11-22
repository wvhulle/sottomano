use super::*;

fn plugged_in(status: ChargeStatus, capacity_percent: u8, limit: f64) -> Battery {
    Battery {
        status,
        capacity: Ratio::new::<percent>(f64::from(capacity_percent)),
        energy: Energy::new::<microwatt_hour>(0.0),
        energy_full: Energy::new::<microwatt_hour>(50e6),
        power: Power::new::<watt>(0.0),
        charge_limit: Ratio::new::<ratio>(limit),
        charge_behaviour: ChargeBehaviour::Auto,
        serial: String::new(),
        identity: DeviceIdentity::default(),
        technology: HardwareText::Unreported,
    }
}

#[test]
fn reads_the_bracketed_charge_behaviour() {
    assert_eq!(
        ChargeBehaviour::selected_in("[auto] inhibit-charge force-discharge"),
        Some(ChargeBehaviour::Auto)
    );
    assert_eq!(
        ChargeBehaviour::selected_in("auto [inhibit-charge] force-discharge"),
        Some(ChargeBehaviour::InhibitCharge)
    );
    assert_eq!(ChargeBehaviour::selected_in("auto inhibit-charge"), None);
}

#[test]
fn supply_status_strings_map_to_charge_statuses() {
    assert_eq!(
        ChargeStatus::from("Not charging"),
        ChargeStatus::NotCharging
    );
    assert_eq!(ChargeStatus::from("Charging"), ChargeStatus::Charging);
    assert_eq!(ChargeStatus::from("Unknown"), ChargeStatus::Unreported);
}

#[test]
fn reported_full_is_full() {
    let battery = plugged_in(ChargeStatus::Full, 100, 1.0);
    assert_eq!(battery.idle_reason(), Some(IdleReason::Full));
}

#[test]
fn not_charging_at_the_limit_is_held_there() {
    let battery = plugged_in(ChargeStatus::NotCharging, 80, 0.8);
    assert_eq!(battery.idle_reason(), Some(IdleReason::AtLimit));
}

#[test]
fn inhibited_charging_is_reported_as_such() {
    let battery = Battery {
        charge_behaviour: ChargeBehaviour::InhibitCharge,
        ..plugged_in(ChargeStatus::NotCharging, 60, 1.0)
    };
    assert_eq!(battery.idle_reason(), Some(IdleReason::Inhibited));
}

#[test]
fn not_charging_below_the_limit_without_a_known_cause_is_unexplained() {
    let battery = plugged_in(ChargeStatus::NotCharging, 99, 1.0);
    assert_eq!(battery.idle_reason(), Some(IdleReason::Unexplained));
}

#[test]
fn charging_or_draining_is_not_idle() {
    assert_eq!(
        plugged_in(ChargeStatus::Charging, 50, 1.0).idle_reason(),
        None
    );
    assert_eq!(
        plugged_in(ChargeStatus::Discharging, 50, 1.0).idle_reason(),
        None
    );
}
