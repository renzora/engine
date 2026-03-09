use bevy::prelude::*;
use bevy::pbr::{Atmosphere, AtmosphereMode, AtmosphereSettings, ScatteringMedium};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AtmosphereComponentSettings {
    pub bottom_radius: f32,
    pub top_radius: f32,
    pub ground_albedo: f32,
    pub scene_units_to_m: f32,
    /// 0 = LookupTexture, 1 = Raymarched
    pub mode: u32,
    pub enabled: bool,
}

impl Default for AtmosphereComponentSettings {
    fn default() -> Self {
        Self {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: 0.3,
            scene_units_to_m: 1.0,
            mode: 0,
            enabled: true,
        }
    }
}

/// Stores the ScatteringMedium handle so we don't recreate it every frame.
#[derive(Component)]
struct AtmosphereMediumHandle(Handle<ScatteringMedium>);

fn sync_atmosphere(
    mut commands: Commands,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
    query: Query<(Entity, &AtmosphereComponentSettings, Option<&AtmosphereMediumHandle>), Changed<AtmosphereComponentSettings>>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for (entity, settings, existing_handle) in &query {
        let target = render_target.0.unwrap_or(entity);

        if !settings.enabled {
            commands.entity(target).remove::<(Atmosphere, AtmosphereSettings)>();
            continue;
        }

        let handle = if let Some(h) = existing_handle {
            h.0.clone()
        } else {
            let h = mediums.add(ScatteringMedium::default());
            commands.entity(entity).insert(AtmosphereMediumHandle(h.clone()));
            h
        };

        let rendering_method = match settings.mode {
            1 => AtmosphereMode::Raymarched,
            _ => AtmosphereMode::LookupTexture,
        };

        commands.entity(target).insert((
            Atmosphere {
                bottom_radius: settings.bottom_radius,
                top_radius: settings.top_radius,
                ground_albedo: Vec3::splat(settings.ground_albedo),
                medium: handle,
            },
            AtmosphereSettings {
                scene_units_to_m: settings.scene_units_to_m,
                rendering_method,
                ..default()
            },
        ));
    }
}

fn cleanup_atmosphere(
    mut commands: Commands,
    mut removed: RemovedComponents<AtmosphereComponentSettings>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for entity in removed.read() {
        // Remove the handle from the source entity
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<AtmosphereMediumHandle>();
        }
        // Remove native Bevy components from the camera target
        if let Some(target) = render_target.0 {
            if let Ok(mut ec) = commands.get_entity(target) {
                ec.remove::<(Atmosphere, AtmosphereSettings)>();
            }
        }
    }
}

#[cfg(feature = "editor")]
const ATMO_MODE_LABELS: [&str; 2] = ["Lookup Texture", "Raymarched"];

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "atmosphere",
        display_name: "Atmosphere",
        icon: regular::CLOUD_SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AtmosphereComponentSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(AtmosphereComponentSettings, Atmosphere, AtmosphereSettings, AtmosphereMediumHandle)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<AtmosphereComponentSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(atmosphere_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn atmosphere_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<AtmosphereComponentSettings>(entity) else { return };
    let mut row = 0;

    // Rendering mode
    let current = settings.mode as usize;
    inline_property(ui, row, "Rendering", theme, |ui| {
        let mut new_idx = current;
        egui::ComboBox::from_id_salt("atmo_mode")
            .selected_text(*ATMO_MODE_LABELS.get(current).unwrap_or(&"Unknown"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, label) in ATMO_MODE_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_idx, i, *label).changed() {
                        let mode = new_idx as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) {
                                s.mode = mode;
                            }
                        });
                    }
                }
            });
    });
    row += 1;

    // Bottom Radius
    let mut bottom = settings.bottom_radius;
    inline_property(ui, row, "Bottom Radius", theme, |ui| {
        let orig = bottom;
        ui.add(egui::DragValue::new(&mut bottom).speed(1000.0).range(0.0..=100_000_000.0));
        if bottom != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.bottom_radius = bottom; }
            });
        }
    });
    row += 1;

    // Top Radius
    let mut top = settings.top_radius;
    inline_property(ui, row, "Top Radius", theme, |ui| {
        let orig = top;
        ui.add(egui::DragValue::new(&mut top).speed(1000.0).range(0.0..=100_000_000.0));
        if top != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.top_radius = top; }
            });
        }
    });
    row += 1;

    // Ground Albedo
    let mut albedo = settings.ground_albedo;
    inline_property(ui, row, "Ground Albedo", theme, |ui| {
        let orig = albedo;
        ui.add(egui::DragValue::new(&mut albedo).speed(0.01).range(0.0..=1.0));
        if albedo != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.ground_albedo = albedo; }
            });
        }
    });
    row += 1;

    // Scene Units to Meters
    let mut scale = settings.scene_units_to_m;
    inline_property(ui, row, "Units to m", theme, |ui| {
        let orig = scale;
        ui.add(egui::DragValue::new(&mut scale).speed(0.1).range(0.001..=10000.0));
        if scale != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.scene_units_to_m = scale; }
            });
        }
    });
}

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AtmosphereComponentSettings>();
        app.add_systems(Update, (sync_atmosphere, cleanup_atmosphere));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
