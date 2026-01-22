//! Script registry - stores all registered scripts

use bevy::prelude::*;
use std::collections::HashMap;

use super::{ScriptContext, ScriptVariableDefinition, ScriptVariables};

/// Trait that all game scripts must implement
pub trait GameScript: Send + Sync + 'static {
    /// Unique identifier for this script
    fn id(&self) -> &'static str;

    /// Display name shown in the editor
    fn name(&self) -> &'static str {
        self.id()
    }

    /// Description shown in the editor
    fn description(&self) -> &'static str {
        ""
    }

    /// Category for organization (e.g., "Movement", "Combat", "UI")
    fn category(&self) -> &'static str {
        "General"
    }

    /// Variables exposed in the inspector
    fn variables(&self) -> Vec<ScriptVariableDefinition> {
        Vec::new()
    }

    /// Called once when the entity is first created or the script is attached
    fn on_ready(&self, _ctx: &mut ScriptContext, _vars: &ScriptVariables) {}

    /// Called every frame
    fn on_update(&self, _ctx: &mut ScriptContext, _vars: &ScriptVariables) {}

    /// Called at a fixed timestep (for physics)
    fn on_fixed_update(&self, _ctx: &mut ScriptContext, _vars: &ScriptVariables) {}

    /// Called when the entity is destroyed
    fn on_destroy(&self, _ctx: &mut ScriptContext, _vars: &ScriptVariables) {}
}

/// Registry that holds all available scripts
#[derive(Resource, Default)]
pub struct ScriptRegistry {
    scripts: HashMap<String, Box<dyn GameScript>>,
    by_category: HashMap<String, Vec<String>>,
}

impl ScriptRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new script
    pub fn register<S: GameScript>(&mut self, script: S) {
        let id = script.id().to_string();
        let category = script.category().to_string();

        self.scripts.insert(id.clone(), Box::new(script));

        self.by_category
            .entry(category)
            .or_default()
            .push(id);
    }

    /// Get a script by ID
    pub fn get(&self, id: &str) -> Option<&dyn GameScript> {
        self.scripts.get(id).map(|b| b.as_ref())
    }

    /// Get all script IDs
    pub fn all_ids(&self) -> impl Iterator<Item = &String> {
        self.scripts.keys()
    }

    /// Get all scripts
    pub fn all(&self) -> impl Iterator<Item = (&String, &Box<dyn GameScript>)> {
        self.scripts.iter()
    }

    /// Get scripts by category
    pub fn by_category(&self, category: &str) -> Option<&Vec<String>> {
        self.by_category.get(category)
    }

    /// Get all categories
    pub fn categories(&self) -> impl Iterator<Item = &String> {
        self.by_category.keys()
    }

    /// Get the number of registered scripts
    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}

/// Macro to easily define a script
///
/// # Example
/// ```ignore
/// define_script! {
///     RotateScript {
///         id: "rotate",
///         name: "Rotate",
///         category: "Movement",
///         variables: [
///             ("speed", Float(1.0), "Rotation speed in degrees per second"),
///             ("axis", Vec3(Vec3::Y), "Rotation axis"),
///         ],
///         on_update: |ctx, vars| {
///             let speed = vars.get_float("speed").unwrap_or(1.0);
///             ctx.rotate_degrees(Vec3::new(0.0, speed * ctx.time.delta, 0.0));
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_script {
    (
        $name:ident {
            id: $id:expr,
            $(name: $display_name:expr,)?
            $(description: $desc:expr,)?
            $(category: $category:expr,)?
            $(variables: [$(($var_name:expr, $var_type:ident($var_default:expr) $(, $var_hint:expr)?)),* $(,)?],)?
            $(on_ready: $on_ready:expr,)?
            $(on_update: $on_update:expr,)?
            $(on_fixed_update: $on_fixed:expr,)?
            $(on_destroy: $on_destroy:expr,)?
        }
    ) => {
        pub struct $name;

        impl $crate::scripting::GameScript for $name {
            fn id(&self) -> &'static str { $id }

            $(fn name(&self) -> &'static str { $display_name })?
            $(fn description(&self) -> &'static str { $desc })?
            $(fn category(&self) -> &'static str { $category })?

            $(
            fn variables(&self) -> Vec<$crate::scripting::ScriptVariableDefinition> {
                vec![
                    $(
                        $crate::scripting::ScriptVariableDefinition::new(
                            $var_name,
                            $crate::scripting::ScriptValue::$var_type($var_default)
                        )
                        $(.with_hint($var_hint))?
                    ),*
                ]
            }
            )?

            $(
            fn on_ready(&self, ctx: &mut $crate::scripting::ScriptContext, vars: &$crate::scripting::ScriptVariables) {
                let f: fn(&mut $crate::scripting::ScriptContext, &$crate::scripting::ScriptVariables) = $on_ready;
                f(ctx, vars)
            }
            )?

            $(
            fn on_update(&self, ctx: &mut $crate::scripting::ScriptContext, vars: &$crate::scripting::ScriptVariables) {
                let f: fn(&mut $crate::scripting::ScriptContext, &$crate::scripting::ScriptVariables) = $on_update;
                f(ctx, vars)
            }
            )?

            $(
            fn on_fixed_update(&self, ctx: &mut $crate::scripting::ScriptContext, vars: &$crate::scripting::ScriptVariables) {
                let f: fn(&mut $crate::scripting::ScriptContext, &$crate::scripting::ScriptVariables) = $on_fixed;
                f(ctx, vars)
            }
            )?

            $(
            fn on_destroy(&self, ctx: &mut $crate::scripting::ScriptContext, vars: &$crate::scripting::ScriptVariables) {
                let f: fn(&mut $crate::scripting::ScriptContext, &$crate::scripting::ScriptVariables) = $on_destroy;
                f(ctx, vars)
            }
            )?
        }
    };
}

#[allow(unused_imports)]
pub use define_script;
