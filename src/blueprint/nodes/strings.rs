//! String manipulation nodes
//!
//! Nodes for string operations like concatenation, substring, search, and formatting.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// STRING CREATION & CONCATENATION
// =============================================================================

/// Concatenate two strings
pub static CONCAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/concat",
    display_name: "Concat",
    category: "String",
    description: "Concatenate two strings together",
    create_pins: || vec![
        Pin::input("a", "A", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("b", "B", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Concatenate multiple strings
pub static CONCAT_MULTI: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/concat_multi",
    display_name: "Concat Multiple",
    category: "String",
    description: "Concatenate up to 4 strings together",
    create_pins: || vec![
        Pin::input("a", "A", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("b", "B", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("c", "C", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("d", "D", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Join strings with separator
pub static JOIN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/join",
    display_name: "Join",
    category: "String",
    description: "Join strings with a separator",
    create_pins: || vec![
        Pin::input("a", "A", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("b", "B", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("separator", "Separator", PinType::String).with_default(PinValue::String(", ".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// STRING QUERIES
// =============================================================================

/// Get string length
pub static STRING_LENGTH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/length",
    display_name: "String Length",
    category: "String",
    description: "Get the length of a string",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("length", "Length", PinType::Int),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Check if string is empty
pub static IS_EMPTY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/is_empty",
    display_name: "Is Empty",
    category: "String",
    description: "Check if a string is empty",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("is_empty", "Is Empty", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Check if string contains substring
pub static CONTAINS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/contains",
    display_name: "Contains",
    category: "String",
    description: "Check if string contains a substring",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("substring", "Substring", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("contains", "Contains", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Check if string starts with prefix
pub static STARTS_WITH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/starts_with",
    display_name: "Starts With",
    category: "String",
    description: "Check if string starts with a prefix",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("prefix", "Prefix", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("starts_with", "Starts With", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Check if string ends with suffix
pub static ENDS_WITH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/ends_with",
    display_name: "Ends With",
    category: "String",
    description: "Check if string ends with a suffix",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("suffix", "Suffix", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("ends_with", "Ends With", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Find index of substring
pub static INDEX_OF: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/index_of",
    display_name: "Index Of",
    category: "String",
    description: "Find the index of a substring (-1 if not found)",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("substring", "Substring", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("index", "Index", PinType::Int),
        Pin::output("found", "Found", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Compare two strings
pub static STRING_EQUALS: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/equals",
    display_name: "String Equals",
    category: "String",
    description: "Check if two strings are equal",
    create_pins: || vec![
        Pin::input("a", "A", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("b", "B", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("equals", "Equals", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Compare strings (ignore case)
pub static STRING_EQUALS_IGNORE_CASE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/equals_ignore_case",
    display_name: "Equals (Ignore Case)",
    category: "String",
    description: "Check if two strings are equal (case insensitive)",
    create_pins: || vec![
        Pin::input("a", "A", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("b", "B", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("equals", "Equals", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// STRING MANIPULATION
// =============================================================================

/// Get substring
pub static SUBSTRING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/substring",
    display_name: "Substring",
    category: "String",
    description: "Extract a substring from start index with given length",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("start", "Start", PinType::Int).with_default(PinValue::Int(0)),
        Pin::input("length", "Length", PinType::Int).with_default(PinValue::Int(1)),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Get character at index
pub static CHAR_AT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/char_at",
    display_name: "Char At",
    category: "String",
    description: "Get character at a specific index",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("index", "Index", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("char", "Char", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Replace substring
pub static REPLACE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/replace",
    display_name: "Replace",
    category: "String",
    description: "Replace occurrences of a substring",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("from", "From", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("to", "To", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Split string
pub static SPLIT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/split",
    display_name: "Split",
    category: "String",
    description: "Split a string by delimiter (returns first and second parts)",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("delimiter", "Delimiter", PinType::String).with_default(PinValue::String(",".into())),
        Pin::output("first", "First", PinType::String),
        Pin::output("rest", "Rest", PinType::String),
        Pin::output("count", "Count", PinType::Int),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// CASE CONVERSION
// =============================================================================

/// Convert to uppercase
pub static TO_UPPER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/to_upper",
    display_name: "To Upper",
    category: "String",
    description: "Convert string to uppercase",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Convert to lowercase
pub static TO_LOWER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/to_lower",
    display_name: "To Lower",
    category: "String",
    description: "Convert string to lowercase",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Capitalize first letter
pub static CAPITALIZE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/capitalize",
    display_name: "Capitalize",
    category: "String",
    description: "Capitalize the first letter of the string",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// WHITESPACE
// =============================================================================

/// Trim whitespace
pub static TRIM: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/trim",
    display_name: "Trim",
    category: "String",
    description: "Remove leading and trailing whitespace",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Trim start
pub static TRIM_START: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/trim_start",
    display_name: "Trim Start",
    category: "String",
    description: "Remove leading whitespace",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Trim end
pub static TRIM_END: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/trim_end",
    display_name: "Trim End",
    category: "String",
    description: "Remove trailing whitespace",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Pad left
pub static PAD_LEFT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/pad_left",
    display_name: "Pad Left",
    category: "String",
    description: "Pad the string on the left to a target length",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("length", "Length", PinType::Int).with_default(PinValue::Int(10)),
        Pin::input("char", "Char", PinType::String).with_default(PinValue::String(" ".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Pad right
pub static PAD_RIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/pad_right",
    display_name: "Pad Right",
    category: "String",
    description: "Pad the string on the right to a target length",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("length", "Length", PinType::Int).with_default(PinValue::Int(10)),
        Pin::input("char", "Char", PinType::String).with_default(PinValue::String(" ".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// FORMATTING & CONVERSION
// =============================================================================

/// Format string with placeholders
pub static FORMAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/format",
    display_name: "Format",
    category: "String",
    description: "Format a string with placeholders {0}, {1}, {2}, {3}",
    create_pins: || vec![
        Pin::input("template", "Template", PinType::String).with_default(PinValue::String("Hello {0}!".into())),
        Pin::input("arg0", "Arg 0", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("arg1", "Arg 1", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("arg2", "Arg 2", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("arg3", "Arg 3", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Int to string
pub static INT_TO_STRING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/int_to_string",
    display_name: "Int To String",
    category: "String",
    description: "Convert an integer to a string",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Float to string
pub static FLOAT_TO_STRING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/float_to_string",
    display_name: "Float To String",
    category: "String",
    description: "Convert a float to a string",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("decimals", "Decimals", PinType::Int).with_default(PinValue::Int(2)),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Bool to string
pub static BOOL_TO_STRING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/bool_to_string",
    display_name: "Bool To String",
    category: "String",
    description: "Convert a boolean to a string",
    create_pins: || vec![
        Pin::input("value", "Value", PinType::Bool).with_default(PinValue::Bool(false)),
        Pin::input("true_text", "True Text", PinType::String).with_default(PinValue::String("true".into())),
        Pin::input("false_text", "False Text", PinType::String).with_default(PinValue::String("false".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// String to int
pub static STRING_TO_INT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/string_to_int",
    display_name: "String To Int",
    category: "String",
    description: "Parse a string as an integer",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("0".into())),
        Pin::input("default", "Default", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("value", "Value", PinType::Int),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// String to float
pub static STRING_TO_FLOAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/string_to_float",
    display_name: "String To Float",
    category: "String",
    description: "Parse a string as a float",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("0.0".into())),
        Pin::input("default", "Default", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("value", "Value", PinType::Float),
        Pin::output("success", "Success", PinType::Bool),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SPECIAL
// =============================================================================

/// Repeat string
pub static REPEAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/repeat",
    display_name: "Repeat",
    category: "String",
    description: "Repeat a string N times",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("count", "Count", PinType::Int).with_default(PinValue::Int(1)),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};

/// Reverse string
pub static REVERSE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "string/reverse",
    display_name: "Reverse",
    category: "String",
    description: "Reverse a string",
    create_pins: || vec![
        Pin::input("string", "String", PinType::String).with_default(PinValue::String("".into())),
        Pin::output("result", "Result", PinType::String),
    ],
    color: [200, 180, 100],
    is_event: false,
    is_comment: false,
};
