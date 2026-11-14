use std::{
    fmt::{Debug, Formatter, Result as FormatResult},
    marker::PhantomData,
};

use iced::{
    Color, Rectangle,
    advanced::graphics::color,
    wgpu,
    widget::shader::{Pipeline, Primitive, Viewport},
};

const REPLACE_PIXELS_WITH_BLEND_CONSTANT: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::Constant,
    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
    operation: wgpu::BlendOperation::Add,
};

pub(crate) trait FadeShader: Send + Sync + 'static {
    const LABEL: &'static str;
    const SOURCE: &'static str;
    const FRAGMENT_ENTRY: &'static str;
}

pub(crate) struct ReplacingFade<Shader> {
    packed_parameters: u32,
    restored_fill: Color,
    shader: PhantomData<Shader>,
}

impl<Shader> ReplacingFade<Shader> {
    pub(crate) fn new(packed_parameters: u32, restored_fill: Color) -> Self {
        Self {
            packed_parameters,
            restored_fill,
            shader: PhantomData,
        }
    }
}

impl<Shader> Debug for ReplacingFade<Shader> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FormatResult {
        formatter
            .debug_struct("ReplacingFade")
            .field("packed_parameters", &self.packed_parameters)
            .field("restored_fill", &self.restored_fill)
            .finish()
    }
}

impl<Shader: FadeShader> Primitive for ReplacingFade<Shader> {
    type Pipeline = ReplacingFadePipeline<Shader>;

    fn prepare(
        &self,
        _pipeline: &mut ReplacingFadePipeline<Shader>,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
    }

    fn draw(
        &self,
        pipeline: &ReplacingFadePipeline<Shader>,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        render_pass.set_pipeline(&pipeline.fade_pipeline);
        render_pass.set_blend_constant(premultiplied_blend_constant(self.restored_fill));
        render_pass.draw(0..3, self.packed_parameters..self.packed_parameters + 1);
        true
    }
}

pub(crate) struct ReplacingFadePipeline<Shader> {
    fade_pipeline: wgpu::RenderPipeline,
    shader: PhantomData<Shader>,
}

impl<Shader: FadeShader> Pipeline for ReplacingFadePipeline<Shader> {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            fade_pipeline: replacing_fade_pipeline(
                device,
                format,
                Shader::LABEL,
                Shader::SOURCE,
                Shader::FRAGMENT_ENTRY,
            ),
            shader: PhantomData,
        }
    }
}

pub(crate) fn premultiplied_blend_constant(fill: Color) -> wgpu::Color {
    let [red, green, blue, alpha] = color::pack(fill).components();
    wgpu::Color {
        r: f64::from(red * alpha),
        g: f64::from(green * alpha),
        b: f64::from(blue * alpha),
        a: f64::from(alpha),
    }
}

fn replacing_fade_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader_label: &str,
    shader_source: &str,
    fragment_entry: &str,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(shader_label),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(shader_label),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("covering_triangle"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: REPLACE_PIXELS_WITH_BLEND_CONSTANT,
                    alpha: REPLACE_PIXELS_WITH_BLEND_CONSTANT,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}
