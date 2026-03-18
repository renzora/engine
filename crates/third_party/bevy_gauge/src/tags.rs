use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

/// A bitmask representing a set of tags on a modifier or a tag query.
///
/// Tags enable filtered attribute evaluation — e.g., "fire sword damage" uses
/// only modifiers that apply to fire and/or sword damage.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TagMask(pub u64);

impl TagMask {
    /// The empty tag mask (matches everything in queries, applies to everything as a modifier).
    pub const NONE: TagMask = TagMask(0);

    /// Create a tag mask from a raw u64 value.
    pub const fn new(bits: u64) -> Self {
        Self(bits)
    }

    /// Create a tag mask with a single bit set.
    pub const fn bit(index: u32) -> Self {
        Self(1u64 << index)
    }

    /// Combine two tag masks (bitwise OR).
    pub const fn union(self, other: TagMask) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if this mask satisfies a query mask.
    ///
    /// A modifier with `self` tags satisfies query `q` when `(self & q) == q`.
    /// This means the modifier has at least all the bits the query asks for.
    ///
    /// Special case: query of NONE (0) is satisfied by everything.
    pub const fn satisfies(self, query: TagMask) -> bool {
        query.0 == 0 || (self.0 & query.0) == query.0
    }

    /// Check whether a modifier with this tag should participate in a given query.
    ///
    /// A modifier with tag `self` matches query `q` when:
    /// - `self` is NONE (the modifier is global — it applies to every query), OR
    /// - All of `self`'s tag bits are present in `q` (the modifier's tags are a
    ///   subset of the query).
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_gauge::prelude::TagMask;
    /// let fire = TagMask::bit(0);
    /// let physical = TagMask::bit(1);
    /// let melee = TagMask::bit(2);
    ///
    /// // Global modifier (NONE) matches any query
    /// assert!(TagMask::NONE.matches_query(fire));
    ///
    /// // FIRE modifier matches a FIRE query
    /// assert!(fire.matches_query(fire));
    ///
    /// // FIRE modifier matches a FIRE|MELEE query (fire ⊆ fire|melee)
    /// assert!(fire.matches_query(fire | melee));
    ///
    /// // FIRE modifier does NOT match a PHYSICAL query
    /// assert!(!fire.matches_query(physical));
    ///
    /// // FIRE|MELEE modifier does NOT match a FIRE-only query (melee bit missing)
    /// assert!(!(fire | melee).matches_query(fire));
    /// ```
    pub const fn matches_query(self, query: TagMask) -> bool {
        self.0 == 0 || (self.0 & query.0) == self.0
    }

    /// Check if this mask is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for TagMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for TagMask {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// ---------------------------------------------------------------------------
// TagResolver — ECS resource mapping tag name strings to TagMask values
// ---------------------------------------------------------------------------

/// ECS resource that maps tag name strings to [`TagMask`] values.
///
/// This replaces the need for a global static configuration. Tag names are
/// registered at app startup (manually or via a future `define_tags!` macro)
/// and used at expression compile time to resolve `{FIRE|SPELL}` syntax.
#[derive(Resource, Default, Debug)]
pub struct TagResolver {
    tags: HashMap<String, TagMask>,
    /// Reverse mapping: bit position → registered tag name.
    /// Only populated for single-bit masks registered via [`register`](Self::register).
    reverse_tags: HashMap<u32, String>,
    /// Short names that have been registered by more than one namespace.
    /// These require fully-qualified `Namespace::TAG` syntax in expressions.
    ambiguous: HashSet<String>,
    /// Tracks which namespace owns each short name (first registrant).
    /// Used to detect when a second namespace tries to register the same short name.
    short_name_owner: HashMap<String, String>,
}

impl TagResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tag name → mask mapping.
    ///
    /// If the name was already registered, the old mapping is overwritten.
    /// Single-bit masks also populate a reverse lookup (bit position → name)
    /// used by [`decompose`](Self::decompose).
    pub fn register(&mut self, name: &str, mask: TagMask) {
        let upper = name.to_uppercase();
        self.tags.insert(upper.clone(), mask);
        // Record reverse mapping for single-bit masks
        if mask.0.count_ones() == 1 {
            self.reverse_tags.insert(mask.0.trailing_zeros(), upper);
        }
    }

    /// Register a tag name with a namespace.
    ///
    /// Registers both the short form (`"FIRE"`) and the namespaced form
    /// (`"Element::FIRE"`). If the short form was already registered by
    /// a different namespace, it is marked as ambiguous — callers must
    /// use the fully-qualified `Namespace::TAG` form in expressions.
    pub fn register_namespaced(&mut self, namespace: &str, name: &str, mask: TagMask) {
        let upper_name = name.to_uppercase();
        let upper_ns = namespace.to_uppercase();
        let namespaced = format!("{}::{}", upper_ns, upper_name);

        self.tags.insert(namespaced, mask);

        if let Some(existing_ns) = self.short_name_owner.get(&upper_name) {
            if *existing_ns != upper_ns {
                self.ambiguous.insert(upper_name.clone());
            }
        } else {
            self.short_name_owner.insert(upper_name.clone(), upper_ns);
            self.tags.insert(upper_name.clone(), mask);
        }

        if mask.0.count_ones() == 1 {
            self.reverse_tags.insert(mask.0.trailing_zeros(), upper_name);
        }
    }

    /// Resolve a tag name to its mask. Case-insensitive.
    ///
    /// Supports both short names (`"FIRE"`) and namespaced names
    /// (`"Element::FIRE"`). Returns `None` if the name is unregistered
    /// or if a short name is ambiguous (registered by multiple namespaces).
    pub fn resolve(&self, name: &str) -> Option<TagMask> {
        let upper = name.to_uppercase();
        if self.ambiguous.contains(&upper) {
            return None;
        }
        self.tags.get(&upper).copied()
    }

    /// Return the list of namespaced forms available for an ambiguous short name.
    ///
    /// Returns `None` if the name is not ambiguous.
    pub fn ambiguous_alternatives(&self, name: &str) -> Option<Vec<String>> {
        let upper = name.to_uppercase();
        if !self.ambiguous.contains(&upper) {
            return None;
        }
        let mut alternatives = Vec::new();
        let suffix = format!("::{}", upper);
        for key in self.tags.keys() {
            if key.ends_with(&suffix) {
                alternatives.push(key.clone());
            }
        }
        alternatives.sort();
        Some(alternatives)
    }

    /// Resolve multiple tag names and OR them together.
    /// Unknown tag names are silently ignored (contribute 0 bits).
    pub fn resolve_set(&self, names: &[&str]) -> TagMask {
        names
            .iter()
            .filter_map(|name| self.resolve(name))
            .fold(TagMask::NONE, |acc, m| acc | m)
    }

    /// Decompose a [`TagMask`] into the registered names for each set bit.
    ///
    /// Returns `None` if any set bit in the mask doesn't have a registered
    /// single-bit name. Returns an empty `Vec` for [`TagMask::NONE`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// resolver.register("FIRE", TagMask::bit(0));
    /// resolver.register("MELEE", TagMask::bit(2));
    ///
    /// let names = resolver.decompose(TagMask::bit(0) | TagMask::bit(2));
    /// assert_eq!(names, Some(vec!["FIRE", "MELEE"]));
    /// ```
    pub fn decompose(&self, mask: TagMask) -> Option<Vec<&str>> {
        if mask.is_empty() {
            return Some(Vec::new());
        }
        let mut names = Vec::new();
        let mut bits = mask.0;
        while bits != 0 {
            let bit_pos = bits.trailing_zeros();
            let name = self.reverse_tags.get(&bit_pos)?;
            names.push(name.as_str());
            bits &= bits - 1; // clear lowest set bit
        }
        Some(names)
    }

    /// Build a `{TAG1|TAG2}` expression-syntax suffix string for the given mask.
    ///
    /// Returns `None` if the mask can't be decomposed (see [`decompose`](Self::decompose)).
    /// Returns `Some("")` for [`TagMask::NONE`].
    pub fn tag_suffix(&self, mask: TagMask) -> Option<String> {
        let names = self.decompose(mask)?;
        if names.is_empty() {
            Some(String::new())
        } else {
            Some(format!("{{{}}}", names.join("|")))
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-registration via inventory
// ---------------------------------------------------------------------------

/// A registration entry submitted by [`define_tags!`](crate::define_tags) via
/// `inventory`. Each entry carries a function that registers one tag struct's
/// names with a [`TagResolver`].
pub struct TagRegistration {
    pub register_fn: fn(&mut TagResolver),
}

inventory::collect!(TagRegistration);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        let fire = TagMask::bit(0);
        let sword = TagMask::bit(4);
        let both = fire | sword;

        assert!(fire.satisfies(TagMask::NONE));
        assert!(sword.satisfies(TagMask::NONE));
        assert!(both.satisfies(TagMask::NONE));
        assert!(TagMask::NONE.satisfies(TagMask::NONE));
    }

    #[test]
    fn exact_match() {
        let fire = TagMask::bit(0);
        assert!(fire.satisfies(fire));
    }

    #[test]
    fn superset_satisfies_subset() {
        let fire = TagMask::bit(0);
        let sword = TagMask::bit(4);
        let fire_sword = fire | sword;

        // fire|sword modifier satisfies a query for just fire
        assert!(fire_sword.satisfies(fire));
        // fire|sword modifier satisfies a query for just sword
        assert!(fire_sword.satisfies(sword));
        // fire|sword modifier satisfies a query for fire|sword
        assert!(fire_sword.satisfies(fire_sword));
    }

    #[test]
    fn subset_does_not_satisfy_superset() {
        let fire = TagMask::bit(0);
        let sword = TagMask::bit(4);
        let fire_sword = fire | sword;

        // A fire-only modifier does NOT satisfy a query for fire|sword
        assert!(!fire.satisfies(fire_sword));
    }

    #[test]
    fn disjoint_does_not_satisfy() {
        let fire = TagMask::bit(0);
        let cold = TagMask::bit(1);
        assert!(!fire.satisfies(cold));
    }

    // --- matches_query tests ---

    #[test]
    fn global_modifier_matches_any_query() {
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);
        assert!(TagMask::NONE.matches_query(fire));
        assert!(TagMask::NONE.matches_query(physical));
        assert!(TagMask::NONE.matches_query(fire | physical));
        assert!(TagMask::NONE.matches_query(TagMask::NONE));
    }

    #[test]
    fn exact_tag_matches_query() {
        let fire = TagMask::bit(0);
        assert!(fire.matches_query(fire));
    }

    #[test]
    fn subset_modifier_matches_superset_query() {
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        // FIRE modifier matches FIRE|MELEE query
        assert!(fire.matches_query(fire | melee));
    }

    #[test]
    fn superset_modifier_does_not_match_subset_query() {
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        // FIRE|MELEE modifier does NOT match a FIRE-only query
        assert!(!(fire | melee).matches_query(fire));
    }

    #[test]
    fn disjoint_modifier_does_not_match() {
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);
        assert!(!fire.matches_query(physical));
    }

    // --- TagResolver tests ---

    #[test]
    fn resolver_register_and_resolve() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        resolver.register("FIRE", fire);
        assert_eq!(resolver.resolve("FIRE"), Some(fire));
        assert_eq!(resolver.resolve("fire"), Some(fire)); // case insensitive
    }

    #[test]
    fn resolver_unknown_tag_returns_none() {
        let resolver = TagResolver::new();
        assert_eq!(resolver.resolve("UNKNOWN"), None);
    }

    #[test]
    fn resolver_resolve_set() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        resolver.register("FIRE", fire);
        resolver.register("MELEE", melee);
        assert_eq!(resolver.resolve_set(&["FIRE", "MELEE"]), fire | melee);
    }

    #[test]
    fn resolver_resolve_set_ignores_unknown() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        resolver.register("FIRE", fire);
        assert_eq!(resolver.resolve_set(&["FIRE", "NOPE"]), fire);
    }

    // --- decompose tests ---

    #[test]
    fn decompose_empty_mask() {
        let resolver = TagResolver::new();
        assert_eq!(resolver.decompose(TagMask::NONE), Some(vec![]));
    }

    #[test]
    fn decompose_single_bit() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        assert_eq!(resolver.decompose(TagMask::bit(0)), Some(vec!["FIRE"]));
    }

    #[test]
    fn decompose_multi_bit() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        resolver.register("FIRE", fire);
        resolver.register("MELEE", melee);

        let names = resolver.decompose(fire | melee).unwrap();
        assert!(names.contains(&"FIRE"));
        assert!(names.contains(&"MELEE"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn decompose_unregistered_bit_returns_none() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        // bit 1 is not registered
        assert_eq!(resolver.decompose(TagMask::bit(0) | TagMask::bit(1)), None);
    }

    #[test]
    fn tag_suffix_string() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        resolver.register("MELEE", TagMask::bit(2));

        // Single tag
        let s = resolver.tag_suffix(TagMask::bit(0)).unwrap();
        assert_eq!(s, "{FIRE}");

        // Multi-tag
        let s = resolver.tag_suffix(TagMask::bit(0) | TagMask::bit(2)).unwrap();
        assert!(s.starts_with('{') && s.ends_with('}'));
        assert!(s.contains("FIRE") && s.contains("MELEE"));

        // Empty
        assert_eq!(resolver.tag_suffix(TagMask::NONE), Some(String::new()));

        // Unresolvable
        assert_eq!(resolver.tag_suffix(TagMask::bit(5)), None);
    }

    // --- Namespaced resolution tests ---

    #[test]
    fn namespaced_register_resolves_short_and_qualified() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        resolver.register_namespaced("Element", "FIRE", fire);

        assert_eq!(resolver.resolve("FIRE"), Some(fire));
        assert_eq!(resolver.resolve("Element::FIRE"), Some(fire));
        assert_eq!(resolver.resolve("fire"), Some(fire)); // case insensitive
        assert_eq!(resolver.resolve("element::fire"), Some(fire));
    }

    #[test]
    fn namespaced_single_namespace_no_ambiguity() {
        let mut resolver = TagResolver::new();
        resolver.register_namespaced("Element", "FIRE", TagMask::bit(0));
        resolver.register_namespaced("Element", "COLD", TagMask::bit(1));

        assert_eq!(resolver.resolve("FIRE"), Some(TagMask::bit(0)));
        assert_eq!(resolver.resolve("COLD"), Some(TagMask::bit(1)));
        assert!(resolver.ambiguous_alternatives("FIRE").is_none());
    }

    #[test]
    fn namespaced_collision_marks_ambiguous() {
        let mut resolver = TagResolver::new();
        resolver.register_namespaced("Element", "FIRE", TagMask::bit(0));
        resolver.register_namespaced("Weapon", "FIRE", TagMask::bit(4));

        // Short name is now ambiguous
        assert_eq!(resolver.resolve("FIRE"), None);

        // Qualified names still work (case insensitive)
        assert_eq!(resolver.resolve("ELEMENT::FIRE"), Some(TagMask::bit(0)));
        assert_eq!(resolver.resolve("WEAPON::FIRE"), Some(TagMask::bit(4)));
        assert_eq!(resolver.resolve("Element::FIRE"), Some(TagMask::bit(0)));
    }

    #[test]
    fn namespaced_ambiguous_alternatives() {
        let mut resolver = TagResolver::new();
        resolver.register_namespaced("Element", "FIRE", TagMask::bit(0));
        resolver.register_namespaced("Weapon", "FIRE", TagMask::bit(4));

        let alts = resolver.ambiguous_alternatives("FIRE").unwrap();
        assert_eq!(alts.len(), 2);
        assert!(alts.contains(&"ELEMENT::FIRE".to_string()));
        assert!(alts.contains(&"WEAPON::FIRE".to_string()));
    }

    #[test]
    fn namespaced_non_colliding_tags_resolve_normally() {
        let mut resolver = TagResolver::new();
        resolver.register_namespaced("Element", "FIRE", TagMask::bit(0));
        resolver.register_namespaced("Weapon", "SWORD", TagMask::bit(4));

        assert_eq!(resolver.resolve("FIRE"), Some(TagMask::bit(0)));
        assert_eq!(resolver.resolve("SWORD"), Some(TagMask::bit(4)));
        assert!(resolver.ambiguous_alternatives("FIRE").is_none());
        assert!(resolver.ambiguous_alternatives("SWORD").is_none());
    }

    #[test]
    fn namespaced_mixed_with_plain_register() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        resolver.register_namespaced("Element", "COLD", TagMask::bit(1));

        assert_eq!(resolver.resolve("FIRE"), Some(TagMask::bit(0)));
        assert_eq!(resolver.resolve("COLD"), Some(TagMask::bit(1)));
        assert_eq!(resolver.resolve("ELEMENT::COLD"), Some(TagMask::bit(1)));
    }
}
