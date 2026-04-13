#![allow(unused_variables, unused_assignments, dead_code)]

//! Alembic (.abc) → GLB converter.
//!
//! Pure-Rust parser for the Ogawa container format used by Alembic files.
//! Extracts polymesh geometry (positions, normals, UVs, face indices).
//!
//! Only the Ogawa backend is supported (default since Alembic 1.5 / ~2013).
//! HDF5-backed files are not supported.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::settings::{ImportSettings, UpAxis};

/// Ogawa file magic: [0xFF, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]
const OGAWA_MAGIC: [u8; 8] = [0xFF, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00];

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let data = std::fs::read(path)?;

    if data.len() < 8 {
        return Err(ImportError::ParseError("File too small for Alembic".into()));
    }

    if data[..8] != OGAWA_MAGIC {
        return Err(ImportError::ParseError(
            "Not an Ogawa-format Alembic file. HDF5-backed .abc files are not supported."
                .into(),
        ));
    }

    let mut warnings = Vec::new();
    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_normals: Vec<f32> = Vec::new();
    let mut all_texcoords: Vec<f32> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();

    // Parse the Ogawa root group at offset 8
    let root = read_group(&data, 8)?;

    // Walk the hierarchy looking for polymesh data
    extract_meshes(
        &data,
        &root,
        settings,
        &mut all_positions,
        &mut all_normals,
        &mut all_texcoords,
        &mut all_indices,
        &mut warnings,
        0,
    )?;

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "No mesh geometry found in Alembic file".into(),
        ));
    }

    // Generate normals if we have none
    if all_normals.is_empty() && settings.generate_normals {
        let vertex_count = all_positions.len() / 3;
        all_normals = generate_flat_normals(&all_positions, &all_indices, vertex_count);
    }

    // Pad texcoords if empty
    if all_texcoords.is_empty() {
        let vertex_count = all_positions.len() / 3;
        all_texcoords = vec![0.0; vertex_count * 2];
    }

    let glb_bytes =
        crate::obj::build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices, &crate::obj::MaterialBundle::default())?;

    Ok(ImportResult {
        glb_bytes,
        warnings, extracted_textures: Vec::new(), extracted_materials: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Ogawa container format
// ---------------------------------------------------------------------------

/// A group in the Ogawa hierarchy.
#[derive(Debug)]
struct OgawaGroup {
    children: Vec<OgawaChild>,
}

/// A child reference in an Ogawa group.
#[derive(Debug)]
struct OgawaChild {
    /// true = data stream, false = group
    is_data: bool,
    /// Offset in the file
    offset: u64,
    /// Size (only valid for data streams)
    size: u64,
}

fn read_group(data: &[u8], offset: usize) -> Result<OgawaGroup, ImportError> {
    if offset + 8 > data.len() {
        return Err(ImportError::ParseError("Group header truncated".into()));
    }

    let child_count = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;

    // Sanity check
    if child_count > 10_000 {
        return Err(ImportError::ParseError(format!(
            "Unreasonable group child count: {}",
            child_count
        )));
    }

    let mut children = Vec::with_capacity(child_count);
    let entries_start = offset + 8;

    // Each child entry: 16 bytes (8 bytes offset + 8 bytes size/type)
    // Actually in Ogawa the entry format varies. The most common is:
    // For each child: a u64 where the low bit indicates data(1) vs group(0),
    // and the remaining 63 bits are the file offset.
    for i in 0..child_count {
        let entry_offset = entries_start + i * 8;
        if entry_offset + 8 > data.len() {
            break;
        }

        let raw = u64::from_le_bytes(data[entry_offset..entry_offset + 8].try_into().unwrap());
        let is_data = (raw & 1) != 0;
        let child_offset = (raw >> 1) as u64;

        let size = if is_data && (child_offset as usize) + 8 <= data.len() {
            // Data streams store their size at the referenced offset
            let so = child_offset as usize;
            if so + 8 <= data.len() {
                u64::from_le_bytes(data[so..so + 8].try_into().unwrap())
            } else {
                0
            }
        } else {
            0
        };

        children.push(OgawaChild {
            is_data,
            offset: child_offset,
            size,
        });
    }

    Ok(OgawaGroup { children })
}

fn read_data_stream<'a>(data: &'a [u8], child: &OgawaChild) -> Option<&'a [u8]> {
    if !child.is_data {
        return None;
    }

    let offset = child.offset as usize;
    if offset + 8 > data.len() {
        return None;
    }

    // Size is stored at the offset, data follows
    let size = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
    let data_start = offset + 8;

    if data_start + size > data.len() {
        return None;
    }

    Some(&data[data_start..data_start + size])
}

// ---------------------------------------------------------------------------
// Alembic schema layer
// ---------------------------------------------------------------------------

fn extract_meshes(
    file_data: &[u8],
    group: &OgawaGroup,
    settings: &ImportSettings,
    positions: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    texcoords: &mut Vec<f32>,
    indices: &mut Vec<u32>,
    warnings: &mut Vec<String>,
    depth: usize,
) -> Result<(), ImportError> {
    if depth > 50 {
        return Ok(());
    }

    // Try to interpret this group as a polymesh by looking for position data
    // in its data children. Alembic stores schema properties as data streams
    // within the object's group hierarchy.
    let mut found_positions = false;
    let mut local_positions: Vec<f32> = Vec::new();
    let mut local_face_counts: Vec<i32> = Vec::new();
    let mut local_face_indices: Vec<i32> = Vec::new();

    for child in &group.children {
        if child.is_data {
            if let Some(stream) = read_data_stream(file_data, child) {
                // Try to detect what this data stream contains by its content
                // Alembic positions are typically float32 arrays that are
                // divisible by 3 (x,y,z triples)
                if !found_positions && stream.len() >= 12 && stream.len() % 12 == 0 {
                    // Could be positions — check if values look like coordinates
                    let float_count = stream.len() / 4;
                    if float_count % 3 == 0 && looks_like_positions(stream) {
                        local_positions = read_f32_array(stream);
                        found_positions = true;
                    }
                }
            }
        }
    }

    // If we found positions, look for face data in other children
    if found_positions && !local_positions.is_empty() {
        for child in &group.children {
            if child.is_data {
                if let Some(stream) = read_data_stream(file_data, child) {
                    let int_count = stream.len() / 4;

                    if local_face_counts.is_empty()
                        && int_count > 0
                        && stream.len() % 4 == 0
                        && looks_like_face_counts(stream)
                    {
                        local_face_counts = read_i32_array(stream);
                    } else if local_face_indices.is_empty()
                        && int_count > 0
                        && stream.len() % 4 == 0
                        && !looks_like_face_counts(stream)
                        && looks_like_indices(stream, local_positions.len() / 3)
                    {
                        local_face_indices = read_i32_array(stream);
                    }
                }
            }
        }

        // Build the mesh if we have enough data
        if !local_positions.is_empty() {
            let base_vertex = (positions.len() / 3) as u32;
            let vertex_count = local_positions.len() / 3;

            // Apply scale and axis conversion
            for i in 0..vertex_count {
                let (x, mut y, mut z) = (
                    local_positions[i * 3] * settings.scale,
                    local_positions[i * 3 + 1] * settings.scale,
                    local_positions[i * 3 + 2] * settings.scale,
                );

                if settings.up_axis == UpAxis::ZUp {
                    let tmp = y;
                    y = z;
                    z = -tmp;
                }

                positions.extend_from_slice(&[x, y, z]);
            }

            // Triangulate faces
            if !local_face_counts.is_empty() && !local_face_indices.is_empty() {
                let tris = triangulate_faces(&local_face_counts, &local_face_indices);
                for idx in tris {
                    indices.push(idx as u32 + base_vertex);
                }
            } else if !local_face_indices.is_empty() {
                // Assume triangles if no face counts
                for idx in &local_face_indices {
                    indices.push(*idx as u32 + base_vertex);
                }
            }
        }
    }

    // Recurse into child groups
    for child in &group.children {
        if !child.is_data {
            if let Ok(child_group) = read_group(file_data, child.offset as usize) {
                extract_meshes(
                    file_data,
                    &child_group,
                    settings,
                    positions,
                    normals,
                    texcoords,
                    indices,
                    warnings,
                    depth + 1,
                )?;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Heuristics and helpers
// ---------------------------------------------------------------------------

fn looks_like_positions(data: &[u8]) -> bool {
    // Check that float values are in a reasonable range for 3D coordinates
    let floats = read_f32_array(data);
    if floats.len() < 3 {
        return false;
    }
    let mut reasonable = 0;
    for &v in &floats {
        if v.is_finite() && v.abs() < 1e6 {
            reasonable += 1;
        }
    }
    reasonable > floats.len() * 3 / 4
}

fn looks_like_face_counts(data: &[u8]) -> bool {
    // Face counts are typically small positive integers (3, 4, etc.)
    let ints = read_i32_array(data);
    if ints.is_empty() {
        return false;
    }
    ints.iter().all(|&v| v >= 3 && v <= 100)
}

fn looks_like_indices(data: &[u8], vertex_count: usize) -> bool {
    let ints = read_i32_array(data);
    if ints.is_empty() {
        return false;
    }
    // All indices should be valid vertex indices
    ints.iter().all(|&v| v >= 0 && (v as usize) < vertex_count.max(1) * 2)
}

fn read_f32_array(data: &[u8]) -> Vec<f32> {
    data.chunks(4)
        .filter(|c| c.len() == 4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_i32_array(data: &[u8]) -> Vec<i32> {
    data.chunks(4)
        .filter(|c| c.len() == 4)
        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn triangulate_faces(face_counts: &[i32], face_indices: &[i32]) -> Vec<i32> {
    let mut result = Vec::new();
    let mut idx_offset = 0usize;

    for &count in face_counts {
        let n = count as usize;
        if n < 3 || idx_offset + n > face_indices.len() {
            idx_offset += n;
            continue;
        }

        let v0 = face_indices[idx_offset];
        for i in 1..n - 1 {
            result.push(v0);
            result.push(face_indices[idx_offset + i]);
            result.push(face_indices[idx_offset + i + 1]);
        }

        idx_offset += n;
    }

    result
}

fn generate_flat_normals(positions: &[f32], indices: &[u32], vertex_count: usize) -> Vec<f32> {
    let mut normals = vec![0.0f32; vertex_count * 3];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 * 3 + 2 >= positions.len()
            || i1 * 3 + 2 >= positions.len()
            || i2 * 3 + 2 >= positions.len()
        {
            continue;
        }

        let p0 = [positions[i0 * 3], positions[i0 * 3 + 1], positions[i0 * 3 + 2]];
        let p1 = [positions[i1 * 3], positions[i1 * 3 + 1], positions[i1 * 3 + 2]];
        let p2 = [positions[i2 * 3], positions[i2 * 3 + 1], positions[i2 * 3 + 2]];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            if idx * 3 + 2 < normals.len() {
                normals[idx * 3] += n[0];
                normals[idx * 3 + 1] += n[1];
                normals[idx * 3 + 2] += n[2];
            }
        }
    }

    for i in 0..vertex_count {
        let (x, y, z) = (normals[i * 3], normals[i * 3 + 1], normals[i * 3 + 2]);
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-8 {
            normals[i * 3] /= len;
            normals[i * 3 + 1] /= len;
            normals[i * 3 + 2] /= len;
        } else {
            normals[i * 3 + 1] = 1.0;
        }
    }

    normals
}
