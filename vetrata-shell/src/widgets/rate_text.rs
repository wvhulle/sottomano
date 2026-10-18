use bytesize::ByteSize;
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::byte,
    time::second,
};

#[must_use]
pub fn per_second_text(rate: InformationRate) -> String {
    let in_one_second: Information = (rate * Time::new::<second>(1.0)).into();
    let bytes = ByteSize::b(in_one_second.get::<byte>().round() as u64);
    format!("{}/s", bytes.display().iec())
}

#[cfg(test)]
mod tests {
    use uom::si::information_rate::{byte_per_second, kibibyte_per_second, mebibyte_per_second};

    use super::*;

    #[test]
    fn small_rates_keep_their_precision() {
        assert_eq!(
            per_second_text(InformationRate::new::<byte_per_second>(312.0)),
            "312 B/s"
        );
        assert_eq!(
            per_second_text(InformationRate::new::<kibibyte_per_second>(845.2)),
            "845.2 KiB/s"
        );
    }

    #[test]
    fn large_rates_switch_prefix() {
        assert_eq!(
            per_second_text(InformationRate::new::<mebibyte_per_second>(1.8)),
            "1.8 MiB/s"
        );
    }
}
