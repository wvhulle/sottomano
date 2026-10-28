use std::{cell::RefCell, collections::HashMap, time::Duration};

use bytesize::ByteSize;
use chrono::Locale;
use fixed_decimal::{Decimal, FloatPrecision};
use icu_decimal::{DecimalFormatter, options::DecimalFormatterOptions};
use sottomano_sources::ai_usage_spend::spend_money::Money;
use uom::si::{
    angle::degree,
    electric_current::ampere,
    electric_potential::volt,
    energy::watt_hour,
    f64::{
        Angle, ElectricCurrent, ElectricPotential, Energy, Frequency, InformationRate, Length,
        Power, Ratio, ThermodynamicTemperature, Velocity,
    },
    frequency::{gigahertz, hertz, megahertz},
    information_rate::{byte_per_second, megabit_per_second},
    length::meter,
    power::watt,
    ratio::percent,
    thermodynamic_temperature::{degree_celsius, kelvin},
    velocity::kilometer_per_hour,
};

use crate::widgets::{
    amount_text::{spend_text, token_count},
    localized_durations::icu_locale,
};

const IEC_STEP: f64 = 1024.0;
const IEC_PREFIXES: [&str; 5] = ["", "Ki", "Mi", "Gi", "Ti"];
const METERS_PER_KILOMETER: f64 = 1000.0;
const MILLIMETERS_PER_KILOMETER: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantity {
    Measured(Measurement),
    Count(i64),
    CompactCount(u64),
    InformationSize(ByteSize),
    Spend(Money),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Measurement {
    magnitude_bits: u64,
    unit: DisplayUnit,
    decimals: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayUnit {
    Percent,
    SignedPercent,
    DegreeCelsius,
    Kelvin,
    Watt,
    Volt,
    Ampere,
    WattHour,
    Gigahertz,
    Megahertz,
    Hertz,
    BytesPerSecond,
    MegabitPerSecond,
    KilometerPerHour,
    MillimeterPerHour,
    MeterOrKilometer,
    Kilometer,
    Degree,
    TokensPerSecond,
    MegatransfersPerSecond,
    MillionsPerSecond,
    PerHour,
    PerSecond,
    Millisecond,
    GrainsPerCubicMeter,
    Unitless,
}

impl DisplayUnit {
    fn symbol(self) -> &'static str {
        match self {
            DisplayUnit::Percent | DisplayUnit::SignedPercent => "%",
            DisplayUnit::DegreeCelsius => "°C",
            DisplayUnit::Kelvin => "K",
            DisplayUnit::Watt => "W",
            DisplayUnit::Volt => "V",
            DisplayUnit::Ampere => "A",
            DisplayUnit::WattHour => "Wh",
            DisplayUnit::Gigahertz => "GHz",
            DisplayUnit::Megahertz => "MHz",
            DisplayUnit::Hertz => "Hz",
            DisplayUnit::BytesPerSecond => "B/s",
            DisplayUnit::MegabitPerSecond => "Mb/s",
            DisplayUnit::KilometerPerHour => "km/h",
            DisplayUnit::MillimeterPerHour => "mm/h",
            DisplayUnit::MeterOrKilometer => "m",
            DisplayUnit::Kilometer => "km",
            DisplayUnit::Degree => "°",
            DisplayUnit::TokensPerSecond => "tok/s",
            DisplayUnit::MegatransfersPerSecond => "MT/s",
            DisplayUnit::MillionsPerSecond => "M/s",
            DisplayUnit::PerHour => "/h",
            DisplayUnit::PerSecond => "/s",
            DisplayUnit::Millisecond => "ms",
            DisplayUnit::GrainsPerCubicMeter => "grains/m³",
            DisplayUnit::Unitless => "",
        }
    }
}

impl Measurement {
    fn magnitude(self) -> f64 {
        f64::from_bits(self.magnitude_bits)
    }

    fn localized_text(self, locale: Locale) -> String {
        match self.unit {
            DisplayUnit::BytesPerSecond => {
                let (scaled, prefix) = iec_scaled(self.magnitude());
                let decimals = if prefix.is_empty() { 0 } else { self.decimals };
                format!(
                    "{} {prefix}B/s",
                    localized_decimal(scaled, decimals, locale)
                )
            }
            DisplayUnit::MeterOrKilometer if self.magnitude() >= METERS_PER_KILOMETER => format!(
                "{} {}",
                localized_decimal(self.magnitude() / METERS_PER_KILOMETER, 1, locale),
                DisplayUnit::Kilometer.symbol()
            ),
            DisplayUnit::Unitless => localized_decimal(self.magnitude(), self.decimals, locale),
            DisplayUnit::SignedPercent => format!(
                "{}{} {}",
                if self.magnitude() < 0.0 { '−' } else { '+' },
                localized_decimal(self.magnitude().abs(), self.decimals, locale),
                self.unit.symbol()
            ),
            DisplayUnit::Degree | DisplayUnit::PerHour | DisplayUnit::PerSecond => format!(
                "{}{}",
                localized_decimal(self.magnitude(), self.decimals, locale),
                self.unit.symbol()
            ),
            unit => format!(
                "{} {}",
                localized_decimal(self.magnitude(), self.decimals, locale),
                unit.symbol()
            ),
        }
    }
}

impl Quantity {
    #[must_use]
    pub fn measured(magnitude: f64, unit: DisplayUnit, decimals: u8) -> Self {
        Quantity::Measured(Measurement {
            magnitude_bits: magnitude.to_bits(),
            unit,
            decimals,
        })
    }

    #[must_use]
    pub fn decimals(self, decimals: u8) -> Self {
        match self {
            Quantity::Measured(measurement) => Quantity::Measured(Measurement {
                decimals,
                ..measurement
            }),
            other => other,
        }
    }

    #[must_use]
    pub fn share(share: Ratio) -> Self {
        Self::measured(share.get::<percent>(), DisplayUnit::Percent, 0)
    }

    #[must_use]
    pub fn signed_share(change: Ratio) -> Self {
        Self::measured(change.get::<percent>(), DisplayUnit::SignedPercent, 0)
    }

    #[must_use]
    pub fn whole_percent(percentage: impl Into<f64>) -> Self {
        Self::measured(percentage.into(), DisplayUnit::Percent, 0)
    }

    #[must_use]
    pub fn temperature(temperature: ThermodynamicTemperature) -> Self {
        Self::measured(
            temperature.get::<degree_celsius>(),
            DisplayUnit::DegreeCelsius,
            0,
        )
    }

    #[must_use]
    pub fn color_temperature(temperature: ThermodynamicTemperature) -> Self {
        Self::measured(temperature.get::<kelvin>(), DisplayUnit::Kelvin, 0)
    }

    #[must_use]
    pub fn power(power: Power) -> Self {
        Self::measured(power.get::<watt>(), DisplayUnit::Watt, 1)
    }

    #[must_use]
    pub fn voltage(potential: ElectricPotential) -> Self {
        Self::measured(potential.get::<volt>(), DisplayUnit::Volt, 0)
    }

    #[must_use]
    pub fn current(current: ElectricCurrent) -> Self {
        Self::measured(current.get::<ampere>(), DisplayUnit::Ampere, 2)
    }

    #[must_use]
    pub fn energy(energy: Energy) -> Self {
        Self::measured(energy.get::<watt_hour>(), DisplayUnit::WattHour, 1)
    }

    #[must_use]
    pub fn clock_gigahertz(frequency: Frequency) -> Self {
        Self::measured(frequency.get::<gigahertz>(), DisplayUnit::Gigahertz, 2)
    }

    #[must_use]
    pub fn clock_megahertz(frequency: Frequency) -> Self {
        Self::measured(frequency.get::<megahertz>(), DisplayUnit::Megahertz, 0)
    }

    #[must_use]
    pub fn refresh_hertz(frequency: Frequency) -> Self {
        Self::measured(frequency.get::<hertz>(), DisplayUnit::Hertz, 2)
    }

    #[must_use]
    pub fn transfer_rate(rate: InformationRate) -> Self {
        Self::measured(
            rate.get::<byte_per_second>(),
            DisplayUnit::BytesPerSecond,
            1,
        )
    }

    #[must_use]
    pub fn link_rate(rate: InformationRate) -> Self {
        Self::measured(
            rate.get::<megabit_per_second>(),
            DisplayUnit::MegabitPerSecond,
            0,
        )
    }

    #[must_use]
    pub fn round_trip(round_trip: Duration) -> Self {
        Self::measured(
            round_trip.as_secs_f64() * 1_000.0,
            DisplayUnit::Millisecond,
            0,
        )
    }

    #[must_use]
    pub fn per_second(events_per_second: f64) -> Self {
        Self::measured(events_per_second, DisplayUnit::PerSecond, 1)
    }

    #[must_use]
    pub fn wind_speed(speed: Velocity) -> Self {
        Self::measured(
            speed.get::<kilometer_per_hour>(),
            DisplayUnit::KilometerPerHour,
            0,
        )
    }

    #[must_use]
    pub fn precipitation_rate(rate: Velocity) -> Self {
        Self::measured(
            rate.get::<kilometer_per_hour>() * MILLIMETERS_PER_KILOMETER,
            DisplayUnit::MillimeterPerHour,
            1,
        )
    }

    #[must_use]
    pub fn distance(length: Length) -> Self {
        Self::measured(length.get::<meter>(), DisplayUnit::MeterOrKilometer, 0)
    }

    #[must_use]
    pub fn angle(angle: Angle) -> Self {
        Self::measured(angle.get::<degree>(), DisplayUnit::Degree, 0)
    }

    #[must_use]
    pub fn count(count: impl TryInto<i64>) -> Self {
        Quantity::Count(count.try_into().unwrap_or(i64::MAX))
    }

    #[must_use]
    pub fn size(size: ByteSize) -> Self {
        Quantity::InformationSize(size)
    }

    #[must_use]
    pub fn compact_count(count: u64) -> Self {
        Quantity::CompactCount(count)
    }

    #[must_use]
    pub fn spend(money: Money) -> Self {
        Quantity::Spend(money)
    }

    #[must_use]
    pub fn localized_text(self, locale: Locale) -> String {
        match self {
            Quantity::Measured(measurement) => measurement.localized_text(locale),
            #[allow(clippy::cast_precision_loss)]
            Quantity::Count(count) => localized_decimal(count as f64, 0, locale),
            Quantity::CompactCount(count) => token_count(count),
            Quantity::InformationSize(size) => {
                #[allow(clippy::cast_precision_loss)]
                let (scaled, prefix) = iec_scaled(size.as_u64() as f64);
                let decimals = u8::from(!prefix.is_empty());
                format!("{} {prefix}B", localized_decimal(scaled, decimals, locale))
            }
            Quantity::Spend(money) => spend_text(&money),
        }
    }
}

fn iec_scaled(magnitude: f64) -> (f64, &'static str) {
    IEC_PREFIXES
        .iter()
        .scan(magnitude, |remaining, prefix| {
            let current = *remaining;
            *remaining /= IEC_STEP;
            Some((current, *prefix))
        })
        .take_while(|(scaled, prefix)| prefix.is_empty() || *scaled >= 1.0)
        .last()
        .unwrap_or((magnitude, ""))
}

thread_local! {
    static DECIMAL_FORMATTERS: RefCell<HashMap<Locale, DecimalFormatter>> =
        RefCell::new(HashMap::new());
}

fn new_decimal_formatter(locale: Locale) -> DecimalFormatter {
    DecimalFormatter::try_new(
        (&icu_locale(locale)).into(),
        DecimalFormatterOptions::default(),
    )
    .expect("compiled ICU data covers every locale through fallback")
}

#[must_use]
pub fn localized_decimal(magnitude: f64, decimals: u8, locale: Locale) -> String {
    let position = -i16::from(decimals);
    let Ok(mut decimal) = Decimal::try_from_f64(magnitude, FloatPrecision::RoundTrip) else {
        return magnitude.to_string();
    };
    decimal.round(position);
    decimal.absolute.pad_end(position);
    DECIMAL_FORMATTERS.with_borrow_mut(|formatters| {
        formatters
            .entry(locale)
            .or_insert_with(|| new_decimal_formatter(locale))
            .format_to_string(&decimal)
    })
}

#[cfg(test)]
mod tests {
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
}
