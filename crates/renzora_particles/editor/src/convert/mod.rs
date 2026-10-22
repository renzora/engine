//! `.particle` (the retired Hanabi system) to `.vfx` (this one), on disk.
//!
//! The conversion itself (parse, map, lay out) is in the runtime crate,
//! `renzora_particles::convert`, because a shipped game has to read old files
//! too; it is re-exported here so callers have one place to look. What is left
//! here is the part only the editor does: writing a converted copy beside the
//! user's file.
//!
//! ## Files
//!
//! [`convert_beside`] writes `<name>.vfx` next to the `.particle`, and never
//! deletes or overwrites anything. When that name is taken by a different
//! effect, it writes `<name> (converted).vfx` instead (then `(converted 2)`,
//! ...); when the taken name already holds this exact conversion, it reuses it.
//! A dialog was the other choice, and it was rejected: the same call runs for a
//! drop on an inspector slot and for a whole folder at once, where a question
//! per file is the wrong shape, and a new name loses nothing the user can not
//! delete in one click. Reuse is what makes converting twice harmless.
//!
//! The automatic conversion on project open ([`crate::legacy`]) is stricter
//! still: it writes only the plain `<name>.vfx`, and only when nothing is
//! there, so it never adds a second copy of anything.

use std::path::{Path, PathBuf};

pub use renzora_particles::convert::{
    build, convert_definition, convert_text, format, is_particle_path, map, parse_particle, sibling_vfx,
    sibling_vfx_str, vfx_text, Conversion, Warning, PARTICLE_EXTENSION,
};

/// Read and convert a `.particle` file. An empty effect name is filled from the
/// file name, so the editor has something to show.
pub fn convert_file(path: &Path) -> Result<Conversion, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    convert_text(&text, &stem(path))
}

fn stem(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("effect").to_string()
}

/// Every name [`convert_beside`] may use, in the order it tries them.
fn candidates(particle: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    let dir = particle.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = stem(particle);
    (0..100).map(move |i| match i {
        0 => dir.join(format!("{stem}.vfx")),
        1 => dir.join(format!("{stem} (converted).vfx")),
        n => dir.join(format!("{stem} (converted {n}).vfx")),
    })
}

/// What [`write_beside`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A new file was written.
    Written,
    /// A file with exactly this conversion was already there.
    Reused,
}

/// A conversion on disk.
#[derive(Clone, Debug)]
pub struct Converted {
    pub path: PathBuf,
    pub outcome: Outcome,
    pub warnings: Vec<Warning>,
}

/// Write `conversion` next to `particle` under the first free name, or reuse
/// one that already holds it. See the module docs.
pub fn write_beside(particle: &Path, conversion: &Conversion) -> Result<Converted, String> {
    let source = particle.file_name().and_then(|n| n.to_str()).unwrap_or("a .particle file").to_string();
    write_beside_as(particle, &source, conversion)
}

/// [`write_beside`], naming the source in the file's header as `source`. For
/// an effect that never had a file of its own (one stored inside a scene),
/// where `particle` is only the name the `.vfx` is derived from.
pub fn write_beside_as(particle: &Path, source: &str, conversion: &Conversion) -> Result<Converted, String> {
    let text = vfx_text(conversion, source)?;
    for path in candidates(particle) {
        match std::fs::read_to_string(&path) {
            Ok(existing) if existing == text => {
                return Ok(Converted { path, outcome: Outcome::Reused, warnings: conversion.warnings.clone() });
            }
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::write(&path, &text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
                return Ok(Converted { path, outcome: Outcome::Written, warnings: conversion.warnings.clone() });
            }
            // Something is there that cannot be read (a folder, a locked
            // file): it is not ours to replace, so try the next name.
            Err(_) => continue,
        }
    }
    Err(format!("no free name for the converted copy of {source}"))
}

/// Convert a `.particle` file and write it beside itself.
pub fn convert_beside(particle: &Path) -> Result<Converted, String> {
    let conversion = convert_file(particle)?;
    write_beside(particle, &conversion)
}

/// Write the plain `<name>.vfx` beside `particle`, but only if nothing at all
/// is there. `Ok(None)` when something is: an earlier conversion, or a file
/// the user made, and either way not this function's to touch. Used by the
/// project-open pass, which must never create a second copy or replace one.
pub fn convert_if_missing(particle: &Path) -> Result<Option<Converted>, String> {
    let target = sibling_vfx(particle);
    if target.exists() {
        return Ok(None);
    }
    let conversion = convert_file(particle)?;
    let name = particle.file_name().and_then(|n| n.to_str()).unwrap_or("a .particle file").to_string();
    let text = vfx_text(&conversion, &name)?;
    // `create_new` so a file that appeared since the check above (another
    // editor, a sync client) is left alone rather than overwritten.
    use std::io::Write as _;
    let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&target) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(None),
        Err(e) => return Err(format!("could not write {}: {e}", target.display())),
    };
    file.write_all(text.as_bytes()).map_err(|e| format!("could not write {}: {e}", target.display()))?;
    Ok(Some(Converted { path: target, outcome: Outcome::Written, warnings: conversion.warnings }))
}

#[cfg(test)]
mod tests;
