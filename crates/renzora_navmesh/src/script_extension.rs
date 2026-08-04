//! Navigation scripting bindings — owned by `renzora_navmesh`.
//!
//! Reads (`has_path`, `distance_to_destination`, `is_at_destination`) go
//! through `get("NavReadState.*")` on the auto-mirrored component, so only the
//! two mutations need declaring.

use renzora_scripting::extension::{Bind, Binding, ScriptExtension};

pub struct NavScriptExtension;

impl ScriptExtension for NavScriptExtension {
    fn name(&self) -> &str {
        "navigation"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            // The three coordinates become one `target` argument rather than
            // three, which is what the existing action handler reads.
            Bind::action("nav_set_destination", "nav_set_destination")
                .vec3("target")
                .doc("Path the agent to a world position.")
                .build(),
            Bind::action("nav_clear_destination", "nav_clear_destination")
                .doc("Cancel the current path.")
                .build(),
            Bind::action("nav_stop", "nav_clear_destination")
                .doc("Alias for nav_clear_destination.")
                .build(),
        ]
    }
}
