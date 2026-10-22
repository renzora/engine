//! `.particle` (the retired Hanabi system) to `.vfx` (this one), in memory.
//!
//! The old format is a flat list of fields, one per feature
//! ([`format::HanabiEffectDefinition`]); the new one is a node graph per
//! emitter. The conversion builds the graph those fields describe ([`map`]),
//! lays it out so it reads well in the graph view ([`build`]), and lists
//! everything it could not carry over as a [`Warning`]. Nothing is dropped
//! without one, and [`vfx_text`] writes the warnings into the head of a `.vfx`
//! as comments too, so they survive the console scrolling away.
//!
//! ## Why this is in the runtime crate
//!
//! It used to live with the editor, which was enough while Hanabi still
//! shipped: an old effect played through Hanabi and the converter was an
//! editor convenience. With Hanabi gone, a shipped game built from a project
//! nobody reopened in the editor still holds `.particle` files and scenes that
//! name them, and the only thing left that can play them is this conversion.
//! So the pure part (parse, map, lay out) is here, used by the `.particle`
//! asset loader and the scene migration in [`crate::legacy`]; the editor keeps
//! only what touches the user's files (writing a `.vfx` beside the old one).
//!
//! ## Wind
//!
//! The old format has no wind at all (its only weather-like setting is noise
//! turbulence, which maps to Curl Noise), so nothing is converted to a
//! `force/wind` and no warning is due: a converted effect ignores the world
//! wind, exactly as it did under Hanabi, until a Wind Force is wired in. Its
//! `erosion` switch does map, to the render erosion.

pub mod build;
pub mod format;
pub mod map;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::asset::{write_vfx, VfxAsset};
use format::HanabiEffectDefinition;

pub use map::convert_definition;

/// The old effect file's extension, without the dot.
pub const PARTICLE_EXTENSION: &str = "particle";

/// One thing a conversion could not carry over, or only approximated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// Short name of the feature, e.g. `kill zones`.
    pub feature: String,
    /// What happened to it, and what to do instead when there is something.
    pub detail: String,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.feature, self.detail)
    }
}

/// A converted effect and its warnings.
#[derive(Clone, Debug)]
pub struct Conversion {
    pub asset: VfxAsset,
    pub warnings: Vec<Warning>,
}

/// Parse a `.particle` document. Strict, unlike the Hanabi editor's loader,
/// which fell back to a default effect on a parse error: converting that
/// default would produce a plausible-looking effect that has nothing to do
/// with the file.
pub fn parse_particle(text: &str) -> Result<HanabiEffectDefinition, String> {
    ron::from_str::<HanabiEffectDefinition>(text).map_err(|e| format!("not a valid .particle file: {e}"))
}

/// Parse and convert a `.particle` document. An empty effect name is filled
/// from `fallback_name` (the file's stem), so an editor has something to show.
pub fn convert_text(text: &str, fallback_name: &str) -> Result<Conversion, String> {
    let mut def = parse_particle(text)?;
    if def.name.trim().is_empty() {
        def.name = fallback_name.to_string();
    }
    Ok(convert_definition(&def))
}

/// The `.vfx` text for a conversion: a comment header naming the source and
/// the warnings, then the effect. RON allows `//` comments, and the loader
/// skips them.
pub fn vfx_text(conversion: &Conversion, source_name: &str) -> Result<String, String> {
    let body = write_vfx(&conversion.asset).map_err(|e| format!("could not write the effect: {e}"))?;
    let mut text = format!("// Converted from {source_name} (the old particle system).\n");
    if !conversion.warnings.is_empty() {
        text.push_str("// Not carried over, or approximated:\n");
        for w in &conversion.warnings {
            text.push_str(&format!("//   - {w}\n"));
        }
    }
    text.push_str(&body);
    text.push('\n');
    Ok(text)
}

/// Whether `path` names an old effect file (by extension, any case).
pub fn is_particle_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(PARTICLE_EXTENSION))
}

/// The plain sibling name: `fire.particle` gives `fire.vfx`. This is the one
/// name the editor's automatic conversion writes, and the one the scene
/// migration looks for, so the two agree on which `.vfx` belongs to which
/// `.particle` without any record kept between them.
pub fn sibling_vfx(particle: &Path) -> PathBuf {
    particle.with_extension("vfx")
}

/// [`sibling_vfx`] for a scene's asset path, which is a `/`-separated string
/// rather than a filesystem path, and must stay one.
pub fn sibling_vfx_str(particle: &str) -> String {
    match particle.rfind('.') {
        Some(dot) if !particle[dot..].contains(['/', '\\']) => format!("{}.vfx", &particle[..dot]),
        _ => format!("{particle}.vfx"),
    }
}

#[cfg(test)]
mod tests;
