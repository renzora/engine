//! Effect builder - converts HanabiEffectDefinition to bevy_hanabi EffectAsset
//!
//! This module provides the conversion from our serializable effect definition
//! to the actual bevy_hanabi effect asset that can be used for rendering.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;

use super::data::{
    HanabiEffectDefinition, HanabiEmitShape, SpawnMode, VelocityMode,
    ShapeDimension as OurShapeDimension,
};

/// Build a complete EffectAsset with all modifiers from our definition
pub fn build_complete_effect(def: &HanabiEffectDefinition) -> EffectAsset {
    let writer = ExprWriter::new();

    // Build spawner based on spawn mode
    let spawner = match def.spawn_mode {
        SpawnMode::Rate => SpawnerSettings::rate(def.spawn_rate.into()),
        SpawnMode::Burst => SpawnerSettings::once((def.spawn_count as f32).into()),
        SpawnMode::BurstRate => SpawnerSettings::burst((def.spawn_count as f32).into(), (1.0 / def.spawn_rate).into()),
    };

    // Create lifetime expression
    let lifetime = writer.lit(def.lifetime_min).uniform(writer.lit(def.lifetime_max));
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime.expr());

    // Create position based on shape
    let init_pos = match &def.emit_shape {
        HanabiEmitShape::Point => {
            SetPositionSphereModifier {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(0.001).expr(),
                dimension: ShapeDimension::Volume,
            }
        }
        HanabiEmitShape::Circle { radius, dimension } => {
            let dim = match dimension {
                OurShapeDimension::Volume => ShapeDimension::Volume,
                OurShapeDimension::Surface => ShapeDimension::Surface,
            };
            SetPositionSphereModifier {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(*radius).expr(),
                dimension: dim,
            }
        }
        HanabiEmitShape::Sphere { radius, dimension } => {
            let dim = match dimension {
                OurShapeDimension::Volume => ShapeDimension::Volume,
                OurShapeDimension::Surface => ShapeDimension::Surface,
            };
            SetPositionSphereModifier {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(*radius).expr(),
                dimension: dim,
            }
        }
        HanabiEmitShape::Cone { base_radius, .. } => {
            // Approximate cone with sphere
            SetPositionSphereModifier {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(*base_radius).expr(),
                dimension: ShapeDimension::Volume,
            }
        }
        HanabiEmitShape::Rect { half_extents, .. } => {
            // Approximate rect with sphere
            let radius = half_extents[0].max(half_extents[1]);
            SetPositionSphereModifier {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(radius).expr(),
                dimension: ShapeDimension::Volume,
            }
        }
        HanabiEmitShape::Box { half_extents } => {
            // Approximate box with sphere
            let radius = (half_extents[0].powi(2) + half_extents[1].powi(2) + half_extents[2].powi(2)).sqrt();
            SetPositionSphereModifier {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(radius).expr(),
                dimension: ShapeDimension::Volume,
            }
        }
    };

    // Create velocity
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(def.velocity_magnitude).expr(),
    };

    // Create size
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(def.size_start).expr(),
    );

    // Acceleration
    let accel = Vec3::from_array(def.acceleration);
    let update_accel = AccelModifier::new(writer.lit(accel).expr());

    // Size over lifetime - use Vec3 for size
    let size_gradient = HanabiGradient::linear(
        Vec3::splat(def.size_start),
        Vec3::splat(def.size_end),
    );
    let render_size = SizeOverLifetimeModifier {
        gradient: size_gradient,
        screen_space_size: false,
    };

    // Color over lifetime
    let mut color_gradient: HanabiGradient<Vec4> = HanabiGradient::new();
    for stop in &def.color_gradient {
        color_gradient.add_key(stop.position, Vec4::from_array(stop.color));
    }
    let render_color = ColorOverLifetimeModifier {
        gradient: color_gradient,
        blend: ColorBlendMode::Modulate,
        mask: ColorBlendMask::RGBA,
    };

    let module = writer.finish();

    EffectAsset::new(def.capacity, spawner, module)
        .with_name(&def.name)
        .init(init_lifetime)
        .init(init_pos)
        .init(init_vel)
        .init(init_size)
        .update(update_accel)
        .render(render_size)
        .render(render_color)
}
