//! Inspector registration for AnimatorComponent.

use renzora_editor::InspectorEntry;

use crate::component::AnimatorComponent;

pub fn animator_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "animator",
        display_name: "Animator",
        icon: egui_phosphor::regular::PLAY,
        category: "animation",
        has_fn: |world, entity| world.get::<AnimatorComponent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AnimatorComponent::new());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AnimatorComponent>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![renzora_editor::float_field!(
            "Blend Duration",
            AnimatorComponent,
            blend_duration,
            0.01,
            0.0,
            5.0
        )],
    }
}
