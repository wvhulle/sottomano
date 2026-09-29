use iced::{
    Element,
    widget::{container, image, svg},
};

use crate::{data::source::tray::IconSource, widgets::svg_icon::PHOSPHOR_INK_FRACTION};

pub const NATIVE_TO_PHOSPHOR_SCALE: f32 = PHOSPHOR_INK_FRACTION / (224.0 / 232.0);

pub fn native_icon<'a, Message: 'a>(
    source: &IconSource,
    side: f32,
    opacity: f32,
) -> Element<'a, Message> {
    let drawing = side * NATIVE_TO_PHOSPHOR_SCALE;
    let icon: Element<'a, Message> = match source {
        IconSource::Pixmap(handle) => image(handle.clone())
            .width(drawing)
            .height(drawing)
            .opacity(opacity)
            .into(),
        IconSource::Themed(path) => {
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
            {
                svg(svg::Handle::from_path(path))
                    .width(drawing)
                    .height(drawing)
                    .opacity(opacity)
                    .into()
            } else {
                image(image::Handle::from_path(path))
                    .width(drawing)
                    .height(drawing)
                    .opacity(opacity)
                    .into()
            }
        }
    };
    container(icon).center(side).into()
}
