use std::path::PathBuf;

use iced::{
    Element,
    widget::{image, svg},
};

#[derive(Debug, Clone, PartialEq)]
pub enum IconSource {
    Pixmap(image::Handle),
    Themed(PathBuf),
}

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
