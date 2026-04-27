use bevy::prelude::*;
use bevy::anti_alias::fxaa::{Fxaa, Sensitivity};
use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, toggle_switch, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

// ── FXAA Settings ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct FxaaSettings {
    /// 0=Low, 1=Medium, 2=High, 3=Ultra, 4=Extreme
    pub edge_threshold: u32,
    pub edge_threshold_min: u32,
    pub enabled: bool,
}

impl Default for FxaaSettings {
    fn default() -> Self {
        Self {
            edge_threshold: 2,
            edge_threshold_min: 2,
            enabled: true,
        }
    }
}

fn idx_to_sensitivity(i: u32) -> Sensitivity {
    match i {
        0 => Sensitivity::Low,
        1 => Sensitivity::Medium,
        2 => Sensitivity::High,
        3 => Sensitivity::Ultra,
        4 => Sensitivity::Extreme,
        _ => Sensitivity::High,
    }
}

fn sync_fxaa(
    mut commands: Commands,
    sources: Query<(Entity, Ref<FxaaSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() { found = true; break; }
                if settings.enabled {
                    commands.entity(*target).insert(Fxaa {
                        enabled: true,
                        edge_threshold: idx_to_sensitivity(settings.edge_threshold),
                        edge_threshold_min: idx_to_sensitivity(settings.edge_threshold_min),
                    });
                } else {
                    commands.entity(*target).remove::<Fxaa>();
                }
                found = true; break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<Fxaa>(); }
        }
    }
}

// ── SMAA Settings ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SmaaSettings {
    /// 0=Low, 1=Medium, 2=High, 3=Ultra
    pub preset: u32,
    pub enabled: bool,
}

impl Default for SmaaSettings {
    fn default() -> Self {
        Self { preset: 2, enabled: true }
    }
}

fn sync_smaa(
    mut commands: Commands,
    sources: Query<(Entity, Ref<SmaaSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() { found = true; break; }
                if settings.enabled {
                    let preset = match settings.preset {
                        0 => SmaaPreset::Low,
                        1 => SmaaPreset::Medium,
                        2 => SmaaPreset::High,
                        3 => SmaaPreset::Ultra,
                        _ => SmaaPreset::High,
                    };
                    commands.entity(*target).insert(Smaa { preset });
                } else {
                    commands.entity(*target).remove::<Smaa>();
                }
                found = true; break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<Smaa>(); }
        }
    }
}

// ── TAA Settings ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TaaSettings {
    pub reset: bool,
    pub enabled: bool,
}

impl Default for TaaSettings {
    fn default() -> Self {
        Self { reset: false, enabled: true }
    }
}

fn sync_taa(
    mut commands: Commands,
    sources: Query<(Entity, Ref<TaaSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() { found = true; break; }
                if settings.enabled {
                    commands.entity(*target)
                        .insert(Msaa::Off)
                        .insert(TemporalAntiAliasing { reset: settings.reset });
                } else {
                    commands.entity(*target).remove::<TemporalAntiAliasing>();
                }
                found = true; break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<TemporalAntiAliasing>(); }
        }
    }
}

fn cleanup_taa(
    mut commands: Commands,
    mut removed: RemovedComponents<TaaSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<TemporalAntiAliasing>(); }
        }
    }
}

// ── CAS Settings ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CasSettings {
    pub sharpening_strength: f32,
    pub denoise: bool,
    pub enabled: bool,
}

impl Default for CasSettings {
    fn default() -> Self {
        Self {
            sharpening_strength: 0.6,
            denoise: false,
            enabled: true,
        }
    }
}

fn sync_cas(
    mut commands: Commands,
    sources: Query<(Entity, Ref<CasSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() { found = true; break; }
                if settings.enabled {
                    commands.entity(*target).insert(ContrastAdaptiveSharpening {
                        enabled: true,
                        sharpening_strength: settings.sharpening_strength,
                        denoise: settings.denoise,
                    });
                } else {
                    commands.entity(*target).remove::<ContrastAdaptiveSharpening>();
                }
                found = true; break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) { ec.remove::<ContrastAdaptiveSharpening>(); }
        }
    }
}

// ── Inspector entries ──

#[cfg(feature = "editor")]
const SENSITIVITY_LABELS: [&str; 5] = ["Low", "Medium", "High", "Ultra", "Extreme"];

#[cfg(feature = "editor")]
const SMAA_LABELS: [&str; 4] = ["Low", "Medium", "High", "Ultra"];

#[cfg(feature = "editor")]
fn fxaa_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "fxaa",
        display_name: "FXAA",
        icon: regular::GRID_FOUR,
        category: "rendering",
        has_fn: |world, entity| world.get::<FxaaSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(FxaaSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(FxaaSettings, Fxaa)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<FxaaSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<FxaaSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(fxaa_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn fxaa_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<FxaaSettings>(entity) else { return };
    let mut row = 0;

    // Edge Threshold
    let et = settings.edge_threshold as usize;
    inline_property(ui, row, "Edge Threshold", theme, |ui| {
        let mut new_et = et;
        egui::ComboBox::from_id_salt("fxaa_et")
            .selected_text(*SENSITIVITY_LABELS.get(et).unwrap_or(&"?"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, l) in SENSITIVITY_LABELS.iter().enumerate() {
                    ui.selectable_value(&mut new_et, i, *l);
                }
            });
        if new_et != et {
            let val = new_et as u32;
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<FxaaSettings>(entity) { s.edge_threshold = val; }
            });
        }
    });
    row += 1;

    // Edge Threshold Min
    let etm = settings.edge_threshold_min as usize;
    inline_property(ui, row, "Edge Thr. Min", theme, |ui| {
        let mut new_etm = etm;
        egui::ComboBox::from_id_salt("fxaa_etm")
            .selected_text(*SENSITIVITY_LABELS.get(etm).unwrap_or(&"?"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, l) in SENSITIVITY_LABELS.iter().enumerate() {
                    ui.selectable_value(&mut new_etm, i, *l);
                }
            });
        if new_etm != etm {
            let val = new_etm as u32;
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<FxaaSettings>(entity) { s.edge_threshold_min = val; }
            });
        }
    });
}

#[cfg(feature = "editor")]
fn smaa_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "smaa",
        display_name: "SMAA",
        icon: regular::GRID_FOUR,
        category: "rendering",
        has_fn: |world, entity| world.get::<SmaaSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SmaaSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(SmaaSettings, Smaa)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SmaaSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SmaaSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(smaa_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn smaa_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<SmaaSettings>(entity) else { return };

    let current = settings.preset as usize;
    inline_property(ui, 0, "Preset", theme, |ui| {
        let mut new_idx = current;
        egui::ComboBox::from_id_salt("smaa_preset")
            .selected_text(*SMAA_LABELS.get(current).unwrap_or(&"?"))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, l) in SMAA_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_idx, i, *l).changed() {
                        let preset = new_idx as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<SmaaSettings>(entity) {
                                s.preset = preset;
                            }
                        });
                    }
                }
            });
    });
}

#[cfg(feature = "editor")]
fn taa_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "taa",
        display_name: "TAA",
        icon: regular::GRID_FOUR,
        category: "rendering",
        has_fn: |world, entity| world.get::<TaaSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(TaaSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(TaaSettings, TemporalAntiAliasing)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<TaaSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<TaaSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(taa_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn taa_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<TaaSettings>(entity) else { return };

    let reset = settings.reset;
    inline_property(ui, 0, "Reset", theme, |ui| {
        let id = ui.id().with("taa_reset");
        if toggle_switch(ui, id, reset) {
            let new_val = !reset;
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<TaaSettings>(entity) { s.reset = new_val; }
            });
        }
    });
}

#[cfg(feature = "editor")]
fn cas_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "cas",
        display_name: "Sharpening (CAS)",
        icon: regular::DIAMONDS_FOUR,
        category: "rendering",
        has_fn: |world, entity| world.get::<CasSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CasSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(CasSettings, ContrastAdaptiveSharpening)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<CasSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<CasSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(cas_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn cas_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<CasSettings>(entity) else { return };
    let mut row = 0;

    // Strength
    let mut strength = settings.sharpening_strength;
    inline_property(ui, row, "Strength", theme, |ui| {
        let orig = strength;
        ui.add(egui::DragValue::new(&mut strength).speed(0.01).range(0.0..=1.0));
        if strength != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<CasSettings>(entity) { s.sharpening_strength = strength; }
            });
        }
    });
    row += 1;

    // Denoise
    let denoise = settings.denoise;
    inline_property(ui, row, "Denoise", theme, |ui| {
        let id = ui.id().with("cas_denoise");
        if toggle_switch(ui, id, denoise) {
            let new_val = !denoise;
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<CasSettings>(entity) { s.denoise = new_val; }
            });
        }
    });
}

pub struct AntiAliasingPlugin;

impl Plugin for AntiAliasingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AntiAliasingPlugin");
        app.register_type::<FxaaSettings>();
        app.register_type::<SmaaSettings>();
        app.register_type::<TaaSettings>();
        app.register_type::<CasSettings>();
        app.add_systems(Update, (sync_fxaa, sync_smaa, sync_taa, sync_cas, cleanup_taa));
        #[cfg(feature = "editor")]
        {
            app.register_inspector(fxaa_entry());
            app.register_inspector(smaa_entry());
            app.register_inspector(taa_entry());
            app.register_inspector(cas_entry());
        }
    }
}
