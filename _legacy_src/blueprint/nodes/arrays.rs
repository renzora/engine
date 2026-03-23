//! Array operation nodes
//!
//! Nodes for creating and manipulating arrays/lists of values.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// ARRAY CREATION
// =============================================================================

/// Create empty array
pub static CREATE_ARRAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/create",
    display_name: "Create Array",
    category: "Array",
    description: "Create a new empty array",
    create_pins: || vec![
        Pin::output("array", "Array", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Create array with initial values
pub static CREATE_ARRAY_WITH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/create_with",
    display_name: "Create Array With",
    category: "Array",
    description: "Create an array with up to 4 initial values",
    create_pins: || vec![
        Pin::input("item0", "Item 0", PinType::Any),
        Pin::input("item1", "Item 1", PinType::Any),
        Pin::input("item2", "Item 2", PinType::Any),
        Pin::input("item3", "Item 3", PinType::Any),
        Pin::output("array", "Array", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Create array of integers
pub static CREATE_INT_ARRAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/create_int",
    display_name: "Create Int Array",
    category: "Array",
    description: "Create an array of integers",
    create_pins: || vec![
        Pin::input("item0", "Item 0", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("item1", "Item 1", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("item2", "Item 2", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("item3", "Item 3", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("array", "Array", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Create array of floats
pub static CREATE_FLOAT_ARRAY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/create_float",
    display_name: "Create Float Array",
    category: "Array",
    description: "Create an array of floats",
    create_pins: || vec![
        Pin::input("item0", "Item 0", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("item1", "Item 1", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("item2", "Item 2", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("item3", "Item 3", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("array", "Array", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ARRAY MODIFICATION
// =============================================================================

/// Push item to array
pub static ARRAY_PUSH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/push",
    display_name: "Array Push",
    category: "Array",
    description: "Add an item to the end of an array",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("item", "Item", PinType::Any),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Pop item from array
pub static ARRAY_POP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/pop",
    display_name: "Array Pop",
    category: "Array",
    description: "Remove and return the last item from an array",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("item", "Item", PinType::Any),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Insert item at index
pub static ARRAY_INSERT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/insert",
    display_name: "Array Insert",
    category: "Array",
    description: "Insert an item at a specific index",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("item", "Item", PinType::Any),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Remove item at index
pub static ARRAY_REMOVE_AT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/remove_at",
    display_name: "Array Remove At",
    category: "Array",
    description: "Remove an item at a specific index",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("item", "Item", PinType::Any),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Remove first occurrence of item
pub static ARRAY_REMOVE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/remove",
    display_name: "Array Remove",
    category: "Array",
    description: "Remove the first occurrence of an item",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("item", "Item", PinType::Any),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("removed", "Removed", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Set item at index
pub static ARRAY_SET: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/set",
    display_name: "Array Set",
    category: "Array",
    description: "Set the item at a specific index",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("item", "Item", PinType::Any),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Clear array
pub static ARRAY_CLEAR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/clear",
    display_name: "Array Clear",
    category: "Array",
    description: "Remove all items from an array",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ARRAY ACCESS
// =============================================================================

/// Get item at index
pub static ARRAY_GET: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/get",
    display_name: "Array Get",
    category: "Array",
    description: "Get the item at a specific index",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("item", "Item", PinType::Any),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get first item
pub static ARRAY_FIRST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/first",
    display_name: "Array First",
    category: "Array",
    description: "Get the first item in an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("item", "Item", PinType::Any),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get last item
pub static ARRAY_LAST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/last",
    display_name: "Array Last",
    category: "Array",
    description: "Get the last item in an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("item", "Item", PinType::Any),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Get random item
pub static ARRAY_RANDOM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/random",
    display_name: "Array Random",
    category: "Array",
    description: "Get a random item from an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("item", "Item", PinType::Any),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ARRAY QUERIES
// =============================================================================

/// Get array length
pub static ARRAY_LENGTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/length",
    display_name: "Array Length",
    category: "Array",
    description: "Get the number of items in an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("length", "Length", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Check if array is empty
pub static ARRAY_IS_EMPTY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/is_empty",
    display_name: "Array Is Empty",
    category: "Array",
    description: "Check if an array is empty",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("is_empty", "Is Empty", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Check if array contains item
pub static ARRAY_CONTAINS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/contains",
    display_name: "Array Contains",
    category: "Array",
    description: "Check if an array contains an item",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("item", "Item", PinType::Any),
        Pin::output("contains", "Contains", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Find index of item
pub static ARRAY_FIND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/find",
    display_name: "Array Find",
    category: "Array",
    description: "Find the index of an item (-1 if not found)",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("item", "Item", PinType::Any),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("found", "Found", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Check if index is valid
pub static ARRAY_IS_VALID_INDEX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/is_valid_index",
    display_name: "Is Valid Index",
    category: "Array",
    description: "Check if an index is valid for the array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("valid", "Valid", PinType::Bool),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ARRAY MANIPULATION
// =============================================================================

/// Shuffle array
pub static ARRAY_SHUFFLE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/shuffle",
    display_name: "Array Shuffle",
    category: "Array",
    description: "Randomly shuffle the items in an array",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Reverse array
pub static ARRAY_REVERSE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/reverse",
    display_name: "Array Reverse",
    category: "Array",
    description: "Reverse the order of items in an array",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Sort array
pub static ARRAY_SORT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/sort",
    display_name: "Array Sort",
    category: "Array",
    description: "Sort the items in an array",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("ascending", "Ascending", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Clone/copy array
pub static ARRAY_COPY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/copy",
    display_name: "Array Copy",
    category: "Array",
    description: "Create a copy of an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("copy", "Copy", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Slice array
pub static ARRAY_SLICE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/slice",
    display_name: "Array Slice",
    category: "Array",
    description: "Get a slice of an array from start to end index",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::input("start", "Start", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("end", "End", PinType::Int).with_default(PinValue::Int(-1)),
        Pin::output("slice", "Slice", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Concatenate arrays
pub static ARRAY_CONCAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/concat",
    display_name: "Array Concat",
    category: "Array",
    description: "Concatenate two arrays",
    create_pins: || vec![
        Pin::input("array_a", "Array A", PinType::EntityArray),
        Pin::input("array_b", "Array B", PinType::EntityArray),
        Pin::output("result", "Result", PinType::EntityArray),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// NUMERIC ARRAY OPERATIONS
// =============================================================================

/// Sum of numeric array
pub static ARRAY_SUM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/sum",
    display_name: "Array Sum",
    category: "Array",
    description: "Get the sum of all numbers in an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("sum", "Sum", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Average of numeric array
pub static ARRAY_AVERAGE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/average",
    display_name: "Array Average",
    category: "Array",
    description: "Get the average of all numbers in an array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("average", "Average", PinType::Float),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Min value in numeric array
pub static ARRAY_MIN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/min",
    display_name: "Array Min",
    category: "Array",
    description: "Get the minimum value in a numeric array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("min", "Min", PinType::Float),
        Pin::output("index", "Index", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};

/// Max value in numeric array
pub static ARRAY_MAX: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "array/max",
    display_name: "Array Max",
    category: "Array",
    description: "Get the maximum value in a numeric array",
    create_pins: || vec![
        Pin::input("array", "Array", PinType::EntityArray),
        Pin::output("max", "Max", PinType::Float),
        Pin::output("index", "Index", PinType::Int),
    ],
    color: [180, 140, 200],
    is_event: false,
    is_comment: false,
};
