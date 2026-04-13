//! Native mixer implementation — full Kira audio integration.

use super::{inspectors, render};

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_audio::{ChannelStrip, MixerState};
use renzora_editor_framework::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

// ---------------------------------------------------------------------------
// Shared state bridge
// ---------------------------------------------------------------------------

#[derive(Default)]
struct MixerSnapshot {
    master: ChannelStrip,
    sfx: ChannelStrip,
    music: ChannelStrip,
    ambient: ChannelStrip,
    custom_buses: Vec<(String, ChannelStrip)>,
    adding_bus: bool,
    new_bus_name: String,
    renaming_bus: Option<usize>,
    rename_buf: String,
    dragging_bus: Option<usize>,
}

impl MixerSnapshot {
    fn from_mixer(m: &MixerState) -> Self {
        Self {
            master: m.master.clone(),
            sfx: m.sfx.clone(),
            music: m.music.clone(),
            ambient: m.ambient.clone(),
            custom_buses: m.custom_buses.clone(),
            adding_bus: m.adding_bus,
            new_bus_name: m.new_bus_name.clone(),
            renaming_bus: m.renaming_bus,
            rename_buf: m.rename_buf.clone(),
            dragging_bus: m.dragging_bus,
        }
    }

    fn apply_to(&self, m: &mut MixerState) {
        m.master = self.master.clone();
        m.sfx = self.sfx.clone();
        m.music = self.music.clone();
        m.ambient = self.ambient.clone();
        m.custom_buses = self.custom_buses.clone();
        m.adding_bus = self.adding_bus;
        m.new_bus_name = self.new_bus_name.clone();
        m.renaming_bus = self.renaming_bus;
        m.rename_buf = self.rename_buf.clone();
        m.dragging_bus = self.dragging_bus;
    }

    fn to_mixer_state(&self) -> MixerState {
        MixerState {
            master: self.master.clone(),
            sfx: self.sfx.clone(),
            music: self.music.clone(),
            ambient: self.ambient.clone(),
            custom_buses: self.custom_buses.clone(),
            adding_bus: self.adding_bus,
            new_bus_name: self.new_bus_name.clone(),
            renaming_bus: self.renaming_bus,
            rename_buf: self.rename_buf.clone(),
            dragging_bus: self.dragging_bus,
        }
    }
}

impl Clone for MixerSnapshot {
    fn clone(&self) -> Self {
        Self {
            master: self.master.clone(),
            sfx: self.sfx.clone(),
            music: self.music.clone(),
            ambient: self.ambient.clone(),
            custom_buses: self.custom_buses.clone(),
            adding_bus: self.adding_bus,
            new_bus_name: self.new_bus_name.clone(),
            renaming_bus: self.renaming_bus,
            rename_buf: self.rename_buf.clone(),
            dragging_bus: self.dragging_bus,
        }
    }
}

#[derive(Resource, Clone)]
struct MixerBridge {
    pending: Arc<Mutex<Option<MixerSnapshot>>>,
}

impl Default for MixerBridge {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }
}

// ---------------------------------------------------------------------------
// EditorPanel
// ---------------------------------------------------------------------------

pub struct MixerPanel {
    bridge: Arc<Mutex<Option<MixerSnapshot>>>,
    local: RwLock<MixerSnapshot>,
}

impl MixerPanel {
    fn new(bridge: Arc<Mutex<Option<MixerSnapshot>>>) -> Self {
        Self {
            bridge,
            local: RwLock::new(MixerSnapshot::default()),
        }
    }
}

impl EditorPanel for MixerPanel {
    fn id(&self) -> &str { "mixer" }
    fn title(&self) -> &str { "Mixer" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::SLIDERS_HORIZONTAL) }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(mixer) = world.get_resource::<MixerState>() {
            if let Ok(mut local) = self.local.write() {
                *local = MixerSnapshot::from_mixer(mixer);
            }
        }

        let (panel_bg, muted_color) = if let Some(tm) = world.get_resource::<ThemeManager>() {
            let t = &tm.active_theme;
            (t.surfaces.panel.to_color32(), t.text.muted.to_color32())
        } else {
            (
                egui::Color32::from_rgb(24, 25, 30),
                egui::Color32::from_rgb(110, 113, 132),
            )
        };

        if let Ok(mut snap) = self.local.write() {
            let mut tmp = snap.to_mixer_state();
            render::render_mixer_content(ui, &mut tmp, panel_bg, muted_color);
            *snap = MixerSnapshot::from_mixer(&tmp);
        }

        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                *pending = Some(local.clone());
            }
        }
    }

    fn closable(&self) -> bool { true }
    fn min_size(&self) -> [f32; 2] { [200.0, 180.0] }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

fn sync_mixer_bridge(bridge: Res<MixerBridge>, mut mixer: ResMut<MixerState>) {
    if let Ok(mut pending) = bridge.pending.lock() {
        if let Some(snap) = pending.take() {
            snap.apply_to(&mut mixer);
        }
    }
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

pub fn build(app: &mut App) {
    let bridge = MixerBridge::default();
    let arc = bridge.pending.clone();

    app.insert_resource(bridge);
    app.add_systems(
        Update,
        sync_mixer_bridge.run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
    app.register_panel(MixerPanel::new(arc));
    app.init_resource::<renzora_editor_framework::InspectorRegistry>();
    inspectors::register_audio_inspectors(
        &mut app.world_mut().resource_mut::<renzora_editor_framework::InspectorRegistry>(),
    );
}
