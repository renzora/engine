//! Inspector registration for AnimatorComponent.

use renzora::InspectorEntry;

use renzora_animation::AnimatorComponent;

pub fn animator_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "animator",
        display_name: "Animator",
        icon: "play",
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
        fields: vec![renzora::float_field!(
            "Blend Duration",
            AnimatorComponent,
            blend_duration,
            0.01,
            0.0,
            5.0
        )],
    }
}

/// Follow Node: a camera (or anything) riding a named node, such as the
/// `<camera>_view` node an imported fly-through brings with it.
pub fn follow_node_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "follow_node",
        display_name: "Follow Node",
        icon: "link",
        category: "animation",
        has_fn: |world, entity| world.get::<renzora::FollowNode>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(renzora::FollowNode::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<renzora::FollowNode>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![renzora::string_field!("Target", renzora::FollowNode, target)],
    }
}
