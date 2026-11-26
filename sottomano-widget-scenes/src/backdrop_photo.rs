use iced::{Size, widget::image::Handle};
use image::{ImageFormat, RgbaImage, imageops::fast_blur};

const POLLONIA_PHOTO_JPEG: &[u8] = include_bytes!("../assets/pollonia-milos-unsplash.jpg");
const COMPOSITOR_BLUR_SIGMA: f32 = 18.0;

#[derive(Debug, Clone)]
pub struct BackdropPhotos {
    pub sharp_photo: Handle,
    pub frosted_photo: Handle,
    pub photo_size: Size,
}

impl BackdropPhotos {
    #[must_use]
    pub fn decoded() -> Self {
        let sharp_pixels =
            image::load_from_memory_with_format(POLLONIA_PHOTO_JPEG, ImageFormat::Jpeg)
                .expect("the bundled backdrop photo is a valid jpeg")
                .into_rgba8();
        let frosted_pixels = fast_blur(&sharp_pixels, COMPOSITOR_BLUR_SIGMA);
        #[allow(clippy::cast_precision_loss)]
        let photo_size = Size::new(sharp_pixels.width() as f32, sharp_pixels.height() as f32);
        BackdropPhotos {
            photo_size,
            sharp_photo: handle_of_pixels(sharp_pixels),
            frosted_photo: handle_of_pixels(frosted_pixels),
        }
    }
}

fn handle_of_pixels(pixels: RgbaImage) -> Handle {
    Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
}
