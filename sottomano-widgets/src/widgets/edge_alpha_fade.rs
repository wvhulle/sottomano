use iced::{Color, Rectangle};
use iced_wgpu::{
    graphics::Viewport,
    primitive::{Pipeline, Primitive, Renderer as _},
    wgpu,
};

const EDGE_ALPHA_FADE_SHADER: &str = r"
struct FadeVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) progress_to_edge: f32,
};

fn covering_triangle_corner(vertex_index: u32) -> vec2<f32> {
    return vec2<f32>(
        f32((vertex_index << 1u) & 2u) * 2.0 - 1.0,
        f32(vertex_index & 2u) * 2.0 - 1.0,
    );
}

@vertex
fn fade_towards_left(@builtin(vertex_index) vertex_index: u32) -> FadeVertex {
    let corner = covering_triangle_corner(vertex_index);
    return FadeVertex(vec4<f32>(corner, 0.0, 1.0), (1.0 - corner.x) / 2.0);
}

@vertex
fn fade_towards_right(@builtin(vertex_index) vertex_index: u32) -> FadeVertex {
    let corner = covering_triangle_corner(vertex_index);
    return FadeVertex(vec4<f32>(corner, 0.0, 1.0), (corner.x + 1.0) / 2.0);
}

@fragment
fn fade_strength(@location(0) progress_to_edge: f32) -> @location(0) vec4<f32> {
    return vec4<f32>(smoothstep(0.0, 1.0, progress_to_edge));
}
";

const ERASE_EXISTING_PIXELS: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::Zero,
    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
    operation: wgpu::BlendOperation::Add,
};

const ADD_BLEND_CONSTANT: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::Constant,
    dst_factor: wgpu::BlendFactor::One,
    operation: wgpu::BlendOperation::Add,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadedEdge {
    Left,
    Right,
}

pub fn draw_edge_alpha_fade(
    renderer: &mut iced::Renderer,
    band: Rectangle,
    faded_edge: FadedEdge,
    restored_fill: Color,
) {
    renderer.draw_primitive(
        band,
        EdgeAlphaFade {
            faded_edge,
            restored_fill,
        },
    );
}

#[derive(Debug)]
struct EdgeAlphaFade {
    faded_edge: FadedEdge,
    restored_fill: Color,
}

impl Primitive for EdgeAlphaFade {
    type Pipeline = EdgeAlphaFadePipelines;

    fn prepare(
        &self,
        _pipelines: &mut EdgeAlphaFadePipelines,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
    }

    fn draw(
        &self,
        pipelines: &EdgeAlphaFadePipelines,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        let directed = match self.faded_edge {
            FadedEdge::Left => &pipelines.towards_left,
            FadedEdge::Right => &pipelines.towards_right,
        };
        render_pass.set_pipeline(&directed.erase_pixels);
        render_pass.draw(0..3, 0..1);
        render_pass.set_pipeline(&directed.restore_fill);
        render_pass.set_blend_constant(premultiplied_blend_constant(
            self.restored_fill,
            pipelines.target_is_srgb,
        ));
        render_pass.draw(0..3, 0..1);
        true
    }
}

fn premultiplied_blend_constant(fill: Color, target_is_srgb: bool) -> wgpu::Color {
    let [red, green, blue, alpha] = if target_is_srgb {
        fill.into_linear()
    } else {
        [fill.r, fill.g, fill.b, fill.a]
    };
    wgpu::Color {
        r: f64::from(red * alpha),
        g: f64::from(green * alpha),
        b: f64::from(blue * alpha),
        a: f64::from(alpha),
    }
}

struct DirectedFadePipelines {
    erase_pixels: wgpu::RenderPipeline,
    restore_fill: wgpu::RenderPipeline,
}

struct EdgeAlphaFadePipelines {
    towards_left: DirectedFadePipelines,
    towards_right: DirectedFadePipelines,
    target_is_srgb: bool,
}

impl Pipeline for EdgeAlphaFadePipelines {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("edge alpha fade"),
            source: wgpu::ShaderSource::Wgsl(EDGE_ALPHA_FADE_SHADER.into()),
        });
        let blended_pipeline = |vertex_entry: &str, blend_component: wgpu::BlendComponent| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("edge alpha fade"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: Some(vertex_entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: Some("fade_strength"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: blend_component,
                            alpha: blend_component,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let directed_pipelines = |vertex_entry: &str| DirectedFadePipelines {
            erase_pixels: blended_pipeline(vertex_entry, ERASE_EXISTING_PIXELS),
            restore_fill: blended_pipeline(vertex_entry, ADD_BLEND_CONSTANT),
        };
        Self {
            towards_left: directed_pipelines("fade_towards_left"),
            towards_right: directed_pipelines("fade_towards_right"),
            target_is_srgb: format.is_srgb(),
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::premultiplied_blend_constant;

    #[test]
    fn restored_fill_is_premultiplied_by_its_alpha() {
        let constant = premultiplied_blend_constant(Color::from_rgba(1.0, 0.5, 0.0, 0.5), false);
        assert!((constant.r - 0.5).abs() < 1e-6);
        assert!((constant.g - 0.25).abs() < 1e-6);
        assert!(constant.b.abs() < 1e-6);
        assert!((constant.a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_transparent_panel_restores_nothing() {
        let constant = premultiplied_blend_constant(Color::TRANSPARENT, false);
        assert!(constant.r.abs() + constant.g.abs() + constant.b.abs() + constant.a.abs() < 1e-6);
    }
}
