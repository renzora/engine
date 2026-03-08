//! Thread-local handler for script `get` calls.
//!
//! The execution system sets this before calling a script, providing
//! a closure that reads reflected component fields from the world.

use std::cell::RefCell;

use crate::command::PropertyValue;

/// Signature for the get handler: (entity_name, component_type, field_path) → Option<PropertyValue>.
/// If entity_name is None, reads from the script's own entity.
type GetFn = Box<dyn Fn(Option<&str>, &str, &str) -> Option<PropertyValue>>;

thread_local! {
    static GET_HANDLER: RefCell<Option<GetFn>> = RefCell::new(None);
}

/// Set the get handler for the current script execution.
pub fn set_get_handler(handler: GetFn) {
    GET_HANDLER.with(|h| *h.borrow_mut() = Some(handler));
}

/// Clear the get handler after script execution.
pub fn clear_get_handler() {
    GET_HANDLER.with(|h| *h.borrow_mut() = None);
}

/// Call the get handler. Returns None if no handler is set or the field doesn't exist.
pub fn call_get(entity_name: Option<&str>, component_type: &str, field_path: &str) -> Option<PropertyValue> {
    GET_HANDLER.with(|h| {
        let borrow = h.borrow();
        borrow.as_ref().and_then(|f| f(entity_name, component_type, field_path))
    })
}
