//! IK and constraint scripting bindings, owned by `renzora_ik`.
//!
//! Declared rather than written, so this crate links no interpreter and every
//! language backend gets the same functions. [`crate::script`] does the work.
//! Reads go through the reflection dispatcher like any component field:
//! `get("TwoBoneIk.weight")`.

use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct IkScriptExtension;

impl ScriptExtension for IkScriptExtension {
    fn name(&self) -> &str {
        "ik"
    }

    fn bindings(&self) -> Vec<Binding> {
        let bone = |b: Bind| b.arg("bone", ParamKind::Str);
        vec![
            bone(Bind::action("ik_set_target", "ik_set_target"))
                .vec3("target")
                .doc("Make a TwoBoneIk reach for a world position. Bone \"\" = every TwoBoneIk under this entity.")
                .build(),
            bone(Bind::action("ik_set_target_entity", "ik_set_target_entity"))
                .arg("target", ParamKind::Str)
                .doc("Make a TwoBoneIk reach for the entity with this name.")
                .build(),
            bone(Bind::action("ik_set_pole", "ik_set_pole"))
                .vec3("direction")
                .doc("Point a TwoBoneIk's elbow/knee this way, in the character's space.")
                .build(),
            bone(Bind::action("ik_set_weight", "ik_set_weight"))
                .arg("weight", ParamKind::Float)
                .doc("Blend a TwoBoneIk between the animation (0) and fully solved (1).")
                .build(),
            bone(Bind::action("look_at_set_target", "look_at_set_target"))
                .vec3("target")
                .doc("Make a LookAt aim at a world position.")
                .build(),
            bone(Bind::action("look_at_set_target_entity", "look_at_set_target_entity"))
                .arg("target", ParamKind::Str)
                .doc("Make a LookAt aim at the entity with this name.")
                .build(),
            bone(Bind::action("look_at_set_weight", "look_at_set_weight"))
                .arg("weight", ParamKind::Float)
                .doc("Blend a LookAt between the animation (0) and fully aimed (1).")
                .build(),
            Bind::action("foot_ik_set_weight", "foot_ik_set_weight")
                .arg("weight", ParamKind::Float)
                .doc("Blend foot planting on this character, e.g. 0 while airborne.")
                .build(),
            bone(Bind::action("copy_constraint_set_weight", "copy_constraint_set_weight"))
                .arg("weight", ParamKind::Float)
                .doc("Blend a CopyRotation/CopyPosition between off (0) and full (1).")
                .build(),
            bone(Bind::action("rotation_limit_set_weight", "rotation_limit_set_weight"))
                .arg("weight", ParamKind::Float)
                .doc("Blend a RotationLimit between ignored (0) and enforced (1).")
                .build(),
            bone(Bind::action("spring_bones_set_weight", "spring_bones_set_weight"))
                .arg("weight", ParamKind::Float)
                .doc("Blend a spring chain between the animation (0) and simulated (1). 0 switches it off.")
                .build(),
            bone(Bind::action("spring_bones_reset", "spring_bones_reset"))
                .doc("Snap a spring chain back to the animated pose, at rest.")
                .build(),
        ]
    }
}
