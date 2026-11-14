use iced::{
    Color, Rectangle,
    advanced::{Renderer as _, renderer},
};
use iced_wgpu::primitive::Renderer as _;

use crate::alpha_fade_shaders::{FadeShader, ReplacingFade};

const STRENGTH_SCALE: f32 = 65535.0;

pub(crate) fn fade_toward_color(
    restored_fill: Color,
    fade_strength: f32,
    renderer: &mut iced::Renderer,
    bounds: Rectangle,
) {
    let fade_strength = fade_strength.clamp(0.0, 1.0);
    if fade_strength <= 0.0 || bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    if matches!(renderer, iced::Renderer::Primary(_)) {
        renderer.draw_primitive(
            bounds,
            ReplacingFade::<UniformFadeShader>::new(
                (fade_strength * STRENGTH_SCALE).round() as u32,
                restored_fill,
            ),
        );
    } else {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            restored_fill.scale_alpha(fade_strength),
        );
    }
}

struct UniformFadeShader;

impl FadeShader for UniformFadeShader {
    const LABEL: &'static str = "uniform alpha fade";
    const SOURCE: &'static str = include_str!("uniform_alpha_fade.wgsl");
    const FRAGMENT_ENTRY: &'static str = "uniform_strength";
}
