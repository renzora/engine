//! Parkour scripting bindings — owned by `renzora_parkour`.
//!
//! Declared rather than written, so this crate compiles no interpreter and
//! every language backend gets the same functions.
//!
//! These five replace `move_controller` for a parkour character: the controller
//! owns gravity, ground contact and collision itself, because a hang and a
//! swing are positions it has to *set*, not forces it can ask for. A script
//! that keeps calling `move_controller` on the same entity is fighting it.
//!
//! Reads are not here. `ParkourReadState` is a reflected component, so
//! `get("ParkourReadState.can_vault")` already works through the generic
//! dispatcher — see [`crate::read_state`].

use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct ParkourScriptExtension;

impl ScriptExtension for ParkourScriptExtension {
    fn name(&self) -> &str {
        "parkour"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("parkour_move", "parkour_move")
                .xyz()
                .doc("Movement intent in world space; x/z steer, y climbs. Call every frame.")
                .build(),
            Bind::action("parkour_sprint", "parkour_sprint")
                .arg("on", ParamKind::Bool)
                .doc("Hold to move at run speed instead of walk speed.")
                .build(),
            Bind::action("parkour_jump", "parkour_jump")
                .doc("Jump, wall-jump, climb up from a hang, or let go of a swing.")
                .build(),
            Bind::action("parkour_action", "parkour_action")
                .doc("Context traversal: vault, mantle, grab a ledge, mount a ladder, grab a rope.")
                .build(),
            Bind::action("parkour_release", "parkour_release")
                .doc("Let go of whatever is being held — ledge, ladder or swing.")
                .build(),
        ]
    }
}
