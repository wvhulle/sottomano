use std::path::Path;

use iced::{
    ContentFit, Element, Length,
    widget::{image, svg},
};
use vetrata_sources::{favicons::FaviconKind, tray::IconSource};

#[derive(Debug, Clone)]
pub enum IconHandle {
    Raster(image::Handle),
    Svg(svg::Handle),
}

impl IconHandle {
    #[must_use]
    pub fn at(path: &Path) -> Self {
        let is_svg = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz"));
        if is_svg {
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
        }
    }
}

impl From<&IconSource> for IconHandle {
    fn from(source: &IconSource) -> Self {
        match source {
            IconSource::Pixmap(handle) => Self::Raster(handle.clone()),
            IconSource::Themed(path) => Self::at(path),
        }
    }
}
