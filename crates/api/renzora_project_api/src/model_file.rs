//! Building the engine's model file (`.rvg`) from a GLB, for code that cannot
//! link the builder.
//!
//! An `.rvg` model is a model's scene plus its virtual geometry in one file,
//! and is what an import stores. The builder is `renzora_rvg`'s `bake`
//! feature, which compiles meshoptimizer and is linked only by the editor, so
//! a plugin that writes models of its own (the Blender live link) would
//! otherwise have to store them as `.glb`, and the project would hold that
//! `.glb` beside an `.rvg` holding only its virtual geometry: two files for
//! one model, where an import makes one.
//!
//! So the editor installs the builder here at startup, and anything reaching
//! the contract calls [`build_model_file`]. A static rather than a resource
//! because a build runs off the main thread, where a plugin has no world; it
//! is singular because this crate lives in the shared `renzora_dylib` image.

use std::sync::OnceLock;

/// GLB bytes in, `.rvg` bytes out.
pub type ModelFileBuilder = fn(&[u8]) -> Result<Vec<u8>, String>;

static BUILDER: OnceLock<ModelFileBuilder> = OnceLock::new();

/// Install the builder. Called once by the editor; later calls are ignored.
pub fn install_model_file_builder(builder: ModelFileBuilder) {
    let _ = BUILDER.set(builder);
}

/// Build an `.rvg` model from a GLB, as an import does.
///
/// `None` when this build has no builder (a shipped game, the web editor):
/// keep the `.glb` then, which loads as a model just the same. `Some(Err)`
/// when the GLB could not be built into one, which an import also answers by
/// keeping the `.glb`. Can take seconds for a dense model; call it off the
/// main thread.
pub fn build_model_file(glb: &[u8]) -> Option<Result<Vec<u8>, String>> {
    BUILDER.get().map(|build| build(glb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_builder_installed_is_the_one_used() {
        install_model_file_builder(|glb| Ok(glb.iter().rev().copied().collect()));
        install_model_file_builder(|_| Err("never used".into()));
        assert_eq!(build_model_file(&[1, 2, 3]), Some(Ok(vec![3, 2, 1])));
    }
}
