//! The curated icon set offered by the inspector's entity-icon picker, and the
//! lookup that turns a stored [`renzora::EntityIcon`] string back into a name
//! the UI can draw.
//!
//! Why a curated list rather than the whole Phosphor font: the font ships
//! thousands of glyphs, and a picker over all of them needs a search box, a
//! virtualized grid, and a decision from the user about what "cube" is called
//! this week. Forty-eight icons chosen for the things people actually label in
//! a scene fit in one glance-sized grid with no chrome at all, and the answer
//! to "the one I want isn't here" is to add it to this table.
//!
//! The lookup exists because [`renzora::EntityIcon`] holds a `String` loaded
//! from a scene file, while the hierarchy's `EntityNode` and every icon widget
//! want a `&'static str`. Resolving through this table gets that lifetime and
//! validates the name in the same step — an icon the font has no glyph for
//! would otherwise draw as an empty box with no way back.

/// Every icon the entity-icon picker offers, as `(phosphor name, label)`.
/// Grouped in rows of eight so the picker's 8-wide grid reads as themed bands:
/// shapes, nature, structures, beings, gameplay, systems.
pub const ENTITY_ICON_CHOICES: &[(&str, &str)] = &[
    // Shapes and generic markers.
    ("cube", "Cube"),
    ("sphere", "Sphere"),
    ("cylinder", "Cylinder"),
    ("circle", "Circle"),
    ("diamond", "Diamond"),
    ("sparkle", "Sparkle"),
    ("star", "Star"),
    ("package", "Package"),
    // Nature.
    ("tree", "Tree"),
    ("flower", "Flower"),
    ("leaf", "Leaf"),
    ("mountains", "Mountains"),
    ("waves", "Water"),
    ("drop", "Drop"),
    ("fire", "Fire"),
    ("snowflake", "Snow"),
    // Structures.
    ("house", "House"),
    ("buildings", "Buildings"),
    ("barn", "Barn"),
    ("church", "Church"),
    ("factory", "Factory"),
    ("bridge", "Bridge"),
    ("door", "Door"),
    ("stairs", "Stairs"),
    // Beings.
    ("person", "Person"),
    ("users", "Crowd"),
    ("ghost", "Ghost"),
    ("skull", "Skull"),
    ("robot", "Robot"),
    ("alien", "Alien"),
    ("paw-print", "Creature"),
    ("bone", "Bone"),
    // Gameplay.
    ("sword", "Weapon"),
    ("shield", "Shield"),
    ("target", "Target"),
    ("crosshair", "Spawn"),
    ("key", "Key"),
    ("coins", "Pickup"),
    ("crown", "Objective"),
    ("gift", "Reward"),
    // Systems.
    ("lightbulb", "Light"),
    ("camera", "Camera"),
    ("gear", "System"),
    ("cpu", "Logic"),
    ("database", "Data"),
    ("globe", "World"),
    ("music-note", "Audio"),
    ("game-controller", "Player"),
];

/// Resolve a stored icon name to its `&'static str` entry in
/// [`ENTITY_ICON_CHOICES`], or `None` when it is empty (no override) or not one
/// of the offered icons (a scene authored against a longer table, or a
/// hand-edited file). `None` means "fall back to the archetype icon" at every
/// call site, so a stale name degrades to the old behaviour instead of a blank.
pub fn entity_icon_name(stored: &str) -> Option<&'static str> {
    if stored.is_empty() {
        return None;
    }
    ENTITY_ICON_CHOICES
        .iter()
        .find(|(name, _)| *name == stored)
        .map(|(name, _)| *name)
}
