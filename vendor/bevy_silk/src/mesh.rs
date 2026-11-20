// TODO: mesh generation utils
use bevy::{
    asset::RenderAssetUsages,
    math::Vec3,
    mesh::{Indices, Mesh, PrimitiveTopology},
};

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#[must_use]
/// Creates a cloth ready mesh in a triangle shape
///
/// # Params
///
/// * `size_x` - the size of the cloth in the X axis (should be above 1)
/// * `size_z` - the size of the cloth in the Z axis (should be above 1)
/// * `step_x` - the direction of the cloth propagation in the X axis
/// * `step_y` - the direction of the cloth propagation in the Y axis
/// * `normal` - the normal vector to apply to each vertex
pub fn rectangle_mesh(
    (size_x, size_y): (usize, usize),
    (step_x, step_y): (Vec3, Vec3),
    normal: Vec3,
) -> Mesh {
    let points: Vec<[f32; 3]> = (0..size_y)
        .flat_map(|y| {
            (0..size_x)
                .map(move |x| ((x % size_x) as f32 * step_x + (y as f32) * step_y).to_array())
        })
        .collect();
    let normal = normal.to_array();
    let normals: Vec<[f32; 3]> = (0..points.len()).map(|_| normal).collect();
    let uvs: Vec<[f32; 2]> = (0..size_y)
        .flat_map(|y| {
            (0..size_x).map(move |x| [x as f32 / size_x as f32, y as f32 / size_y as f32])
        })
        .collect();
    let indices: Vec<u32> = (0..points.len())
        .flat_map(|i| {
            let mut res = vec![];
            let i_32 = i as u32;
            if (i + 1) % size_x != 0 && points.get(i + size_x).is_some() {
                // Front triangle
                res.extend(vec![i_32 + 1, i_32, i_32 + size_x as u32]);
                // Back triangle
                // res.extend(vec![i_32 + size_x as u32, i_32, i_32 + 1]);
            }
            if i % size_x != 0
                && i.checked_sub(size_x)
                    .and_then(|i2| points.get(i2))
                    .is_some()
            {
                // Front triangle
                res.extend(vec![i_32 - 1, i_32, i_32 - size_x as u32]);
                // Back triangle
                // res.extend(vec![i_32 - size_x as u32, i_32, i_32 - 1]);
            }
            res
        })
        .collect();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_rectangle_mesh() {
        let mesh = rectangle_mesh((100, 100), (Vec3::X, -Vec3::Y), Vec3::Z);
        assert_eq!(mesh.count_vertices(), 100 * 100);
    }
}
