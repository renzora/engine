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
//!   points; [`expressions`]: what its sliders do, and natural blinks.
//! - [`humanoid`]: a Mixamo-compatible body skeleton from seven points.
//! - [`mirror`]: left and right; [`ik`]: two-bone reaching.
//! - [`weights`]: weights for a skeleton that is already there, generated
//!   and painted.

pub mod expressions;
pub mod face;
pub mod glb;
pub mod humanoid;
pub mod ik;
pub mod mirror;
pub mod skin;
pub mod weights;

pub use glb::{Glb, GlbError};

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
    let bytes = std::fs::read(&original)?;
    let mut glb = Glb::parse(&bytes)?;
    let out = build(&mut glb)?;
    let tmp = path.with_extension("rigging.tmp");
    std::fs::write(&tmp, glb.to_bytes())?;
    std::fs::rename(&tmp, path)?;
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
    let bytes = std::fs::read(path)?;
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
    let tmp = path.with_extension("weights.tmp");
    std::fs::write(&tmp, glb.to_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Where a model's pre-rig original is kept.
pub fn unrigged_path(path: &std::path::Path) -> std::path::PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    path.with_file_name(format!("{stem}.unrigged.glb"))
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
