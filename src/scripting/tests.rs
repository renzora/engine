//! Tests for the scripting system
//!
//! Covers ScriptValue, ScriptVariables, ScriptVariableDefinition,
//! ScriptComponent, ScriptRegistry, and RhaiScriptEngine.

use super::*;
use bevy::prelude::*;

// =============================================================================
// A. ScriptValue
// =============================================================================

#[test]
fn script_value_type_name_float() {
    assert_eq!(ScriptValue::Float(1.0).type_name(), "Float");
}

#[test]
fn script_value_type_name_int() {
    assert_eq!(ScriptValue::Int(1).type_name(), "Int");
}

#[test]
fn script_value_type_name_bool() {
    assert_eq!(ScriptValue::Bool(true).type_name(), "Bool");
}

#[test]
fn script_value_type_name_string() {
    assert_eq!(ScriptValue::String("hi".into()).type_name(), "String");
}

#[test]
fn script_value_type_name_vec2() {
    assert_eq!(ScriptValue::Vec2(Vec2::ZERO).type_name(), "Vec2");
}

#[test]
fn script_value_type_name_vec3() {
    assert_eq!(ScriptValue::Vec3(Vec3::ZERO).type_name(), "Vec3");
}

#[test]
fn script_value_type_name_color() {
    assert_eq!(ScriptValue::Color(Vec4::ONE).type_name(), "Color");
}

#[test]
fn all_script_value_variants_construct() {
    // Just verify no panics
    let _ = ScriptValue::Float(0.0);
    let _ = ScriptValue::Int(0);
    let _ = ScriptValue::Bool(false);
    let _ = ScriptValue::String(String::new());
    let _ = ScriptValue::Vec2(Vec2::ZERO);
    let _ = ScriptValue::Vec3(Vec3::ZERO);
    let _ = ScriptValue::Color(Vec4::ONE);
}

// =============================================================================
// B. ScriptVariables
// =============================================================================

#[test]
fn script_variables_set_then_get() {
    let mut vars = ScriptVariables::default();
    vars.set("speed", ScriptValue::Float(5.0));
    assert!(vars.get("speed").is_some());
}

#[test]
fn script_variables_get_float() {
    let mut vars = ScriptVariables::default();
    vars.set("speed", ScriptValue::Float(5.0));
    assert_eq!(vars.get_float("speed"), Some(5.0));

    vars.set("name", ScriptValue::String("test".into()));
    assert_eq!(vars.get_float("name"), None);
}

#[test]
fn script_variables_get_int() {
    let mut vars = ScriptVariables::default();
    vars.set("count", ScriptValue::Int(42));
    assert_eq!(vars.get_int("count"), Some(42));
    assert_eq!(vars.get_int("missing"), None);
}

#[test]
fn script_variables_get_bool() {
    let mut vars = ScriptVariables::default();
    vars.set("active", ScriptValue::Bool(true));
    assert_eq!(vars.get_bool("active"), Some(true));
    assert_eq!(vars.get_bool("missing"), None);
}

#[test]
fn script_variables_get_string() {
    let mut vars = ScriptVariables::default();
    vars.set("name", ScriptValue::String("hello".into()));
    assert_eq!(vars.get_string("name"), Some("hello"));
    assert_eq!(vars.get_string("missing"), None);
}

#[test]
fn script_variables_get_vec3() {
    let mut vars = ScriptVariables::default();
    vars.set("pos", ScriptValue::Vec3(Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(vars.get_vec3("pos"), Some(Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(vars.get_vec3("missing"), None);
}

#[test]
fn script_variables_get_missing_key() {
    let vars = ScriptVariables::default();
    assert!(vars.get("nonexistent").is_none());
}

#[test]
fn script_variables_iter() {
    let mut vars = ScriptVariables::default();
    vars.set("a", ScriptValue::Float(1.0));
    vars.set("b", ScriptValue::Int(2));
    let items: Vec<_> = vars.iter().collect();
    assert_eq!(items.len(), 2);
}

#[test]
fn script_variables_set_overwrites() {
    let mut vars = ScriptVariables::default();
    vars.set("x", ScriptValue::Float(1.0));
    vars.set("x", ScriptValue::Float(2.0));
    assert_eq!(vars.get_float("x"), Some(2.0));
}

// =============================================================================
// C. ScriptVariableDefinition
// =============================================================================

#[test]
fn script_variable_definition_new() {
    let def = ScriptVariableDefinition::new("speed", ScriptValue::Float(5.0));
    assert_eq!(def.name, "speed");
    assert_eq!(def.display_name, "speed");
    assert!(def.hint.is_none());
}

#[test]
fn script_variable_definition_with_display_name() {
    let def = ScriptVariableDefinition::new("speed", ScriptValue::Float(5.0))
        .with_display_name("Movement Speed");
    assert_eq!(def.display_name, "Movement Speed");
}

#[test]
fn script_variable_definition_with_hint() {
    let def = ScriptVariableDefinition::new("speed", ScriptValue::Float(5.0))
        .with_hint("range:0,100");
    assert_eq!(def.hint, Some("range:0,100".into()));
}

// =============================================================================
// D. ScriptComponent
// =============================================================================

#[test]
fn script_component_new() {
    let comp = ScriptComponent::with_script("my_script");
    assert_eq!(comp.script_id(), "my_script");
    assert_eq!(comp.scripts.len(), 1);
    assert!(comp.scripts[0].enabled);
    assert!(!comp.is_file_script());
}

#[test]
fn script_component_from_file() {
    let comp = ScriptComponent::from_file(std::path::PathBuf::from("scripts/test.rhai"));
    assert!(comp.is_file_script());
    assert_eq!(comp.scripts.len(), 1);
    assert!(comp.scripts[0].enabled);
    assert_eq!(comp.scripts[0].script_path, Some(std::path::PathBuf::from("scripts/test.rhai")));
}

#[test]
fn script_component_with_variable() {
    let comp = ScriptComponent::with_script("test")
        .with_variable("speed", ScriptValue::Float(10.0));
    assert_eq!(comp.scripts[0].variables.get_float("speed"), Some(10.0));
}

#[test]
fn script_component_is_file_script() {
    let file_comp = ScriptComponent::from_file("test.rhai".into());
    assert!(file_comp.is_file_script());

    let id_comp = ScriptComponent::with_script("test");
    assert!(!id_comp.is_file_script());
}

#[test]
fn script_component_multi_script() {
    let mut comp = ScriptComponent::new();
    assert!(comp.scripts.is_empty());

    comp.add_file_script("script_a.rhai".into());
    comp.add_file_script("script_b.rhai".into());
    assert_eq!(comp.scripts.len(), 2);

    comp.remove_script(0);
    assert_eq!(comp.scripts.len(), 1);
    assert_eq!(comp.scripts[0].script_path, Some(std::path::PathBuf::from("script_b.rhai")));
}

// =============================================================================
// E. ScriptRegistry
// =============================================================================

// A minimal test script for registry tests
struct TestScript {
    id: &'static str,
    category: &'static str,
}

impl GameScript for TestScript {
    fn id(&self) -> &'static str { self.id }
    fn category(&self) -> &'static str { self.category }
}

#[test]
fn script_registry_new_is_empty() {
    let registry = ScriptRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn script_registry_register_and_get() {
    let mut registry = ScriptRegistry::new();
    registry.register(TestScript { id: "test_script", category: "Test" });
    assert_eq!(registry.len(), 1);
    assert!(registry.get("test_script").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn script_registry_by_category() {
    let mut registry = ScriptRegistry::new();
    registry.register(TestScript { id: "a", category: "Movement" });
    registry.register(TestScript { id: "b", category: "Movement" });
    registry.register(TestScript { id: "c", category: "Combat" });

    let movement = registry.by_category("Movement");
    assert!(movement.is_some());
    assert_eq!(movement.unwrap().len(), 2);

    let combat = registry.by_category("Combat");
    assert!(combat.is_some());
    assert_eq!(combat.unwrap().len(), 1);
}

#[test]
fn script_registry_len_and_is_empty() {
    let mut registry = ScriptRegistry::new();
    assert!(registry.is_empty());
    registry.register(TestScript { id: "a", category: "Test" });
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
}

// =============================================================================
// F. RhaiScriptEngine
// =============================================================================

#[test]
fn rhai_engine_creates_successfully() {
    let engine = RhaiScriptEngine::new();
    // Just verify it doesn't panic
    let _ = engine;
}

#[test]
fn rhai_engine_valid_source_compiles() {
    // Use rhai::Engine directly since RhaiScriptEngine.engine is private
    let engine = rhai::Engine::new();
    let result = engine.compile("let x = 42;");
    assert!(result.is_ok(), "Valid Rhai source should compile: {:?}", result.err());
}

#[test]
fn rhai_engine_invalid_source_returns_error() {
    let engine = rhai::Engine::new();
    let result = engine.compile("let x = ;; invalid{{{}}}");
    assert!(result.is_err(), "Invalid Rhai source should fail to compile");
}
