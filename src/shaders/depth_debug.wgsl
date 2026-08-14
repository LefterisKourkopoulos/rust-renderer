// Depth debug shader: draws the depth texture to the screen as grayscale.
@group(0) @binding(0)
var t_depth: texture_depth_2d;
@group(0) @binding(1)
var s_depth: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle covering the entire clip space, built from the vertex index alone.
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);

    var out: VertexOutput;
    out.tex_coords = vec2<f32>(x, y);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let depth = textureSample(t_depth, s_depth, in.tex_coords);
    return vec4<f32>(depth, depth, depth, 1.0);
}
