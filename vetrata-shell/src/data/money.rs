use rust_decimal::Decimal;
use rusty_money::iso;
use serde::{Deserialize, Deserializer};

pub type Money = rusty_money::Money<'static, iso::Currency>;

#[must_use]
pub fn usd(amount: Decimal) -> Money {
    Money::from_decimal(amount, iso::USD)
}

pub fn deserialize_usd<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Money, D::Error> {
    <Decimal as Deserialize>::deserialize(deserializer).map(usd)
}

#[must_use]
pub fn from_minor_units(
    minor_units: Decimal,
    currency: &'static iso::Currency,
    exponent: u32,
) -> Money {
    Money::from_decimal(minor_units * Decimal::new(1, exponent), currency)
}
