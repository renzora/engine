//! NavMesh editor panel — lists volumes, their build status, and exposes
//! per-volume rebuild + debug-draw controls plus a global auto-rebuild
//! toggle. Only compiled under the `editor` feature.

use std::sync::Mutex;

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_editor_framework::{EditorPanel, PanelLocation};
use vleue_navigator::{
    NavMesh,
    prelude::{ManagedNavMesh, NavMeshStatus, NavMeshUpdateMode},
};

use crate::{NavMeshVolume, NavPath};

/// Actions queued from the UI closure (which has `&World` only) and
/// drained each frame by [`drain_panel_actions`].
enum PanelAction {
    RebuildVolume(Entity),
    ToggleVolumeDebug(Entity),
    SetShowAgentPaths(bool),
    SetAutoRebuild(bool),
    ResetAgents,
    BakeToDisk,
}

#[derive(Default)]
struct Shared {
    show_agent_paths: bool,
    auto_rebuild: bool,
    pending: Vec<PanelAction>,
    /// True when the toggle has changed this frame (consumed by the
    /// auto-rebuild sync system so flipping off doesn't keep rewriting
    /// every volume every frame).
    auto_rebuild_dirty: bool,
}

#[derive(Resource)]
pub struct NavMeshPanelState {
    shared: Mutex<Shared>,
}

impl Default for NavMeshPanelState {
    fn default() -> Self {
        Self {
            shared: Mutex::new(Shared {
                show_agent_paths: true,
                auto_rebuild: true,
                pending: Vec::new(),
                auto_rebuild_dirty: false,
            }),
        }
    }
}

impl NavMeshPanelState {
    /// True when agent path gizmos should render. Called from the nav
    /// plugin's `draw_agent_paths` system.
    pub fn show_agent_paths(&self) -> bool {
        self.shared.lock().map(|s| s.show_agent_paths).unwrap_or(true)
    }
}

/// Mirror of volume rows, rebuilt each frame by [`refresh_panel_mirror`]
/// so the panel UI (which gets `&World`) doesn't need to drive a query.
#[derive(Resource, Default)]
pub struct NavMeshPanelMirror {
    pub volumes: Vec<VolumeRow>,
    pub agent_count: usize,
}

pub struct VolumeRow {
    pub entity: Entity,
    pub name: String,
    pub debug_draw: bool,
    pub status: NavMeshStatus,
    pub polygon_count: Option<usize>,
}

pub struct NavMeshPanel;

impl EditorPanel for NavMeshPanel {
    fn id(&self) -> &str { "navmesh" }
    fn title(&self) -> &str { "NavMesh" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::POLYGON) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [220.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let Some(state) = world.get_resource::<NavMeshPanelState>() else {
            ui.label("NavMeshPanelState missing");
            return;
        };
        let mirror = world.get_resource::<NavMeshPanelMirror>();

        let (mut show_paths, mut auto_rebuild) = {
            let s = state.shared.lock().unwrap();
            (s.show_agent_paths, s.auto_rebuild)
        };

        ui.horizontal(|ui| {
            if ui.checkbox(&mut show_paths, "Show Agent Paths").changed() {
                let mut s = state.shared.lock().unwrap();
                s.pending.push(PanelAction::SetShowAgentPaths(show_paths));
            }
        });
        ui.horizontal(|ui| {
            if ui.checkbox(&mut auto_rebuild, "Auto Rebuild").changed() {
                let mut s = state.shared.lock().unwrap();
                s.pending.push(PanelAction::SetAutoRebuild(auto_rebuild));
            }
            if ui.button("Rebuild All").clicked() {
                let mut s = state.shared.lock().unwrap();
                if let Some(m) = mirror {
                    for row in &m.volumes {
                        s.pending.push(PanelAction::RebuildVolume(row.entity));
                    }
                }
            }
            if ui.button("Reset Agents").clicked() {
                state.shared.lock().unwrap().pending.push(PanelAction::ResetAgents);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Bake to Disk").clicked() {
                state.shared.lock().unwrap().pending.push(PanelAction::BakeToDisk);
            }
        });

        ui.separator();

        let Some(mirror) = mirror else {
            ui.label("No NavMeshPanelMirror");
            return;
        };

        ui.label(format!(
            "Volumes: {}   Agents: {}",
            mirror.volumes.len(),
            mirror.agent_count
        ));
        ui.add_space(4.0);

        if mirror.volumes.is_empty() {
            ui.label("No NavMesh Volumes in scene. Add the component on any entity to create one.");
            return;
        }

        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            for row in &mirror.volumes {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&row.name);
                        ui.weak(format!("({:?})", row.entity));
                    });
                    ui.horizontal(|ui| {
                        let (label, color) = match row.status {
                            NavMeshStatus::Built => ("Built", egui::Color32::from_rgb(120, 220, 120)),
                            NavMeshStatus::Building => ("Building…", egui::Color32::from_rgb(240, 200, 90)),
                            NavMeshStatus::Failed => ("Failed", egui::Color32::from_rgb(230, 90, 90)),
                            NavMeshStatus::Cancelled => ("Cancelled", egui::Color32::from_rgb(180, 180, 180)),
                            NavMeshStatus::Invalid => ("Invalid", egui::Color32::from_rgb(180, 180, 180)),
                        };
                        ui.colored_label(color, label);
                        if let Some(n) = row.polygon_count {
                            ui.weak(format!("{} polygons", n));
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut debug = row.debug_draw;
                        if ui.checkbox(&mut debug, "Debug Draw").changed() {
                            state.shared.lock().unwrap().pending
                                .push(PanelAction::ToggleVolumeDebug(row.entity));
                        }
                        if ui.button("Rebuild").clicked() {
                            state.shared.lock().unwrap().pending
                                .push(PanelAction::RebuildVolume(row.entity));
                        }
                    });
                });
            }
        });
    }
}

/// Rebuild the panel mirror from the current ECS state.
pub fn refresh_panel_mirror(
    mut mirror: ResMut<NavMeshPanelMirror>,
    volumes: Query<(
        Entity,
        Option<&Name>,
        &NavMeshVolume,
        Option<&NavMeshStatus>,
        Option<&ManagedNavMesh>,
    )>,
    navmeshes: Res<Assets<NavMesh>>,
    agents: Query<(), With<NavPath>>,
) {
    mirror.volumes.clear();
    for (entity, name, vol, status, managed) in &volumes {
        let polygon_count = managed
            .and_then(|m| navmeshes.get(m))
            .and_then(|nm| nm.get().layers.first().map(|l| l.polygons.len()));
        mirror.volumes.push(VolumeRow {
            entity,
            name: name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("Volume {:?}", entity)),
            debug_draw: vol.debug_draw,
            status: status.copied().unwrap_or(NavMeshStatus::Invalid),
            polygon_count,
        });
    }
    mirror.agent_count = agents.iter().count();
}

/// Apply queued UI actions to the world.
pub fn drain_panel_actions(
    state: Res<NavMeshPanelState>,
    mut volumes: Query<(&mut NavMeshVolume, Option<&mut NavMeshUpdateMode>)>,
    mut agents: Query<(&mut crate::NavAgent, &mut NavPath)>,
    mut bake_req: ResMut<NavMeshBakeRequest>,
) {
    let actions: Vec<PanelAction> = {
        let mut s = state.shared.lock().unwrap();
        std::mem::take(&mut s.pending)
    };
    for action in actions {
        let mut s = state.shared.lock().unwrap();
        match action {
            PanelAction::SetShowAgentPaths(v) => {
                s.show_agent_paths = v;
            }
            PanelAction::SetAutoRebuild(v) => {
                s.auto_rebuild = v;
                s.auto_rebuild_dirty = true;
            }
            PanelAction::ToggleVolumeDebug(e) => {
                drop(s);
                if let Ok((mut v, _)) = volumes.get_mut(e) {
                    v.debug_draw = !v.debug_draw;
                }
            }
            PanelAction::RebuildVolume(e) => {
                drop(s);
                if let Ok((_, Some(mut mode))) = volumes.get_mut(e) {
                    *mode = NavMeshUpdateMode::OnDemand(true);
                }
            }
            PanelAction::ResetAgents => {
                drop(s);
                for (mut agent, mut path) in &mut agents {
                    agent.target = None;
                    path.waypoints.clear();
                }
            }
            PanelAction::BakeToDisk => {
                drop(s);
                bake_req.0 = true;
            }
        }
    }
}

/// Resource flag set by `drain_panel_actions` when the user clicks
/// "Bake to Disk". Consumed by the exclusive `flush_bake_request` system
/// which has full `&World` access.
#[derive(Resource, Default)]
pub struct NavMeshBakeRequest(pub bool);

/// Apply queued bake requests (exclusive system — needs `&mut World`).
pub fn flush_bake_request(world: &mut World) {
    let should_bake = world
        .get_resource::<NavMeshBakeRequest>()
        .map(|r| r.0)
        .unwrap_or(false);
    if !should_bake {
        return;
    }
    world.resource_mut::<NavMeshBakeRequest>().0 = false;
    crate::persistence::bake_navmesh_to_disk(world);
}

/// When "Auto Rebuild" toggles, flip every volume's `NavMeshUpdateMode`
/// between Direct (rebuild on change) and OnDemand(false) (manual only).
pub fn apply_auto_rebuild_setting(
    state: Res<NavMeshPanelState>,
    mut volumes: Query<&mut NavMeshUpdateMode, With<NavMeshVolume>>,
) {
    let (dirty, auto) = {
        let mut s = state.shared.lock().unwrap();
        let out = (s.auto_rebuild_dirty, s.auto_rebuild);
        s.auto_rebuild_dirty = false;
        out
    };
    if !dirty { return; }
    let target = if auto {
        NavMeshUpdateMode::Direct
    } else {
        NavMeshUpdateMode::OnDemand(false)
    };
    for mut mode in &mut volumes {
        *mode = target;
    }
}
