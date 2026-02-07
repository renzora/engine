//! Tests for the component system
//!
//! Covers ComponentCategory metadata, ComponentDefinition queries,
//! ComponentRegistry operations, and entity presets.

use super::definition::*;
use super::registry::*;
use super::presets::*;

// =============================================================================
// Helpers
// =============================================================================

/// Create a test ComponentDefinition with the given type_id and category
fn test_def(type_id: &'static str, category: ComponentCategory, priority: i32) -> ComponentDefinition {
    ComponentDefinition {
        type_id,
        display_name: type_id,
        category,
        icon: "",
        priority,
        add_fn: |_, _, _, _| {},
        remove_fn: |_, _| {},
        has_fn: |_, _| false,
        serialize_fn: |_, _| None,
        deserialize_fn: |_, _, _, _| {},
        inspector_fn: |_, _, _, _, _| false,
        conflicts_with: &[],
        requires: &[],
    }
}

/// Create a test def with conflicts_with
fn test_def_with_conflicts(
    type_id: &'static str,
    category: ComponentCategory,
    conflicts: &'static [&'static str],
    requires: &'static [&'static str],
) -> ComponentDefinition {
    ComponentDefinition {
        type_id,
        display_name: type_id,
        category,
        icon: "",
        priority: 0,
        add_fn: |_, _, _, _| {},
        remove_fn: |_, _| {},
        has_fn: |_, _| false,
        serialize_fn: |_, _| None,
        deserialize_fn: |_, _, _, _| {},
        inspector_fn: |_, _, _, _, _| false,
        conflicts_with: conflicts,
        requires,
    }
}

// =============================================================================
// A. ComponentCategory
// =============================================================================

#[test]
fn category_display_name_non_empty() {
    let categories = ComponentCategory::all_in_order();
    for cat in categories {
        assert!(!cat.display_name().is_empty(), "{:?} should have a display name", cat);
    }
}

#[test]
fn category_icon_non_empty() {
    let categories = ComponentCategory::all_in_order();
    for cat in categories {
        assert!(!cat.icon().is_empty(), "{:?} should have an icon", cat);
    }
}

#[test]
fn category_count() {
    let categories = ComponentCategory::all_in_order();
    assert_eq!(categories.len(), 10, "Expected 10 categories");
}

#[test]
fn category_all_unique() {
    let categories = ComponentCategory::all_in_order();
    for i in 0..categories.len() {
        for j in (i + 1)..categories.len() {
            assert_ne!(categories[i], categories[j], "Duplicate category");
        }
    }
}

// =============================================================================
// B. ComponentDefinition
// =============================================================================

#[test]
fn definition_conflicts_with_type() {
    let def = test_def_with_conflicts("test", ComponentCategory::Rendering, &["other_type"], &[]);
    assert!(def.conflicts_with_type("other_type"));
    assert!(!def.conflicts_with_type("unrelated"));
}

#[test]
fn definition_requires_type() {
    let def = test_def_with_conflicts("test", ComponentCategory::Rendering, &[], &["required_type"]);
    assert!(def.requires_type("required_type"));
    assert!(!def.requires_type("unrelated"));
}

#[test]
fn definition_no_conflicts_or_requires() {
    let def = test_def("test", ComponentCategory::Rendering, 0);
    assert!(!def.conflicts_with_type("anything"));
    assert!(!def.requires_type("anything"));
}

// =============================================================================
// C. ComponentRegistry
// =============================================================================

#[test]
fn registry_new_is_empty() {
    let registry = ComponentRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn registry_register_and_get() {
    let mut registry = ComponentRegistry::new();
    registry.register_owned(test_def("test_comp", ComponentCategory::Rendering, 0));
    assert_eq!(registry.len(), 1);
    assert!(registry.get("test_comp").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn registry_get_by_category() {
    let mut registry = ComponentRegistry::new();
    registry.register_owned(test_def("light1", ComponentCategory::Lighting, 0));
    registry.register_owned(test_def("light2", ComponentCategory::Lighting, 1));
    registry.register_owned(test_def("mesh1", ComponentCategory::Rendering, 0));

    let lights = registry.get_by_category(ComponentCategory::Lighting);
    assert_eq!(lights.len(), 2);

    let rendering = registry.get_by_category(ComponentCategory::Rendering);
    assert_eq!(rendering.len(), 1);

    let physics = registry.get_by_category(ComponentCategory::Physics);
    assert_eq!(physics.len(), 0);
}

#[test]
fn registry_categories_with_components() {
    let mut registry = ComponentRegistry::new();
    registry.register_owned(test_def("light1", ComponentCategory::Lighting, 0));
    registry.register_owned(test_def("mesh1", ComponentCategory::Rendering, 0));

    let cats: Vec<_> = registry.categories_with_components().collect();
    assert!(cats.contains(&ComponentCategory::Lighting));
    assert!(cats.contains(&ComponentCategory::Rendering));
    assert!(!cats.contains(&ComponentCategory::Physics));
}

#[test]
fn registry_priority_ordering() {
    let mut registry = ComponentRegistry::new();
    registry.register_owned(test_def("low_priority", ComponentCategory::Rendering, 10));
    registry.register_owned(test_def("high_priority", ComponentCategory::Rendering, 0));

    let defs = registry.get_by_category(ComponentCategory::Rendering);
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].type_id, "high_priority");
    assert_eq!(defs[1].type_id, "low_priority");
}

// =============================================================================
// D. EntityPresets
// =============================================================================

#[test]
fn preset_categories_have_display_names() {
    let categories = PresetCategory::all_in_order();
    for cat in categories {
        assert!(!cat.display_name().is_empty(), "{:?} should have a display name", cat);
    }
}

#[test]
fn preset_categories_have_icons() {
    let categories = PresetCategory::all_in_order();
    for cat in categories {
        assert!(!cat.icon().is_empty(), "{:?} should have an icon", cat);
    }
}

#[test]
fn preset_categories_all_unique() {
    let categories = PresetCategory::all_in_order();
    for i in 0..categories.len() {
        for j in (i + 1)..categories.len() {
            assert_ne!(categories[i], categories[j], "Duplicate preset category");
        }
    }
}
