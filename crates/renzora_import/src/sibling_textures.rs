//! Finding the texture set a geometry-only model shipped next to it.
//!
//! STL, and to a lesser extent PLY, store geometry and nothing else — no
//! materials, no texture references. Asset packs work around that by dropping a
//! `textures/` folder beside the model and leaving you to wire it up by hand.
//! This module finds those files and groups them into **sets**, so the importer
//! can offer a choice rather than either ignoring them or guessing.
//!
//! Guessing is the thing to avoid. Real packs routinely ship several competing
//! sets in one folder — one has two full PBR sets for a single rifle
//! (`KSR29sniperrifle_*` and `Sniper_KSR_29_*`), another has five surface
//! materials shared across ten different buildings. Auto-binding "the base
//! colour" would silently pick one of four in the second case and be wrong most
//! of the time, which is worse than doing nothing, because it looks deliberate.
//!
//! What this module will not tell you is whether a texture *suits* the model.
//! A map baked for a specific UV unwrap cannot work on a format that stores no
//! UVs, and detecting that from the image is unreliable — an edge-continuity
//! test read a packed atlas as seamless often enough to be useless. The
//! importer says the true thing instead: these coordinates were projected, so a
//! baked map will not line up. See `stl::box_project_uvs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which PBR slot a discovered file fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MapRole {
    BaseColor,
    Normal,
    Roughness,
    Metallic,
    Occlusion,
    Specular,
    Emissive,
    Opacity,
}

impl MapRole {
    /// Short label for the inspector.
    pub fn label(self) -> &'static str {
        match self {
            MapRole::BaseColor => "base color",
            MapRole::Normal => "normal",
            MapRole::Roughness => "roughness",
            MapRole::Metallic => "metallic",
            MapRole::Occlusion => "occlusion",
            MapRole::Specular => "specular",
            MapRole::Emissive => "emissive",
            MapRole::Opacity => "opacity",
        }
    }
}

/// One texture file, with the role its name implies.
#[derive(Debug, Clone)]
pub struct SiblingMap {
    pub path: PathBuf,
    pub role: MapRole,
}

/// A group of texture files that name the same material.
#[derive(Debug, Clone)]
pub struct TextureSet {
    /// The shared prefix the files were grouped under, e.g. `KSR29sniperrifle`.
    /// Doubles as the identifier [`ImportSettings::texture_set`] stores.
    pub stem: String,
    /// One entry per role, role-ordered, at most one file per role.
    pub maps: Vec<SiblingMap>,
}

impl TextureSet {
    pub fn get(&self, role: MapRole) -> Option<&Path> {
        self.maps
            .iter()
            .find(|m| m.role == role)
            .map(|m| m.path.as_path())
    }

    /// `base color, normal, roughness` — the inspector's one-line summary.
    pub fn role_summary(&self) -> String {
        self.maps
            .iter()
            .map(|m| m.role.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Image extensions worth considering as a texture.
const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "tga", "bmp", "tif", "tiff", "dds", "exr", "hdr", "webp", "ktx2",
];

/// Folder names that conventionally hold a model's textures. Matched
/// case-insensitively against real directory entries, so `Textures/` on a
/// case-sensitive filesystem is found too.
const TEXTURE_DIRS: &[&str] = &["textures", "texture", "tex", "maps", "materials"];

/// Role suffixes, longest first within each role so `base_color` is tried
/// before `col` and `normal_opengl` before `nor`.
///
/// Single-letter suffixes (`_c`, `_n`, `_s`) are a real and common convention —
/// this is how the `Steel_C` / `Steel_N` / `Steel_S` sets in the wild are named
/// — but they are only accepted after a separator, so a file ending in `_1`
/// or a name like `Residential Buildings 001` cannot be mistaken for one.
const ROLE_SUFFIXES: &[(MapRole, &[&str])] = &[
    (
        MapRole::Occlusion,
        &["ambientocclusion", "ambient_occlusion", "occlusion", "ao"],
    ),
    (
        MapRole::BaseColor,
        &[
            "basecolor",
            "base_color",
            "basecolour",
            "albedo",
            "diffuse",
            "colour",
            "color",
            "diff",
            "col",
            "c",
            "d",
        ],
    ),
    (
        MapRole::Normal,
        &[
            "normal_opengl",
            "normal_directx",
            "normalgl",
            "normaldx",
            "normal",
            "norm",
            "nrm",
            "nor",
            "n",
        ],
    ),
    (
        MapRole::Roughness,
        &["roughness", "rough", "rgh", "r"],
    ),
    (
        MapRole::Metallic,
        &["metallic", "metalness", "metal", "mtl", "m"],
    ),
    (
        MapRole::Specular,
        &["specularity", "specular", "spec", "s"],
    ),
    (
        MapRole::Emissive,
        &["emissive", "emission", "emit", "e"],
    ),
    (
        MapRole::Opacity,
        &["transparency", "opacity", "alpha"],
    ),
];

/// Split a file stem into `(set stem, role)` if its tail names a PBR slot.
///
/// Returns `None` for a file whose name carries no role — an environment map
/// like `SKY.jpg` or a reference sheet like `REF 1.jpg` sitting in the same
/// folder should not invent a set of its own.
fn classify(stem: &str) -> Option<(String, MapRole)> {
    let lower = stem.to_lowercase();
    let mut best: Option<(usize, MapRole)> = None;
    for (role, suffixes) in ROLE_SUFFIXES {
        for suffix in *suffixes {
            let Some(head) = lower.strip_suffix(suffix) else {
                continue;
            };
            // The suffix has to be its own token. Without this, `Grass_col`
            // would be fine but so would any word merely *ending* in the
            // letters — `nor` would claim `manor`, and `c` would claim
            // essentially everything.
            if !head.is_empty() && !head.ends_with(['_', '-', '.', ' ']) {
                continue;
            }
            // Prefer the longest match so `base_color` beats a trailing `r`.
            if best.is_none_or(|(len, _)| suffix.len() > len) {
                best = Some((suffix.len(), *role));
            }
        }
    }
    let (len, role) = best?;
    let head = &stem[..stem.len() - len];
    let head = head.trim_end_matches(['_', '-', '.', ' ']);
    if head.is_empty() {
        return None;
    }
    Some((head.to_string(), role))
}

/// Merge a stem into an existing group when one is a prefix of the other.
///
/// Real names are messier than a clean `<name>_<role>`: the same rifle ships
/// `KSR29sniperrifle_Base_Color` alongside
/// `KSR29sniperrifle_low_Material.005_AmbientOcclusion`, which strips to a
/// longer stem and would otherwise become a second, one-map set. Requiring the
/// remainder to begin with a separator keeps `Steel` from swallowing an
/// unrelated `Steelworks`.
fn merge_key(stems: &[String], candidate: &str) -> Option<String> {
    let cand_lower = candidate.to_lowercase();
    stems.iter().find_map(|existing| {
        let ex_lower = existing.to_lowercase();
        if ex_lower.len() < 3 || cand_lower.len() < 3 {
            return None;
        }
        let (short, long) = if ex_lower.len() <= cand_lower.len() {
            (&ex_lower, &cand_lower)
        } else {
            (&cand_lower, &ex_lower)
        };
        let rest = long.strip_prefix(short.as_str())?;
        (rest.is_empty() || rest.starts_with(['_', '-', '.', ' ']))
            .then(|| if ex_lower.len() <= cand_lower.len() { existing.clone() } else { candidate.to_string() })
    })
}

/// True for a format that stores geometry and nothing else, so a sibling
/// texture set is the only way it can be textured.
///
/// Deliberately narrow. A format that names its own textures must never have a
/// folder full of guesses bound over the top of what the file actually said.
pub fn is_geometry_only(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("stl"))
}

/// Find the texture sets sitting beside `model_path`.
///
/// Looks in the conventional `textures/`-style subfolders first and the model's
/// own folder second, so a pack that puts both a model and its maps in one
/// directory still works. Results are ordered by set name for stability — the
/// chosen set is stored by name, so a reshuffle between runs would silently
/// rebind a different set.
pub fn discover(model_path: &Path) -> Vec<TextureSet> {
    let Some(dir) = model_path.parent() else {
        return Vec::new();
    };

    let mut search = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if TEXTURE_DIRS.contains(&name.as_str()) {
                search.push(entry.path());
            }
        }
    }
    search.sort();
    // The model's own folder last: a dedicated `textures/` is the stronger
    // signal, and a duplicate role there should not displace it.
    search.push(dir.to_path_buf());

    // stem → role → path. `BTreeMap` so both levels come out ordered.
    let mut groups: BTreeMap<String, BTreeMap<MapRole, PathBuf>> = BTreeMap::new();
    for folder in &search {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
            })
            .collect();
        // Directory order is filesystem-dependent; sort so grouping is stable.
        files.sort();

        for path in files {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some((set_stem, role)) = classify(stem) else {
                continue;
            };
            let keys: Vec<String> = groups.keys().cloned().collect();
            let key = merge_key(&keys, &set_stem).unwrap_or_else(|| set_stem.clone());
            // A merge can shorten the key an existing group is filed under.
            if key != set_stem {
                if let Some(existing) = groups.remove(&set_stem) {
                    groups.entry(key.clone()).or_default().extend(existing);
                }
            }
            groups
                .entry(key)
                .or_default()
                .entry(role)
                // First writer wins, which is why `textures/` is searched first.
                .or_insert(path);
        }
    }

    groups
        .into_iter()
        .map(|(stem, maps)| TextureSet {
            stem,
            maps: maps
                .into_iter()
                .map(|(role, path)| SiblingMap { path, role })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_a_full_pbr_name() {
        assert_eq!(
            classify("KSR29sniperrifle_Base_Color"),
            Some(("KSR29sniperrifle".into(), MapRole::BaseColor))
        );
        assert_eq!(
            classify("KSR29sniperrifle_Normal_OpenGL"),
            Some(("KSR29sniperrifle".into(), MapRole::Normal))
        );
        assert_eq!(
            classify("KSR29sniperrifle_Roughness"),
            Some(("KSR29sniperrifle".into(), MapRole::Roughness))
        );
    }

    #[test]
    fn classifies_single_letter_conventions() {
        // `Steel_C` / `_N` / `_S` is how a real pack names its maps.
        assert_eq!(
            classify("Steel_C"),
            Some(("Steel".into(), MapRole::BaseColor))
        );
        assert_eq!(classify("Steel_N"), Some(("Steel".into(), MapRole::Normal)));
        assert_eq!(
            classify("Steel_S"),
            Some(("Steel".into(), MapRole::Specular))
        );
        assert_eq!(classify("Box_D"), Some(("Box".into(), MapRole::BaseColor)));
    }

    #[test]
    fn a_suffix_must_be_its_own_token() {
        // Without the separator rule these would match `nor`, `c` and `s`, and
        // every unrelated image in the folder would invent a set.
        assert_eq!(classify("manor"), None);
        assert_eq!(classify("SKY"), None);
        assert_eq!(classify("Reflexion"), None);
    }

    #[test]
    fn a_bare_role_name_is_not_a_set() {
        // `normal.png` on its own has no material name to group under.
        assert_eq!(classify("normal"), None);
        assert_eq!(classify("_ao"), None);
    }

    #[test]
    fn longest_suffix_wins() {
        // `_AmbientOcclusion` must not be read as a trailing `n` for normal.
        let (_, role) = classify("Hotel_Hous_AmbientOcclusion").expect("classified");
        assert_eq!(role, MapRole::Occlusion);
    }

    #[test]
    fn a_longer_stem_merges_into_its_prefix() {
        // The rifle's AO map strips to a longer stem than its siblings; left
        // alone it would show up as a second one-map set.
        let existing = vec!["KSR29sniperrifle".to_string()];
        assert_eq!(
            merge_key(&existing, "KSR29sniperrifle_low_Material.005"),
            Some("KSR29sniperrifle".to_string())
        );
    }

    #[test]
    fn unrelated_stems_do_not_merge() {
        let existing = vec!["KSR29sniperrifle".to_string()];
        assert_eq!(merge_key(&existing, "Sniper_KSR_29"), None);
        // A prefix that isn't followed by a separator is a different word.
        assert_eq!(merge_key(&["Steel".to_string()], "Steelworks"), None);
    }

    #[test]
    fn role_summary_lists_what_the_set_fills() {
        let set = TextureSet {
            stem: "Steel".into(),
            maps: vec![
                SiblingMap {
                    path: "Steel_C.jpg".into(),
                    role: MapRole::BaseColor,
                },
                SiblingMap {
                    path: "Steel_N.jpg".into(),
                    role: MapRole::Normal,
                },
            ],
        };
        assert_eq!(set.role_summary(), "base color, normal");
        assert_eq!(set.get(MapRole::Normal), Some(Path::new("Steel_N.jpg")));
        assert_eq!(set.get(MapRole::Roughness), None);
    }
}
