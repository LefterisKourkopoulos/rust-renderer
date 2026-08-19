@group(0) @binding(0)
var t_shadow: texture_depth_2d_array;
@group(0) @binding(1)
var s_shadow: sampler;

struct LayerUniform {
    index: vec4<u32>,
};
@group(0) @binding(2)
var<uniform> layer: LayerUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);

    var out: VertexOutput;
    out.tex_coords = vec2<f32>(x, y);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let depth = textureSample(t_shadow, s_shadow, in.tex_coords, layer.index.x);
    return vec4<f32>(depth, depth, depth, 1.0);
}
