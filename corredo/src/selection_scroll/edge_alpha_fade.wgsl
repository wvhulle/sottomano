struct FadeVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) centered_unit: vec2<f32>,
    @location(1) @interpolate(flat) outward: vec2<f32>,
    @location(2) @interpolate(flat) radius_fraction: f32,
};

fn outward_of_edge(edge_index: u32) -> vec2<f32> {
    switch edge_index {
        case 0u: { return vec2<f32>(-1.0, 0.0); }
        case 1u: { return vec2<f32>(1.0, 0.0); }
        case 2u: { return vec2<f32>(0.0, -1.0); }
        default: { return vec2<f32>(0.0, 1.0); }
    }
}

@vertex
fn covering_triangle(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) packed_band_shape: u32,
) -> FadeVertex {
    let corner = vec2<f32>(
        f32((vertex_index << 1u) & 2u) * 2.0 - 1.0,
        f32(vertex_index & 2u) * 2.0 - 1.0,
    );
    return FadeVertex(
        vec4<f32>(corner, 0.0, 1.0),
        vec2<f32>(corner.x, -corner.y),
        outward_of_edge(packed_band_shape & 3u),
        f32(packed_band_shape >> 2u) / 65535.0,
    );
}

fn distance_outside_rounded_edge(along_px: f32, across_px: f32, half_along_px: f32, half_across_px: f32, radius_px: f32) -> f32 {
    let past_corner_center = vec2<f32>(
        across_px - (half_across_px - radius_px),
        along_px - (half_along_px - radius_px),
    );
    return length(max(past_corner_center, vec2<f32>(0.0)))
        + min(max(past_corner_center.x, past_corner_center.y), 0.0)
        - radius_px;
}

@fragment
fn fade_strength(
    @location(0) centered_unit: vec2<f32>,
    @location(1) @interpolate(flat) outward: vec2<f32>,
    @location(2) @interpolate(flat) radius_fraction: f32,
) -> @location(0) vec4<f32> {
    let along_unit = dot(centered_unit, outward);
    let across_unit = dot(centered_unit, outward.yx);
    let half_along_px = 1.0 / fwidth(along_unit);
    let half_across_px = 1.0 / fwidth(across_unit);
    let outside = distance_outside_rounded_edge(
        along_unit * half_along_px,
        abs(across_unit) * half_across_px,
        half_along_px,
        half_across_px,
        radius_fraction * 2.0 * half_along_px,
    );
    let coverage = clamp(0.5 - outside, 0.0, 1.0);
    let progress_to_edge = along_unit / 2.0 + 0.5;
    return vec4<f32>(smoothstep(0.0, 1.0, progress_to_edge) * coverage);
}
