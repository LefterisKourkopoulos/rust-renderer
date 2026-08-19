use crate::scene::model::ModelVertex;

pub fn generate(vertices: &mut [ModelVertex], indices: &[u32]) {
    let mut triangle_count = vec![0u32; vertices.len()];

    for chunk in indices.chunks_exact(3) {
        let [i0, i1, i2] = [chunk[0] as usize, chunk[1] as usize, chunk[2] as usize];
        let (pos0, pos1, pos2) = (
            cgmath::Vector3::from(vertices[i0].position),
            cgmath::Vector3::from(vertices[i1].position),
            cgmath::Vector3::from(vertices[i2].position),
        );
        let (uv0, uv1, uv2) = (
            cgmath::Vector2::from(vertices[i0].tex_coords),
            cgmath::Vector2::from(vertices[i1].tex_coords),
            cgmath::Vector2::from(vertices[i2].tex_coords),
        );

        let delta_pos1 = pos1 - pos0;
        let delta_pos2 = pos2 - pos0;
        let delta_uv1 = uv1 - uv0;
        let delta_uv2 = uv2 - uv0;

        let determinant = delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x;
        if determinant.abs() < 1e-12 {
            continue;
        }

        let r = 1.0 / determinant;
        let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
        let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

        for i in [i0, i1, i2] {
            vertices[i].tangent = (cgmath::Vector3::from(vertices[i].tangent) + tangent).into();
            vertices[i].bitangent =
                (cgmath::Vector3::from(vertices[i].bitangent) + bitangent).into();
            triangle_count[i] += 1;
        }
    }

    for (vertex, &count) in vertices.iter_mut().zip(triangle_count.iter()) {
        if count > 0 {
            vertex.tangent = (cgmath::Vector3::from(vertex.tangent) / count as f32).into();
            vertex.bitangent = (cgmath::Vector3::from(vertex.bitangent) / count as f32).into();
        }
    }
}
