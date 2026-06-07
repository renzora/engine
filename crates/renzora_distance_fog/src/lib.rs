use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora::{AppEditorExt, InspectorEntry},
};

/// Fog falloff mode:
/// 0 = Linear, 1 = Exponential, 2 = ExponentialSquared, 3 = Atmospheric
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DistanceFogSettings {
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub directional_light_color_r: f32,
    pub directional_light_color_g: f32,
    pub directional_light_color_b: f32,
    pub directional_light_exponent: f32,
    /// 0=Linear, 1=Exponential, 2=ExponentialSquared, 3=Atmospheric
    pub mode: u32,
    pub start: f32,
    pub end: f32,
    pub density: f32,
    pub extinction_r: f32,
    pub extinction_g: f32,
    pub extinction_b: f32,
    pub inscattering_r: f32,
    pub inscattering_g: f32,
    pub inscattering_b: f32,
    pub enabled: bool,
}

impl Default for DistanceFogSettings {
    fn default() -> Self {
        Self {
            color_r: 0.72,
            color_g: 0.78,
            color_b: 0.9,
            directional_light_color_r: 1.0,
            directional_light_color_g: 0.92,
            directional_light_color_b: 0.75,
            directional_light_exponent: 12.0,
            mode: 3,
            start: 50.0,
            end: 800.0,
            density: 0.005,
            extinction_r: 0.006,
            extinction_g: 0.005,
            extinction_b: 0.004,
            inscattering_r: 0.008,
            inscattering_g: 0.01,
            inscattering_b: 0.014,
            enabled: true,
        }
    }
}

fn sync_distance_fog(
    mut commands: Commands,
    sources: Query<(Entity, Ref<DistanceFogSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                if !settings.enabled {
                    commands.entity(*target).remove::<DistanceFog>();
                    found = true;
                    break;
                }
                let falloff = match settings.mode {
                    1 => FogFalloff::Exponential {
                        density: settings.density,
                    },
                    2 => FogFalloff::ExponentialSquared {
                        density: settings.density,
                    },
                    3 => FogFalloff::Atmospheric {
                        extinction: Vec3::new(
                            settings.extinction_r,
                            settings.extinction_g,
                            settings.extinction_b,
                        ),
                        inscattering: Vec3::new(
                            settings.inscattering_r,
                            settings.inscattering_g,
                            settings.inscattering_b,
                        ),
                    },
                    _ => FogFalloff::Linear {
                        start: settings.start,
                        end: settings.end,
                    },
                };
                commands.entity(*target).insert(DistanceFog {
                    color: Color::srgb(settings.color_r, settings.color_g, settings.color_b),
                    directional_light_color: Color::srgb(
                        settings.directional_light_color_r,
                        settings.directional_light_color_g,
                        settings.directional_light_color_b,
                    ),
                    directional_light_exponent: settings.directional_light_exponent,
                    falloff,
                });
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<DistanceFog>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "distance_fog",
        display_name: "Distance Fog",
        icon: regular::CLOUD_FOG,
        category: "rendering",
        has_fn: |world, entity| world.get::<DistanceFogSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(DistanceFogSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(DistanceFogSettings, DistanceFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<DistanceFogSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
    }
}

/// Native (bevy_ui) inspector drawer — the bevy_ui analog of `fog_custom_ui`.
/// Demonstrates custom UI that declarative fields can't compose: a "Light Color"
/// swatch built from three separate float channels, plus bound numeric rows.
#[cfg(feature = "editor")]
fn fog_native_ui(world: &mut World, entity: Entity) -> Entity {
    use renzora_ember::inspector::{color_field, inspector_body, inspector_row, inspector_stripe};
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::widgets::{drag_value, DragRange};

    // Read initial values up front (inspector_body borrows World).
    let Some(s) = world.get::<DistanceFogSettings>(entity) else {
        return world.spawn(Node::default()).id();
    };
    let (start, end, density, exponent) =
        (s.start, s.end, s.density, s.directional_light_exponent);

    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();
        let mut kids: Vec<Entity> = Vec::new();

        // "Light Color" → a proper HSV picker, two-way bound to the three
        // directional-light channels.
        let color = color_field(
            commands,
            move |w| {
                w.get::<DistanceFogSettings>(entity)
                    .map(|s| {
                        [
                            s.directional_light_color_r,
                            s.directional_light_color_g,
                            s.directional_light_color_b,
                        ]
                    })
                    .unwrap_or([0.0; 3])
            },
            move |w, rgb: [f32; 3]| {
                if let Some(mut s) = w.get_mut::<DistanceFogSettings>(entity) {
                    s.directional_light_color_r = rgb[0];
                    s.directional_light_color_g = rgb[1];
                    s.directional_light_color_b = rgb[2];
                }
            },
        );
        kids.push(inspector_row(commands, &fonts.ui, "Light Color", color));

        // Bound numeric rows (label + scrubbable value, two-way bound).
        macro_rules! frow {
            ($label:expr, $field:ident, $init:expr, $speed:expr, $min:expr, $max:expr) => {{
                let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), $init, $speed);
                commands.entity(dv).insert(DragRange { min: $min, max: $max });
                bind_2way(
                    commands,
                    dv,
                    move |w| w.get::<DistanceFogSettings>(entity).map(|s| s.$field).unwrap_or(0.0),
                    move |w, v: &f32| {
                        if let Some(mut s) = w.get_mut::<DistanceFogSettings>(entity) {
                            s.$field = *v;
                        }
                    },
                );
                inspector_row(commands, &fonts.ui, $label, dv)
            }};
        }
        kids.push(frow!("Light Exponent", directional_light_exponent, exponent, 0.1, 1.0, 64.0));
        kids.push(frow!("Start", start, start, 0.5, 0.0, 10000.0));
        kids.push(frow!("End", end, end, 0.5, 0.0, 10000.0));
        kids.push(frow!("Density", density, density, 0.001, 0.0, 1.0));

        for (i, &row) in kids.iter().enumerate() {
            commands
                .entity(row)
                .insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(col).add_children(&kids);
        col
    })
}

fn cleanup_distance_fog(
    mut commands: Commands,
    mut removed: RemovedComponents<DistanceFogSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<DistanceFog>();
            }
        }
    }
}

#[derive(Default)]
pub struct DistanceFogPlugin;

impl Plugin for DistanceFogPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DistanceFogPlugin");
        app.register_type::<DistanceFogSettings>();
        app.add_systems(Update, (sync_distance_fog, cleanup_distance_fog));
        #[cfg(feature = "editor")]
        {
            app.register_inspector(inspector_entry());
            // The bevy_ui inspector uses this native drawer (conditional/composed
            // UI that declarative fields can't express).
            app.register_native_inspector_ui("distance_fog", fog_native_ui);
        }
    }
}

renzora::add!(DistanceFogPlugin);
