//! Ragdoll scripting bindings, owned by `renzora_ragdoll`.
//!
//! All of them act on the script's own entity;
//! `toggle::handle_ragdoll_script_actions` does the work. Reads go through
//! `get("Ragdoll.active")` like any other component field.

use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct RagdollScriptExtension;

impl ScriptExtension for RagdollScriptExtension {
    fn name(&self) -> &str {
        "ragdoll"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("enable_ragdoll", "enable_ragdoll")
                .doc("Hand the whole skeleton over to the physics solver.")
                .build(),
            Bind::action("disable_ragdoll", "disable_ragdoll")
                .doc("Return the skeleton to animation control, blending over the get-up time.")
                .build(),
            Bind::action("enable_partial_ragdoll", "enable_partial_ragdoll")
                .arg("bone", ParamKind::Str)
                .arg("blend", ParamKind::Float)
                .doc("Ragdoll only this bone's subtree, mixed with the animation by blend (0..1).")
                .build(),
            Bind::action("set_ragdoll_blend", "set_ragdoll_blend")
                .arg("blend", ParamKind::Float)
                .doc("How much of the pose the physics owns while ragdolling, 0..1.")
                .build(),
            Bind::action("set_ragdoll_get_up_time", "set_ragdoll_get_up_time")
                .arg("seconds", ParamKind::Float)
                .doc("Seconds to blend back to the animation when the ragdoll switches off.")
                .build(),
        ]
    }
}
