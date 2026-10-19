//! Pointing an entity at a `.material` file, and saying when that pointer has
//! been resolved.

// For the `ReflectComponent` / `ReflectSerialize` / `ReflectDeserialize` types
// the `#[reflect(..)]` attribute below expands to by bare name.
use bevy::prelude::*;

/// Reference to a material file. Add to any entity with `Mesh3d` to assign a material.
#[derive(
    bevy::prelude::Component,
    serde::Serialize,
    serde::Deserialize,
    bevy::prelude::Reflect,
    Clone,
    Debug,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MaterialRef(pub String);

/// What a material file can be worn by, read from the file itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialTarget {
    /// A 3D mesh (`Mesh3d`): every `.material` domain but Sprite, and a
    /// hand-written `.wgsl` / `.wesl` code shader.
    Mesh,
    /// A 2D sprite: a Sprite-domain `.material`.
    Sprite,
}

impl MaterialTarget {
    /// Read a material file's target, or `None` when it cannot be read or is
    /// not a material. For the editor's drop targets, which have to refuse a
    /// sprite material over a mesh before the drop rather than after, and
    /// which must not link the shader crate to find out.
    ///
    /// Only the graph's `domain` field is looked at, so this stays the same if
    /// the rest of the format grows.
    pub fn of_file(path: &std::path::Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "wgsl" | "wesl" => Some(Self::Mesh),
            "material" => {
                #[derive(serde::Deserialize)]
                struct DomainOnly {
                    #[serde(default)]
                    domain: Option<String>,
                }
                let text = std::fs::read_to_string(path).ok()?;
                let parsed: DomainOnly = serde_json::from_str(&text).ok()?;
                Some(if parsed.domain.as_deref() == Some("Sprite") { Self::Sprite } else { Self::Mesh })
            }
            _ => None,
        }
    }

    /// Whether an entity with these components can wear a material of this
    /// target.
    pub fn fits(self, has_mesh: bool, has_sprite: bool) -> bool {
        match self {
            Self::Mesh => has_mesh,
            Self::Sprite => has_sprite,
        }
    }
}

/// Marker added by the material resolver once a [`MaterialRef`] has been loaded,
/// compiled and attached. Removing it is how *any* crate says "this entity's
/// material changed, resolve it again" — which is why the marker lives here and
/// not in `renzora_shader`: the editor panels that rebind a material (the
/// hierarchy's drag-to-attach, the material inspector, the viewport drop) would
/// otherwise each have to link the whole shader crate for one component.
#[derive(bevy::prelude::Component)]
pub struct MaterialResolved {
    /// The `MaterialRef` path this entity was resolved from.
    pub source_path: String,
}

/// Per-entity override of the alpha mode a resolved [`MaterialRef`] renders
/// with.
///
/// A `.material` file declares one alpha mode for every mesh that wears it,
/// and for a mesh that is right: transparency is part of the look the author
/// signed off on. For an *overlay* it is wrong. A terrain paint layer wears
/// the same rock or water material a cliff or a lake does, but its edges have
/// to fade out through the per-vertex coverage alpha the overlay mesh carries,
/// whatever the material itself says about transparency. Without this the only
/// options are editing the material (which changes it everywhere it is used)
/// or keeping a near-duplicate `.material` per overlay.
///
/// The resolver honours it by cloning the resolved asset, never by touching
/// the cached master, so a second alpha mode costs one asset and no recompile.
#[derive(bevy::prelude::Component, Clone, Copy, Debug, PartialEq)]
pub struct MaterialAlphaOverride {
    pub mode: super::components::PbrAlphaMode,
    /// Discard threshold for [`super::components::PbrAlphaMode::Mask`].
    /// Ignored in the other modes.
    pub cutoff: f32,
}

impl MaterialAlphaOverride {
    /// Standard alpha blending — what an overlay wants, so its coverage
    /// feather is actually drawn.
    pub const BLEND: Self = Self {
        mode: super::components::PbrAlphaMode::Blend,
        cutoff: 0.5,
    };

    /// Hashable identity, for keying a cache of derived material variants.
    /// `f32` is not `Hash`/`Eq`, so the cutoff travels as its bit pattern.
    pub fn key(&self) -> (u8, u32) {
        use super::components::PbrAlphaMode;
        let mode = match self.mode {
            PbrAlphaMode::Opaque => 0,
            PbrAlphaMode::Mask => 1,
            PbrAlphaMode::Blend => 2,
        };
        (mode, self.cutoff.to_bits())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_material_file_says_what_it_can_be_worn_by() {
        let dir = std::env::temp_dir().join(format!("renzora_material_target_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sprite = dir.join("glow.material");
        let surface = dir.join("rock.material");
        let old = dir.join("old.material");
        std::fs::write(&sprite, r#"{"name":"Glow","domain":"Sprite","nodes":[]}"#).unwrap();
        std::fs::write(&surface, r#"{"name":"Rock","domain":"Surface","nodes":[]}"#).unwrap();
        // A file from before domains were written still loads as 3D.
        std::fs::write(&old, r#"{"name":"Old","nodes":[]}"#).unwrap();

        assert_eq!(MaterialTarget::of_file(&sprite), Some(MaterialTarget::Sprite));
        assert_eq!(MaterialTarget::of_file(&surface), Some(MaterialTarget::Mesh));
        assert_eq!(MaterialTarget::of_file(&old), Some(MaterialTarget::Mesh));
        assert_eq!(MaterialTarget::of_file(&dir.join("code.wesl")), Some(MaterialTarget::Mesh));
        assert_eq!(MaterialTarget::of_file(&dir.join("missing.material")), None);
        assert_eq!(MaterialTarget::of_file(&dir.join("tree.png")), None);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(MaterialTarget::Sprite.fits(false, true));
        assert!(!MaterialTarget::Sprite.fits(true, false));
        assert!(MaterialTarget::Mesh.fits(true, false));
        assert!(!MaterialTarget::Mesh.fits(false, true));
    }
}
