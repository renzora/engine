use std::sync::{Arc, OnceLock};

use lasso::{Spur, ThreadedRodeo};

static GLOBAL_RODEO: OnceLock<Arc<ThreadedRodeo>> = OnceLock::new();

/// Access the global interner rodeo.
///
/// Panics if [`AttributesPlugin`](crate::plugin::AttributesPlugin) has not
/// been added to the app yet.
pub(crate) fn global_rodeo() -> &'static Arc<ThreadedRodeo> {
    GLOBAL_RODEO
        .get()
        .expect("Global interner not initialized — add AttributesPlugin first")
}

/// A lightweight handle to an interned attribute name.
///
/// Cheap to copy, hash, and compare (u32 under the hood).
/// All attribute references use `AttributeId` internally — no heap-allocated
/// strings in hot paths.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AttributeId(pub(crate) Spur);

/// String interner for attribute names.
///
/// All attribute name strings are interned here, converting them to lightweight
/// `AttributeId` handles. This eliminates heap string allocations and makes
/// comparisons O(1).
///
/// At runtime, [`AttributesPlugin`](crate::plugin::AttributesPlugin) initializes
/// a single global `Interner` accessible via [`Interner::global()`]. Unit tests
/// can create isolated instances via [`Interner::new()`].
#[derive(Clone)]
pub struct Interner {
    rodeo: Arc<ThreadedRodeo>,
}

impl Interner {
    /// Create a new empty interner.
    pub fn new() -> Self {
        Self {
            rodeo: Arc::new(ThreadedRodeo::default()),
        }
    }

    /// Get the global `Interner`.
    ///
    /// Panics if [`AttributesPlugin`](crate::plugin::AttributesPlugin) has not
    /// been added yet.
    pub fn global() -> Self {
        Self {
            rodeo: Arc::clone(global_rodeo()),
        }
    }

    /// Publish this interner's rodeo to the global static so that
    /// [`Attributes::value`](crate::attributes::Attributes::value) can
    /// resolve names without an explicit `Res<Interner>`.
    ///
    /// Called automatically by [`AttributesPlugin`](crate::plugin::AttributesPlugin).
    pub fn set_global(&self) {
        let _ = GLOBAL_RODEO.set(Arc::clone(&self.rodeo));
    }

    /// Intern a string, returning its `AttributeId`. If the string was already
    /// interned, returns the existing handle.
    pub fn get_or_intern(&self, s: &str) -> AttributeId {
        AttributeId(self.rodeo.get_or_intern(s))
    }

    /// Look up a string that may or may not have been interned.
    /// Returns `None` if the string hasn't been interned yet.
    pub fn get(&self, s: &str) -> Option<AttributeId> {
        self.rodeo.get(s).map(AttributeId)
    }

    /// Resolve a `AttributeId` back to its string representation.
    pub fn resolve(&self, id: AttributeId) -> &str {
        self.rodeo.resolve(&id.0)
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_and_resolve() {
        let interner = Interner::new();
        let id = interner.get_or_intern("Strength");
        assert_eq!(interner.resolve(id), "Strength");
    }

    #[test]
    fn same_string_same_id() {
        let interner = Interner::new();
        let a = interner.get_or_intern("Damage.current");
        let b = interner.get_or_intern("Damage.current");
        assert_eq!(a, b);
    }

    #[test]
    fn different_strings_different_ids() {
        let interner = Interner::new();
        let a = interner.get_or_intern("Strength");
        let b = interner.get_or_intern("Dexterity");
        assert_ne!(a, b);
    }

    #[test]
    fn get_returns_none_for_unknown() {
        let interner = Interner::new();
        assert!(interner.get("NotInterned").is_none());
    }

    #[test]
    fn get_returns_some_for_known() {
        let interner = Interner::new();
        let id = interner.get_or_intern("Health");
        assert_eq!(interner.get("Health"), Some(id));
    }
}
