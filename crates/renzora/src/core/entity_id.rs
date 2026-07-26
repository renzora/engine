//! Canonical entity identifiers.
//!
//! One identity per entity, stored in its `Name`: unique across the scene and
//! restricted to `[a-z0-9_-]` (lowercase, no spaces; `_` and `-` allowed as
//! separators). This replaces the old split of a duplicable `Name` *plus* a
//! separate unique [`EntityTag`], which two concepts this collapses into one.
//!
//! Why snake_case specifically — this is the load-bearing reason, don't loosen
//! it: dotted binding / reflection paths (`{{ player.health }}`,
//! `set_on("turret.Transform.translation", …)`, `toggles="pause_menu"`)
//! disambiguate their *first* segment by asking "is this a registered component
//! type?". Every component short type-path is PascalCase (`Transform`,
//! `PointLight`, `WorldEnvironment`). A lowercase snake_case id can never match
//! one, so an entity id is never mistaken for a component. Allowing capitals
//! would reintroduce that ambiguity (`Camera` the id vs `Camera` the component).
//!
//! [`EntityTag`]: crate::core::components::EntityTag

use bevy::prelude::*;

/// Coerce arbitrary text into the id charset `[a-z0-9_-]`. Lowercases; keeps
/// authored `_`/`-` separators; turns every run of other characters (spaces,
/// punctuation) into a single `_`; drops leading/trailing separators; and
/// guarantees a non-empty result that never starts with a digit (so it's always
/// a valid path segment):
///
/// - `"Directional Light"` → `"directional_light"`
/// - `"Camera-3D"`         → `"camera-3d"`
/// - `"Player #2!"`        → `"player_2"`
/// - `"  weird__name  "`   → `"weird__name"`
/// - `""`                  → `"entity"`
/// - `"123abc"`            → `"n_123abc"`
pub fn sanitize_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    // Emit a `_` separator lazily for runs of junk: only once a real character
    // has already landed, so we never get a leading or trailing separator. An
    // authored `_`/`-` is kept verbatim instead of becoming a `_`.
    let mut pending_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('_');
            }
            pending_sep = false;
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '_' || ch == '-') && !out.is_empty() {
            // A literal separator the author wrote — keep it, and it satisfies
            // any pending junk-run separator too.
            pending_sep = false;
            out.push(ch);
        } else {
            pending_sep = true;
        }
    }
    // Trailing separators (from a literal `_`/`-` at the very end) leave nothing
    // useful; strip them.
    while out.ends_with('_') || out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return "entity".to_string();
    }
    // A leading digit would read oddly as a path segment (`1foo.bar`); prefix it.
    if out.as_bytes()[0].is_ascii_digit() {
        return format!("n_{out}");
    }
    out
}

/// Return `desired` if `is_taken(&desired)` is false, else append `_1`, `_2`, …
/// until free — so the base id stays unnumbered and the *first* duplicate is
/// `_1`. `desired` must already be [`sanitize_id`]-clean. `is_taken` decides
/// collisions — typically "some *other* entity already has this id".
pub fn unique_id(desired: &str, mut is_taken: impl FnMut(&str) -> bool) -> String {
    if !is_taken(desired) {
        return desired.to_string();
    }
    // Strip an existing numeric suffix so re-uniquifying `light_1` yields
    // `light_2`, not `light_1_1`.
    let base = strip_numeric_suffix(desired);
    let mut n = 1u32;
    loop {
        let candidate = format!("{base}_{n}");
        if !is_taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// `light_2` → `light`; `light` → `light`; `a_1b` → `a_1b` (suffix must be all
/// digits).
fn strip_numeric_suffix(s: &str) -> &str {
    if let Some(pos) = s.rfind('_') {
        let tail = &s[pos + 1..];
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
            return &s[..pos];
        }
    }
    s
}

/// Sanitize `desired` and make it unique among every *other* entity's `Name` in
/// the world. Use when spawning or renaming: pass the entity being (re)named as
/// `self_entity` so keeping its own current id doesn't count as a collision.
pub fn unique_entity_name(world: &mut World, desired: &str, self_entity: Entity) -> String {
    let clean = sanitize_id(desired);
    let mut taken: bevy::platform::collections::HashSet<String> = Default::default();
    let mut q = world.query::<(Entity, &Name)>();
    for (e, name) in q.iter(world) {
        if e != self_entity {
            taken.insert(name.as_str().to_string());
        }
    }
    unique_id(&clean, |id| taken.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_to_id_charset() {
        assert_eq!(sanitize_id("Directional Light"), "directional_light");
        assert_eq!(sanitize_id("Player #2!"), "player_2");
        assert_eq!(sanitize_id(""), "entity");
        assert_eq!(sanitize_id("   "), "entity");
        assert_eq!(sanitize_id("123abc"), "n_123abc");
        assert_eq!(sanitize_id("already_clean"), "already_clean");
        // Dashes are allowed and kept; authored `_`/`-` survive verbatim.
        assert_eq!(sanitize_id("Camera-3D"), "camera-3d");
        assert_eq!(sanitize_id("weird__name"), "weird__name");
        // Leading/trailing separators are trimmed.
        assert_eq!(sanitize_id("-foo-"), "foo");
        // No separators in the input → one lowercased run.
        assert_eq!(sanitize_id("WorldEnvironment"), "worldenvironment");
    }

    #[test]
    fn dedupes_first_duplicate_as_1() {
        // Only the base exists → first duplicate is `_1` (not `_2`).
        let one = ["camera_3d"];
        assert_eq!(
            unique_id("camera_3d", |s| one.iter().any(|t| *t == s)),
            "camera_3d_1"
        );
        // Base + `_1` exist → next is `_2`.
        let two = ["camera_3d", "camera_3d_1"];
        let has = |s: &str| two.iter().any(|t| *t == s);
        assert_eq!(unique_id("camera_3d", has), "camera_3d_2");
        assert_eq!(unique_id("fresh", has), "fresh");
        // Already-suffixed desired strips before re-numbering.
        assert_eq!(unique_id("camera_3d_1", has), "camera_3d_2");
    }
}
