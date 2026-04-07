//! State resources for physics panels

use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};

const MAX_SAMPLES: usize = 120;

// ============================================================================
// Physics Debug State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColliderShapeType {
    Sphere,
    Box,
    Capsule,
    Cylinder,
    Cone,
    ConvexHull,
    TriMesh,
    HeightField,
    Compound,
    Unknown,
}

impl std::fmt::Display for ColliderShapeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sphere => write!(f, "Sphere"),
            Self::Box => write!(f, "Box"),
            Self::Capsule => write!(f, "Capsule"),
            Self::Cylinder => write!(f, "Cylinder"),
            Self::Cone => write!(f, "Cone"),
            Self::ConvexHull => write!(f, "Convex Hull"),
            Self::TriMesh => write!(f, "Trimesh"),
            Self::HeightField => write!(f, "Heightfield"),
            Self::Compound => write!(f, "Compound"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PhysicsDebugToggles {
    pub show_colliders: bool,
    pub show_contacts: bool,
    pub show_aabbs: bool,
    pub show_velocities: bool,
    pub show_center_of_mass: bool,
    pub show_joints: bool,
}

#[derive(Clone, Debug)]
pub struct CollisionPairInfo {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub contact_count: usize,
}

#[derive(Resource, Clone)]
pub struct PhysicsDebugState {
    pub simulation_running: bool,
    pub dynamic_body_count: usize,
    pub kinematic_body_count: usize,
    pub static_body_count: usize,
    pub collider_count: usize,
    pub colliders_by_type: HashMap<ColliderShapeType, usize>,
    pub collision_pair_count: usize,
    pub collision_pairs: Vec<CollisionPairInfo>,
    pub step_time_history: VecDeque<f32>,
    pub step_time_ms: f32,
    pub avg_step_time_ms: f32,
    pub debug_toggles: PhysicsDebugToggles,
    pub show_collision_pairs: bool,
    pub update_interval: f32,
    pub time_since_update: f32,
    pub physics_available: bool,
}

impl Default for PhysicsDebugState {
    fn default() -> Self {
        Self {
            simulation_running: false,
            dynamic_body_count: 0,
            kinematic_body_count: 0,
            static_body_count: 0,
            collider_count: 0,
            colliders_by_type: HashMap::new(),
            collision_pair_count: 0,
            collision_pairs: Vec::new(),
            step_time_history: VecDeque::with_capacity(MAX_SAMPLES),
            step_time_ms: 0.0,
            avg_step_time_ms: 0.0,
            debug_toggles: PhysicsDebugToggles::default(),
            show_collision_pairs: false,
            update_interval: 0.1,
            time_since_update: 0.0,
            physics_available: false,
        }
    }
}

impl PhysicsDebugState {
    pub fn push_step_time(&mut self, time_ms: f32) {
        if self.step_time_history.len() >= MAX_SAMPLES {
            self.step_time_history.pop_front();
        }
        self.step_time_history.push_back(time_ms);
        if !self.step_time_history.is_empty() {
            self.avg_step_time_ms =
                self.step_time_history.iter().sum::<f32>() / self.step_time_history.len() as f32;
        }
    }

    pub fn total_body_count(&self) -> usize {
        self.dynamic_body_count + self.kinematic_body_count + self.static_body_count
    }
}

// ============================================================================
// Physics Playground State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlaygroundShape {
    #[default]
    Sphere,
    Box,
    Capsule,
    Cylinder,
}

impl PlaygroundShape {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sphere => "Sphere",
            Self::Box => "Box",
            Self::Capsule => "Capsule",
            Self::Cylinder => "Cylinder",
        }
    }
    pub const ALL: &'static [Self] = &[Self::Sphere, Self::Box, Self::Capsule, Self::Cylinder];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpawnPattern {
    #[default]
    Single,
    Stack,
    Wall,
    Rain,
    Pyramid,
    Explosion,
}

impl SpawnPattern {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Stack => "Stack",
            Self::Wall => "Wall",
            Self::Rain => "Rain",
            Self::Pyramid => "Pyramid",
            Self::Explosion => "Explosion",
        }
    }
    pub const ALL: &'static [Self] = &[
        Self::Single, Self::Stack, Self::Wall, Self::Rain, Self::Pyramid, Self::Explosion,
    ];
}

#[derive(Clone, Debug)]
pub enum PlaygroundCommand {
    Spawn,
    ClearAll,
}

#[derive(Component)]
pub struct PlaygroundEntity;

#[derive(Resource, Clone)]
pub struct PlaygroundState {
    pub shape: PlaygroundShape,
    pub pattern: SpawnPattern,
    pub count: u32,
    pub mass: f32,
    pub restitution: f32,
    pub friction: f32,
    pub spawn_height: f32,
    pub alive_count: usize,
    pub commands: Vec<PlaygroundCommand>,
}

impl Default for PlaygroundState {
    fn default() -> Self {
        Self {
            shape: PlaygroundShape::Sphere,
            pattern: SpawnPattern::Single,
            count: 10,
            mass: 1.0,
            restitution: 0.3,
            friction: 0.5,
            spawn_height: 10.0,
            alive_count: 0,
            commands: Vec::new(),
        }
    }
}

// ============================================================================
// Physics Scenarios State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScenarioType {
    #[default]
    NewtonsCradle,
    DominoChain,
    WreckingBall,
    StackTest,
    BilliardBreak,
    InclinedPlane,
    Pendulum,
    ProjectileLaunch,
    Gauntlet,
    Avalanche,
    WedgeStress,
    BowlingAlley,
    Catapult,
    BalancingAct,
    Marbles,
    Jenga,
    Plinko,
    Conveyor,
    Trebuchet,
    RubeGoldberg,
    Waterfall,
    Cannon,
    Bridge,
    Elevator,
    Pinball,
    Spinner,
    Hourglass,
    MassComparison,
    Ricochet,
    BoxFort,
    Spiral,
    Trampoline,
    ChainReaction,
    Freefall,
    Slingshot,
}

impl ScenarioType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NewtonsCradle => "Newton's Cradle",
            Self::DominoChain => "Domino Chain",
            Self::WreckingBall => "Wrecking Ball",
            Self::StackTest => "Stack Test",
            Self::BilliardBreak => "Billiard Break",
            Self::InclinedPlane => "Inclined Plane",
            Self::Pendulum => "Pendulum",
            Self::ProjectileLaunch => "Projectile",
            Self::Gauntlet => "Gauntlet",
            Self::Avalanche => "Avalanche",
            Self::WedgeStress => "Wedge Stress",
            Self::BowlingAlley => "Bowling",
            Self::Catapult => "Catapult",
            Self::BalancingAct => "Balancing Act",
            Self::Marbles => "Marbles",
            Self::Jenga => "Jenga Tower",
            Self::Plinko => "Plinko",
            Self::Conveyor => "Conveyor",
            Self::Trebuchet => "Trebuchet",
            Self::RubeGoldberg => "Rube Goldberg",
            Self::Waterfall => "Waterfall",
            Self::Cannon => "Cannon",
            Self::Bridge => "Bridge",
            Self::Elevator => "Elevator",
            Self::Pinball => "Pinball",
            Self::Spinner => "Spinner",
            Self::Hourglass => "Hourglass",
            Self::MassComparison => "Mass Compare",
            Self::Ricochet => "Ricochet",
            Self::BoxFort => "Box Fort",
            Self::Spiral => "Spiral Drop",
            Self::Trampoline => "Trampoline",
            Self::ChainReaction => "Chain React",
            Self::Freefall => "Free Fall",
            Self::Slingshot => "Slingshot",
        }
    }
    pub fn icon(&self) -> &'static str {
        use renzora::egui_phosphor::regular::*;
        match self {
            Self::NewtonsCradle => ATOM,
            Self::DominoChain => CARDS,
            Self::WreckingBall => BASEBALL,
            Self::StackTest => STACK,
            Self::BilliardBreak => CIRCLE,
            Self::InclinedPlane => ARROW_FAT_RIGHT,
            Self::Pendulum => METRONOME,
            Self::ProjectileLaunch => ROCKET_LAUNCH,
            Self::Gauntlet => SWORD,
            Self::Avalanche => MOUNTAINS,
            Self::WedgeStress => WARNING,
            Self::BowlingAlley => BOWLING_BALL,
            Self::Catapult => ARROW_FAT_UP,
            Self::BalancingAct => SCALES,
            Self::Marbles => CIRCLES_THREE,
            Self::Jenga => WALL,
            Self::Plinko => FUNNEL,
            Self::Conveyor => ARROWS_SPLIT,
            Self::Trebuchet => ARROWS_OUT_LINE_VERTICAL,
            Self::RubeGoldberg => GEAR_SIX,
            Self::Waterfall => DROP,
            Self::Cannon => FIRE_SIMPLE,
            Self::Bridge => BRIDGE,
            Self::Elevator => ELEVATOR,
            Self::Pinball => DISC,
            Self::Spinner => FAN,
            Self::Hourglass => HOURGLASS,
            Self::MassComparison => BARBELL,
            Self::Ricochet => PATH,
            Self::BoxFort => WAREHOUSE,
            Self::Spiral => SPIRAL,
            Self::Trampoline => ARROWS_OUT_LINE_VERTICAL,
            Self::ChainReaction => LIGHTNING,
            Self::Freefall => ARROW_FAT_DOWN,
            Self::Slingshot => ARROW_FAT_LINE_UP,
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            Self::NewtonsCradle => "5 spheres on joints \u{2014} conservation of momentum",
            Self::DominoChain => "20 dominoes in a curved line \u{2014} chain reaction",
            Self::WreckingBall => "Heavy sphere smashes a wall of boxes",
            Self::StackTest => "Tower of boxes \u{2014} stability and solver test",
            Self::BilliardBreak => "Triangle of spheres + cue ball",
            Self::InclinedPlane => "Static ramp with objects sliding down",
            Self::Pendulum => "Sphere on a distance joint \u{2014} harmonic motion",
            Self::ProjectileLaunch => "Projectile at angle \u{2014} ballistic trajectory",
            Self::Gauntlet => "Obstacle course: ramps, pendulums, platforms",
            Self::Avalanche => "Steep slope with pile of mixed shapes",
            Self::WedgeStress => "V-wedges, tight corners, funnels \u{2014} collision stress",
            Self::BowlingAlley => "10 pins at the end of a lane + heavy ball",
            Self::Catapult => "Lever arm launches a projectile at a target",
            Self::BalancingAct => "Seesaw with objects of different masses",
            Self::Marbles => "Spheres rolling down a spiral track",
            Self::Jenga => "Alternating cross-stacked layers of blocks",
            Self::Plinko => "Balls bounce through rows of pegs",
            Self::Conveyor => "Angled platforms pass objects along a chain",
            Self::Trebuchet => "Counterweight arm flings a projectile",
            Self::RubeGoldberg => "Multi-stage chain reaction machine",
            Self::Waterfall => "Stream of spheres pouring off a ledge",
            Self::Cannon => "Heavy ball fired at high speed into a fortress",
            Self::Bridge => "Plank bridge over a gap \u{2014} load it until it breaks",
            Self::Elevator => "Stacked platforms with objects riding up and down",
            Self::Pinball => "Ball launched into a field of round bumpers",
            Self::Spinner => "Central spinning arm sweeping objects off platforms",
            Self::Hourglass => "Two funnels connected \u{2014} objects flow top to bottom",
            Self::MassComparison => "Side-by-side drops of light vs heavy objects",
            Self::Ricochet => "Ball bouncing off angled walls in a closed chamber",
            Self::BoxFort => "Hollow fortress of boxes \u{2014} bombard it",
            Self::Spiral => "Objects spiral down a helical ramp",
            Self::Trampoline => "High-restitution floor \u{2014} objects bounce forever",
            Self::ChainReaction => "Falling blocks trigger progressively larger topples",
            Self::Freefall => "Objects dropped from great height onto a target",
            Self::Slingshot => "Y-frame launcher flings objects at a wall",
        }
    }
    pub const ALL: &'static [Self] = &[
        Self::NewtonsCradle, Self::DominoChain, Self::WreckingBall, Self::StackTest,
        Self::BilliardBreak, Self::InclinedPlane, Self::Pendulum, Self::ProjectileLaunch,
        Self::Gauntlet, Self::Avalanche, Self::WedgeStress, Self::BowlingAlley,
        Self::Catapult, Self::BalancingAct, Self::Marbles, Self::Jenga,
        Self::Plinko, Self::Conveyor, Self::Trebuchet, Self::RubeGoldberg,
        Self::Waterfall, Self::Cannon, Self::Bridge, Self::Elevator,
        Self::Pinball, Self::Spinner, Self::Hourglass, Self::MassComparison,
        Self::Ricochet, Self::BoxFort, Self::Spiral, Self::Trampoline,
        Self::ChainReaction, Self::Freefall, Self::Slingshot,
    ];
}

#[derive(Clone, Debug)]
pub enum ScenarioCommand {
    Spawn,
    ClearScenario,
}

#[derive(Component)]
pub struct ScenarioEntity;

#[derive(Resource, Clone)]
pub struct ScenariosState {
    pub selected_scenario: ScenarioType,
    pub scale: f32,
    pub commands: Vec<ScenarioCommand>,
    pub alive_count: usize,
}

impl Default for ScenariosState {
    fn default() -> Self {
        Self {
            selected_scenario: ScenarioType::NewtonsCradle,
            scale: 1.0,
            commands: Vec::new(),
            alive_count: 0,
        }
    }
}

// ============================================================================
// Physics Forces State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ForceMode {
    #[default]
    Force,
    Impulse,
    Torque,
    VelocityOverride,
}

impl ForceMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Force => "Force",
            Self::Impulse => "Impulse",
            Self::Torque => "Torque",
            Self::VelocityOverride => "Velocity",
        }
    }
    pub const ALL: &'static [Self] = &[Self::Force, Self::Impulse, Self::Torque, Self::VelocityOverride];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DirectionPreset {
    #[default]
    Up,
    Down,
    Left,
    Right,
    Forward,
    Back,
    Custom,
}

impl DirectionPreset {
    pub fn to_vec3(&self) -> Vec3 {
        match self {
            Self::Up => Vec3::Y,
            Self::Down => Vec3::NEG_Y,
            Self::Left => Vec3::NEG_X,
            Self::Right => Vec3::X,
            Self::Forward => Vec3::NEG_Z,
            Self::Back => Vec3::Z,
            Self::Custom => Vec3::Y,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Forward => "Fwd",
            Self::Back => "Back",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ForceCommand {
    Apply {
        entity: Entity,
        mode: ForceMode,
        direction: Vec3,
        magnitude: f32,
    },
    Explosion {
        origin: Vec3,
        radius: f32,
        magnitude: f32,
    },
    SetVelocity {
        entity: Entity,
        linear: Vec3,
        angular: Vec3,
    },
    ZeroMotion {
        entity: Entity,
    },
}

#[derive(Resource, Clone)]
pub struct ForcesState {
    pub mode: ForceMode,
    pub direction_preset: DirectionPreset,
    pub custom_direction: Vec3,
    pub magnitude: f32,
    pub explosion_radius: f32,
    pub explosion_magnitude: f32,
    pub velocity_linear: Vec3,
    pub velocity_angular: Vec3,
    pub selected_entity: Option<Entity>,
    pub selected_has_rigidbody: bool,
    pub selected_linear_velocity: Vec3,
    pub selected_angular_velocity: Vec3,
    pub commands: Vec<ForceCommand>,
}

impl Default for ForcesState {
    fn default() -> Self {
        Self {
            mode: ForceMode::Impulse,
            direction_preset: DirectionPreset::Up,
            custom_direction: Vec3::Y,
            magnitude: 10.0,
            explosion_radius: 10.0,
            explosion_magnitude: 20.0,
            velocity_linear: Vec3::ZERO,
            velocity_angular: Vec3::ZERO,
            selected_entity: None,
            selected_has_rigidbody: false,
            selected_linear_velocity: Vec3::ZERO,
            selected_angular_velocity: Vec3::ZERO,
            commands: Vec::new(),
        }
    }
}

// ============================================================================
// Physics Metrics State
// ============================================================================

const MAX_HISTORY: usize = 300;

#[derive(Resource, Clone)]
pub struct MetricsState {
    pub total_kinetic_energy: f64,
    pub total_potential_energy: f64,
    pub total_energy: f64,
    pub energy_history: VecDeque<f64>,
    pub avg_velocity: f32,
    pub max_velocity: f32,
    pub active_bodies: usize,
    pub sleeping_bodies: usize,
    pub total_momentum: Vec3,
    pub frame_physics_time_ms: f32,
    pub physics_time_history: VecDeque<f32>,
    pub collision_count: usize,
    pub collision_pairs_history: VecDeque<usize>,
    pub tracking_enabled: bool,
    pub update_interval: f32,
    pub time_since_update: f32,
}

impl Default for MetricsState {
    fn default() -> Self {
        Self {
            total_kinetic_energy: 0.0,
            total_potential_energy: 0.0,
            total_energy: 0.0,
            energy_history: VecDeque::with_capacity(MAX_HISTORY),
            avg_velocity: 0.0,
            max_velocity: 0.0,
            active_bodies: 0,
            sleeping_bodies: 0,
            total_momentum: Vec3::ZERO,
            frame_physics_time_ms: 0.0,
            physics_time_history: VecDeque::with_capacity(MAX_HISTORY),
            collision_count: 0,
            collision_pairs_history: VecDeque::with_capacity(MAX_HISTORY),
            tracking_enabled: true,
            update_interval: 0.05,
            time_since_update: 0.0,
        }
    }
}

// ============================================================================
// Arena Presets State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ArenaType {
    #[default]
    Walled,
    StairFall,
    MovingPlatforms,
    HillSlide,
    Pinball,
    GolfCourse,
    Funnel,
    Colosseum,
    Maze,
    Halfpipe,
    TieredPlatforms,
    Canyon,
    Fortress,
    Spiral,
    Bowl,
    Pillars,
    IceRink,
    Volcano,
    Labyrinth,
}

impl ArenaType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Walled => "Walled Arena",
            Self::StairFall => "Stair Fall",
            Self::MovingPlatforms => "Moving Platforms",
            Self::HillSlide => "Hill Slide",
            Self::Pinball => "Pinball",
            Self::GolfCourse => "Golf Course",
            Self::Funnel => "Funnel",
            Self::Colosseum => "Colosseum",
            Self::Maze => "Maze",
            Self::Halfpipe => "Halfpipe",
            Self::TieredPlatforms => "Tiered",
            Self::Canyon => "Canyon",
            Self::Fortress => "Fortress",
            Self::Spiral => "Spiral",
            Self::Bowl => "Bowl",
            Self::Pillars => "Pillar Field",
            Self::IceRink => "Ice Rink",
            Self::Volcano => "Volcano",
            Self::Labyrinth => "Labyrinth",
        }
    }

    pub fn icon(&self) -> &'static str {
        use renzora::egui_phosphor::regular::*;
        match self {
            Self::Walled => SQUARE,
            Self::StairFall => STAIRS,
            Self::MovingPlatforms => ARROWS_OUT_LINE_HORIZONTAL,
            Self::HillSlide => MOUNTAINS,
            Self::Pinball => DISC,
            Self::GolfCourse => GOLF,
            Self::Funnel => FUNNEL,
            Self::Colosseum => BUILDINGS,
            Self::Maze => GRID_FOUR,
            Self::Halfpipe => PIPE,
            Self::TieredPlatforms => STACK,
            Self::Canyon => SPLIT_VERTICAL,
            Self::Fortress => CASTLE_TURRET,
            Self::Spiral => SPIRAL,
            Self::Bowl => BOWL_FOOD,
            Self::Pillars => COLUMNS,
            Self::IceRink => SNOWFLAKE,
            Self::Volcano => FIRE,
            Self::Labyrinth => COMPASS_TOOL,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Walled => "Flat floor with walls, ramps, and pillars",
            Self::StairFall => "Giant staircase \u{2014} objects tumble down steps",
            Self::MovingPlatforms => "Kinematic platforms that move back and forth",
            Self::HillSlide => "Steep angled slope \u{2014} objects slide and roll to the bottom",
            Self::Pinball => "Vertical board with bumpers, flippers, and gutters",
            Self::GolfCourse => "Rolling hills with holes \u{2014} objects settle into valleys",
            Self::Funnel => "Large funnel that channels objects down into a tight space",
            Self::Colosseum => "Circular arena with tiered seating walls and central pit",
            Self::Maze => "Grid of walls forming a maze with dead ends",
            Self::Halfpipe => "U-shaped ramp for back-and-forth rolling",
            Self::TieredPlatforms => "Cascading platforms at different heights",
            Self::Canyon => "Narrow canyon with high walls and rocky floor",
            Self::Fortress => "Castle-like structure with walls, towers, and courtyard",
            Self::Spiral => "Helical ramp winding down around a central column",
            Self::Bowl => "Large concave bowl \u{2014} objects roll to the center",
            Self::Pillars => "Open field of randomly placed pillars to weave through",
            Self::IceRink => "Flat low-friction surface with bumper walls",
            Self::Volcano => "Cone with a crater \u{2014} objects slide down the sides",
            Self::Labyrinth => "Complex winding corridors with multiple paths",
        }
    }

    pub const ALL: &'static [Self] = &[
        Self::Walled, Self::StairFall, Self::MovingPlatforms,
        Self::HillSlide, Self::Pinball, Self::GolfCourse, Self::Funnel,
        Self::Colosseum, Self::Maze, Self::Halfpipe, Self::TieredPlatforms,
        Self::Canyon, Self::Fortress, Self::Spiral, Self::Bowl,
        Self::Pillars, Self::IceRink, Self::Volcano, Self::Labyrinth,
    ];
}

#[derive(Clone, Debug)]
pub enum ArenaCommand {
    Spawn,
    Clear,
}

#[derive(Component)]
pub struct ArenaEntity;

#[derive(Resource, Clone)]
pub struct ArenaPresetsState {
    pub arena_type: ArenaType,
    pub scale: f32,
    pub arena_entity_count: usize,
    pub commands: Vec<ArenaCommand>,
    pub has_active_arena: bool,
}

impl Default for ArenaPresetsState {
    fn default() -> Self {
        Self {
            arena_type: ArenaType::default(),
            scale: 1.0,
            arena_entity_count: 0,
            commands: Vec::new(),
            has_active_arena: false,
        }
    }
}

impl MetricsState {
    pub fn push_energy(&mut self, value: f64) {
        if self.energy_history.len() >= MAX_HISTORY { self.energy_history.pop_front(); }
        self.energy_history.push_back(value);
    }
    pub fn push_physics_time(&mut self, value: f32) {
        if self.physics_time_history.len() >= MAX_HISTORY { self.physics_time_history.pop_front(); }
        self.physics_time_history.push_back(value);
    }
    pub fn push_collisions(&mut self, value: usize) {
        if self.collision_pairs_history.len() >= MAX_HISTORY { self.collision_pairs_history.pop_front(); }
        self.collision_pairs_history.push_back(value);
    }
}
