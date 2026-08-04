//! Physics scripting bindings — owned by `renzora_physics`.
//!
//! Declared rather than written, so this crate compiles no interpreter and
//! every language backend gets the same functions. Advanced users can still
//! call `action("apply_force", {x=1, y=0, z=0})` directly; these are sugar over
//! exactly that.
//!
//! Reads are not here: `grounded` and friends go through
//! `get("PhysicsReadState.*")`, which the generic reflect-path dispatcher
//! already handles for every component.

use renzora_scripting::extension::{Bind, Binding, ScriptExtension};

pub struct PhysicsScriptExtension;

impl ScriptExtension for PhysicsScriptExtension {
    fn name(&self) -> &str {
        "physics"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("move_controller", "kinematic_slide")
                .xyz()
                .doc("Move a kinematic controller with collide-and-slide.")
                .build(),
            Bind::action("apply_force", "apply_force")
                .xyz()
                .doc("Apply a continuous force in world space.")
                .build(),
            Bind::action("apply_impulse", "apply_impulse")
                .xyz()
                .doc("Apply an instantaneous impulse in world space.")
                .build(),
            Bind::action("set_linear_velocity", "set_velocity")
                .xyz()
                .doc("Set the body's linear velocity directly.")
                .build(),
        ]
    }
}
