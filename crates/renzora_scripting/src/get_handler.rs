//! Thread-local handlers for script `get` calls.
//!
//! The execution system sets these before calling a script, providing
//! closures that read reflected component data from the world.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::command::PropertyValue;

/// Signature for the get-field handler: (entity_name, component_type, field_path) → Option<PropertyValue>.
type GetFn = Box<dyn Fn(Option<&str>, &str, &str) -> Option<PropertyValue>>;

/// Signature for get-component handler: (entity_name, component_type) → Option<HashMap<field, value>>.
type GetComponentFn = Box<dyn Fn(Option<&str>, &str) -> Option<HashMap<String, PropertyValue>>>;

/// Signature for get-components handler: (entity_name) → Vec<component_type_name>.
type GetComponentsFn = Box<dyn Fn(Option<&str>) -> Vec<String>>;

thread_local! {
    static GET_HANDLER: RefCell<Option<GetFn>> = RefCell::new(None);
    static GET_COMPONENT_HANDLER: RefCell<Option<GetComponentFn>> = RefCell::new(None);
    static GET_COMPONENTS_HANDLER: RefCell<Option<GetComponentsFn>> = RefCell::new(None);
}

/// Set the get-field handler for the current script execution.
pub fn set_get_handler(handler: GetFn) {
    GET_HANDLER.with(|h| *h.borrow_mut() = Some(handler));
}

/// Set the get-component handler for the current script execution.
pub fn set_get_component_handler(handler: GetComponentFn) {
    GET_COMPONENT_HANDLER.with(|h| *h.borrow_mut() = Some(handler));
}

/// Set the get-components handler for the current script execution.
pub fn set_get_components_handler(handler: GetComponentsFn) {
    GET_COMPONENTS_HANDLER.with(|h| *h.borrow_mut() = Some(handler));
}

/// Clear all handlers after script execution.
pub fn clear_get_handler() {
    GET_HANDLER.with(|h| *h.borrow_mut() = None);
    GET_COMPONENT_HANDLER.with(|h| *h.borrow_mut() = None);
    GET_COMPONENTS_HANDLER.with(|h| *h.borrow_mut() = None);
}

/// Read a single field from a component.
pub fn call_get(entity_name: Option<&str>, component_type: &str, field_path: &str) -> Option<PropertyValue> {
    GET_HANDLER.with(|h| {
        let borrow = h.borrow();
        borrow.as_ref().and_then(|f| f(entity_name, component_type, field_path))
    })
}

/// Read all fields from a component as a HashMap.
pub fn call_get_component(entity_name: Option<&str>, component_type: &str) -> Option<HashMap<String, PropertyValue>> {
    GET_COMPONENT_HANDLER.with(|h| {
        let borrow = h.borrow();
        borrow.as_ref().and_then(|f| f(entity_name, component_type))
    })
}

/// List all reflected component type names on an entity.
pub fn call_get_components(entity_name: Option<&str>) -> Vec<String> {
    GET_COMPONENTS_HANDLER.with(|h| {
        let borrow = h.borrow();
        borrow.as_ref().map(|f| f(entity_name)).unwrap_or_default()
    })
}
