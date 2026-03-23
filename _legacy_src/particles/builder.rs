//! Effect builder - converts HanabiEffectDefinition to bevy_hanabi EffectAsset
//!
//! This module provides the conversion from our serializable effect definition
//! to the actual bevy_hanabi effect asset that can be used for rendering.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;
use bevy_hanabi::AlphaMode as HanabiAlphaMode;

use super::data::{
    HanabiEffectDefinition, HanabiEmitShape, SpawnMode, VelocityMode,
    ShapeDimension as OurShapeDimension,
    SimulationSpace as OurSimulationSpace,
    SimulationCondition as OurSimulationCondition,
    ParticleAlphaMode, ParticleOrientMode, ParticleColorBlendMode,
    MotionIntegrationMode, KillZone,
};

fn map_dimension(dim: OurShapeDimension) -> ShapeDimension {
    match dim {
        OurShapeDimension::Volume => ShapeDimension::Volume,
        OurShapeDimension::Surface => ShapeDimension::Surface,
    }
}

/// Build a complete EffectAsset with all modifiers from our definition
pub fn build_complete_effect(def: &HanabiEffectDefinition) -> EffectAsset {
    let writer = ExprWriter::new();

    // ========================================================================
    // Resolve effective speed range
    // ========================================================================
    let (speed_min, speed_max) = if def.velocity_speed_max > 0.0 {
        let lo = def.velocity_speed_min.min(def.velocity_speed_max);
        let hi = def.velocity_speed_min.max(def.velocity_speed_max);
        (lo, hi)
    } else {
        (def.velocity_magnitude, def.velocity_magnitude)
    };

    // ========================================================================
    // Lifetime
    // ========================================================================
    let lifetime_expr = writer.lit(def.lifetime_min).uniform(writer.lit(def.lifetime_max)).expr();

    // ========================================================================
    // Position expressions (shape-specific)
    // ========================================================================
    enum PosModifier {
        Attribute(ExprHandle),
        Circle { center: ExprHandle, axis: ExprHandle, radius: ExprHandle, dim: ShapeDimension },
        Sphere { center: ExprHandle, radius: ExprHandle, dim: ShapeDimension },
        Cone { height: ExprHandle, base_radius: ExprHandle, top_radius: ExprHandle, dim: ShapeDimension },
    }

    let pos_modifier = match &def.emit_shape {
        HanabiEmitShape::Point => {
            PosModifier::Attribute(writer.lit(Vec3::ZERO).expr())
        }
        HanabiEmitShape::Circle { radius, dimension } => {
            PosModifier::Circle {
                center: writer.lit(Vec3::ZERO).expr(),
                axis: writer.lit(Vec3::Y).expr(),
                radius: writer.lit(*radius).expr(),
                dim: map_dimension(*dimension),
            }
        }
        HanabiEmitShape::Sphere { radius, dimension } => {
            PosModifier::Sphere {
                center: writer.lit(Vec3::ZERO).expr(),
                radius: writer.lit(*radius).expr(),
                dim: map_dimension(*dimension),
            }
        }
        HanabiEmitShape::Cone { base_radius, top_radius, height, dimension } => {
            PosModifier::Cone {
                height: writer.lit(*height).expr(),
                base_radius: writer.lit(*base_radius).expr(),
                top_radius: writer.lit(*top_radius).expr(),
                dim: map_dimension(*dimension),
            }
        }
        HanabiEmitShape::Rect { half_extents, .. } => {
            let half = Vec3::new(half_extents[0], half_extents[1], 0.0);
            PosModifier::Attribute(writer.lit(-half).uniform(writer.lit(half)).expr())
        }
        HanabiEmitShape::Box { half_extents } => {
            let half = Vec3::from_array(*half_extents);
            PosModifier::Attribute(writer.lit(-half).uniform(writer.lit(half)).expr())
        }
    };

    // ========================================================================
    // Velocity expressions (mode-specific)
    // ========================================================================
    enum VelModifier {
        Attribute(ExprHandle),
        Sphere { center: ExprHandle, speed: ExprHandle },
        Tangent { origin: ExprHandle, axis: ExprHandle, speed: ExprHandle },
    }

    let vel_modifier = match def.velocity_mode {
        VelocityMode::Directional => {
            let dir = Vec3::from_array(def.velocity_direction).normalize_or_zero();
            if def.velocity_spread > 0.001 {
                let avg_speed = (speed_min + speed_max) / 2.0;
                let spread_range = avg_speed * def.velocity_spread.sin();
                let vel_min = dir * speed_min - Vec3::splat(spread_range);
                let vel_max = dir * speed_max + Vec3::splat(spread_range);
                VelModifier::Attribute(writer.lit(vel_min).uniform(writer.lit(vel_max)).expr())
            } else if speed_min != speed_max {
                VelModifier::Attribute(
                    writer.lit(dir * speed_min).uniform(writer.lit(dir * speed_max)).expr()
                )
            } else {
                VelModifier::Attribute(writer.lit(dir * speed_min).expr())
            }
        }
        VelocityMode::Radial => {
            let speed_expr = if speed_min != speed_max {
                writer.lit(speed_min).uniform(writer.lit(speed_max)).expr()
            } else {
                writer.lit(speed_min).expr()
            };
            VelModifier::Sphere {
                center: writer.lit(Vec3::ZERO).expr(),
                speed: speed_expr,
            }
        }
        VelocityMode::Tangent => {
            let speed_expr = if speed_min != speed_max {
                writer.lit(speed_min).uniform(writer.lit(speed_max)).expr()
            } else {
                writer.lit(speed_min).expr()
            };
            VelModifier::Tangent {
                origin: writer.lit(Vec3::ZERO).expr(),
                axis: writer.lit(Vec3::from_array(def.velocity_axis)).expr(),
                speed: speed_expr,
            }
        }
        VelocityMode::Random => {
            let speed_expr = if speed_min != speed_max {
                writer.lit(speed_min).uniform(writer.lit(speed_max)).expr()
            } else {
                writer.lit(speed_min).expr()
            };
            VelModifier::Sphere {
                center: writer.lit(Vec3::ZERO).expr(),
                speed: speed_expr,
            }
        }
    };

    // ========================================================================
    // Size init expression
    // ========================================================================
    let size_init_expr = if def.size_start_min > 0.0 && def.size_start_max > 0.0
        && (def.size_start_min - def.size_start_max).abs() > 0.0001
    {
        let lo = def.size_start_min.min(def.size_start_max);
        let hi = def.size_start_min.max(def.size_start_max);
        writer.lit(lo).uniform(writer.lit(hi)).expr()
    } else {
        writer.lit(def.size_start).expr()
    };

    // ========================================================================
    // Force expressions
    // ========================================================================
    let accel_expr = writer.lit(Vec3::from_array(def.acceleration)).expr();

    let drag_expr = if def.linear_drag > 0.0 {
        Some(writer.lit(def.linear_drag).expr())
    } else {
        None
    };

    let radial_accel_data = if def.radial_acceleration.abs() > 0.001 {
        Some((
            writer.lit(Vec3::ZERO).expr(),
            writer.lit(def.radial_acceleration).expr(),
        ))
    } else {
        None
    };

    let tangent_accel_data = if def.tangent_acceleration.abs() > 0.001 {
        Some((
            writer.lit(Vec3::ZERO).expr(),
            writer.lit(Vec3::from_array(def.tangent_accel_axis)).expr(),
            writer.lit(def.tangent_acceleration).expr(),
        ))
    } else {
        None
    };

    // ========================================================================
    // Conform-to-sphere expressions
    // ========================================================================
    let conform_data = def.conform_to_sphere.as_ref().map(|c| {
        let origin = writer.lit(Vec3::from_array(c.origin)).expr();
        let radius = writer.lit(c.radius).expr();
        let influence = writer.lit(c.influence_dist).expr();
        let accel = writer.lit(c.attraction_accel).expr();
        let max_speed = writer.lit(c.max_attraction_speed).expr();
        let thickness = writer.lit(c.shell_half_thickness).expr();
        let sticky = writer.lit(c.sticky_factor).expr();
        (origin, radius, influence, accel, max_speed, thickness, sticky)
    });

    // ========================================================================
    // Kill zone expressions
    // ========================================================================
    let kill_zone_data: Vec<_> = def.kill_zones.iter().map(|zone| match zone {
        KillZone::Sphere { center, radius, kill_inside } => {
            let c = writer.lit(Vec3::from_array(*center)).expr();
            let sqr_r = writer.lit(radius * radius).expr();
            (KillZoneType::Sphere(c, sqr_r), *kill_inside)
        }
        KillZone::Aabb { center, half_size, kill_inside } => {
            let c = writer.lit(Vec3::from_array(*center)).expr();
            let h = writer.lit(Vec3::from_array(*half_size)).expr();
            (KillZoneType::Aabb(c, h), *kill_inside)
        }
    }).collect();

    // ========================================================================
    // Render expressions
    // ========================================================================
    let mask_threshold_expr = if def.alpha_mode == ParticleAlphaMode::Mask {
        Some(writer.lit(def.alpha_mask_threshold).expr())
    } else {
        None
    };

    let rotation_expr = if def.rotation_speed.abs() > 0.001 {
        Some((writer.attr(Attribute::AGE) * writer.lit(def.rotation_speed)).expr())
    } else {
        None
    };

    let roundness_expr = if def.roundness > 0.001 {
        Some(writer.lit(def.roundness).expr())
    } else {
        None
    };

    let flipbook_sprite_init = if def.flipbook.is_some() {
        Some(writer.lit(0.0f32).expr())
    } else {
        None
    };

    let texture_slot_expr = if def.texture_path.is_some() {
        Some(writer.lit(0u32).expr())
    } else {
        None
    };

    // Ensure mask threshold always has a value if needed
    let mask_threshold_expr = if def.alpha_mode == ParticleAlphaMode::Mask && mask_threshold_expr.is_none() {
        Some(writer.lit(0.5f32).expr())
    } else {
        mask_threshold_expr
    };

    // ========================================================================
    // Finish the expression writer → Module
    // ========================================================================
    let module = writer.finish();

    // ========================================================================
    // Build spawner
    // ========================================================================
    let spawner = build_spawner(def);

    // ========================================================================
    // Assemble the EffectAsset
    // ========================================================================
    let mut effect = EffectAsset::new(def.capacity, spawner, module)
        .with_name(&def.name);

    // ---- Init modifiers ----

    // Lifetime
    effect = effect.init(SetAttributeModifier::new(Attribute::LIFETIME, lifetime_expr));

    // Position
    match pos_modifier {
        PosModifier::Attribute(expr) => {
            effect = effect.init(SetAttributeModifier::new(Attribute::POSITION, expr));
        }
        PosModifier::Circle { center, axis, radius, dim } => {
            effect = effect.init(SetPositionCircleModifier {
                center, axis, radius, dimension: dim,
            });
        }
        PosModifier::Sphere { center, radius, dim } => {
            effect = effect.init(SetPositionSphereModifier {
                center, radius, dimension: dim,
            });
        }
        PosModifier::Cone { height, base_radius, top_radius, dim } => {
            effect = effect.init(SetPositionCone3dModifier {
                height, base_radius, top_radius, dimension: dim,
            });
        }
    }

    // Velocity
    match vel_modifier {
        VelModifier::Attribute(expr) => {
            effect = effect.init(SetAttributeModifier::new(Attribute::VELOCITY, expr));
        }
        VelModifier::Sphere { center, speed } => {
            effect = effect.init(SetVelocitySphereModifier { center, speed });
        }
        VelModifier::Tangent { origin, axis, speed } => {
            effect = effect.init(SetVelocityTangentModifier { origin, axis, speed });
        }
    }

    // Size init
    effect = effect.init(SetAttributeModifier::new(Attribute::SIZE, size_init_expr));

    // Flipbook sprite index init
    if let Some(sprite_expr) = flipbook_sprite_init {
        effect = effect.init(SetAttributeModifier::new(Attribute::SPRITE_INDEX, sprite_expr));
    }

    // ---- Update modifiers ----

    // Acceleration (always applied, even if zero - cheap and consistent)
    effect = effect.update(AccelModifier::new(accel_expr));

    // Linear drag
    if let Some(drag) = drag_expr {
        effect = effect.update(LinearDragModifier::new(drag));
    }

    // Radial acceleration
    if let Some((origin, accel)) = radial_accel_data {
        effect = effect.update(RadialAccelModifier::new(origin, accel));
    }

    // Tangent acceleration
    if let Some((origin, axis, accel)) = tangent_accel_data {
        effect = effect.update(TangentAccelModifier::new(origin, axis, accel));
    }

    // Conform to sphere
    if let Some((origin, radius, influence, accel, max_speed, thickness, sticky)) = conform_data {
        let mut conform = ConformToSphereModifier::new(origin, radius, influence, accel, max_speed);
        conform.shell_half_thickness = Some(thickness);
        conform.sticky_factor = Some(sticky);
        effect = effect.update(conform);
    }

    // Kill zones
    for (zone_type, kill_inside) in &kill_zone_data {
        match zone_type {
            KillZoneType::Sphere(center, sqr_radius) => {
                effect = effect.update(
                    KillSphereModifier::new(*center, *sqr_radius).with_kill_inside(*kill_inside)
                );
            }
            KillZoneType::Aabb(center, half_size) => {
                effect = effect.update(
                    KillAabbModifier::new(*center, *half_size).with_kill_inside(*kill_inside)
                );
            }
        }
    }

    // ---- Render modifiers ----

    // Size over lifetime
    let size_gradient = if def.size_non_uniform {
        HanabiGradient::linear(
            Vec3::new(def.size_start_x, def.size_start_y, 0.0),
            Vec3::new(def.size_end_x, def.size_end_y, 0.0),
        )
    } else {
        HanabiGradient::linear(
            Vec3::splat(def.size_start),
            Vec3::splat(def.size_end),
        )
    };
    effect = effect.render(SizeOverLifetimeModifier {
        gradient: size_gradient,
        screen_space_size: def.screen_space_size,
    });

    // Screen-space size modifier
    if def.screen_space_size {
        effect = effect.render(ScreenSpaceSizeModifier);
    }

    // Color
    let color_blend = match def.color_blend_mode {
        ParticleColorBlendMode::Modulate => ColorBlendMode::Modulate,
        ParticleColorBlendMode::Overwrite => ColorBlendMode::Overwrite,
        ParticleColorBlendMode::Add => ColorBlendMode::Add,
    };

    let hdr_mult = if def.use_hdr_color { def.hdr_intensity.max(1.0) } else { 1.0 };

    if def.use_flat_color {
        let c = Vec4::new(
            def.flat_color[0] * hdr_mult,
            def.flat_color[1] * hdr_mult,
            def.flat_color[2] * hdr_mult,
            def.flat_color[3],
        );
        effect = effect.render(SetColorModifier {
            color: c.into(),
            blend: color_blend,
            mask: ColorBlendMask::RGBA,
        });
    } else {
        let mut color_gradient: HanabiGradient<Vec4> = HanabiGradient::new();
        for stop in &def.color_gradient {
            let c = Vec4::new(
                stop.color[0] * hdr_mult,
                stop.color[1] * hdr_mult,
                stop.color[2] * hdr_mult,
                stop.color[3],
            );
            color_gradient.add_key(stop.position, c);
        }
        effect = effect.render(ColorOverLifetimeModifier {
            gradient: color_gradient,
            blend: color_blend,
            mask: ColorBlendMask::RGBA,
        });
    }

    // Orient modifier
    let orient_mode = match def.orient_mode {
        ParticleOrientMode::ParallelCameraDepthPlane => OrientMode::ParallelCameraDepthPlane,
        ParticleOrientMode::FaceCameraPosition => OrientMode::FaceCameraPosition,
        ParticleOrientMode::AlongVelocity => OrientMode::AlongVelocity,
    };
    let mut orient = OrientModifier::new(orient_mode);
    if let Some(rot) = rotation_expr {
        orient = orient.with_rotation(rot);
    }
    effect = effect.render(orient);

    // Roundness
    if let Some(roundness) = roundness_expr {
        effect = effect.render(RoundModifier { roundness });
    }

    // Flipbook
    if let Some(ref fb) = def.flipbook {
        effect = effect.render(FlipbookModifier {
            sprite_grid_size: UVec2::new(fb.grid_columns, fb.grid_rows),
        });
    }

    // Texture
    if let Some(tex_slot) = texture_slot_expr {
        // ParticleTextureModifier tells the shader to sample the texture
        // The actual texture handle is bound by the sync system
        effect = effect.render(ParticleTextureModifier::new(tex_slot));
    }

    // ---- Asset-level configuration ----

    // Alpha mode (use bevy_hanabi's AlphaMode, not bevy's)
    let alpha_mode = match def.alpha_mode {
        ParticleAlphaMode::Blend => HanabiAlphaMode::Blend,
        ParticleAlphaMode::Premultiply => HanabiAlphaMode::Premultiply,
        ParticleAlphaMode::Add => HanabiAlphaMode::Add,
        ParticleAlphaMode::Multiply => HanabiAlphaMode::Multiply,
        ParticleAlphaMode::Mask => {
            // mask_threshold_expr is guaranteed to be Some at this point
            HanabiAlphaMode::Mask(mask_threshold_expr.unwrap())
        }
        ParticleAlphaMode::Opaque => HanabiAlphaMode::Opaque,
    };
    effect = effect.with_alpha_mode(alpha_mode);

    // Simulation space
    let sim_space = match def.simulation_space {
        OurSimulationSpace::Local => SimulationSpace::Local,
        OurSimulationSpace::World => SimulationSpace::Global,
    };
    effect = effect.with_simulation_space(sim_space);

    // Simulation condition
    let sim_cond = match def.simulation_condition {
        OurSimulationCondition::Always => SimulationCondition::Always,
        OurSimulationCondition::WhenVisible => SimulationCondition::WhenVisible,
    };
    effect = effect.with_simulation_condition(sim_cond);

    // Motion integration
    let motion = match def.motion_integration {
        MotionIntegrationMode::PostUpdate => MotionIntegration::PostUpdate,
        MotionIntegrationMode::PreUpdate => MotionIntegration::PreUpdate,
        MotionIntegrationMode::None => MotionIntegration::None,
    };
    effect = effect.with_motion_integration(motion);

    effect
}

/// Build the spawner settings from the effect definition
fn build_spawner(def: &HanabiEffectDefinition) -> SpawnerSettings {
    let has_custom_duration = def.spawn_duration > 0.0;
    let has_custom_cycles = def.spawn_cycle_count > 0;

    let spawner = match def.spawn_mode {
        SpawnMode::Rate => {
            if has_custom_duration || has_custom_cycles {
                let duration = if has_custom_duration { def.spawn_duration } else { f32::MAX };
                SpawnerSettings::new(
                    def.spawn_rate.into(),
                    duration.into(),
                    duration.into(),
                    def.spawn_cycle_count,
                )
            } else {
                SpawnerSettings::rate(def.spawn_rate.into())
            }
        }
        SpawnMode::Burst => {
            if has_custom_cycles && def.spawn_cycle_count != 1 {
                SpawnerSettings::new(
                    (def.spawn_count as f32).into(),
                    0.0.into(),
                    0.0.into(),
                    def.spawn_cycle_count,
                )
            } else {
                SpawnerSettings::once((def.spawn_count as f32).into())
            }
        }
        SpawnMode::BurstRate => {
            let period = if def.spawn_rate > 0.0 { 1.0 / def.spawn_rate } else { 1.0 };
            if has_custom_cycles {
                SpawnerSettings::new(
                    (def.spawn_count as f32).into(),
                    0.0.into(),
                    period.into(),
                    def.spawn_cycle_count,
                )
            } else {
                SpawnerSettings::burst((def.spawn_count as f32).into(), period.into())
            }
        }
    };

    if def.spawn_starts_active {
        spawner
    } else {
        spawner.with_starts_active(false)
    }
}

// Helper enum for kill zone expression data
enum KillZoneType {
    Sphere(ExprHandle, ExprHandle),
    Aabb(ExprHandle, ExprHandle),
}
