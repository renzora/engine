//! State recorder — record physics state snapshots, replay as ghosts, compare runs

use bevy::prelude::*;
use std::collections::VecDeque;

/// Recorder mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RecorderMode {
    #[default]
    Idle,
    Recording,
    Replaying,
}

/// Snapshot of a single entity's state
#[derive(Clone, Debug)]
pub struct EntitySnapshot {
    pub entity: Entity,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_vel: Vec3,
    pub angular_vel: Vec3,
}

/// A single frame's worth of entity snapshots
#[derive(Clone, Debug)]
pub struct FrameSnapshot {
    pub entities: Vec<EntitySnapshot>,
}

/// A complete recording
#[derive(Clone, Debug)]
pub struct Recording {
    pub name: String,
    pub frames: Vec<FrameSnapshot>,
    pub created_at: f64,
}

impl Recording {
    pub fn duration_secs(&self) -> f32 {
        // Assume ~60 fps capture
        self.frames.len() as f32 / 60.0
    }
}

/// Commands from the recorder UI
#[derive(Clone, Debug)]
pub enum RecorderCommand {
    StartRecording,
    StopRecording,
    StartReplay(usize),
    StopReplay,
    DeleteRecording(usize),
    NameRecording(usize, String),
}

/// State resource for the State Recorder panel
#[derive(Resource)]
pub struct StateRecorderState {
    /// Current mode
    pub mode: RecorderMode,
    /// Stored recordings
    pub recordings: Vec<Recording>,
    /// Active recording (during recording)
    pub active_recording: Option<Recording>,
    /// Current replay frame index
    pub replay_frame: usize,
    /// Replay speed multiplier
    pub replay_speed: f32,
    /// Whether to show ghost overlays
    pub show_ghost: bool,
    /// Pending commands
    pub commands: Vec<RecorderCommand>,
    /// Which recording is being replayed
    pub replaying_index: Option<usize>,
    /// Frame accumulator for replay speed
    pub replay_accumulator: f32,
    /// Name input buffer for renaming
    pub rename_buffer: String,
}

impl Default for StateRecorderState {
    fn default() -> Self {
        Self {
            mode: RecorderMode::Idle,
            recordings: Vec::new(),
            active_recording: None,
            replay_frame: 0,
            replay_speed: 1.0,
            show_ghost: true,
            commands: Vec::new(),
            replaying_index: None,
            replay_accumulator: 0.0,
            rename_buffer: String::new(),
        }
    }
}

/// System that processes recorder commands and captures/replays state
pub fn process_recorder_commands(
    mut state: ResMut<StateRecorderState>,
    time: Res<Time>,
    bodies: Query<(
        Entity,
        &Transform,
        &avian3d::prelude::RigidBody,
        Option<&avian3d::prelude::LinearVelocity>,
        Option<&avian3d::prelude::AngularVelocity>,
    )>,
) {
    // Process commands
    let cmds: Vec<RecorderCommand> = state.commands.drain(..).collect();
    for cmd in cmds {
        match cmd {
            RecorderCommand::StartRecording => {
                state.mode = RecorderMode::Recording;
                state.active_recording = Some(Recording {
                    name: format!("Recording {}", state.recordings.len() + 1),
                    frames: Vec::new(),
                    created_at: time.elapsed_secs_f64(),
                });
            }
            RecorderCommand::StopRecording => {
                if let Some(recording) = state.active_recording.take() {
                    state.recordings.push(recording);
                }
                state.mode = RecorderMode::Idle;
            }
            RecorderCommand::StartReplay(idx) => {
                if idx < state.recordings.len() {
                    state.mode = RecorderMode::Replaying;
                    state.replaying_index = Some(idx);
                    state.replay_frame = 0;
                    state.replay_accumulator = 0.0;
                }
            }
            RecorderCommand::StopReplay => {
                state.mode = RecorderMode::Idle;
                state.replaying_index = None;
            }
            RecorderCommand::DeleteRecording(idx) => {
                if idx < state.recordings.len() {
                    state.recordings.remove(idx);
                    if state.replaying_index == Some(idx) {
                        state.mode = RecorderMode::Idle;
                        state.replaying_index = None;
                    }
                }
            }
            RecorderCommand::NameRecording(idx, name) => {
                if idx < state.recordings.len() {
                    state.recordings[idx].name = name;
                }
            }
        }
    }

    // Recording: capture a frame snapshot
    if state.mode == RecorderMode::Recording {
        if let Some(ref mut recording) = state.active_recording {
            let mut entities = Vec::new();
            for (entity, transform, body, lin_vel, ang_vel) in bodies.iter() {
                if *body != avian3d::prelude::RigidBody::Dynamic {
                    continue;
                }
                entities.push(EntitySnapshot {
                    entity,
                    position: transform.translation,
                    rotation: transform.rotation,
                    linear_vel: lin_vel.map(|v| v.0).unwrap_or(Vec3::ZERO),
                    angular_vel: ang_vel.map(|v| v.0).unwrap_or(Vec3::ZERO),
                });
            }
            recording.frames.push(FrameSnapshot { entities });
        }
    }

    // Replaying: advance frame counter
    if state.mode == RecorderMode::Replaying {
        state.replay_accumulator += state.replay_speed;
        while state.replay_accumulator >= 1.0 {
            state.replay_frame += 1;
            state.replay_accumulator -= 1.0;
        }

        // Check if replay is done
        if let Some(idx) = state.replaying_index {
            if idx < state.recordings.len() {
                if state.replay_frame >= state.recordings[idx].frames.len() {
                    state.replay_frame = 0; // Loop
                }
            }
        }
    }
}

/// System that renders ghost wireframes at recorded positions
pub fn render_recorder_ghosts(
    state: Res<StateRecorderState>,
    mut gizmos: Gizmos<crate::gizmo::physics_viz::PhysicsVizGizmoGroup>,
) {
    if state.mode != RecorderMode::Replaying || !state.show_ghost {
        return;
    }

    let Some(idx) = state.replaying_index else { return };
    let Some(recording) = state.recordings.get(idx) else { return };

    if state.replay_frame >= recording.frames.len() {
        return;
    }

    let frame = &recording.frames[state.replay_frame];
    let ghost_color = Color::srgba(0.3, 0.8, 1.0, 0.4);

    for snapshot in &frame.entities {
        let pos = snapshot.position;
        let size = 0.3;

        // Draw a wireframe cube as ghost indicator
        let corners = [
            Vec3::new(-size, -size, -size),
            Vec3::new( size, -size, -size),
            Vec3::new( size, -size,  size),
            Vec3::new(-size, -size,  size),
            Vec3::new(-size,  size, -size),
            Vec3::new( size,  size, -size),
            Vec3::new( size,  size,  size),
            Vec3::new(-size,  size,  size),
        ];
        let t: Vec<Vec3> = corners.iter().map(|c| pos + snapshot.rotation * *c).collect();

        gizmos.line(t[0], t[1], ghost_color);
        gizmos.line(t[1], t[2], ghost_color);
        gizmos.line(t[2], t[3], ghost_color);
        gizmos.line(t[3], t[0], ghost_color);
        gizmos.line(t[4], t[5], ghost_color);
        gizmos.line(t[5], t[6], ghost_color);
        gizmos.line(t[6], t[7], ghost_color);
        gizmos.line(t[7], t[4], ghost_color);
        gizmos.line(t[0], t[4], ghost_color);
        gizmos.line(t[1], t[5], ghost_color);
        gizmos.line(t[2], t[6], ghost_color);
        gizmos.line(t[3], t[7], ghost_color);
    }
}
