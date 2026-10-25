//! AnimatorComponent: scene-serializable animation controller.
//! AnimatorState: ephemeral runtime state rebuilt on load.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::controller::{BaseController, LayerController};
use crate::graph_builder::GraphLayout;
use crate::layers::AnimationLayer;
use crate::markers::NodeCursor;
use crate::retarget::RestSkeleton;
use crate::root_motion::RootMotion;
use crate::state_machine::{AnimParams, AnimationStateMachine};

/// Serializable clip slot: a named reference to a `.anim` file.
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
pub struct AnimClipSlot {
    /// Label used by scripting ("idle", "walk", etc.).
    pub name: String,
    /// Asset-relative path to the `.anim` file (e.g. `animations/walk.anim`).
    pub path: String,
    /// Whether this clip loops by default.
    pub looping: bool,
    /// Default playback speed.
    pub speed: f32,
    /// Override crossfade time when transitioning INTO this clip.
    /// `None` → fall back to [`AnimatorComponent::blend_duration`].
    #[serde(default)]
    pub blend_in: Option<f32>,
    /// Override crossfade time when transitioning OUT of this clip.
    /// `None` → fall back to [`AnimatorComponent::blend_duration`].
    #[serde(default)]
    pub blend_out: Option<f32>,
    /// Asset-relative path to the `.skel` rest pose of the skeleton this clip
    /// was authored on. When set, the clip is retargeted onto this animator's
    /// skeleton (see [`crate::retarget`]); when not, it binds by bone name
    /// alone, as clips always did.
    #[serde(default)]
    #[reflect(default)]
    pub source_skeleton: Option<String>,
}

impl AnimClipSlot {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            looping: true,
            speed: 1.0,
            blend_in: None,
            blend_out: None,
            source_skeleton: None,
        }
    }

    /// The source skeleton path, if one is set.
    pub fn skeleton_path(&self) -> Option<&str> {
        self.source_skeleton.as_deref().map(str::trim).filter(|p| !p.is_empty())
    }
}

/// Scene-persistent component that defines which animation clips an entity has.
///
/// Attach this to a GLTF model entity. The runtime will build an AnimationGraph,
/// locate the AnimationPlayer in children, and drive playback.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AnimatorComponent {
    /// Named clip slots.
    pub clips: Vec<AnimClipSlot>,
    /// Which clip to auto-play on spawn (matches `AnimClipSlot::name`).
    pub default_clip: Option<String>,
    /// Crossfade duration in seconds when transitioning between clips.
    pub blend_duration: f32,
    /// Optional path to a `.animsm` state machine file.
    pub state_machine: Option<String>,
    /// Animation layers, stacked above the base motion in order.
    pub layers: Vec<AnimationLayer>,
    /// Root motion: move the entity by the root bone's movement instead of
    /// letting the clip carry the mesh away from it. Off by default.
    #[serde(default)]
    #[reflect(default)]
    pub root_motion: RootMotion,
    /// Source bone name to this rig's bone name, for retargeting clips whose
    /// bones are named differently. Either spelling of a name matches.
    #[serde(default)]
    #[reflect(default)]
    pub bone_map: HashMap<String, String>,
}

impl AnimatorComponent {
    pub fn new() -> Self {
        Self {
            clips: Vec::new(),
            default_clip: None,
            blend_duration: 0.2,
            state_machine: None,
            layers: Vec::new(),
            root_motion: RootMotion::default(),
            bone_map: HashMap::new(),
        }
    }

    pub fn add_clip(&mut self, slot: AnimClipSlot) {
        self.clips.push(slot);
    }

    pub fn get_slot(&self, name: &str) -> Option<&AnimClipSlot> {
        self.clips.iter().find(|s| s.name == name)
    }

    pub fn get_slot_mut(&mut self, name: &str) -> Option<&mut AnimClipSlot> {
        self.clips.iter_mut().find(|s| s.name == name)
    }

    /// A fingerprint of everything the animation graph is built from besides
    /// the clips themselves (layers and their masks, retargeting). When it
    /// changes, the graph is rebuilt; a layer's weight or current clip is not
    /// in it, since those are applied per frame.
    pub fn graph_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for layer in &self.layers {
            layer.name.hash(&mut h);
            (layer.blend_mode as u8).hash(&mut h);
            layer.active_mask().hash(&mut h);
        }
        for slot in &self.clips {
            slot.skeleton_path().hash(&mut h);
        }
        let mut map: Vec<_> = self.bone_map.iter().collect();
        map.sort();
        map.hash(&mut h);
        self.root_motion.mode.hash(&mut h);
        self.root_motion.bone.hash(&mut h);
        h.finish()
    }
}

/// Runtime-only state for an animator. Not serialized; rebuilt on scene load.
#[derive(Component)]
pub struct AnimatorState {
    /// Currently active clip name (for a blend tree, its heaviest clip).
    pub current_clip: Option<String>,
    /// Whether playback is paused.
    pub is_paused: bool,
    /// Loaded clip handles keyed by slot name.
    pub clip_handles: HashMap<String, Handle<AnimationClip>>,
    /// Base node index in the AnimationGraph per clip name. The same nodes as
    /// `layout.base`, kept as its own map because the editor preview reads it.
    pub node_indices: HashMap<String, AnimationNodeIndex>,
    /// Every clip node of the graph by role.
    pub layout: GraphLayout,
    /// The AnimationGraph handle assigned to the player.
    pub graph_handle: Option<Handle<AnimationGraph>>,
    /// The entity that has the AnimationPlayer (found in children).
    pub player_entity: Option<Entity>,
    /// Whether initialization is complete.
    pub initialized: bool,
    /// Frames since `initialize_animation_graphs` finished, used to bound
    /// the cost of `ensure_animation_targets`, which only needs to run until
    /// the asynchronously-spawning GLB skeleton has stabilized.
    pub frames_since_init: u32,
    /// Number of AnimationTargetId-bearing descendants seen on the last
    /// `ensure_animation_targets` pass. When it stops growing we stop walking.
    pub last_target_count: u32,
    /// Frames initialisation has waited for a skeleton or a sidecar asset.
    pub init_wait_frames: u32,
    /// [`AnimatorComponent::graph_signature`] the graph was built from.
    pub graph_signature: u64,
    /// Loaded state machine asset handle.
    pub sm_handle: Option<Handle<AnimationStateMachine>>,
    /// Current state name in the state machine.
    pub current_state: Option<String>,
    /// Time spent in the current state (seconds).
    pub state_time: f32,
    /// The current state's motion has been handed to the base controller
    /// (states entered while editing only set the current clip).
    pub sm_playing: bool,
    /// Runtime animation parameters (floats, bools, triggers).
    pub params: AnimParams,
    /// Loaded property-animation clip handles keyed by slot name (the
    /// `property_tracks` half of each `.anim`, sampled by the custom property
    /// sampler rather than Bevy's `AnimationPlayer`).
    pub prop_clip_handles: HashMap<String, Handle<crate::property_playback::PropertyClip>>,
    /// Source skeletons (`.skel`) keyed by slot name.
    pub skeleton_handles: HashMap<String, Handle<crate::retarget::SourceSkeleton>>,
    /// Clips built for this animator (retargeted or difference clips), held so
    /// their assets live as long as the graph that plays them.
    pub derived_clips: Vec<Handle<AnimationClip>>,
    /// The skeleton's rest pose, captured once at first initialisation and
    /// kept across graph rebuilds (a rebuild mid-play would capture a pose).
    pub target_rest: Option<RestSkeleton>,
    /// Playback time (seconds) for property animation, advanced independently
    /// of the skeletal `AnimationPlayer` so property-only entities still animate.
    pub prop_time: f32,
    /// Whether the property sampler's clock has advanced since its clip
    /// started (so its first frame counts a marker at time 0).
    pub prop_started: bool,
    /// When true, property animation is explicitly stopped (set by the `Stop`
    /// command) and won't auto-play the default clip. Cleared by `Play`/`Resume`.
    pub prop_stopped: bool,
    /// Playback speed multiplier for property animation (set by `Play`/`SetSpeed`).
    pub prop_speed: f32,
    /// The base motion's crossfades and blend tree.
    pub base: BaseController,
    /// One per layer, in layer order.
    pub layer_controllers: Vec<LayerController>,
    /// Per-node playback cursors for markers and root motion.
    pub cursors: HashMap<AnimationNodeIndex, NodeCursor>,
    /// Slots whose markers are muted this frame (the lighter clips of a tree).
    pub marker_suppressed: Vec<String>,
    /// The root bone entity and its name, once found.
    pub root_bone: Option<(Entity, String)>,
    /// Root motion applied last frame, in the entity's parent space.
    pub root_motion_delta: Vec3,
    /// Root yaw applied last frame (radians).
    pub root_motion_yaw: f32,
}

impl Default for AnimatorState {
    fn default() -> Self {
        Self {
            current_clip: None,
            is_paused: false,
            clip_handles: HashMap::new(),
            node_indices: HashMap::new(),
            layout: GraphLayout::default(),
            graph_handle: None,
            player_entity: None,
            initialized: false,
            frames_since_init: 0,
            last_target_count: 0,
            init_wait_frames: 0,
            graph_signature: 0,
            sm_handle: None,
            current_state: None,
            state_time: 0.0,
            sm_playing: false,
            params: AnimParams::default(),
            prop_clip_handles: HashMap::new(),
            skeleton_handles: HashMap::new(),
            derived_clips: Vec::new(),
            target_rest: None,
            prop_time: 0.0,
            prop_started: false,
            prop_stopped: false,
            prop_speed: 1.0,
            base: BaseController::default(),
            layer_controllers: Vec::new(),
            cursors: HashMap::new(),
            marker_suppressed: Vec::new(),
            root_bone: None,
            root_motion_delta: Vec3::ZERO,
            root_motion_yaw: 0.0,
        }
    }
}

impl AnimatorState {
    /// Throw the graph away so the next frame rebuilds it. Playback starts
    /// over (the base and layers are cleared, and the state machine re-enters
    /// its default state), because a rebuild renumbers every node.
    pub fn reset_graph(&mut self) {
        self.initialized = false;
        self.init_wait_frames = 0;
        self.current_clip = None;
        self.current_state = None;
        self.sm_playing = false;
        self.node_indices.clear();
        self.layout = GraphLayout::default();
        self.graph_handle = None;
        self.derived_clips.clear();
        self.base.clear();
        self.layer_controllers.clear();
        self.cursors.clear();
        self.marker_suppressed.clear();
        self.root_bone = None;
    }
}
