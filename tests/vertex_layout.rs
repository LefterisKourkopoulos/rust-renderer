use std::mem::size_of;

use rust_renderer::Vertex;
use rust_renderer::scene::instance::InstanceRaw;
use rust_renderer::scene::model::ModelVertex;

#[test]
fn model_vertex_layout_matches_its_field_offsets() {
    let desc = ModelVertex::desc();

    assert_eq!(
        size_of::<ModelVertex>(),
        32,
        "position + tex_coords + normal"
    );
    assert_eq!(
        desc.array_stride,
        size_of::<ModelVertex>() as wgpu::BufferAddress
    );
    assert_eq!(desc.step_mode, wgpu::VertexStepMode::Vertex);

    let expected = [
        (0, 0, wgpu::VertexFormat::Float32x3),
        (12, 1, wgpu::VertexFormat::Float32x2),
        (20, 2, wgpu::VertexFormat::Float32x3),
    ];

    assert_eq!(desc.attributes.len(), expected.len());
    for (attribute, (offset, location, format)) in desc.attributes.iter().zip(expected) {
        assert_eq!(
            attribute.offset, offset,
            "offset of shader location {location}"
        );
        assert_eq!(attribute.shader_location, location);
        assert_eq!(
            attribute.format, format,
            "format of shader location {location}"
        );
    }
}

#[test]
fn model_vertex_attributes_exactly_cover_the_stride() {
    let desc = ModelVertex::desc();

    let last = desc.attributes.last().expect("layout has attributes");
    assert_eq!(
        last.offset + last.format.size(),
        desc.array_stride,
        "no padding or overlap between the final attribute and the stride"
    );

    for pair in desc.attributes.windows(2) {
        assert_eq!(
            pair[0].offset + pair[0].format.size(),
            pair[1].offset,
            "attributes must be packed back to back"
        );
    }
}

#[test]
fn instance_raw_layout_is_four_consecutive_vec4_columns() {
    let desc = InstanceRaw::desc();

    assert_eq!(size_of::<InstanceRaw>(), 64, "a mat4x4 of f32");
    assert_eq!(
        desc.array_stride,
        size_of::<InstanceRaw>() as wgpu::BufferAddress
    );
    assert_eq!(desc.step_mode, wgpu::VertexStepMode::Instance);

    let expected = [(0, 5), (16, 6), (32, 7), (48, 8)];

    assert_eq!(desc.attributes.len(), expected.len());
    for (attribute, (offset, location)) in desc.attributes.iter().zip(expected) {
        assert_eq!(
            attribute.offset, offset,
            "offset of shader location {location}"
        );
        assert_eq!(attribute.shader_location, location);
        assert_eq!(attribute.format, wgpu::VertexFormat::Float32x4);
    }
}

#[test]
fn vertex_and_instance_shader_locations_do_not_collide() {
    let vertex_locations: Vec<u32> = ModelVertex::desc()
        .attributes
        .iter()
        .map(|a| a.shader_location)
        .collect();
    let instance_locations: Vec<u32> = InstanceRaw::desc()
        .attributes
        .iter()
        .map(|a| a.shader_location)
        .collect();

    for location in &instance_locations {
        assert!(
            !vertex_locations.contains(location),
            "shader location {location} is used by both buffers"
        );
    }
}
