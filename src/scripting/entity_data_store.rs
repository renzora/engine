//! Thread-local entity data store for cross-entity property access in scripts.
//!
//! Populated once per frame before the script loop, then queried by
//! `entity()`, `parent()`, `child()`, `children()` API functions.

use rhai::{Dynamic, Map};
use std::cell::RefCell;
use std::collections::HashMap;

/// Per-entity flat property map (property_name → Dynamic value)
pub type EntityProperties = HashMap<String, Dynamic>;

/// Entity data store — populated once per frame, read by script API functions.
pub struct EntityDataStore {
    /// entity_id → properties
    pub entities: HashMap<u64, EntityProperties>,
    /// entity_name → entity_id (for `entity("Name")` lookup)
    pub name_to_id: HashMap<String, u64>,
}

impl EntityDataStore {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            name_to_id: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        self.name_to_id.clear();
    }

    /// Build a Rhai Map from the stored properties for an entity.
    /// Includes a hidden `_id` field for `set()` to identify the target.
    pub fn build_entity_map(&self, entity_id: u64) -> Option<Map> {
        let props = self.entities.get(&entity_id)?;
        let mut map = Map::new();
        map.insert("_id".into(), Dynamic::from(entity_id as i64));
        for (key, value) in props {
            map.insert(key.clone().into(), value.clone());
        }
        Some(map)
    }
}

/// Hierarchy context set before each script execution.
pub struct HierarchyContext {
    pub self_entity_id: u64,
    pub parent_entity_id: Option<u64>,
    pub children_entity_ids: Vec<u64>,
}

thread_local! {
    /// The per-frame entity data store.
    static ENTITY_DATA: RefCell<EntityDataStore> = RefCell::new(EntityDataStore::new());

    /// Per-frame Map cache (entity_id → built Map) so repeated `entity("X")` calls don't rebuild.
    static ENTITY_MAP_CACHE: RefCell<HashMap<u64, Map>> = RefCell::new(HashMap::new());

    /// Hierarchy context for the currently executing script.
    static HIERARCHY_CTX: RefCell<Option<HierarchyContext>> = RefCell::new(None);
}

// ===========================================================================
// Public API for populating the store (called from runtime.rs)
// ===========================================================================

/// Clear store and cache at the start of each frame.
pub fn clear_store() {
    ENTITY_DATA.with(|s| s.borrow_mut().clear());
    ENTITY_MAP_CACHE.with(|c| c.borrow_mut().clear());
}

/// Insert properties for an entity.
pub fn insert_entity(entity_id: u64, name: &str, props: EntityProperties) {
    ENTITY_DATA.with(|s| {
        let mut store = s.borrow_mut();
        store.name_to_id.insert(name.to_string(), entity_id);
        store.entities.insert(entity_id, props);
    });
}

/// Merge additional properties into an existing entity's data.
/// Used to add transform data for scripted entities in a second pass.
pub fn merge_entity_props(entity_id: u64, props: EntityProperties) {
    ENTITY_DATA.with(|s| {
        let mut store = s.borrow_mut();
        if let Some(existing) = store.entities.get_mut(&entity_id) {
            existing.extend(props);
        }
    });
}

/// Set the hierarchy context for the currently executing script.
pub fn set_hierarchy_context(ctx: HierarchyContext) {
    HIERARCHY_CTX.with(|h| *h.borrow_mut() = Some(ctx));
}

/// Clear the hierarchy context after script execution.
pub fn clear_hierarchy_context() {
    HIERARCHY_CTX.with(|h| *h.borrow_mut() = None);
}

// ===========================================================================
// Public API for reading from the store (called from rhai_api/entity_access.rs)
// ===========================================================================

/// Look up entity by name, return a cached Map.
pub fn get_entity_by_name(name: &str) -> Option<Map> {
    let entity_id = ENTITY_DATA.with(|s| s.borrow().name_to_id.get(name).copied())?;
    get_entity_map(entity_id)
}

/// Look up entity by ID, return a cached Map.
pub fn get_entity_map(entity_id: u64) -> Option<Map> {
    // Check cache first
    let cached = ENTITY_MAP_CACHE.with(|c| c.borrow().get(&entity_id).cloned());
    if let Some(map) = cached {
        return Some(map);
    }

    // Build and cache
    let map = ENTITY_DATA.with(|s| s.borrow().build_entity_map(entity_id))?;
    ENTITY_MAP_CACHE.with(|c| c.borrow_mut().insert(entity_id, map.clone()));
    Some(map)
}

/// Get the parent entity Map for the current script context.
pub fn get_parent_map() -> Option<Map> {
    let parent_id = HIERARCHY_CTX.with(|h| {
        h.borrow().as_ref().and_then(|ctx| ctx.parent_entity_id)
    })?;
    get_entity_map(parent_id)
}

/// Get a named child entity Map for the current script context.
pub fn get_child_by_name(name: &str) -> Option<Map> {
    let children_ids = HIERARCHY_CTX.with(|h| {
        h.borrow().as_ref().map(|ctx| ctx.children_entity_ids.clone())
    })?;

    // Look up each child to find the one with matching name
    for &child_id in &children_ids {
        let has_name = ENTITY_DATA.with(|s| {
            s.borrow().entities.get(&child_id)
                .and_then(|props| props.get("name"))
                .and_then(|v| v.clone().into_immutable_string().ok())
                .map(|n| n.as_str() == name)
                .unwrap_or(false)
        });
        if has_name {
            return get_entity_map(child_id);
        }
    }
    None
}

/// Get all children entity Maps for the current script context.
pub fn get_children_maps() -> Vec<Map> {
    let children_ids = HIERARCHY_CTX.with(|h| {
        h.borrow().as_ref().map(|ctx| ctx.children_entity_ids.clone()).unwrap_or_default()
    });

    children_ids.iter()
        .filter_map(|&id| get_entity_map(id))
        .collect()
}
