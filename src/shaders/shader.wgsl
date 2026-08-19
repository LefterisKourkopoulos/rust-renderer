// Vertex shader
struct CameraUniform {
    view_pos: vec4<f32>,
    view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
};
@group(1) @binding(0) // 1.
var<uniform> camera: CameraUniform;

const LIGHT_DIRECTIONAL: u32 = 0u;
const LIGHT_POINT: u32 = 1u;
const LIGHT_SPOT: u32 = 2u;

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
@group(2) @binding(0)
var<storage, read> lights: array<Light>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) world_bitangent: vec3<f32>,
    @location(4) world_normal: vec3<f32>,
}

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let normal_matrix = mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );

    let world_position = (model_matrix * vec4<f32>(model.position, 1.0)).xyz;

    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.world_position = world_position;
    out.world_tangent = normalize(normal_matrix * model.tangent);
    out.world_bitangent = normalize(normal_matrix * model.bitangent);
    out.world_normal = normalize(normal_matrix * model.normal);
    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    return out;
}

// Fragment shader
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;

struct MaterialUniform {
    base_color: vec4<f32>,
    emissive: vec3<f32>,
    metallic: f32,
    roughness: f32,
};
@group(0) @binding(4)
var<uniform> material: MaterialUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let object_color = textureSample(t_diffuse, s_diffuse, in.tex_coords) * material.base_color;
    let object_normal = textureSample(t_normal, s_normal, in.tex_coords);

    let tangent_matrix = mat3x3<f32>(
        in.world_tangent,
        in.world_bitangent,
        in.world_normal,
    );
    let world_normal = normalize(tangent_matrix * (object_normal.xyz * 2.0 - 1.0));

    let view_dir = normalize(camera.view_pos.xyz - in.world_position);

    let shininess = mix(4.0, 256.0, 1.0 - material.roughness);

    var result = vec3<f32>(0.0);

    for (var i = 0u; i < arrayLength(&lights); i++) {
        let light = lights[i];

        var light_dir: vec3<f32>;
        var attenuation = 1.0;

        if light.kind == LIGHT_DIRECTIONAL {
            light_dir = -light.direction;
        } else {
            let to_light = light.position - in.world_position;
            let distance = length(to_light);
            if distance < 1e-4 {
                continue;
            }
            light_dir = to_light / distance;

            attenuation = 1.0 / (distance * distance);

            if light.range > 0.0 {
                let fade = clamp(1.0 - distance / light.range, 0.0, 1.0);
                attenuation *= fade * fade;
            }

            if light.kind == LIGHT_SPOT {
                let cos_angle = dot(light.direction, -light_dir);
                attenuation *= smoothstep(light.cos_outer, light.cos_inner, cos_angle);
            }
        }

        let radiance = light.color * light.intensity * attenuation;

        let diffuse_strength = max(dot(world_normal, light_dir), 0.0);
        let diffuse_color = radiance * diffuse_strength;

        let half_dir = normalize(view_dir + light_dir);
        let specular_strength = select(
            0.0,
            pow(max(dot(world_normal, half_dir), 0.0), shininess),
            diffuse_strength > 0.0,
        );
        let specular_color = specular_strength * radiance;

        result += (diffuse_color + specular_color) * object_color.xyz;
    }

    let ambient_strength = 0.1;
    result += ambient_strength * object_color.xyz;
    result += material.emissive;

    return vec4<f32>(result, object_color.a);
}