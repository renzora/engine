//! Old Hanabi effects keep working: `.particle` files load as effects, and a
//! scene's `HanabiEffect` component becomes a [`ParticleSystem`] as it loads.
//!
//! The Hanabi integration (`renzora_hanabi` over a vendored `bevy_hanabi`) was
//! removed once this system could do everything it did. Projects made before
//! that are full of its two artefacts, and neither may break:
//!
//! - **`.particle` files.** [`ParticleFileLoader`] claims the extension and
//!   converts the file in memory ([`crate::convert`]) into the same
//!   [`VfxAsset`] a `.vfx` loads as. So a `ParticleSystem` can point straight
//!   at one, in the editor and in a shipped game alike, and hot reload works
//!   because the asset server reloads it like any other file. Nothing is
//!   written: the user's file is read and never touched.
//! - **Scenes.** A saved scene names the old component by its full type path,
//!   `renzora_hanabi::data::HanabiEffect`. [`HanabiEffect`] here has the same
//!   short name and the same serde shape, and the scene loader falls back to
//!   the short name when a full path no longer resolves (`renzora_bsn`'s
//!   `resolve_registration`), so the old line deserializes into it with no
//!   alias table and no text rewrite. An observer then swaps it for a
//!   `ParticleSystem` (see [`migrate_effect`]) the moment it is inserted, so
//!   nothing else ever sees it, and the next save writes the new component.
//!
//! A text rewrite of the scene before parsing was the other way to do the
//! scene half, and it was not taken: the component body is RON nested inside
//! the scene's own grammar, and rewriting it means parsing it, which is what
//! the reflection path already does correctly. The observer also catches the
//! component however it arrives (a prefab, a plugin inserting it by name, a
//! network replica), not only from a `.bsn`.
//!
//! The editor builds on this (`renzora_particles_editor::legacy`): on project
//! open it writes a `.vfx` beside every `.particle` that has none, and points
//! the scene's effects at those, so the effects become editable. A shipped
//! game does not need that step, which is why the in-memory path exists.

use std::collections::HashMap;

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::asset::VfxAsset;
use crate::component::{ParticlePlayback, ParticleSystem, PlayState};
use crate::convert::format::{EffectSource, EffectVariable};
use crate::convert::{self, Conversion, PARTICLE_EXTENSION};

/// Console category for everything this module reports.
pub const CONSOLE_CATEGORY: &str = "Particles";

// ── The old scene component ─────────────────────────────────────────────────

/// The retired Hanabi integration's scene component, kept only so old scenes
/// deserialize. It never stays on an entity: [`migrate_on_insert`] replaces it
/// with a [`ParticleSystem`] as soon as it is inserted.
///
/// Serde does the (de)serializing (`ReflectDeserialize`), which is what reads
/// a scene's RON body exactly as the old type did. It is reflected field by
/// field as well, not opaque, because a plugin may insert it by name from a
/// dynamic value (see [`crate::convert::format`]), and only real fields let
/// `FromReflect` build it from one.
///
/// `rate_multiplier`, `scale_multiplier`, `color_tint`, `time_scale` and
/// `variable_overrides` are accepted and then ignored, deliberately: the old
/// runtime stored them (scripts could set them) but never applied any of them
/// to the effect, so honouring them now would change how an old scene looks.
#[derive(Component, Clone, Debug, Serialize, Deserialize, Reflect)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HanabiEffect {
    pub source: EffectSource,
    pub playing: bool,
    pub rate_multiplier: f32,
    pub scale_multiplier: f32,
    pub color_tint: [f32; 4],
    pub time_scale: f32,
    #[reflect(ignore)]
    pub variable_overrides: HashMap<String, EffectVariable>,
}

impl Default for HanabiEffect {
    fn default() -> Self {
        Self {
            source: EffectSource::default(),
            playing: true,
            rate_multiplier: 1.0,
            scale_multiplier: 1.0,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            time_scale: 1.0,
            variable_overrides: HashMap::new(),
        }
    }
}

/// Put on an entity whose old component held its effect inline (inside the
/// scene, not in a `.particle`). Its [`ParticleSystem`] plays the conversion
/// from an in-memory handle, which a scene cannot save, so this carries the
/// conversion for the editor to write into the project as a `.vfx` and point
/// the entity at. A shipped game leaves it in place, which costs nothing.
/// Not saved and not reflected.
#[derive(Component, Clone, Debug)]
pub struct InlineLegacyEffect {
    pub conversion: Conversion,
}

/// Put on a migrated entity until its effect has loaded, so
/// [`drop_superseded_light`] can tell whether a `PointLight` on it is the old
/// effect's own light. Transient: not saved, not reflected.
///
/// The old runtime put a `PointLight` on the effect entity itself for an
/// effect with a light, and a scene saved while it played kept that light.
/// The conversion turns the old light into the emitter's own light, so
/// keeping the saved one as well would light the scene twice. Hanabi owned
/// that slot outright (its teardown removed any `PointLight` there), so one
/// found on an entity whose converted effect has a light is taken as that.
#[derive(Component)]
pub struct PendingLegacyLight;

/// What an old component becomes.
#[derive(Debug)]
pub struct Migration {
    pub system: ParticleSystem,
    pub playback: ParticlePlayback,
    /// The converted effect, when the old one was inline.
    pub inline: Option<Conversion>,
}

/// Turn an old component into the new pair. Pure, so it is tested without an
/// app.
///
/// An `Asset` source keeps its path, `.particle` and all: [`ParticleFileLoader`]
/// plays it as it is. The editor re-points it at the converted `.vfx` once one
/// exists beside it; the runtime does not look for one itself, because a
/// shipped game may read its assets from an archive where there is no
/// directory to look in, and the `.particle` plays the same either way.
pub fn migrate_effect(effect: &HanabiEffect) -> Migration {
    let playback = ParticlePlayback {
        // The old `playing` switched the spawner off and let live particles
        // die out. Stopped (not emitting) is the nearest state; the difference
        // is only the particles already in the air on the first frame.
        state: if effect.playing { PlayState::Playing } else { PlayState::Stopped },
        ..Default::default()
    };
    match &effect.source {
        EffectSource::Asset { path } => Migration {
            system: ParticleSystem::from_path(path.clone()),
            playback,
            inline: None,
        },
        EffectSource::Inline { definition } => Migration {
            system: ParticleSystem::from_path(String::new()),
            playback,
            inline: Some(convert::convert_definition(definition)),
        },
    }
}

/// Swap an inserted [`HanabiEffect`] for a [`ParticleSystem`].
///
/// An observer rather than a system so the old component is gone before any
/// system runs: nothing has to know it ever existed, and an entity is never
/// seen half migrated.
pub fn migrate_on_insert(
    insert: On<Insert<HanabiEffect>>,
    effects: Query<&HanabiEffect>,
    mut assets: ResMut<Assets<VfxAsset>>,
    mut commands: Commands,
) {
    let entity = insert.entity;
    let Ok(effect) = effects.get(entity) else { return };
    let mut migration = migrate_effect(effect);
    let what = match &effect.source {
        EffectSource::Asset { path } => path.clone(),
        EffectSource::Inline { definition } => format!("inline effect \"{}\"", definition.name),
    };
    let mut entity_commands = commands.entity(entity);
    entity_commands.remove::<HanabiEffect>();
    if let Some(conversion) = migration.inline.take() {
        migration.system.asset = assets.add(conversion.asset.clone());
        entity_commands.insert(InlineLegacyEffect { conversion });
    }
    entity_commands.insert((migration.system, migration.playback, PendingLegacyLight));
    info!("[particles] migrated an old Hanabi effect component to a Particle System ({what})");
}

/// Remove the old effect's own `PointLight` once the converted effect is
/// known to bring its own (see [`PendingLegacyLight`]). Only in builds with the
/// 3D renderer, which are the only ones with point lights.
#[cfg(feature = "render_3d")]
pub fn drop_superseded_light(
    mut commands: Commands,
    assets: Res<Assets<VfxAsset>>,
    server: Res<AssetServer>,
    pending: Query<(Entity, &ParticleSystem, Has<bevy::light::PointLight>), With<PendingLegacyLight>>,
) {
    use crate::asset::LightMode;
    for (entity, system, has_light) in &pending {
        let done = if !has_light {
            true
        } else if let Some(asset) = assets.get(&system.asset) {
            if asset.emitters.iter().any(|e| e.light.mode != LightMode::Off) {
                commands.entity(entity).remove::<bevy::light::PointLight>();
                info!("[particles] removed the old effect's point light; the converted effect lights the scene itself");
            }
            true
        } else {
            // Never resolving (a bad path, a broken file): keep the light and
            // stop waiting.
            server.load_state(&system.asset).is_failed() || (system.path.trim().is_empty() && !assets.contains(&system.asset))
        };
        if done {
            commands.entity(entity).remove::<PendingLegacyLight>();
        }
    }
}

// ── The old effect file ─────────────────────────────────────────────────────

/// Why a `.particle` did not load.
#[derive(Debug)]
pub enum ParticleFileError {
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    Convert(String),
}

impl std::fmt::Display for ParticleFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read the effect: {e}"),
            Self::Utf8(e) => write!(f, "the effect is not UTF-8 text: {e}"),
            Self::Convert(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ParticleFileError {}

impl From<std::io::Error> for ParticleFileError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Parse and convert a `.particle` file's bytes. `name` fills an empty effect
/// name, the way the editor would show it.
pub fn load_particle_bytes(bytes: &[u8], name: &str) -> Result<Conversion, ParticleFileError> {
    let text = std::str::from_utf8(bytes).map_err(ParticleFileError::Utf8)?;
    convert::convert_text(text, name).map_err(ParticleFileError::Convert)
}

/// Bevy loader for `.particle`: the old effect, converted in memory into a
/// [`VfxAsset`]. See the module docs.
#[derive(Default, TypePath)]
pub struct ParticleFileLoader;

impl AssetLoader for ParticleFileLoader {
    type Asset = VfxAsset;
    type Settings = ();
    type Error = ParticleFileError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let path = load_context.path().path();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("effect").to_string();
        let conversion = load_particle_bytes(&bytes, &name)?;
        // One line, not one per warning: the editor's on-open conversion lists
        // them in the Console when it writes the `.vfx`, and a shipped game's
        // log only needs to say why an effect looks different.
        if conversion.warnings.is_empty() {
            debug!("[particles] {} converted in memory from the old format", path.display());
        } else {
            info!(
                "[particles] {} converted in memory from the old format; {} thing(s) did not carry over",
                path.display(),
                conversion.warnings.len()
            );
        }
        Ok(conversion.asset)
    }

    fn extensions(&self) -> &[&str] {
        &[PARTICLE_EXTENSION]
    }
}

/// Register the loader, the old component and its migration.
pub fn plugin(app: &mut App) {
    app.init_asset_loader::<ParticleFileLoader>()
        .register_type::<HanabiEffect>()
        .add_observer(migrate_on_insert);
    #[cfg(feature = "render_3d")]
    app.add_systems(Update, drop_superseded_light);
}
