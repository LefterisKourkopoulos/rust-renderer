// Vertex shader

struct Camera {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> camera: Camera;

const LIGHT_DIRECTIONAL: u32 = 0u;

struct Light {
    position: vec3<f32>,
    kind: u32,
    color: vec3<f32>,
    intensity: f32,
    direction: vec3<f32>,
    range: f32,
    cos_inner: f32,
    cos_outer: f32,
}
@group(1) @binding(0)
var<storage, read> lights: array<Light>;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let light = lights[instance_index];

    let scale = select(0.25, 0.0, light.kind == LIGHT_DIRECTIONAL);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position * scale + light.position, 1.0);
    out.color = light.color;
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
