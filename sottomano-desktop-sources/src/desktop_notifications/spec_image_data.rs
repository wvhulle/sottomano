use image::RgbaImage;
use zbus::zvariant::Value;

pub fn image_data_pixels(value: &Value) -> Option<RgbaImage> {
    let (width, height, rowstride, has_alpha, _bits, channels, data): (
        i32,
        i32,
        i32,
        bool,
        i32,
        i32,
        Vec<u8>,
    ) = value
        .try_into()
        .inspect_err(|type_mismatch| {
            tracing::warn!(
                "notifications: image data hint has an unexpected type: {type_mismatch}"
            );
        })
        .ok()?;
    let (Ok(width), Ok(height), Ok(rowstride), Ok(channels)) = (
        usize::try_from(width),
        usize::try_from(height),
        usize::try_from(rowstride),
        usize::try_from(channels),
    ) else {
        tracing::warn!(
            "notifications: image data hint has negative layout {width}x{height}, rowstride \
             {rowstride}, {channels} channels"
        );
        return None;
    };
    if width == 0 || height == 0 || channels < 3 {
        tracing::warn!(
            "notifications: image data hint has an empty layout {width}x{height} with {channels} \
             channels"
        );
        return None;
    }
    let required_bytes = (height - 1) * rowstride + width * channels;
    if data.len() < required_bytes {
        tracing::warn!(
            "notifications: image data hint holds {} bytes, its {width}x{height} layout needs \
             {required_bytes}",
            data.len()
        );
        return None;
    }
    let alpha_channel = if has_alpha && channels >= 4 {
        AlphaChannel::Present
    } else {
        AlphaChannel::Absent
    };
    let rgba = (0..height)
        .flat_map(|row_index| {
            (0..width).map(move |column_index| row_index * rowstride + column_index * channels)
        })
        .flat_map(|pixel_start| {
            let alpha = match alpha_channel {
                AlphaChannel::Present => data[pixel_start + 3],
                AlphaChannel::Absent => u8::MAX,
            };
            [
                data[pixel_start],
                data[pixel_start + 1],
                data[pixel_start + 2],
                alpha,
            ]
        })
        .collect();
    Some(
        RgbaImage::from_raw(
            u32::try_from(width).expect("image width came from a non-negative i32"),
            u32::try_from(height).expect("image height came from a non-negative i32"),
            rgba,
        )
        .expect("the rgba buffer holds four bytes for every pixel of the layout"),
    )
}

#[derive(Clone, Copy)]
enum AlphaChannel {
    Present,
    Absent,
}
