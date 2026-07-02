//! Editor-only half of the 2D lighting plugin.
//!
//! Compiled only with the `editor` feature. `Light2dPlugin` loads at Runtime
//! scope, so in a shipped game these registrations are harmless no-ops (the
//! editor registries / marker entities they target don't exist); in the editor
//! they wire:
//!
//! - **Inspector entries** for [`FireflyConfig`] (camera), [`PointLight2d`]
//!   and [`Occluder2d`];
//! - **Add-Entity presets** ("Point Light 2D", "Occluder 2D") under the 2D
//!   nodes category — `Node2d`-tagged so selecting them auto-switches the
//!   viewport to 2D view;
//! - the **editor-camera lightmap preview** — mirrors the scene camera's
//!   `FireflyConfig` onto `EditorCamera2d`;
//! - **selection gizmos** outlining the selected light's range / occluder's
//!   shape while editing.

use bevy::prelude::*;
use bevy_firefly::occluders::Occluder2dShape;
use bevy_firefly::prelude::*;
use renzora::core::{DefaultCamera, EditorCamera2d, Node2d, PlayModeState};
use renzora::{
    AppEditorExt, EditorSelection, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry,
};

pub(super) fn register(app: &mut App) {
    info!("[editor] Light2d inspectors + presets");
    app.register_inspector(firefly_config_entry());
    app.register_inspector(point_light_entry());
    app.register_inspector(occluder_entry());
    register_presets(app);
    app.add_systems(
        Update,
        (sync_editor_camera_lighting, draw_selection_gizmos, draw_light_markers),
    );
}

// ============================================================================
// Add-Entity presets
// ============================================================================

fn register_presets(app: &mut App) {
    app.register_entity_preset(EntityPreset {
        id: "point_light_2d",
        display_name: "Point Light 2D",
        icon: "lightbulb",
        category: "nodes_2d",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Point Light 2D"),
                    Transform::default(),
                    Visibility::default(),
                    Node2d,
                    PointLight2d::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "occluder_2d",
        display_name: "Occluder 2D",
        icon: "moon",
        category: "nodes_2d",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Occluder 2D"),
                    Transform::default(),
                    Visibility::default(),
                    Node2d,
                    Occluder2d::circle(25.0),
                ))
                .id()
        },
    });
}

// ============================================================================
// Editor-camera lightmap preview
// ============================================================================

/// Mirror the scene's 2D-camera lighting onto the editor 2D camera.
///
/// The editor viewport (and in-editor play, which renders the game through
/// `EditorCamera2d`) never uses the scene's own camera entity — so a
/// `FireflyConfig` authored on the game camera would light the exported game
/// but not the editor. Preference order:
///
/// 1. the config the game would use (`DefaultCamera` first, else the first
///    configured scene `Camera2d`) — WYSIWYG;
/// 2. when the scene has 2D lights but no configured camera yet, a neutral
///    preview config (full white ambient) so a freshly placed light shows up
///    immediately instead of doing nothing;
/// 3. neither → no config, zero lighting overhead in the viewport.
fn sync_editor_camera_lighting(
    mut commands: Commands,
    editor_cams: Query<(Entity, Has<FireflyConfig>), With<EditorCamera2d>>,
    scene_cams: Query<
        (Ref<FireflyConfig>, Has<DefaultCamera>),
        (With<Camera2d>, Without<EditorCamera2d>),
    >,
    mut removed_configs: RemovedComponents<FireflyConfig>,
    lights: Query<(), With<PointLight2d>>,
    mut applied: Local<Option<&'static str>>,
) {
    let Ok((editor_cam, has_config)) = editor_cams.single() else {
        return;
    };

    let source = scene_cams.iter().max_by_key(|(_, is_default)| *is_default);
    let (want, kind): (Option<FireflyConfig>, Option<&'static str>) = match source {
        Some((config, _)) => (Some((*config).clone()), Some("scene")),
        None if !lights.is_empty() => (
            Some(FireflyConfig {
                ambient_brightness: 1.0,
                ..default()
            }),
            Some("preview"),
        ),
        None => (None, None),
    };

    // Only touch the camera when something actually moved — re-inserting every
    // frame would spam change detection. `removed_configs` covers the case
    // where the preferred source camera loses its config while another keeps
    // one (`is_changed` alone wouldn't fire there).
    let scene_changed = scene_cams.iter().any(|(config, _)| config.is_changed());
    let removed_any = removed_configs.read().next().is_some();
    let dirty = *applied != kind
        || want.is_some() != has_config
        || (kind == Some("scene") && (scene_changed || removed_any));
    if !dirty {
        return;
    }
    *applied = kind;
    match want {
        Some(config) => {
            commands.entity(editor_cam).insert(config);
        }
        None => {
            commands.entity(editor_cam).remove::<FireflyConfig>();
        }
    }
}

// ============================================================================
// Selection gizmos
// ============================================================================

/// Outline the selected light's core/outer range and the selected occluder's
/// shape. Selection-only (unlike firefly's own `FireflyGizmosPlugin`, which
/// draws every entity, every frame) and suppressed in play mode — the editor
/// 2D camera is the presenting camera there, so gizmos would leak into the
/// game view.
fn draw_selection_gizmos(
    mut gizmos: Gizmos,
    selection: Option<Res<EditorSelection>>,
    play_mode: Option<Res<PlayModeState>>,
    lights: Query<(&GlobalTransform, &PointLight2d)>,
    occluders: Query<(&GlobalTransform, &Occluder2d)>,
) {
    let Some(selection) = selection else { return };
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }

    const LIGHT_COLOR: Color = Color::srgb(1.0, 0.8, 0.2);
    const LIGHT_CORE_COLOR: Color = Color::srgb(1.0, 1.0, 0.7);
    const OCCLUDER_COLOR: Color = Color::srgb(0.3, 0.8, 1.0);

    for entity in selection.get_all() {
        if let Ok((transform, light)) = lights.get(entity) {
            let center = transform.translation().truncate() + light.offset.truncate();
            let isometry = Isometry2d::from_translation(center);
            gizmos.circle_2d(isometry, light.radius, LIGHT_COLOR);
            if light.core.radius > 0.0 {
                gizmos.circle_2d(isometry, light.core.radius, LIGHT_CORE_COLOR);
            }
            // Constrained lights: show the cone edges around the entity's UP
            // direction (firefly's angle convention).
            if light.angle.outer < 360.0 {
                let up = (transform.rotation() * Vec3::Y).truncate().normalize_or_zero();
                let half = light.angle.outer.to_radians() * 0.5;
                for side in [-1.0, 1.0] {
                    let dir = Rot2::radians(half * side) * up;
                    gizmos.line_2d(center, center + dir * light.radius, LIGHT_COLOR);
                }
            }
        }

        if let Ok((transform, occluder)) = occluders.get(entity) {
            let center = transform.translation().truncate() + occluder.offset.truncate();
            let rotation = Rot2::radians(transform.rotation().to_euler(EulerRot::XYZ).2);
            match occluder.shape() {
                Occluder2dShape::Polygon { vertices, .. } => {
                    let mut points: Vec<Vec2> =
                        vertices.iter().map(|v| center + rotation * *v).collect();
                    if let Some(&first) = points.first() {
                        points.push(first);
                    }
                    gizmos.linestrip_2d(points, OCCLUDER_COLOR);
                }
                Occluder2dShape::Polyline { vertices } => {
                    gizmos.linestrip_2d(
                        vertices.iter().map(|v| center + rotation * *v),
                        OCCLUDER_COLOR,
                    );
                }
                Occluder2dShape::RoundRectangle {
                    half_width,
                    half_height,
                    radius,
                } => {
                    if *half_width == 0.0 && *half_height == 0.0 {
                        gizmos.circle_2d(
                            Isometry2d::from_translation(center),
                            *radius,
                            OCCLUDER_COLOR,
                        );
                    } else {
                        // Firefly's round rect is a 2hw x 2hh body with a
                        // radius-sized pad all around — i.e. an outer box of
                        // (2hw + 2r, 2hh + 2r) with corner radius r.
                        gizmos
                            .rounded_rect_2d(
                                Isometry2d::new(center, rotation),
                                Vec2::new(
                                    2.0 * (half_width + radius),
                                    2.0 * (half_height + radius),
                                ),
                                OCCLUDER_COLOR,
                            )
                            .corner_radius(*radius);
                    }
                }
            }
        }
    }
}

/// Always-visible bulb markers for EVERY 2D light — selected or not — so
/// lights are findable in the viewport (an unselected `PointLight2d` renders
/// nothing of its own; without a marker it's an invisible entity you can only
/// reach through the hierarchy). Sun-glyph style: a core dot, a ring, and
/// four rays, in the light's own colour, plus a very faint range circle.
/// Sized in world units from the editor camera's ortho scale so the glyph
/// stays a constant ~pixel size at any zoom. 2D view + edit mode only, and
/// respects the viewport's "scene icons" display toggle (same switch that
/// gates the 3D light-bulb icons).
fn draw_light_markers(
    mut gizmos: Gizmos,
    settings: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
    play_mode: Option<Res<PlayModeState>>,
    lights: Query<(&GlobalTransform, &PointLight2d)>,
    editor_cam: Query<&Projection, With<EditorCamera2d>>,
) {
    use renzora::core::viewport_types::ViewportView;

    let Some(settings) = settings else { return };
    if settings.viewport_view != ViewportView::Two || !settings.show_scene_icons {
        return;
    }
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    // World units per render-image pixel at the current zoom.
    let scale = match editor_cam.single() {
        Ok(Projection::Orthographic(o)) => o.scale,
        _ => return,
    };
    let px = |n: f32| n * scale;

    for (transform, light) in &lights {
        let center = transform.translation().truncate() + light.offset.truncate();
        let color = light.color.with_alpha(1.0);
        let isometry = Isometry2d::from_translation(center);
        gizmos.circle_2d(isometry, px(3.0), color);
        gizmos.circle_2d(isometry, px(7.0), color);
        for i in 0..4 {
            let dir = Rot2::degrees(45.0 + 90.0 * i as f32) * Vec2::X;
            gizmos.line_2d(center + dir * px(9.0), center + dir * px(13.0), color);
        }
        // Whisper-faint range ring so light coverage is readable at a glance
        // without selecting; the selection gizmos draw the bright version.
        if light.radius > 0.0 {
            gizmos.circle_2d(isometry, light.radius, color.with_alpha(0.08));
        }
    }
}

// ============================================================================
// Inspector entries
// ============================================================================

/// `FireflyConfig` lives on the game's `Camera2d`. The section is **inherent to
/// every 2D camera** (`has_fn` = has `Camera2d`) rather than an Add-Component
/// entry: the overlay gates the whole `"camera"` category behind `Camera3d`
/// (the curated image-quality set is 3D-only), so an addable entry would never
/// be offered on a 2D camera — and lighting is core enough that it should read
/// as part of the camera, not as a hidden extra. The header toggle maps to the
/// component's presence, so lighting is truly zero-cost while off; the fields'
/// `get_fn`s return `None` until it's enabled, which hides the rows.
fn firefly_config_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "light2d_config",
        display_name: "2D Lighting",
        icon: "sun-dim",
        category: "camera",
        has_fn: |world, entity| world.get::<Camera2d>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: Some(|world, entity| world.get::<FireflyConfig>(entity).is_some()),
        set_enabled_fn: Some(|world, entity, val| {
            if val {
                if world.get::<FireflyConfig>(entity).is_none() {
                    // Default ambient is 0 (pitch black without lights) — start
                    // with a dim ambient instead so flipping lighting on doesn't
                    // read as "my scene disappeared".
                    world.entity_mut(entity).insert(FireflyConfig {
                        ambient_brightness: 0.1,
                        ..Default::default()
                    });
                }
            } else {
                world.entity_mut(entity).remove::<FireflyConfig>();
            }
        }),
        fields: vec![
            FieldDef {
                name: "Ambient Color",
                field_type: FieldType::Color,
                get_fn: |w, e| {
                    w.get::<FireflyConfig>(e).map(|c| {
                        let srgba = c.ambient_color.to_srgba();
                        FieldValue::Color([srgba.red, srgba.green, srgba.blue])
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Color(rgb), Some(mut c)) =
                        (v, w.get_mut::<FireflyConfig>(e))
                    {
                        c.ambient_color = Color::srgb(rgb[0], rgb[1], rgb[2]);
                    }
                },
            },
            renzora::float_field!("Ambient Brightness", FireflyConfig, ambient_brightness, 0.01, 0.0, 5.0),
            renzora::bool_field!("Soft Shadows", FireflyConfig, soft_shadows),
            renzora::bool_field!("Z Sorting", FireflyConfig, z_sorting),
            // `light_bands: Option<f32>` — 0 disables banding, matching the
            // upstream semantics of `None`.
            FieldDef {
                name: "Light Bands",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |w, e| {
                    w.get::<FireflyConfig>(e)
                        .map(|c| FieldValue::Float(c.light_bands.unwrap_or(0.0)))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut c)) = (v, w.get_mut::<FireflyConfig>(e))
                    {
                        c.light_bands = (f > 0.0).then_some(f);
                    }
                },
            },
            FieldDef {
                name: "Normal Mode",
                field_type: FieldType::Enum {
                    options: &["None", "Simple", "Top Down (Y)", "Top Down (Z)"],
                },
                get_fn: |w, e| {
                    w.get::<FireflyConfig>(e).map(|c| {
                        FieldValue::Enum(
                            match c.normal_mode {
                                NormalMode::None => "None",
                                NormalMode::Simple => "Simple",
                                NormalMode::TopDownY => "Top Down (Y)",
                                NormalMode::TopDownZ => "Top Down (Z)",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(s), Some(mut c)) = (v, w.get_mut::<FireflyConfig>(e)) {
                        c.normal_mode = match s.as_str() {
                            "Simple" => NormalMode::Simple,
                            "Top Down (Y)" => NormalMode::TopDownY,
                            "Top Down (Z)" => NormalMode::TopDownZ,
                            _ => NormalMode::None,
                        };
                    }
                },
            },
            renzora::float_field!("Normal Attenuation", FireflyConfig, normal_attenuation, 0.01, 0.0, 1.0),
            renzora::bool_field!("Lightmap Filtering", FireflyConfig, lightmap_filtering),
        ],
    }
}

fn point_light_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "point_light_2d",
        display_name: "Point Light 2D",
        icon: "lightbulb",
        category: "lighting",
        has_fn: |world, entity| world.get::<PointLight2d>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PointLight2d::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PointLight2d>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |w, e| {
                    w.get::<PointLight2d>(e).map(|l| {
                        let srgba = l.color.to_srgba();
                        FieldValue::Color([srgba.red, srgba.green, srgba.blue])
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Color(rgb), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e))
                    {
                        l.color = Color::srgb(rgb[0], rgb[1], rgb[2]);
                    }
                },
            },
            renzora::float_field!("Intensity", PointLight2d, intensity, 0.05, 0.0, 100.0),
            renzora::float_field!("Radius", PointLight2d, radius, 1.0, 0.0, 100_000.0),
            FieldDef {
                name: "Core Radius",
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 100_000.0 },
                get_fn: |w, e| {
                    w.get::<PointLight2d>(e).map(|l| FieldValue::Float(l.core.radius))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        l.core.radius = f;
                    }
                },
            },
            FieldDef {
                name: "Core Boost",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 1000.0 },
                get_fn: |w, e| w.get::<PointLight2d>(e).map(|l| FieldValue::Float(l.core.boost)),
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        l.core.boost = f;
                    }
                },
            },
            FieldDef {
                name: "Falloff",
                field_type: FieldType::Enum { options: &["Inverse Square", "Linear", "None"] },
                get_fn: |w, e| {
                    w.get::<PointLight2d>(e).map(|l| {
                        FieldValue::Enum(
                            match l.falloff {
                                Falloff::InverseSquare { .. } => "Inverse Square",
                                Falloff::Linear { .. } => "Linear",
                                Falloff::None => "None",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(s), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        // Keep the tuning value when switching curve shape.
                        let intensity = l.falloff.intensity();
                        l.falloff = match s.as_str() {
                            "Linear" => Falloff::Linear { intensity },
                            "None" => Falloff::None,
                            _ => Falloff::InverseSquare { intensity },
                        };
                    }
                },
            },
            FieldDef {
                name: "Falloff Intensity",
                field_type: FieldType::Float { speed: 0.05, min: -10.0, max: 10.0 },
                get_fn: |w, e| {
                    w.get::<PointLight2d>(e).map(|l| FieldValue::Float(l.falloff.intensity()))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        l.falloff = match l.falloff {
                            Falloff::InverseSquare { .. } => Falloff::InverseSquare { intensity: f },
                            Falloff::Linear { .. } => Falloff::Linear { intensity: f },
                            Falloff::None => Falloff::None,
                        };
                    }
                },
            },
            FieldDef {
                name: "Inner Angle",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 360.0 },
                get_fn: |w, e| w.get::<PointLight2d>(e).map(|l| FieldValue::Float(l.angle.inner)),
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        l.angle.inner = f.min(l.angle.outer);
                    }
                },
            },
            FieldDef {
                name: "Outer Angle",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 360.0 },
                get_fn: |w, e| w.get::<PointLight2d>(e).map(|l| FieldValue::Float(l.angle.outer)),
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(f), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        l.angle.outer = f.max(l.angle.inner);
                    }
                },
            },
            renzora::bool_field!("Cast Shadows", PointLight2d, cast_shadows),
            FieldDef {
                name: "Offset",
                field_type: FieldType::Vec3 { speed: 0.1 },
                get_fn: |w, e| {
                    w.get::<PointLight2d>(e).map(|l| FieldValue::Vec3(l.offset.to_array()))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Vec3(a), Some(mut l)) = (v, w.get_mut::<PointLight2d>(e)) {
                        l.offset = Vec3::from_array(a);
                    }
                },
            },
        ],
    }
}

/// `Occluder2d`'s shape is constructor-only (private field with vertex-order
/// invariants), so dimension edits rebuild the component from the current
/// round-rect geometry and re-insert. Polygon/polyline occluders are authored
/// in code or scripts — their dimension fields read as absent here.
fn round_rect_params(occluder: &Occluder2d) -> Option<(f32, f32, f32)> {
    match occluder.shape() {
        Occluder2dShape::RoundRectangle { half_width, half_height, radius } => {
            Some((*half_width, *half_height, *radius))
        }
        _ => None,
    }
}

fn replace_shape(world: &mut World, entity: Entity, width: f32, height: f32, radius: f32) {
    let Some(old) = world.get::<Occluder2d>(entity) else {
        return;
    };
    let new = Occluder2d::round_rectangle(width.max(0.0), height.max(0.0), radius.max(0.0))
        .with_color(old.color)
        .with_opacity(old.opacity)
        .with_z_sorting(old.z_sorting)
        .with_offset(old.offset);
    world.entity_mut(entity).insert(new);
}

fn occluder_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "occluder_2d",
        display_name: "Occluder 2D",
        icon: "moon",
        category: "lighting",
        has_fn: |world, entity| world.get::<Occluder2d>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Occluder2d::circle(25.0));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(Occluder2d, Occluder2dEnabled)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<Occluder2dEnabled>(entity).map(|e| e.0).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut enabled) = world.get_mut::<Occluder2dEnabled>(entity) {
                enabled.0 = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Shape",
                field_type: FieldType::Enum {
                    options: &["Circle", "Rectangle", "Rounded Rectangle", "Capsule"],
                },
                get_fn: |w, e| {
                    w.get::<Occluder2d>(e).map(|o| {
                        FieldValue::Enum(
                            match o.shape() {
                                Occluder2dShape::Polygon { .. } => "Polygon",
                                Occluder2dShape::Polyline { .. } => "Polyline",
                                Occluder2dShape::RoundRectangle {
                                    half_width,
                                    half_height,
                                    radius,
                                } => {
                                    if *half_width == 0.0 && *half_height == 0.0 {
                                        "Circle"
                                    } else if *radius == 0.0 {
                                        "Rectangle"
                                    } else if *half_width == 0.0 || *half_height == 0.0 {
                                        "Capsule"
                                    } else {
                                        "Rounded Rectangle"
                                    }
                                }
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    let FieldValue::Enum(label) = v else { return };
                    let Some(occluder) = w.get::<Occluder2d>(e) else { return };
                    let (hw, hh, r) = round_rect_params(occluder).unwrap_or((25.0, 25.0, 5.0));
                    match label.as_str() {
                        "Circle" => replace_shape(w, e, 0.0, 0.0, if r > 0.0 { r } else { 25.0 }),
                        "Rectangle" => {
                            replace_shape(w, e, (hw * 2.0).max(10.0), (hh * 2.0).max(10.0), 0.0)
                        }
                        "Rounded Rectangle" => replace_shape(
                            w,
                            e,
                            (hw * 2.0).max(10.0),
                            (hh * 2.0).max(10.0),
                            r.max(5.0),
                        ),
                        "Capsule" => {
                            replace_shape(w, e, 0.0, (hh * 2.0).max(25.0), r.max(10.0))
                        }
                        _ => {}
                    }
                },
            },
            FieldDef {
                name: "Width",
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 100_000.0 },
                get_fn: |w, e| {
                    w.get::<Occluder2d>(e)
                        .and_then(round_rect_params)
                        .map(|(hw, _, _)| FieldValue::Float(hw * 2.0))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(width) = v {
                        if let Some((_, hh, r)) = w.get::<Occluder2d>(e).and_then(round_rect_params)
                        {
                            replace_shape(w, e, width, hh * 2.0, r);
                        }
                    }
                },
            },
            FieldDef {
                name: "Height",
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 100_000.0 },
                get_fn: |w, e| {
                    w.get::<Occluder2d>(e)
                        .and_then(round_rect_params)
                        .map(|(_, hh, _)| FieldValue::Float(hh * 2.0))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(height) = v {
                        if let Some((hw, _, r)) = w.get::<Occluder2d>(e).and_then(round_rect_params)
                        {
                            replace_shape(w, e, hw * 2.0, height, r);
                        }
                    }
                },
            },
            FieldDef {
                name: "Corner Radius",
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 100_000.0 },
                get_fn: |w, e| {
                    w.get::<Occluder2d>(e)
                        .and_then(round_rect_params)
                        .map(|(_, _, r)| FieldValue::Float(r))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(radius) = v {
                        if let Some((hw, hh, _)) =
                            w.get::<Occluder2d>(e).and_then(round_rect_params)
                        {
                            replace_shape(w, e, hw * 2.0, hh * 2.0, radius);
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadow Color",
                field_type: FieldType::Color,
                get_fn: |w, e| {
                    w.get::<Occluder2d>(e).map(|o| {
                        let srgba = o.color.to_srgba();
                        FieldValue::Color([srgba.red, srgba.green, srgba.blue])
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Color(rgb), Some(mut o)) = (v, w.get_mut::<Occluder2d>(e)) {
                        o.color = Color::srgb(rgb[0], rgb[1], rgb[2]);
                    }
                },
            },
            renzora::float_field!("Opacity", Occluder2d, opacity, 0.01, 0.0, 1.0),
            renzora::bool_field!("Z Sorting", Occluder2d, z_sorting),
            FieldDef {
                name: "Offset",
                field_type: FieldType::Vec3 { speed: 0.1 },
                get_fn: |w, e| {
                    w.get::<Occluder2d>(e).map(|o| FieldValue::Vec3(o.offset.to_array()))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Vec3(a), Some(mut o)) = (v, w.get_mut::<Occluder2d>(e)) {
                        o.offset = Vec3::from_array(a);
                    }
                },
            },
        ],
    }
}
