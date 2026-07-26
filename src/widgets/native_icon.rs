//! A native application icon, sized and faded: a decoded pixmap or a themed icon
//! file. Themed icons ship scalable SVGs while hicolor app icons are raster, and
//! iced decodes them through different widgets, so the extension picks the path —
//! done here once so every consumer (tray bar icons, tray menu entries, and in
//! time the window list / quick-launch) renders both the same way.

use std::path::PathBuf;

use iced::{
    Element,
    widget::{image, svg},
};

/// Where a native icon's pixels come from.
#[derive(Debug, Clone)]
pub enum IconSource {
    /// An already-decoded pixmap (e.g. an SNI `IconPixmap`, ARGB→RGBA).
    Pixmap(image::Handle),
    /// A resolved icon-theme file on disk (svg or raster).
    Themed(PathBuf),
}

/// The icon at `side` px square, drawn at `opacity`.
pub fn native_icon<'a, Message: 'a>(
    source: &IconSource,
    side: f32,
    opacity: f32,
) -> Element<'a, Message> {
    match source {
        IconSource::Pixmap(handle) => image(handle.clone())
            .width(side)
            .height(side)
            .opacity(opacity)
            .into(),
        IconSource::Themed(path) => {
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
            {
                svg(svg::Handle::from_path(path))
                    .width(side)
                    .height(side)
                    .opacity(opacity)
                    .into()
            } else {
                image(image::Handle::from_path(path))
                    .width(side)
                    .height(side)
                    .opacity(opacity)
                    .into()
            }
        }
    }
}
