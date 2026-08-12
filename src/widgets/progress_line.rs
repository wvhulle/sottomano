use iced::{
    Color, Element, Length,
    widget::{Row, container},
};

use crate::widgets::activity_line::activity_line;

const PORTIONS: f32 = 1000.0;

pub fn progress_line<'a, Message: 'a>(
    filled: Color,
    track: Color,
    fraction: f32,
    thickness: f32,
) -> Element<'a, Message> {
    let segment = |color: Color, portion| {
        container(activity_line::<Message>(color, 1.0, thickness))
            .width(Length::FillPortion(portion))
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let done = (fraction.clamp(0.0, 1.0) * PORTIONS) as u16;
    let remaining = PORTIONS as u16 - done;
    Row::new()
        .extend((done > 0).then(|| segment(filled, done).into()))
        .extend((remaining > 0).then(|| segment(track, remaining).into()))
        .width(Length::Fill)
        .into()
}
