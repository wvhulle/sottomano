struct UniformFadeVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) strength: f32,
};

@vertex
fn covering_triangle(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) packed_strength: u32,
) -> UniformFadeVertex {
    let corner = vec2<f32>(
        f32((vertex_index << 1u) & 2u) * 2.0 - 1.0,
        f32(vertex_index & 2u) * 2.0 - 1.0,
    );
    return UniformFadeVertex(
        vec4<f32>(corner, 0.0, 1.0),
        f32(packed_strength) / 65535.0,
    );
}

@fragment
fn uniform_strength(@location(0) @interpolate(flat) strength: f32) -> @location(0) vec4<f32> {
    return vec4<f32>(strength);
}
