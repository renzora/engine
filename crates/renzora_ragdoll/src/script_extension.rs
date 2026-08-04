//! Ragdoll scripting bindings — owned by `renzora_ragdoll`.
//!
//! Both toggle ragdoll simulation on the script's own entity;
//! `toggle::handle_ragdoll_script_actions` does the work. Reads go through
//! `get("Ragdoll.active")` like any other component field.

use renzora_scripting::extension::{Bind, Binding, ScriptExtension};

pub struct RagdollScriptExtension;

impl ScriptExtension for RagdollScriptExtension {
    fn name(&self) -> &str {
        "ragdoll"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("enable_ragdoll", "enable_ragdoll")
                .doc("Hand the skeleton over to the physics solver.")
                .build(),
            Bind::action("disable_ragdoll", "disable_ragdoll")
                .doc("Return the skeleton to animation control.")
                .build(),
        ]
    }
}
