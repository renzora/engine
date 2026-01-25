//! 2D/UI rendering system for the editor viewport
//!
//! This module handles rendering 2D and UI nodes in the editor's 2D viewport mode.
//! It creates sprite/mesh representations for nodes that have data components but
//! no visual representation.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::camera::visibility::RenderLayers;

use crate::core::{ViewportMode, ViewportState};
use crate::gizmo::GIZMO_RENDER_LAYER;
use crate::shared::{
    Camera2DData, Sprite2DData, UIButtonData, UIImageData, UILabelData, UIPanelData,
};

/// Marker component for editor-generated 2D visuals
#[derive(Component)]
pub struct Editor2DVisual {
    /// The entity this visual belongs to
    pub source_entity: Entity,
}

/// System that creates and updates visual representations for 2D/UI nodes
pub fn update_2d_visuals(
    mut commands: Commands,
    viewport: Res<ViewportState>,
    // Query for entities with data components but no visual marker
    panels: Query<(Entity, &Transform, &UIPanelData), Without<Editor2DVisual>>,
    labels: Query<(Entity, &Transform, &UILabelData), Without<Editor2DVisual>>,
    buttons: Query<(Entity, &Transform, &UIButtonData), Without<Editor2DVisual>>,
    images: Query<(Entity, &Transform, &UIImageData), Without<Editor2DVisual>>,
    sprites: Query<(Entity, &Transform, &Sprite2DData), Without<Editor2DVisual>>,
    cameras_2d: Query<(Entity, &Transform, &Camera2DData), Without<Editor2DVisual>>,
    // Query for existing visuals to update
    mut existing_visuals: Query<(Entity, &Editor2DVisual, &mut Transform, &mut Sprite), Without<UIPanelData>>,
    // Query source entities for updates
    panel_sources: Query<(&Transform, &UIPanelData)>,
    // Clean up orphaned visuals
    mut orphan_check: Local<u32>,
) {
    // Only create visuals in 2D mode
    if viewport.viewport_mode != ViewportMode::Mode2D {
        return;
    }

    // Create visuals for UI Panels
    for (entity, transform, panel) in panels.iter() {
        let color = Color::srgba(
            panel.background_color.x,
            panel.background_color.y,
            panel.background_color.z,
            panel.background_color.w,
        );

        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(panel.width, panel.height)),
                ..default()
            },
            Anchor::CENTER,
            Transform::from_translation(transform.translation),
            Visibility::default(),
            Editor2DVisual {
                source_entity: entity,
            },
            RenderLayers::layer(GIZMO_RENDER_LAYER),
        ));
    }

    // Create visuals for UI Labels
    for (entity, transform, label) in labels.iter() {
        // For labels, we'll create a small indicator sprite
        // Text rendering would require more complex setup
        let color = Color::srgba(
            label.color.x,
            label.color.y,
            label.color.z,
            label.color.w * 0.5, // Semi-transparent indicator
        );

        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(label.font_size * 4.0, label.font_size)),
                ..default()
            },
            Anchor::CENTER_LEFT,
            Transform::from_translation(transform.translation),
            Visibility::default(),
            Editor2DVisual {
                source_entity: entity,
            },
            RenderLayers::layer(GIZMO_RENDER_LAYER),
        ));
    }

    // Create visuals for UI Buttons
    for (entity, transform, button) in buttons.iter() {
        let color = Color::srgba(
            button.normal_color.x,
            button.normal_color.y,
            button.normal_color.z,
            button.normal_color.w,
        );

        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(button.width, button.height)),
                ..default()
            },
            Anchor::CENTER,
            Transform::from_translation(transform.translation),
            Visibility::default(),
            Editor2DVisual {
                source_entity: entity,
            },
            RenderLayers::layer(GIZMO_RENDER_LAYER),
        ));
    }

    // Create visuals for UI Images
    for (entity, transform, image) in images.iter() {
        let color = Color::srgba(
            image.tint.x,
            image.tint.y,
            image.tint.z,
            image.tint.w,
        );

        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(image.width, image.height)),
                ..default()
            },
            Anchor::CENTER,
            Transform::from_translation(transform.translation),
            Visibility::default(),
            Editor2DVisual {
                source_entity: entity,
            },
            RenderLayers::layer(GIZMO_RENDER_LAYER),
        ));
    }

    // Create visuals for Sprite2D (placeholder when no texture)
    for (entity, transform, sprite) in sprites.iter() {
        let color = Color::srgba(
            sprite.color.x,
            sprite.color.y,
            sprite.color.z,
            sprite.color.w,
        );

        // Convert anchor from [0,1] range to [-0.5, 0.5] range for Bevy's Anchor
        let anchor_offset = sprite.anchor - Vec2::new(0.5, 0.5);

        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(64.0, 64.0)), // Default size for placeholder
                flip_x: sprite.flip_x,
                flip_y: sprite.flip_y,
                ..default()
            },
            Anchor(anchor_offset),
            Transform::from_translation(transform.translation),
            Visibility::default(),
            Editor2DVisual {
                source_entity: entity,
            },
            RenderLayers::layer(GIZMO_RENDER_LAYER),
        ));
    }

    // Create visuals for Camera2D (icon indicator)
    for (entity, transform, _camera) in cameras_2d.iter() {
        commands.spawn((
            Sprite {
                color: Color::srgba(0.3, 0.7, 1.0, 0.8), // Blue camera indicator
                custom_size: Some(Vec2::new(40.0, 30.0)),
                ..default()
            },
            Anchor::CENTER,
            Transform::from_translation(transform.translation),
            Visibility::default(),
            Editor2DVisual {
                source_entity: entity,
            },
            RenderLayers::layer(GIZMO_RENDER_LAYER),
        ));
    }

    // Update existing visuals to follow their source entities
    for (visual_entity, visual, mut visual_transform, mut sprite) in existing_visuals.iter_mut() {
        if let Ok((source_transform, panel)) = panel_sources.get(visual.source_entity) {
            // Update position
            visual_transform.translation = source_transform.translation;
            visual_transform.rotation = source_transform.rotation;
            visual_transform.scale = source_transform.scale;

            // Update size and color
            sprite.custom_size = Some(Vec2::new(panel.width, panel.height));
            sprite.color = Color::srgba(
                panel.background_color.x,
                panel.background_color.y,
                panel.background_color.z,
                panel.background_color.w,
            );
        } else {
            // Source entity no longer exists, mark for cleanup
            *orphan_check += 1;
            if *orphan_check % 60 == 0 {
                // Periodic cleanup
                commands.entity(visual_entity).despawn();
            }
        }
    }
}

/// System to clean up visuals when their source entities are removed or mode changes
pub fn cleanup_2d_visuals(
    mut commands: Commands,
    viewport: Res<ViewportState>,
    visuals: Query<(Entity, &Editor2DVisual)>,
    source_exists: Query<Entity>,
) {
    // Clean up all visuals when switching away from 2D mode
    if viewport.is_changed() && viewport.viewport_mode != ViewportMode::Mode2D {
        for (entity, _) in visuals.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    // Clean up orphaned visuals (source entity was deleted)
    for (entity, visual) in visuals.iter() {
        if source_exists.get(visual.source_entity).is_err() {
            commands.entity(entity).despawn();
        }
    }
}
