//! Effect builder - converts HanabiEffectDefinition to bevy_hanabi EffectAsset

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::AlphaMode as HanabiAlphaMode;
use bevy_hanabi::Gradient as HanabiGradient;
use bevy_hanabi::{ErosionModifier, ImageSampleMapping};

use crate::data::{
    HanabiEffectDefinition, HanabiEmitShape, KillZone, MotionIntegrationMode, ParticleAlphaMode,
    ParticleColorBlendMode, ParticleOrientMode, ShapeDimension as OurShapeDimension,
    SimulationCondition as OurSimulationCondition, SimulationSpace as OurSimulationSpace,
    SpawnMode, VelocityMode,
};

fn map_dimension(dim: OurShapeDimension) -> ShapeDimension {
    match dim {
        OurShapeDimension::Volume => ShapeDimension::Volume,
        OurShapeDimension::Surface => ShapeDimension::Surface,
    }
}

/// Approximate blackbody radiation colour for a temperature in Kelvin, returned
/// as linear-ish RGB in 0..1 (Tanner Helland's well-known approximation). Used
/// for physically-plausible fire colour ramps.
fn kelvin_to_rgb(kelvin: f32) -> [f32; 3] {
    let t = (kelvin / 100.0).clamp(10.0, 400.0);
    let r = if t <= 66.0 {
        255.0
    } else {
        (329.698_73 * (t - 60.0).powf(-0.133_204_76)).clamp(0.0, 255.0)
    };
    let g = if t <= 66.0 {
        (99.470_8 * t.ln() - 161.119_57).clamp(0.0, 255.0)
    } else {
        (288.122_17 * (t - 60.0).powf(-0.075_514_85)).clamp(0.0, 255.0)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        (138.517_73 * (t - 10.0).ln() - 305.044_8).clamp(0.0, 255.0)
    };
    [r / 255.0, g / 255.0, b / 255.0]
}

/// Build a complete EffectAsset with all modifiers from our definition.
pub fn build_complete_effect(def: &HanabiEffectDefinition) -> EffectAsset {
    let writer = ExprWriter::new();

    // Resolve effective speed range
    let (speed_min, speed_max) = if def.velocity_speed_max > 0.0 {
        let lo = def.velocity_speed_min.min(def.velocity_speed_max);
        let hi = def.velocity_speed_min.max(def.velocity_speed_max);
        (lo, hi)
    } else {
        (def.velocity_magnitude, def.velocity_magnitude)
    };

    // Lifetime
    let lifetime_expr = writer
        .lit(def.lifetime_min)
        .uniform(writer.lit(def.lifetime_max))
        .expr();

    // Position
    enum PosModifier {
        Attribute(ExprHandle),
        Circle {
            center: ExprHandle,
            axis: ExprHandle,
            radius: ExprHandle,
            dim: ShapeDimension,
        },
        Sphere {
            center: ExprHandle,
            radius: ExprHandle,
            dim: ShapeDimension,
        },
        Cone {
            height: ExprHandle,
            base_radius: ExprHandle,
            top_radius: ExprHandle,
            dim: ShapeDimension,
        },
    }

    // 2D effects live in the XY plane (the 2D camera looks down -Z): circles
    // face the camera instead of lying flat on the 3D ground plane, and
    // spheres flatten to a disc so particles don't scatter along Z — which is
    // sprite sort order in 2D, not visible depth.
    let circle_axis = if def.plane_2d { Vec3::Z } else { Vec3::Y };

    let pos_modifier = match &def.emit_shape {
        HanabiEmitShape::Point => PosModifier::Attribute(writer.lit(Vec3::ZERO).expr()),
        HanabiEmitShape::Circle { radius, dimension } => PosModifier::Circle {
            center: writer.lit(Vec3::ZERO).expr(),
            axis: writer.lit(circle_axis).expr(),
            radius: writer.lit(*radius).expr(),
            dim: map_dimension(*dimension),
        },
        HanabiEmitShape::Sphere { radius, dimension } if def.plane_2d => PosModifier::Circle {
            center: writer.lit(Vec3::ZERO).expr(),
            axis: writer.lit(Vec3::Z).expr(),
            radius: writer.lit(*radius).expr(),
            dim: map_dimension(*dimension),
        },
        HanabiEmitShape::Sphere { radius, dimension } => PosModifier::Sphere {
            center: writer.lit(Vec3::ZERO).expr(),
            radius: writer.lit(*radius).expr(),
            dim: map_dimension(*dimension),
        },
        HanabiEmitShape::Cone {
            base_radius,
            top_radius,
            height,
            dimension,
        } => PosModifier::Cone {
            height: writer.lit(*height).expr(),
            base_radius: writer.lit(*base_radius).expr(),
            top_radius: writer.lit(*top_radius).expr(),
            dim: map_dimension(*dimension),
        },
        HanabiEmitShape::Rect { half_extents, .. } => {
            let half = Vec3::new(half_extents[0], half_extents[1], 0.0);
            PosModifier::Attribute(writer.lit(-half).uniform(writer.lit(half)).expr())
        }
        HanabiEmitShape::Box { half_extents } => {
            let half = Vec3::from_array(*half_extents);
            PosModifier::Attribute(writer.lit(-half).uniform(writer.lit(half)).expr())
        }
    };

    // Velocity
    enum VelModifier {
        Attribute(ExprHandle),
        Sphere {
            center: ExprHandle,
            speed: ExprHandle,
        },
        Circle {
            center: ExprHandle,
            axis: ExprHandle,
            speed: ExprHandle,
        },
        Tangent {
            origin: ExprHandle,
            axis: ExprHandle,
            speed: ExprHandle,
        },
    }

    let vel_modifier = match def.velocity_mode {
        VelocityMode::Directional => {
            let dir = Vec3::from_array(def.velocity_direction).normalize_or_zero();
            if def.velocity_spread > 0.001 {
                let avg_speed = (speed_min + speed_max) / 2.0;
                let spread_range = avg_speed * def.velocity_spread.sin();
                // In 2D, spread must stay in-plane: a Z component would move
                // particles through the sprite sort order instead of visibly.
                let spread = if def.plane_2d {
                    Vec3::new(spread_range, spread_range, 0.0)
                } else {
                    Vec3::splat(spread_range)
                };
                let vel_min = dir * speed_min - spread;
                let vel_max = dir * speed_max + spread;
                VelModifier::Attribute(writer.lit(vel_min).uniform(writer.lit(vel_max)).expr())
            } else if speed_min != speed_max {
                VelModifier::Attribute(
                    writer
                        .lit(dir * speed_min)
                        .uniform(writer.lit(dir * speed_max))
                        .expr(),
                )
            } else {
                VelModifier::Attribute(writer.lit(dir * speed_min).expr())
            }
        }
        VelocityMode::Radial | VelocityMode::Random => {
            let speed_expr = if speed_min != speed_max {
                writer.lit(speed_min).uniform(writer.lit(speed_max)).expr()
            } else {
                writer.lit(speed_min).expr()
            };
            if def.plane_2d {
                // In-plane burst: a sphere would waste a third of the motion
                // on the invisible Z axis (and churn the 2D sort order).
                VelModifier::Circle {
                    center: writer.lit(Vec3::ZERO).expr(),
                    axis: writer.lit(Vec3::Z).expr(),
                    speed: speed_expr,
                }
            } else {
                VelModifier::Sphere {
                    center: writer.lit(Vec3::ZERO).expr(),
                    speed: speed_expr,
                }
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
    };

    // Size init
    let size_init_expr = if def.size_start_min > 0.0
        && def.size_start_max > 0.0
        && (def.size_start_min - def.size_start_max).abs() > 0.0001
    {
        let lo = def.size_start_min.min(def.size_start_max);
        let hi = def.size_start_min.max(def.size_start_max);
        writer.lit(lo).uniform(writer.lit(hi)).expr()
    } else {
        writer.lit(def.size_start).expr()
    };

    // Forces
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

    // Conform-to-sphere
    let conform_data = def.conform_to_sphere.as_ref().map(|c| {
        (
            writer.lit(Vec3::from_array(c.origin)).expr(),
            writer.lit(c.radius).expr(),
            writer.lit(c.influence_dist).expr(),
            writer.lit(c.attraction_accel).expr(),
            writer.lit(c.max_attraction_speed).expr(),
            writer.lit(c.shell_half_thickness).expr(),
            writer.lit(c.sticky_factor).expr(),
        )
    });

    // Attractor force fields: each becomes a ConformToSphereModifier (the modern
    // replacement for the removed multi-source ForceFieldModifier).
    let attractor_data: Vec<_> = def
        .attractors
        .iter()
        .map(|a| {
            (
                writer.lit(Vec3::from_array(a.position)).expr(),
                writer.lit(a.radius.max(0.0)).expr(),
                writer.lit(a.influence_dist).expr(),
                writer.lit(a.strength).expr(),
                writer.lit(a.max_speed).expr(),
            )
        })
        .collect();

    // Noise turbulence
    let noise_data = if def.noise_amplitude > 0.001 && def.noise_frequency > 0.001 {
        Some((
            writer.lit(def.noise_frequency).expr(),
            writer.lit(def.noise_amplitude).expr(),
        ))
    } else {
        None
    };

    // Orbit
    let orbit_data = def.orbit.as_ref().map(|o| {
        (
            writer.lit(Vec3::from_array(o.center)).expr(),
            writer
                .lit(Vec3::from_array(o.axis).normalize_or_zero())
                .expr(),
            writer.lit(o.speed).expr(),
            writer.lit(o.radial_pull).expr(),
            writer.lit(o.orbit_radius).expr(),
        )
    });

    // Velocity limit
    let vel_limit_expr = if def.velocity_limit > 0.001 {
        Some(writer.lit(def.velocity_limit).expr())
    } else {
        None
    };

    // Kill zones
    let kill_zone_data: Vec<_> = def
        .kill_zones
        .iter()
        .map(|zone| match zone {
            KillZone::Sphere {
                center,
                radius,
                kill_inside,
            } => {
                let c = writer.lit(Vec3::from_array(*center)).expr();
                let sqr_r = writer.lit(radius * radius).expr();
                (KillZoneType::Sphere(c, sqr_r), *kill_inside)
            }
            KillZone::Aabb {
                center,
                half_size,
                kill_inside,
            } => {
                let c = writer.lit(Vec3::from_array(*center)).expr();
                let h = writer.lit(Vec3::from_array(*half_size)).expr();
                (KillZoneType::Aabb(c, h), *kill_inside)
            }
        })
        .collect();

    // Render expressions
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

    // Every effect samples the built-in soft sprite (slot 0), bound per-entity via
    // `EffectMaterial`, so particles render as soft round blobs instead of hard
    // squares. `ImageSampleMapping::Modulate` multiplies all RGBA channels, which
    // softens both additive (RGB falloff) and alpha-blended (alpha falloff) effects.
    let texture_slot_expr = writer.lit(0u32).expr();
    // Erosion noise lives in texture slot 1 (declared on the module below only
    // when erosion is enabled, and bound as the 2nd EffectMaterial image).
    let erosion_slot_expr = writer.lit(1u32).expr();

    // Ribbon / trail: the engine activates its ribbon render path automatically
    // when RIBBON_ID is present in the particle layout. AGE must also be present
    // (ribbons are ordered by age), so we init both explicitly.
    let ribbon_inits = if def.ribbon.is_some() {
        Some((
            writer.lit(0.0f32).expr(), // AGE = 0 at spawn
            writer.lit(0u32).expr(),   // single continuous ribbon (id 0)
        ))
    } else {
        None
    };

    // Finish writer
    let mut module = writer.finish();
    // Declare the soft-sprite texture slot (slot 0); bound per-entity via EffectMaterial.
    module.add_texture_slot("color");
    // Erosion noise slot (1), only when used. Slot order must match the
    // `EffectMaterial::images` order set when spawning the effect.
    if def.erosion {
        module.add_texture_slot("erosion_noise");
    }

    // Build spawner
    let spawner = build_spawner(def);

    // Assemble EffectAsset
    let mut effect = EffectAsset::new(def.capacity, spawner, module).with_name(&def.name);

    // Init modifiers
    effect = effect.init(SetAttributeModifier::new(
        Attribute::LIFETIME,
        lifetime_expr,
    ));

    match pos_modifier {
        PosModifier::Attribute(expr) => {
            effect = effect.init(SetAttributeModifier::new(Attribute::POSITION, expr));
        }
        PosModifier::Circle {
            center,
            axis,
            radius,
            dim,
        } => {
            effect = effect.init(SetPositionCircleModifier {
                center,
                axis,
                radius,
                dimension: dim,
            });
        }
        PosModifier::Sphere {
            center,
            radius,
            dim,
        } => {
            effect = effect.init(SetPositionSphereModifier {
                center,
                radius,
                dimension: dim,
            });
        }
        PosModifier::Cone {
            height,
            base_radius,
            top_radius,
            dim,
        } => {
            effect = effect.init(SetPositionCone3dModifier {
                height,
                base_radius,
                top_radius,
                dimension: dim,
            });
        }
    }

    match vel_modifier {
        VelModifier::Attribute(expr) => {
            effect = effect.init(SetAttributeModifier::new(Attribute::VELOCITY, expr));
        }
        VelModifier::Sphere { center, speed } => {
            effect = effect.init(SetVelocitySphereModifier { center, speed });
        }
        VelModifier::Circle {
            center,
            axis,
            speed,
        } => {
            effect = effect.init(SetVelocityCircleModifier {
                center,
                axis,
                speed,
            });
        }
        VelModifier::Tangent {
            origin,
            axis,
            speed,
        } => {
            effect = effect.init(SetVelocityTangentModifier {
                origin,
                axis,
                speed,
            });
        }
    }

    effect = effect.init(SetAttributeModifier::new(Attribute::SIZE, size_init_expr));

    if let Some((age_expr, ribbon_id_expr)) = ribbon_inits {
        effect = effect.init(SetAttributeModifier::new(Attribute::AGE, age_expr));
        effect = effect.init(SetAttributeModifier::new(Attribute::RIBBON_ID, ribbon_id_expr));
    }

    if let Some(sprite_expr) = flipbook_sprite_init {
        effect = effect.init(SetAttributeModifier::new(
            Attribute::SPRITE_INDEX,
            sprite_expr,
        ));
    }

    // Update modifiers
    effect = effect.update(AccelModifier::new(accel_expr));

    if let Some(drag) = drag_expr {
        effect = effect.update(LinearDragModifier::new(drag));
    }
    if let Some((origin, accel)) = radial_accel_data {
        effect = effect.update(RadialAccelModifier::new(origin, accel));
    }
    if let Some((origin, axis, accel)) = tangent_accel_data {
        effect = effect.update(TangentAccelModifier::new(origin, axis, accel));
    }
    if let Some((origin, radius, influence, accel, max_speed, thickness, sticky)) = conform_data {
        let mut conform = ConformToSphereModifier::new(origin, radius, influence, accel, max_speed);
        conform.shell_half_thickness = Some(thickness);
        conform.sticky_factor = Some(sticky);
        effect = effect.update(conform);
    }
    for (origin, radius, influence, accel, max_speed) in attractor_data {
        effect = effect.update(ConformToSphereModifier::new(
            origin, radius, influence, accel, max_speed,
        ));
    }
    for (zone_type, kill_inside) in &kill_zone_data {
        match zone_type {
            KillZoneType::Sphere(center, sqr_radius) => {
                effect = effect.update(
                    KillSphereModifier::new(*center, *sqr_radius).with_kill_inside(*kill_inside),
                );
            }
            KillZoneType::Aabb(center, half_size) => {
                effect = effect.update(
                    KillAabbModifier::new(*center, *half_size).with_kill_inside(*kill_inside),
                );
            }
        }
    }
    if let Some((freq, amp)) = noise_data {
        effect = effect.update(NoiseTurbulenceModifier {
            frequency: freq,
            amplitude: amp,
            octaves: def.noise_octaves,
            lacunarity: def.noise_lacunarity,
            planar: def.plane_2d,
        });
    }
    if let Some((center, axis, speed, radial_pull, orbit_radius)) = orbit_data {
        effect = effect.update(OrbitModifier {
            center,
            axis,
            speed,
            radial_pull,
            orbit_radius,
        });
    }
    if let Some(max_speed) = vel_limit_expr {
        effect = effect.update(VelocityLimitModifier { max_speed });
    }

    // Render modifiers
    // Multi-key size curve takes priority; falls back to linear start->end
    // (uniform or non-uniform) when no curve is authored.
    let size_gradient = if !def.size_curve.is_empty() {
        let mut g: HanabiGradient<Vec3> = HanabiGradient::new();
        for p in &def.size_curve {
            g.add_key(p.time.clamp(0.0, 1.0), Vec3::splat(p.value.max(0.0)));
        }
        g
    } else if def.size_non_uniform {
        HanabiGradient::linear(
            Vec3::new(def.size_start_x, def.size_start_y, 0.0),
            Vec3::new(def.size_end_x, def.size_end_y, 0.0),
        )
    } else {
        HanabiGradient::linear(Vec3::splat(def.size_start), Vec3::splat(def.size_end))
    };
    effect = effect.render(SizeOverLifetimeModifier {
        gradient: size_gradient,
        screen_space_size: def.screen_space_size,
    });

    if def.screen_space_size {
        effect = effect.render(ScreenSpaceSizeModifier);
    }

    // Color
    let color_blend = match def.color_blend_mode {
        ParticleColorBlendMode::Modulate => ColorBlendMode::Modulate,
        ParticleColorBlendMode::Overwrite => ColorBlendMode::Overwrite,
        ParticleColorBlendMode::Add => ColorBlendMode::Add,
    };
    let hdr_mult = if def.use_hdr_color {
        def.hdr_intensity.max(1.0)
    } else {
        1.0
    };

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
    } else if let Some([t_start, t_end]) = def.blackbody {
        // Physically-based fire colour from blackbody temperature (Kelvin) over
        // life: hot/white at birth -> cooler red -> fade out. Overrides the
        // authored color_gradient when set.
        let mut g: HanabiGradient<Vec4> = HanabiGradient::new();
        let steps = 6;
        for i in 0..=steps {
            let f = i as f32 / steps as f32;
            let k = t_start + (t_end - t_start) * f;
            let [r, gc, b] = kelvin_to_rgb(k);
            let a = (1.0 - f * f).clamp(0.0, 1.0); // ease alpha to 0 by end of life
            g.add_key(f, Vec4::new(r * hdr_mult, gc * hdr_mult, b * hdr_mult, a));
        }
        effect = effect.render(ColorOverLifetimeModifier {
            gradient: g,
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

    // Orient
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

    if let Some(roundness) = roundness_expr {
        effect = effect.render(RoundModifier { roundness });
    }
    if let Some(ref fb) = def.flipbook {
        effect = effect.render(FlipbookModifier {
            sprite_grid_size: UVec2::new(fb.grid_columns, fb.grid_rows),
        });
    }
    let mut tex_mod = ParticleTextureModifier::new(texture_slot_expr);
    tex_mod.sample_mapping = ImageSampleMapping::Modulate;
    effect = effect.render(tex_mod);

    // Erosion/dissolve: a second texture slot (1) holds grayscale noise; the
    // ErosionModifier dissolves the particle as it fades. Added last so it sees
    // the final colour. The noise image is bound as the 2nd EffectMaterial image.
    if def.erosion {
        effect = effect.render(ErosionModifier::new(erosion_slot_expr));
    }

    // Alpha mode
    let alpha_mode = match def.alpha_mode {
        ParticleAlphaMode::Blend => HanabiAlphaMode::Blend,
        ParticleAlphaMode::Premultiply => HanabiAlphaMode::Premultiply,
        ParticleAlphaMode::Add => HanabiAlphaMode::Add,
        ParticleAlphaMode::Multiply => HanabiAlphaMode::Multiply,
        ParticleAlphaMode::Mask => HanabiAlphaMode::Mask(mask_threshold_expr.unwrap()),
        ParticleAlphaMode::Opaque => HanabiAlphaMode::Opaque,
    };
    effect = effect.with_alpha_mode(alpha_mode);

    // Simulation
    let sim_space = match def.simulation_space {
        OurSimulationSpace::Local => SimulationSpace::Local,
        OurSimulationSpace::World => SimulationSpace::Global,
    };
    effect = effect.with_simulation_space(sim_space);

    let sim_cond = match def.simulation_condition {
        OurSimulationCondition::Always => SimulationCondition::Always,
        OurSimulationCondition::WhenVisible => SimulationCondition::WhenVisible,
    };
    effect = effect.with_simulation_condition(sim_cond);

    let motion = match def.motion_integration {
        MotionIntegrationMode::PostUpdate => MotionIntegration::PostUpdate,
        MotionIntegrationMode::PreUpdate => MotionIntegration::PreUpdate,
        MotionIntegrationMode::None => MotionIntegration::None,
    };
    effect = effect.with_motion_integration(motion);

    effect
}

fn build_spawner(def: &HanabiEffectDefinition) -> SpawnerSettings {
    let has_custom_duration = def.spawn_duration > 0.0;
    let has_custom_cycles = def.spawn_cycle_count > 0;

    let spawner = match def.spawn_mode {
        SpawnMode::Rate => {
            if has_custom_duration || has_custom_cycles {
                let duration = if has_custom_duration {
                    def.spawn_duration
                } else {
                    f32::MAX
                };
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
            let period = if def.spawn_rate > 0.0 {
                1.0 / def.spawn_rate
            } else {
                1.0
            };
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

enum KillZoneType {
    Sphere(ExprHandle, ExprHandle),
    Aabb(ExprHandle, ExprHandle),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::HanabiEffectDefinition;

    /// Every shipped drakkar port must parse against the current schema and build
    /// into an EffectAsset without panicking. Also exercises size curves,
    /// attractors, and ribbons (the features added alongside these effects).
    #[test]
    fn drakkar_effects_parse_and_build() {
        let files: &[(&str, &str)] = &[
            ("fire", include_str!("../../../assets/particles/drakkar_fire.particle")),
            ("bolt", include_str!("../../../assets/particles/drakkar_bolt.particle")),
            ("fire_vortex", include_str!("../../../assets/particles/drakkar_fire_vortex.particle")),
            ("sparks_fire", include_str!("../../../assets/particles/drakkar_sparks_fire.particle")),
            ("sparks_magic", include_str!("../../../assets/particles/drakkar_sparks_magic.particle")),
            ("fire_ring", include_str!("../../../assets/particles/drakkar_fire_ring.particle")),
            ("loot_aura", include_str!("../../../assets/particles/drakkar_loot_aura.particle")),
            ("aura", include_str!("../../../assets/particles/drakkar_aura.particle")),
        ];

        for (name, src) in files {
            let def: HanabiEffectDefinition = ron::from_str(src)
                .unwrap_or_else(|e| panic!("drakkar_{name}.particle failed to parse: {e}"));
            // Must not panic while lowering to a bevy_hanabi EffectAsset.
            let _ = build_complete_effect(&def);
        }
    }

    /// EVERY shipped `.particle` file in `assets/particles/` must parse against
    /// the current schema and build into an EffectAsset without panicking.
    /// Reads the directory at runtime (so new effects are covered automatically).
    #[test]
    fn all_shipped_effects_parse_and_build() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/particles");
        // Recursive walk: shipped effects are organized into subfolders
        // (e.g. `2d/` for the plane_2d pixel-scale variants), and a flat
        // read_dir would silently drop them from coverage.
        let mut pending = vec![std::path::PathBuf::from(dir)];
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        while let Some(d) = pending.pop() {
            let entries = std::fs::read_dir(&d)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", d.display()));
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        let mut count = 0usize;
        let mut failures: Vec<String> = Vec::new();
        for path in files {
            if path.extension().and_then(|s| s.to_str()) != Some("particle") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{name}: read error: {e}"));
                    continue;
                }
            };
            match ron::from_str::<HanabiEffectDefinition>(&src) {
                Ok(def) => {
                    let _ = build_complete_effect(&def);
                    count += 1;
                }
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} .particle files failed to parse:\n{}",
            failures.len(),
            count + failures.len(),
            failures.join("\n")
        );
        assert!(count > 50, "expected many effects, only parsed {count}");
    }

    /// Direct coverage of the three features added to the builder.
    #[test]
    fn size_curve_attractors_and_ribbon_build() {
        use crate::data::{Attractor, CurvePoint, RibbonSettings};

        let mut def = HanabiEffectDefinition::default();
        def.size_curve = vec![
            CurvePoint { time: 0.0, value: 0.1 },
            CurvePoint { time: 0.5, value: 0.4 },
            CurvePoint { time: 1.0, value: 0.0 },
        ];
        def.attractors = vec![
            Attractor::default(),
            Attractor { position: [1.0, 0.0, 0.0], ..Attractor::default() },
        ];
        def.ribbon = Some(RibbonSettings { groups: 1 });
        let _ = build_complete_effect(&def);
    }
}
