use bevy::prelude::*;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::camera::Exposure;
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
pub struct TonemappingSettings {
    /// 0=None, 1=Reinhard, 2=ReinhardLuminance, 3=AcesFitted,
    /// 4=AgX, 5=SomewhatBoring, 6=TonyMcMapface, 7=BlenderFilmic
    pub mode: u32,
    pub ev100: f32,
    pub enabled: bool,
}

impl Default for TonemappingSettings {
    fn default() -> Self {
        Self {
            mode: 6,
            ev100: 9.7,
            enabled: true,
        }
    }
}

fn mode_to_tonemapping(mode: u32) -> Tonemapping {
    match mode {
        0 => Tonemapping::None,
        1 => Tonemapping::Reinhard,
        2 => Tonemapping::ReinhardLuminance,
        3 => Tonemapping::AcesFitted,
        4 => Tonemapping::AgX,
        5 => Tonemapping::SomewhatBoringDisplayTransform,
        6 => Tonemapping::TonyMcMapface,
        7 => Tonemapping::BlenderFilmic,
        _ => Tonemapping::TonyMcMapface,
    }
}

#[cfg(feature = "editor")]
fn tonemapping_to_mode(t: &Tonemapping) -> u32 {
    match t {
        Tonemapping::None => 0,
        Tonemapping::Reinhard => 1,
        Tonemapping::ReinhardLuminance => 2,
        Tonemapping::AcesFitted => 3,
        Tonemapping::AgX => 4,
        Tonemapping::SomewhatBoringDisplayTransform => 5,
        Tonemapping::TonyMcMapface => 6,
        Tonemapping::BlenderFilmic => 7,
    }
}

const MODE_LABELS: [&str; 8] = [
    "None",
    "Reinhard",
    "Reinhard Luminance",
    "ACES Fitted",
    "AgX",
    "Somewhat Boring",
    "Tony McMapface",
    "Blender Filmic",
];

fn sync_tonemapping(
    mut commands: Commands,
    query: Query<(Entity, &TonemappingSettings), Changed<TonemappingSettings>>,
) {
    for (entity, settings) in &query {
        let tm = if settings.enabled {
            mode_to_tonemapping(settings.mode)
        } else {
            Tonemapping::None
        };
        commands
            .entity(entity)
            .insert(tm)
            .insert(Exposure { ev100: settings.ev100 });
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "tonemapping",
        display_name: "Tonemapping",
        icon: regular::SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<TonemappingSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            let mode = world
                .get::<Tonemapping>(entity)
                .map(|t| tonemapping_to_mode(t))
                .unwrap_or(6);
            let ev100 = world
                .get::<Exposure>(entity)
                .map(|e| e.ev100)
                .unwrap_or(9.7);
            world
                .entity_mut(entity)
                .insert(TonemappingSettings { mode, ev100, enabled: true });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity)
                .remove::<TonemappingSettings>()
                .insert((Tonemapping::default(), Exposure::default()));
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<TonemappingSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<TonemappingSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(tonemapping_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn tonemapping_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<TonemappingSettings>(entity) else {
        return;
    };

    let mut row = 0;

    // Mode combo box
    let current_idx = settings.mode as usize;
    inline_property(ui, row, "Mode", theme, |ui| {
        let mut new_idx = current_idx;
        egui::ComboBox::from_id_salt("tonemapping_mode")
            .selected_text(*MODE_LABELS.get(current_idx).unwrap_or(&"Unknown"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, label) in MODE_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_idx, i, *label).changed() {
                        let mode = new_idx as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<TonemappingSettings>(entity) {
                                s.mode = mode;
                            }
                        });
                    }
                }
            });
    });
    row += 1;

    // EV100
    let mut ev100 = settings.ev100;
    inline_property(ui, row, "EV100", theme, |ui| {
        let orig = ev100;
        ui.add(
            egui::DragValue::new(&mut ev100)
                .speed(0.1)
                .range(-16.0..=16.0),
        );
        if ev100 != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<TonemappingSettings>(entity) {
                    s.ev100 = ev100;
                }
            });
        }
    });
}

// ── Deband Dither ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DebandDitherSettings {
    pub enabled: bool,
}

impl Default for DebandDitherSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_deband_dither(
    mut commands: Commands,
    query: Query<(Entity, &DebandDitherSettings), Changed<DebandDitherSettings>>,
) {
    for (entity, settings) in &query {
        commands.entity(entity).insert(if settings.enabled {
            DebandDither::Enabled
        } else {
            DebandDither::Disabled
        });
    }
}

fn cleanup_deband_dither(mut commands: Commands, mut removed: RemovedComponents<DebandDitherSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.insert(DebandDither::Disabled);
        }
    }
}

#[cfg(feature = "editor")]
fn deband_dither_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "deband_dither",
        display_name: "Deband Dither",
        icon: regular::GRADIENT,
        category: "rendering",
        has_fn: |world, entity| world.get::<DebandDitherSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DebandDitherSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<DebandDitherSettings>();
            world.entity_mut(entity).insert(DebandDither::Disabled);
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<DebandDitherSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DebandDitherSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

fn cleanup_tonemapping(mut commands: Commands, mut removed: RemovedComponents<TonemappingSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.insert((Tonemapping::default(), Exposure::default()));
        }
    }
}

pub struct TonemappingPlugin;

impl Plugin for TonemappingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TonemappingSettings>();
        app.register_type::<DebandDitherSettings>();
        app.add_systems(Update, (sync_tonemapping, cleanup_tonemapping, sync_deband_dither, cleanup_deband_dither));
        #[cfg(feature = "editor")]
        {
            app.register_inspector(inspector_entry());
            app.register_inspector(deband_dither_entry());
        }
    }
}
