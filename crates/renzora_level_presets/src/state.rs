//! State resources for level presets panel

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LevelPreset {
    #[default]
    FPS,
    ThirdPerson,
    Platformer,
    TopDown,
    Racing,
    Sandbox,
    Corridor,
    Arena,
    Showcase,
    Terrain,
}

impl LevelPreset {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FPS => "First Person",
            Self::ThirdPerson => "Third Person",
            Self::Platformer => "Platformer",
            Self::TopDown => "Top Down",
            Self::Racing => "Racing",
            Self::Sandbox => "Sandbox",
            Self::Corridor => "Corridor",
            Self::Arena => "Arena",
            Self::Showcase => "Showcase",
            Self::Terrain => "Terrain",
        }
    }

    pub fn icon(&self) -> &'static str {
        use renzora::egui_phosphor::regular::*;
        match self {
            Self::FPS => CROSSHAIR,
            Self::ThirdPerson => PERSON,
            Self::Platformer => STAIRS,
            Self::TopDown => MAP_TRIFOLD,
            Self::Racing => FLAG_CHECKERED,
            Self::Sandbox => CUBE,
            Self::Corridor => PATH,
            Self::Arena => SHIELD,
            Self::Showcase => EYE,
            Self::Terrain => MOUNTAINS,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::FPS => "Enclosed room with cover boxes, ramps, and a second-floor balcony — classic FPS test map",
            Self::ThirdPerson => "Open courtyard with walls, pillars, stairs, and elevated walkways for third-person gameplay",
            Self::Platformer => "Floating platforms at varying heights with gaps to jump across",
            Self::TopDown => "Flat grid arena with low walls forming rooms and corridors — top-down perspective",
            Self::Racing => "Oval track with banked curves, start line, and barrier walls",
            Self::Sandbox => "Large flat ground plane with a sun — blank canvas to build on",
            Self::Corridor => "Long L-shaped hallway with doorway-sized openings and crate obstacles",
            Self::Arena => "Circular walled arena with central pillar and symmetrical cover",
            Self::Showcase => "Pedestal platform with rim lighting — display models and materials",
            Self::Terrain => "Hilly landscape with stepped elevations and a valley",
        }
    }

    pub const ALL: &'static [Self] = &[
        Self::FPS, Self::ThirdPerson, Self::Platformer, Self::TopDown,
        Self::Racing, Self::Sandbox, Self::Corridor, Self::Arena,
        Self::Showcase, Self::Terrain,
    ];
}

#[derive(Clone, Debug)]
pub enum LevelCommand {
    Spawn,
    Clear,
}

/// Marker component for entities spawned by level presets.
#[derive(Component)]
pub struct LevelPresetEntity;

#[derive(Resource, Clone)]
pub struct LevelPresetsState {
    pub selected: LevelPreset,
    pub scale: f32,
    pub entity_count: usize,
    pub commands: Vec<LevelCommand>,
    pub has_active_level: bool,
}

impl Default for LevelPresetsState {
    fn default() -> Self {
        Self {
            selected: LevelPreset::default(),
            scale: 1.0,
            entity_count: 0,
            commands: Vec::new(),
            has_active_level: false,
        }
    }
}
