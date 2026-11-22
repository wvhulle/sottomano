use std::time::Duration;

const NOISE_GLYPHS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789#%&@$*+=<>/\\|";

#[expect(
    clippy::disallowed_methods,
    reason = "hash mixing relies on wrapping arithmetic"
)]
pub(super) fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01B3)
    })
}

#[expect(
    clippy::disallowed_methods,
    reason = "hash mixing relies on wrapping arithmetic"
)]
fn mix(value: u64) -> u64 {
    let value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn unit(value: u64) -> f32 {
    (mix(value) >> 40) as f32 / (1u64 << 24) as f32
}

#[expect(
    clippy::disallowed_methods,
    reason = "hash mixing relies on wrapping arithmetic"
)]
pub(super) fn lock_time(settle: Duration, seed: u64, position: usize, length: usize) -> Duration {
    let order = position as f32 / length as f32;
    let chance = unit(seed.wrapping_add(position as u64));
    settle.mul_f32(0.15 + 0.35 * order + 0.5 * chance)
}

pub(super) fn noise_glyph(seed: u64, position: usize) -> char {
    let pick = mix(seed ^ position as u64);
    char::from(NOISE_GLYPHS[(pick % NOISE_GLYPHS.len() as u64) as usize])
}
