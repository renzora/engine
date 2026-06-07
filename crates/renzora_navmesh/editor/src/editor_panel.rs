//! NavMesh editor panel — lists volumes, their build status, and exposes
//! per-volume rebuild + debug-draw controls plus a global auto-rebuild
//! toggle. Editor-only (lives in renzora_navmesh_editor).

use std::sync::Mutex;

use bevy::prelude::*;
use vleue_navigator::{
    prelude::{ManagedNavMesh, NavMeshStatus, NavMeshUpdateMode},
    NavMesh,
};

use renzora_navmesh::{NavMeshVolume, NavPath};

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
        self.shared
            .lock()
            .map(|s| s.show_agent_paths)
            .unwrap_or(true)
    }

    /// Current "Auto Rebuild" toggle state. Used by the native panel binding.
    pub(crate) fn auto_rebuild(&self) -> bool {
        self.shared.lock().map(|s| s.auto_rebuild).unwrap_or(true)
    }

    /// Queue a "Show Agent Paths" change (drained by `drain_panel_actions`).
    pub(crate) fn queue_show_agent_paths(&self, v: bool) {
        if let Ok(mut s) = self.shared.lock() {
            s.pending.push(PanelAction::SetShowAgentPaths(v));
        }
    }

    /// Queue an "Auto Rebuild" change (drained by `drain_panel_actions`).
    pub(crate) fn queue_auto_rebuild(&self, v: bool) {
        if let Ok(mut s) = self.shared.lock() {
            s.pending.push(PanelAction::SetAutoRebuild(v));
        }
    }

    /// Queue a per-volume debug-draw toggle.
    pub(crate) fn queue_toggle_volume_debug(&self, e: Entity) {
        if let Ok(mut s) = self.shared.lock() {
            s.pending.push(PanelAction::ToggleVolumeDebug(e));
        }
    }

    /// Queue a rebuild of a single volume.
    pub(crate) fn queue_rebuild_volume(&self, e: Entity) {
        if let Ok(mut s) = self.shared.lock() {
            s.pending.push(PanelAction::RebuildVolume(e));
        }
    }

    /// Queue a "Reset Agents" action.
    pub(crate) fn queue_reset_agents(&self) {
        if let Ok(mut s) = self.shared.lock() {
            s.pending.push(PanelAction::ResetAgents);
        }
    }

    /// Queue a "Bake to Disk" action.
    pub(crate) fn queue_bake_to_disk(&self) {
        if let Ok(mut s) = self.shared.lock() {
            s.pending.push(PanelAction::BakeToDisk);
        }
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
    mut agents: Query<(&mut renzora_navmesh::NavAgent, &mut NavPath)>,
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
    if !dirty {
        return;
    }
    let target = if auto {
        NavMeshUpdateMode::Direct
    } else {
        NavMeshUpdateMode::OnDemand(false)
    };
    for mut mode in &mut volumes {
        *mode = target;
    }
}
