//! Plugin-contributed rows in the Assets panel's right-click menu.
//!
//! The menu's own rows (Open, Rename, Duplicate, Delete) apply to any file.
//! Some actions only make sense for one kind of file, and belong to the crate
//! that understands it: "Convert to new particle system" on a `.particle` is
//! the particle editor's, and the asset browser has no business linking the
//! converter to offer it. So a crate registers the action against
//! the extensions it applies to, and the browser shows whatever matches.
//!
//! An action is either for one **file** (shown when the right-clicked file has
//! a matching extension) or for a **folder** (shown on a right-clicked folder
//! and on the folder background, when that folder directly holds at least one
//! matching file, so "Convert all .particle files here" never appears over a
//! folder with none).
//!
//! In the contract crate for the reason [`crate::create_menu`] is: the two
//! crates that need the type are the browser and whatever contributes to it,
//! which may be an installed plugin.

use std::path::Path;

use bevy::prelude::*;

/// What an action applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetActionScope {
    /// The right-clicked file.
    File,
    /// A folder: the right-clicked one, or the one the browser is showing.
    Folder,
}

/// One registered row.
pub struct AssetAction {
    /// Stable key; registering the same id again replaces the first.
    pub id: String,
    /// Phosphor icon name.
    pub icon: String,
    /// Translation key and the text to fall back on (most plugins ship no
    /// translations).
    pub label_key: String,
    pub label_fallback: String,
    /// Lowercase extensions without the dot.
    pub extensions: Vec<String>,
    pub scope: AssetActionScope,
    /// Runs with the file (or folder) as an absolute path.
    pub run: fn(&mut World, &Path),
}

impl AssetAction {
    pub fn new(
        id: impl Into<String>,
        scope: AssetActionScope,
        extensions: &[&str],
        run: fn(&mut World, &Path),
    ) -> Self {
        let id = id.into();
        Self {
            label_key: format!("assets.action.{id}"),
            label_fallback: id.clone(),
            id,
            icon: "lightning".to_string(),
            extensions: extensions.iter().map(|e| e.to_ascii_lowercase()).collect(),
            scope,
            run,
        }
    }

    pub fn label(mut self, key: impl Into<String>, fallback: impl Into<String>) -> Self {
        self.label_key = key.into();
        self.label_fallback = fallback.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn label_text(&self) -> String {
        crate::lang::t_or(&self.label_key, &self.label_fallback)
    }

    fn matches_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| self.extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
    }
}

/// Every registered action, in registration order.
#[derive(Resource, Default)]
pub struct AssetActionRegistry(Vec<AssetAction>);

impl AssetActionRegistry {
    pub fn items(&self) -> &[AssetAction] {
        &self.0
    }

    /// The file actions for `path`.
    pub fn for_file<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a AssetAction> + 'a {
        self.0
            .iter()
            .filter(move |a| a.scope == AssetActionScope::File && a.matches_file(path))
    }

    /// The folder actions for `dir`: those with a matching file directly in it.
    /// Reads the folder once, when a menu opens, which is cheap next to
    /// building the menu.
    pub fn for_folder(&self, dir: &Path) -> Vec<&AssetAction> {
        let folder: Vec<&AssetAction> = self.0.iter().filter(|a| a.scope == AssetActionScope::Folder).collect();
        if folder.is_empty() {
            return folder;
        }
        let files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect())
            .unwrap_or_default();
        folder
            .into_iter()
            .filter(|a| files.iter().any(|f| a.matches_file(f)))
            .collect()
    }
}

pub trait RegisterAssetAction {
    /// Add (or replace, by id) a row in the Assets panel's right-click menu.
    fn register_asset_action(&mut self, action: AssetAction) -> &mut Self;
}

impl RegisterAssetAction for App {
    fn register_asset_action(&mut self, action: AssetAction) -> &mut Self {
        let mut registry = self
            .world_mut()
            .get_resource_or_insert_with(AssetActionRegistry::default);
        registry.0.retain(|a| a.id != action.id);
        registry.0.push(action);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop(_: &mut World, _: &Path) {}

    #[test]
    fn file_actions_match_on_extension_case_insensitively() {
        let mut app = App::new();
        app.register_asset_action(AssetAction::new("convert", AssetActionScope::File, &["particle"], noop));
        let reg = app.world().resource::<AssetActionRegistry>();
        assert_eq!(reg.for_file(Path::new("fx/Fire.PARTICLE")).count(), 1);
        assert_eq!(reg.for_file(Path::new("fx/fire.vfx")).count(), 0);
    }

    #[test]
    fn re_registering_an_id_replaces_it() {
        let mut app = App::new();
        app.register_asset_action(AssetAction::new("a", AssetActionScope::File, &["x"], noop).label("k", "One"));
        app.register_asset_action(AssetAction::new("a", AssetActionScope::File, &["x"], noop).label("k", "Two"));
        let reg = app.world().resource::<AssetActionRegistry>();
        assert_eq!(reg.items().len(), 1);
        assert_eq!(reg.items()[0].label_fallback, "Two");
    }

    #[test]
    fn a_folder_action_needs_a_matching_file_in_the_folder() {
        let dir = std::env::temp_dir().join(format!("renzora_asset_actions_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::new();
        app.register_asset_action(AssetAction::new("all", AssetActionScope::Folder, &["particle"], noop));
        let reg = app.world().resource::<AssetActionRegistry>();
        assert!(reg.for_folder(&dir).is_empty());
        std::fs::write(dir.join("a.particle"), "").unwrap();
        assert_eq!(reg.for_folder(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
