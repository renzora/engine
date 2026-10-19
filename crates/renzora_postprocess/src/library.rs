//! Which effect shaders are loaded, and what each one declares.
//!
//! Keyed by the asset path a [`renzora::PostEffect`] names, so every list that
//! uses the same file shares one load and one parse. The parse is redone when
//! the shader asset changes, which is what makes editing an effect's sliders in
//! the code editor show up in the inspector without a restart.

use bevy::asset::LoadState;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora::PostEffects;

use crate::effect_file::{self, EffectInfo};

#[derive(Resource, Default)]
pub struct EffectLibrary {
    entries: HashMap<String, Entry>,
}

struct Entry {
    handle: Handle<Shader>,
    status: EffectStatus,
}

/// Where one effect file stands.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectStatus {
    Loading,
    Ready(EffectInfo),
    /// The file did not load, or loaded and is not a usable effect. The text
    /// is what the inspector shows.
    Failed(String),
}

impl EffectLibrary {
    /// The handle and parsed settings of an effect that is ready to draw.
    pub fn ready(&self, path: &str) -> Option<(&Handle<Shader>, &EffectInfo)> {
        let entry = self.entries.get(path)?;
        match &entry.status {
            EffectStatus::Ready(info) => Some((&entry.handle, info)),
            _ => None,
        }
    }

    /// The status of `path`, or `None` when nothing has asked for it yet.
    pub fn status(&self, path: &str) -> Option<&EffectStatus> {
        self.entries.get(path).map(|e| &e.status)
    }

    /// A content hash of every status, so an inspector can rebuild when a
    /// shader it shows finishes loading or changes its fields.
    pub fn signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut keys: Vec<(&String, String)> = self
            .entries
            .iter()
            .map(|(k, e)| (k, format!("{:?}", e.status)))
            .collect();
        keys.sort();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        keys.hash(&mut h);
        h.finish()
    }
}

/// Start loading every shader a list names that is not loaded yet.
pub(crate) fn request_shaders(
    lists: Query<&PostEffects, Changed<PostEffects>>,
    mut library: ResMut<EffectLibrary>,
    asset_server: Res<AssetServer>,
) {
    for list in &lists {
        for effect in &list.effects {
            if effect.shader.is_empty() {
                continue;
            }
            if let Some(entry) = library.entries.get_mut(&effect.shader) {
                // A file that was missing may have been installed since: a
                // scene that names it reloading is the moment to look again.
                if matches!(asset_server.load_state(&entry.handle), LoadState::Failed(_)) {
                    asset_server.reload(effect.shader.clone());
                    entry.status = EffectStatus::Loading;
                }
                continue;
            }
            let handle = asset_server.load::<Shader>(effect.shader.clone());
            library.entries.insert(
                effect.shader.clone(),
                Entry {
                    handle,
                    status: EffectStatus::Loading,
                },
            );
        }
    }
}

/// Parse shaders that have arrived or changed.
///
/// Polls the entries still loading rather than relying on the first
/// `AssetEvent`, because a shader something else already loaded never sends
/// one to a handle taken afterwards.
pub(crate) fn read_effects(
    mut library: ResMut<EffectLibrary>,
    mut events: MessageReader<AssetEvent<Shader>>,
    shaders: Res<Assets<Shader>>,
    asset_server: Res<AssetServer>,
) {
    for event in events.read() {
        if let AssetEvent::Modified { id } = event {
            for entry in library.entries.values_mut() {
                if entry.handle.id() == *id {
                    entry.status = EffectStatus::Loading;
                }
            }
        }
    }

    for (path, entry) in library.entries.iter_mut() {
        if entry.status != EffectStatus::Loading {
            continue;
        }
        if let Some(shader) = shaders.get(&entry.handle) {
            let stem = std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(path);
            entry.status = match effect_file::parse(shader.source.as_str(), &effect_file::title_case(stem)) {
                Ok(info) => EffectStatus::Ready(info),
                Err(e) => {
                    renzora::console_log::console_warn("Post Process", format!("{path}: {e}"));
                    EffectStatus::Failed(e)
                }
            };
        } else if matches!(asset_server.load_state(&entry.handle), LoadState::Failed(_)) {
            let message = format!("could not load {path}");
            renzora::console_log::console_warn("Post Process", message.clone());
            entry.status = EffectStatus::Failed(message);
        }
    }
}
