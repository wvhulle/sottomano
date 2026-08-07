use iced::Color;

pub fn darkened(color: Color, factor: f32) -> Color {
    let alpha = factor + color.a * (1.0 - factor);
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    let weight = color.a * (1.0 - factor) / alpha;
    Color {
        r: color.r * weight,
        g: color.g * weight,
        b: color.b * weight,
        a: alpha,
    }
}
