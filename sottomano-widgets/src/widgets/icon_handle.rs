use std::{path::Path, sync::Arc};

use ::image::RgbaImage;
use iced::{
    Color, ContentFit, Element, Length, Theme,
    widget::{image, svg},
};
use sottomano_desktop_sources::desktop_windows_apps::{
    firefox_favicon_lookup::FaviconKind, status_notifier_tray::IconSource,
};

use crate::widgets::configured_theme::text_color;

#[derive(Debug, Clone, PartialEq)]
pub enum IconHandle {
    Raster(image::Handle),
    Svg(svg::Handle),
    SymbolicSvg(svg::Handle),
}

impl IconHandle {
    #[must_use]
    pub fn at(path: &Path) -> Self {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz"))
        {
            Self::Svg(svg::Handle::from_path(path))
        } else {
            Self::Raster(image::Handle::from_path(path))
        }
    }

    #[must_use]
    pub fn from_bytes(kind: FaviconKind, bytes: Vec<u8>) -> Self {
        match kind {
            FaviconKind::Raster => Self::Raster(image::Handle::from_bytes(bytes)),
            FaviconKind::Svg => Self::Svg(svg::Handle::from_memory(bytes)),
        }
    }

    pub fn view<'a, Message: 'a>(&self, side: f32, opacity: f32) -> Element<'a, Message> {
        self.view_with_symbolic_tint(side, opacity, text_color)
    }

    pub fn view_with_symbolic_tint<'a, Message: 'a>(
        &self,
        side: f32,
        opacity: f32,
        symbolic_tint: impl Fn(&Theme) -> Color + 'a,
    ) -> Element<'a, Message> {
        match self {
            Self::Raster(handle) => image(handle.clone())
                .width(side)
                .height(side)
                .opacity(opacity)
                .into(),
            Self::Svg(handle) => svg(handle.clone())
                .width(side)
                .height(side)
                .opacity(opacity)
                .into(),
            Self::SymbolicSvg(handle) => svg(handle.clone())
                .width(side)
                .height(side)
                .opacity(opacity)
                .style(move |theme, _| svg::Style {
                    color: Some(symbolic_tint(theme)),
                })
                .into(),
        }
    }

    pub fn scaled_down<'a, Message: 'a>(&self, opacity: f32) -> Element<'a, Message> {
        match self {
            Self::Raster(handle) => image(handle.clone())
                .width(Length::Fill)
                .content_fit(ContentFit::ScaleDown)
                .opacity(opacity)
                .into(),
            Self::Svg(handle) => svg(handle.clone())
                .width(Length::Fill)
                .content_fit(ContentFit::ScaleDown)
                .opacity(opacity)
                .into(),
            Self::SymbolicSvg(handle) => svg(handle.clone())
                .width(Length::Fill)
                .content_fit(ContentFit::ScaleDown)
                .opacity(opacity)
                .style(|theme, _| svg::Style {
                    color: Some(text_color(theme)),
                })
                .into(),
        }
    }
}

impl From<&RgbaImage> for IconHandle {
    fn from(pixels: &RgbaImage) -> Self {
        Self::Raster(image::Handle::from_rgba(
            pixels.width(),
            pixels.height(),
            pixels.as_raw().clone(),
        ))
    }
}

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
    pub fn refreshed<'a>(&self, pixmaps: impl IntoIterator<Item = &'a Arc<RgbaImage>>) -> Self {
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

    #[must_use]
    pub fn icon_handle(&self, source: &IconSource) -> IconHandle {
        match source {
            IconSource::Pixmap(pixels) => self.pixmap_handle(pixels),
            IconSource::Themed(path) => IconHandle::at(path),
            IconSource::ThemedSymbolic(path) => {
                IconHandle::SymbolicSvg(svg::Handle::from_path(path))
            }
        }
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
