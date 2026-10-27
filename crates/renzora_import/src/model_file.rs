//! The file an imported model is stored as.
//!
//! Every source format is converted to a GLB first, because that is the one
//! form every later step of the import reads (materials, textures, animations,
//! the preview). What goes into the project is an `.rvg` built from it: the
//! model's scene plus its virtual geometry in one file (see
//! `renzora_rvg::bake::build_model`), which loads as a model exactly as the
//! GLB would and takes about half the space of the GLB and its geometry kept
//! side by side.

/// `glb` as the bytes to store and their extension: `.rvg`, or the GLB itself
/// (`.glb`) when it cannot be built into one. An import never fails for this:
/// a model the builder cannot read is kept as it was.
pub fn model_file(glb: &[u8]) -> (Vec<u8>, &'static str) {
    #[cfg(not(target_arch = "wasm32"))]
    match renzora_rvg::bake::build_model(glb) {
        Ok(file) => return (file.to_bytes(), "rvg"),
        Err(e) => renzora::bevy::log::warn!("[import] kept as .glb: could not build its .rvg: {e}"),
    }
    (glb.to_vec(), "glb")
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::glb_build::{build_glb, MaterialBundle};

    /// A model the importer converted, stored as `.rvg`: the file carries the
    /// model's scene and its virtual geometry, and is smaller than its GLB.
    #[test]
    fn an_imported_model_is_stored_as_rvg() {
        let n = 64;
        let (mut positions, mut normals, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for y in 0..=n {
            for x in 0..=n {
                let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
                positions.extend([u * 4.0 - 2.0, 0.05 * (u * 9.0).sin() * (v * 7.0).cos(), v * 4.0 - 2.0]);
                normals.extend([0.0, 1.0, 0.0]);
                uvs.extend([u, v]);
            }
        }
        for y in 0..n {
            for x in 0..n {
                let i = y * (n + 1) + x;
                indices.extend([i, i + n + 1, i + 1, i + 1, i + n + 1, i + n + 2]);
            }
        }
        let glb = build_glb(&positions, &normals, &uvs, &indices, &MaterialBundle::default()).unwrap();
        let (bytes, extension) = model_file(&glb);
        assert_eq!(extension, "rvg");
        let file = renzora_rvg::RvgFile::from_bytes(&bytes).unwrap();
        assert_eq!(file.meshes.len(), 1);
        assert!(file.scene.is_some(), "a model .rvg carries its scene");
        assert!(bytes.len() < glb.len(), "{} bytes from a {} byte GLB", bytes.len(), glb.len());
    }

    #[test]
    fn an_unreadable_model_is_kept_as_it_was() {
        let (bytes, extension) = model_file(b"not a model");
        assert_eq!(extension, "glb");
        assert_eq!(bytes, b"not a model");
    }
}
