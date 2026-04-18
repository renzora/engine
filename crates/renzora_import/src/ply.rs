//! PLY → GLB converter.

use std::path::Path;

use ply_rs::parser::Parser;
use ply_rs::ply::{Property, PropertyAccess};

use crate::convert::{ImportError, ImportResult};
use crate::obj::build_glb;
use crate::settings::{ImportSettings, UpAxis};

#[derive(Default)]
struct Vertex {
    x: f32,
    y: f32,
    z: f32,
    nx: f32,
    ny: f32,
    nz: f32,
    u: f32,
    v: f32,
}

impl PropertyAccess for Vertex {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, key: String, property: Property) {
        match (key.as_str(), property) {
            ("x", Property::Float(v)) => self.x = v,
            ("y", Property::Float(v)) => self.y = v,
            ("z", Property::Float(v)) => self.z = v,
            ("x", Property::Double(v)) => self.x = v as f32,
            ("y", Property::Double(v)) => self.y = v as f32,
            ("z", Property::Double(v)) => self.z = v as f32,
            ("nx", Property::Float(v)) => self.nx = v,
            ("ny", Property::Float(v)) => self.ny = v,
            ("nz", Property::Float(v)) => self.nz = v,
            ("s" | "u" | "texture_u", Property::Float(v)) => self.u = v,
            ("t" | "v" | "texture_v", Property::Float(v)) => self.v = v,
            _ => {}
        }
    }
}

#[derive(Default)]
struct Face {
    vertex_indices: Vec<u32>,
}

impl PropertyAccess for Face {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, key: String, property: Property) {
        match (key.as_str(), property) {
            ("vertex_indices" | "vertex_index", Property::ListInt(v)) => {
                self.vertex_indices = v.iter().map(|&i| i as u32).collect();
            }
            ("vertex_indices" | "vertex_index", Property::ListUInt(v)) => {
                self.vertex_indices = v.iter().map(|&i| i as u32).collect();
            }
            _ => {}
        }
    }
}

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    let vertex_parser = Parser::<Vertex>::new();
    let face_parser = Parser::<Face>::new();

    let header = vertex_parser
        .read_header(&mut reader)
        .map_err(|e| ImportError::ParseError(format!("PLY header: {}", e)))?;

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut faces: Vec<Face> = Vec::new();

    for (_name, element) in &header.elements {
        match _name.as_str() {
            "vertex" => {
                vertices = vertex_parser
                    .read_payload_for_element(&mut reader, element, &header)
                    .map_err(|e| ImportError::ParseError(format!("PLY vertices: {}", e)))?;
            }
            "face" => {
                faces = face_parser
                    .read_payload_for_element(&mut reader, element, &header)
                    .map_err(|e| ImportError::ParseError(format!("PLY faces: {}", e)))?;
            }
            _ => {
                // Skip unknown elements
                let skip_parser = Parser::<Vertex>::new();
                let _ = skip_parser.read_payload_for_element(&mut reader, element, &header);
            }
        }
    }

    if vertices.is_empty() {
        return Err(ImportError::ParseError("PLY file contains no vertices".into()));
    }

    let mut warnings = Vec::new();

    // Build flat arrays
    let mut positions = Vec::with_capacity(vertices.len() * 3);
    let mut normals = Vec::with_capacity(vertices.len() * 3);
    let mut texcoords = Vec::with_capacity(vertices.len() * 2);

    let has_normals = vertices.iter().any(|v| v.nx != 0.0 || v.ny != 0.0 || v.nz != 0.0);

    for vert in &vertices {
        let (x, mut y, mut z) = (
            vert.x * settings.scale,
            vert.y * settings.scale,
            vert.z * settings.scale,
        );

        if settings.up_axis == UpAxis::ZUp {
            let tmp = y;
            y = z;
            z = -tmp;
        }

        positions.extend_from_slice(&[x, y, z]);

        if has_normals {
            let (nx, mut ny, mut nz) = (vert.nx, vert.ny, vert.nz);
            if settings.up_axis == UpAxis::ZUp {
                let tmp = ny;
                ny = nz;
                nz = -tmp;
            }
            normals.extend_from_slice(&[nx, ny, nz]);
        }

        let tv = if settings.flip_uvs { 1.0 - vert.v } else { vert.v };
        texcoords.extend_from_slice(&[vert.u, tv]);
    }

    // Build indices — triangulate faces (fan triangulation)
    let mut indices = Vec::new();

    if faces.is_empty() {
        // Point cloud — no faces, create sequential indices as points
        // For GLB we still need triangles, so skip or warn
        warnings.push("PLY has no face data — point cloud imported as degenerate triangles".into());
        for i in (0..vertices.len()).step_by(3) {
            if i + 2 < vertices.len() {
                indices.extend_from_slice(&[i as u32, (i + 1) as u32, (i + 2) as u32]);
            }
        }
    } else {
        for face in &faces {
            let vi = &face.vertex_indices;
            if vi.len() < 3 {
                continue;
            }
            // Fan triangulation
            for i in 1..vi.len() - 1 {
                indices.extend_from_slice(&[vi[0], vi[i] as u32, vi[i + 1] as u32]);
            }
        }
    }

    // Generate normals if needed
    if !has_normals && settings.generate_normals {
        normals = generate_normals_from_positions(&positions, &indices);
    } else if !has_normals {
        normals = vec![0.0; vertices.len() * 3];
    }

    let glb_bytes = build_glb(&positions, &normals, &texcoords, &indices, &crate::obj::MaterialBundle::default())?;

    Ok(ImportResult {
        glb_bytes,
        warnings, extracted_textures: Vec::new(), extracted_materials: Vec::new(),
    })
}

fn generate_normals_from_positions(positions: &[f32], indices: &[u32]) -> Vec<f32> {
    let vert_count = positions.len() / 3;
    let mut normals = vec![0.0f32; vert_count * 3];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        let p0 = &positions[i0 * 3..i0 * 3 + 3];
        let p1 = &positions[i1 * 3..i1 * 3 + 3];
        let p2 = &positions[i2 * 3..i2 * 3 + 3];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            normals[idx * 3] += n[0];
            normals[idx * 3 + 1] += n[1];
            normals[idx * 3 + 2] += n[2];
        }
    }

    for i in 0..vert_count {
        let x = normals[i * 3];
        let y = normals[i * 3 + 1];
        let z = normals[i * 3 + 2];
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
