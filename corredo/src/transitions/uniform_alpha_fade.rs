use iced::{
    Color, Rectangle,
    advanced::{Renderer as _, renderer},
    wgpu,
    widget::shader::{Pipeline, Primitive, Viewport},
};
use iced_wgpu::primitive::Renderer as _;

use crate::selection_scroll::edge_alpha_fade::{
    premultiplied_blend_constant, replacing_fade_pipeline,
};

const UNIFORM_ALPHA_FADE_SHADER: &str = include_str!("uniform_alpha_fade.wgsl");

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
            UniformAlphaFade {
                packed_strength: (fade_strength * STRENGTH_SCALE).round() as u32,
                restored_fill,
            },
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

#[derive(Debug)]
struct UniformAlphaFade {
    packed_strength: u32,
    restored_fill: Color,
}

impl Primitive for UniformAlphaFade {
    type Pipeline = UniformAlphaFadePipeline;

    fn prepare(
        &self,
        _pipeline: &mut UniformAlphaFadePipeline,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
    }

    fn draw(
        &self,
        pipeline: &UniformAlphaFadePipeline,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        render_pass.set_pipeline(&pipeline.fade_pipeline);
        render_pass.set_blend_constant(premultiplied_blend_constant(self.restored_fill));
        render_pass.draw(0..3, self.packed_strength..self.packed_strength + 1);
        true
    }
}

struct UniformAlphaFadePipeline {
    fade_pipeline: wgpu::RenderPipeline,
}

impl Pipeline for UniformAlphaFadePipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            fade_pipeline: replacing_fade_pipeline(
                device,
                format,
                "uniform alpha fade",
                UNIFORM_ALPHA_FADE_SHADER,
                "uniform_strength",
            ),
        }
    }
}
