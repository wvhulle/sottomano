use super::*;

#[test]
fn signed_shares_always_carry_a_sign() {
    assert_eq!(
        Quantity::signed_share(Ratio::new::<percent>(24.4)).localized_text(Locale::en_US),
        "+24 %"
    );
    assert_eq!(
        Quantity::signed_share(Ratio::new::<percent>(-5.2)).localized_text(Locale::en_US),
        "−5 %"
    );
}

#[test]
fn shares_round_to_whole_percent_with_a_space() {
    assert_eq!(
        Quantity::share(Ratio::new::<percent>(52.4)).localized_text(Locale::en_US),
        "52 %"
    );
}

#[test]
fn grouping_and_decimal_separators_follow_the_locale() {
    let clock = Quantity::measured(1234.5, DisplayUnit::Megahertz, 1);
    assert_eq!(clock.localized_text(Locale::en_US), "1,234.5 MHz");
    assert_eq!(clock.localized_text(Locale::nl_BE), "1.234,5 MHz");
}

#[test]
fn decimals_pad_to_the_chosen_precision() {
    assert_eq!(
        Quantity::power(Power::new::<watt>(8.0)).localized_text(Locale::en_US),
        "8.0 W"
    );
}

#[test]
fn information_sizes_step_through_iec_prefixes() {
    assert_eq!(
        Quantity::size(ByteSize::b(512)).localized_text(Locale::en_US),
        "512 B"
    );
    assert_eq!(
        Quantity::size(ByteSize::mib(3) + ByteSize::kib(512)).localized_text(Locale::en_US),
        "3.5 MiB"
    );
}

#[test]
fn transfer_rates_scale_like_sizes() {
    assert_eq!(
        Quantity::transfer_rate(InformationRate::new::<byte_per_second>(2048.0))
            .localized_text(Locale::en_US),
        "2.0 KiB/s"
    );
}

#[test]
fn distances_switch_to_kilometers_from_one_kilometer() {
    assert_eq!(
        Quantity::distance(Length::new::<meter>(350.0)).localized_text(Locale::en_US),
        "350 m"
    );
    assert_eq!(
        Quantity::distance(Length::new::<meter>(2500.0)).localized_text(Locale::en_US),
        "2.5 km"
    );
}
