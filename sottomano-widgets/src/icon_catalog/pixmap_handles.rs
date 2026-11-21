use std::sync::Arc;

use ::image::RgbaImage;

use crate::icon_catalog::icon_handle::IconHandle;

#[derive(Debug, Clone, PartialEq)]
struct ConvertedPixmap {
    pixels: Arc<RgbaImage>,
    handle: IconHandle,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PixmapHandles {
    converted: Vec<ConvertedPixmap>,
}

impl PixmapHandles {
    #[must_use]
    pub fn refreshed<'pixmaps>(
        &self,
        pixmaps: impl IntoIterator<Item = &'pixmaps Arc<RgbaImage>>,
    ) -> Self {
        Self {
            converted: pixmaps
                .into_iter()
                .map(|pixels| ConvertedPixmap {
                    pixels: pixels.clone(),
                    handle: self.pixmap_handle(pixels),
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn pixmap_handle(&self, pixels: &Arc<RgbaImage>) -> IconHandle {
        self.converted
            .iter()
            .find(|converted| Arc::ptr_eq(&converted.pixels, pixels))
            .or_else(|| {
                self.converted
                    .iter()
                    .find(|converted| converted.pixels.as_ref() == pixels.as_ref())
            })
            .map_or_else(
                || IconHandle::from(pixels.as_ref()),
                |converted| converted.handle.clone(),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ::image::{Rgba, RgbaImage};

    use super::PixmapHandles;

    fn filled_icon(channel_value: u8) -> Arc<RgbaImage> {
        Arc::new(RgbaImage::from_pixel(16, 16, Rgba([channel_value; 4])))
    }

    #[test]
    fn a_resent_icon_with_equal_pixels_keeps_its_handle() {
        let first_delivery = filled_icon(200);
        let handles = PixmapHandles::default().refreshed([&first_delivery]);
        let resent_delivery = filled_icon(200);
        assert_eq!(
            handles.pixmap_handle(&resent_delivery),
            handles.pixmap_handle(&first_delivery)
        );
    }

    #[test]
    fn an_icon_with_new_pixels_gets_a_new_handle() {
        let first_delivery = filled_icon(200);
        let handles = PixmapHandles::default().refreshed([&first_delivery]);
        assert_ne!(
            handles.pixmap_handle(&filled_icon(10)),
            handles.pixmap_handle(&first_delivery)
        );
    }
}
