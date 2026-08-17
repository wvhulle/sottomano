use iced::{Color, Shadow, Vector};

const ALPHA: f32 = 0.44;
const OFFSET_PX: f32 = 4.0;
const BLUR_PX: f32 = 30.0;

pub fn overlay_shadow(fade: f32) -> Shadow {
    Shadow {
        color: Color {
            a: ALPHA * fade,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, OFFSET_PX),
        blur_radius: BLUR_PX,
    }
}
