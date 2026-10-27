//! Rigging: putting a skeleton into a model and moving it well.
//!
//! The animation editor's Rig and Face panels are the front end; this is the
//! part that can be tested without one.
//!
//! - [`glb`]: reading a `.glb` and adding to it without disturbing anything
//!   already there.
//! - [`skin`]: bones plus a weighting rule into joints, a skin and per-vertex
//!   weights.
//! - [`face`]: a face rig (jaw, lips, corners, eyes, lids, brows) from five
//!   points; [`expressions`]: its built-in sliders, and natural blinks.
//! - [`controls`]: sliders a user makes for any rig, saved beside the model,
//!   and how their values mix into a pose.
//! - [`humanoid`]: a Mixamo-compatible body skeleton from seven points.
//! - [`mirror`]: left and right; [`ik`]: two-bone reaching.
//! - [`weights`]: weights for a skeleton that is already there, generated
//!   and painted.
//! - [`morphs`]: blend shapes (glTF morph targets) read from and written into
//!   a model; [`sculpt`]: the brushes that sculpt them.

pub mod chain;
pub mod controls;
pub mod edit;
pub mod export;
pub mod expressions;
pub mod face;
pub mod fingers;
pub mod glb;
pub mod humanoid;
pub mod ik;
pub mod mirror;
pub mod morphs;
pub mod quadruped;
pub mod sculpt;
pub mod skin;
pub mod weights;

pub use glb::{Glb, GlbError};
pub use morphs::write_shapes;

/// Whether `path` is a model stored as `.rvg`.
fn is_rvg(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("rvg"))
}

#[cfg(test)]
mod rvg_tests {
    use super::*;
    use bevy::math::Vec3;

    /// A model stored as `.rvg` is rigged in place like a `.glb`: the result
    /// is still an `.rvg`, its clusters now carry joints and weights, its
    /// scene has the skeleton, and the original is kept beside it.
    #[test]
    fn a_model_stored_as_rvg_is_rigged_in_place() {
        let (rings, segments) = (48u32, 24u32);
        let mut positions = Vec::new();
        for r in 0..=rings {
            for s in 0..segments {
                let theta = std::f32::consts::TAU * s as f32 / segments as f32;
                positions.push(Vec3::new(0.2 * theta.cos(), r as f32 / rings as f32 * 4.0, 0.2 * theta.sin()));
            }
        }
        let normals: Vec<Vec3> = positions.iter().map(|p| Vec3::new(p.x, 0.0, p.z).normalize()).collect();
        let mut indices = Vec::new();
        for r in 0..rings {
            for s in 0..segments {
                let (a, b) = (r * segments + s, r * segments + (s + 1) % segments);
                let (c, d) = (a + segments, b + segments);
                indices.extend([a, c, b, b, c, d]);
            }
        }
        let geometry = glb::Geometry {
            name: "Rope",
            positions: &positions,
            normals: &normals,
            uvs: &[],
            indices: &indices,
            base_color: [1.0; 4],
        };
        let source = Glb::from_geometry(&geometry).unwrap().to_bytes();
        let dir = std::env::temp_dir().join(format!("renzora_rig_rvg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rope.rvg");
        std::fs::write(&path, renzora_rvg::bake::build_model(&source).unwrap().to_bytes()).unwrap();

        rig_file(&path, |glb| {
            let (start, end) = chain::long_axis(glb)?;
            chain::build(glb, &chain::ChainSpec { start, end, segments: 4 })
        })
        .unwrap();

        let rigged = renzora_rvg::RvgFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        assert!(rigged.scene.is_some(), "still a model .rvg");
        assert!(
            rigged.meshes[0].attributes.has(renzora_rvg::Attributes::SKIN),
            "the clusters carry no joints or weights"
        );
        let glb = Glb::parse(&read_model(&path).unwrap()).unwrap();
        assert!(glb.json["skins"].as_array().is_some_and(|s| !s.is_empty()), "the scene has no skin");
        assert!(unrigged_path(&path).exists(), "the original was not kept");
        assert!(unrigged_path(&path).to_string_lossy().ends_with(".unrigged.rvg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A model file's bytes as a GLB at full detail.
///
/// Every tool here edits a GLB. A model stored as `.rvg` (the form imported
/// models take) is read as the full-detail GLB its scene and clusters make
/// together, so rigging, weight painting and blend shapes work on it exactly
/// as on a `.glb`; [`write_model`] stores the result back as `.rvg`.
pub fn read_model(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    if !is_rvg(path) {
        return Ok(bytes);
    }
    let file = renzora_rvg::RvgFile::from_bytes(&bytes)?;
    Ok(renzora_rvg::bake::full_detail_glb(&file)?)
}

/// Store an edited GLB as the model file at `path`: as-is for a `.glb`, built
/// into an `.rvg` (scene plus virtual geometry) for one. Written to a
/// temporary file first and renamed over, so a failure never leaves half a
/// model.
pub fn write_model(path: &std::path::Path, glb: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = if is_rvg(path) {
        renzora_rvg::bake::build_model(glb)?.to_bytes()
    } else {
        glb.to_vec()
    };
    let tmp = path.with_extension("model.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Rig a model file in place, keeping the original beside it as
/// `<stem>.unrigged.glb` the first time.
///
/// The model is rewritten rather than saved as a copy so every scene using it
/// gets the rig without being changed. The untouched original is the way back,
/// and it is never overwritten: rigging again starts from the original, so a
/// face can be re-rigged with better points without stacking a second rig on
/// the first.
pub fn rig_file<T>(
    path: &std::path::Path,
    build: impl FnOnce(&mut Glb) -> Result<T, GlbError>,
) -> Result<T, Box<dyn std::error::Error>> {
    let original = unrigged_path(path);
    if !original.exists() {
        std::fs::copy(path, &original)?;
    }
    let bytes = read_model(&original)?;
    let mut glb = Glb::parse(&bytes)?;
    let out = build(&mut glb)?;
    write_model(path, &glb.to_bytes())?;
    Ok(out)
}

/// Write painted or generated weights into a model file: one
/// `(mesh, primitive, influences)` per primitive changed, the influences in
/// the primitive's own vertex order.
///
/// The file is changed in place, like [`rig_file`], so every scene using the
/// model gets the new weights. It refuses a primitive whose vertex count is not
/// the file's, which is how weights painted on a mesh the engine rebuilt on
/// load (one it gave flat normals, say, by splitting its vertices) are kept
/// from landing on the wrong vertices. The superseded weight data stays in the
/// file, unreferenced; rigging again from the original drops it.
pub fn write_weights(path: &std::path::Path, edits: &[(usize, usize, Vec<weights::Influences>)]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = read_model(path)?;
    let mut glb = Glb::parse(&bytes)?;
    for (mesh, prim, influences) in edits {
        let count = glb.positions(*mesh, *prim)?.len();
        if count != influences.len() {
            return Err(Box::new(GlbError::Unsupported(format!(
                "mesh {mesh} primitive {prim} has {count} vertices in the file but {} were painted",
                influences.len()
            ))));
        }
        glb.set_skin_attributes(*mesh, *prim, influences);
    }
    write_model(path, &glb.to_bytes())
}

/// Where a model's pre-rig original is kept: `<stem>.unrigged.glb` beside a
/// `.glb`, `<stem>.unrigged.rvg` beside an `.rvg`.
pub fn unrigged_path(path: &std::path::Path) -> std::path::PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    let ext = if is_rvg(path) { "rvg" } else { "glb" };
    path.with_file_name(format!("{stem}.unrigged.{ext}"))
}

/// Put the original back, removing the rig.
pub fn unrig_file(path: &std::path::Path) -> std::io::Result<bool> {
    let original = unrigged_path(path);
    if !original.exists() {
        return Ok(false);
    }
    std::fs::copy(&original, path)?;
    std::fs::remove_file(&original)?;
    Ok(true)
}
