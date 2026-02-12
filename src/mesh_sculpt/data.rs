//! Mesh sculpt data components and resources

use bevy::prelude::*;

/// Per-entity sculpt data, lazily attached on first sculpt interaction.
#[derive(Component)]
pub struct MeshSculptData {
    /// Original vertex positions captured from the mesh on init
    pub original_positions: Vec<[f32; 3]>,
    /// Original vertex normals (displacement directions)
    pub original_normals: Vec<[f32; 3]>,
    /// Scalar per-vertex displacement (positive = outward, negative = inward)
    pub displacements: Vec<f32>,
    /// Adjacency list built from the index buffer, for Smooth brush
    pub adjacency: Vec<Vec<usize>>,
    /// Whether the mesh needs GPU update
    pub dirty: bool,
}

impl MeshSculptData {
    /// Initialize from a mesh's positions, normals, and indices.
    pub fn from_mesh(mesh: &Mesh) -> Option<Self> {
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?;
        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)?;

        let pos_values: Vec<[f32; 3]> = match positions {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
            _ => return None,
        };

        let norm_values: Vec<[f32; 3]> = match normals {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
            _ => return None,
        };

        let vertex_count = pos_values.len();
        let displacements = vec![0.0; vertex_count];

        let adjacency = build_adjacency(vertex_count, mesh.indices());

        Some(Self {
            original_positions: pos_values,
            original_normals: norm_values,
            displacements,
            adjacency,
            dirty: false,
        })
    }
}

/// Build per-vertex adjacency list from the index buffer.
fn build_adjacency(vertex_count: usize, indices: Option<&bevy::mesh::Indices>) -> Vec<Vec<usize>> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); vertex_count];

    let Some(indices) = indices else {
        return adj;
    };

    let tri_indices: Vec<usize> = match indices {
        bevy::mesh::Indices::U16(v) => v.iter().map(|&i| i as usize).collect(),
        bevy::mesh::Indices::U32(v) => v.iter().map(|&i| i as usize).collect(),
    };

    for tri in tri_indices.chunks_exact(3) {
        let (a, b, c) = (tri[0], tri[1], tri[2]);
        // Add edges both ways, dedup later
        adj[a].push(b);
        adj[a].push(c);
        adj[b].push(a);
        adj[b].push(c);
        adj[c].push(a);
        adj[c].push(b);
    }

    // Deduplicate
    for list in adj.iter_mut() {
        list.sort_unstable();
        list.dedup();
    }

    adj
}

/// Recompute face-averaged normals from positions and index buffer.
pub fn recompute_normals(
    positions: &[[f32; 3]],
    indices: Option<&bevy::mesh::Indices>,
) -> Vec<[f32; 3]> {
    let mut normals = vec![Vec3::ZERO; positions.len()];

    let Some(indices) = indices else {
        // No indices — return up normals as fallback
        return vec![[0.0, 1.0, 0.0]; positions.len()];
    };

    let tri_indices: Vec<usize> = match indices {
        bevy::mesh::Indices::U16(v) => v.iter().map(|&i| i as usize).collect(),
        bevy::mesh::Indices::U32(v) => v.iter().map(|&i| i as usize).collect(),
    };

    for tri in tri_indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0], tri[1], tri[2]);
        let v0 = Vec3::from(positions[i0]);
        let v1 = Vec3::from(positions[i1]);
        let v2 = Vec3::from(positions[i2]);

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let face_normal = edge1.cross(edge2); // Not normalized — area-weighted

        normals[i0] += face_normal;
        normals[i1] += face_normal;
        normals[i2] += face_normal;
    }

    normals
        .iter()
        .map(|n| {
            let normalized = n.normalize_or_zero();
            if normalized == Vec3::ZERO {
                [0.0, 1.0, 0.0]
            } else {
                [normalized.x, normalized.y, normalized.z]
            }
        })
        .collect()
}

/// Resource tracking the mesh sculpt tool state.
#[derive(Resource, Default)]
pub struct MeshSculptState {
    /// Current hover position on mesh (world space)
    pub hover_position: Option<Vec3>,
    /// Surface normal at hover point (world space)
    pub hover_normal: Option<Vec3>,
    /// The entity being sculpted
    pub active_mesh: Option<Entity>,
    /// Whether sculpting is currently active (mouse held)
    pub is_sculpting: bool,
    /// Whether the brush preview is currently being drawn
    pub brush_visible: bool,
    /// Displacement at the point where flatten started
    pub flatten_start_distance: Option<f32>,
}
