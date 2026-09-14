//! Plugin-extensible "create a new file" menu items.
//!
//! Two menus offer to make a new asset: the Assets panel's **Add** button and
//! right-click menu, and the hierarchy's right-click **Attach**. Both were fixed
//! lists compiled into the editor, which meant the editor decided what file types
//! exist, and a plugin that added one could not say so.
//!
//! That showed up as a menu that lied. The engine offered **Lua Script** whether
//! or not a Lua interpreter was installed, because the entry was a variant of an
//! enum rather than something the interpreter contributed. Picking it produced a
//! `.lua` file that nothing could run, and the engine even shipped the starter
//! text for a language it might have no backend for.
//!
//! So an item is registered rather than enumerated. The Lua plugin contributes
//! its own entry and the starter content that goes with it, and the entry is
//! absent exactly when the plugin is.
//!
//! ```ignore
//! app.register_create_menu_item(
//!     CreateMenuItem::new("lua", "new_script", "lua", starter_lua)
//!         .label("assets.new.lua", "Lua Script")
//!         .icon("code")
//!         .attaches(),
//! );
//! ```
//!
//! # Why it lives in the contract crate
//!
//! The same reason `ScriptBackend` does: the two crates that need this type are
//! the editor and an INSTALLED PLUGIN, and a plugin compiles against the staged
//! SDK, which offers `bevy`, `renzora` and `renzora_ember`. A registry defined in
//! either panel's crate could not be named from a plugin at all.

use bevy::prelude::*;

/// Builds the file's initial contents. The flag is the editor's "include
/// boilerplate" preference: `false` should give something nearly empty.
pub type CreateStarter = Box<dyn Fn(bool) -> String + Send + Sync>;

/// One entry in the create-new menus.
pub struct CreateMenuItem {
    /// Stable key. Registering twice with the same id replaces the first, so a
    /// plugin reloaded in the same session does not stack up duplicates.
    pub id: String,
    /// Filename without the extension, e.g. `new_script`.
    pub stem: String,
    /// Extension without the dot, e.g. `lua`.
    pub extension: String,
    /// Phosphor icon name, shown in the Assets menu.
    pub icon: String,
    /// Translation key and the text to fall back on.
    ///
    /// A key rather than a finished string because the menus are built when they
    /// open and the editor's language can change in between. The fallback is what
    /// a plugin gets when it ships no translations, which is most of them.
    pub label_key: String,
    pub label_fallback: String,
    /// Second line in the Assets menu. Falls back to the label.
    pub subtitle_key: String,
    pub subtitle_fallback: String,
    /// Project-relative folder the hierarchy defaults its destination to, and
    /// pre-creates so the picker has a real row to show even in a project that
    /// has never had one. `scripts` for a language, `materials` for a material.
    pub folder: String,
    /// Whether the hierarchy offers it on an entity.
    ///
    /// Scripts and UI templates attach to something; a material or a scene is a
    /// project-level file with no entity to hang it on.
    pub attaches: bool,
    pub starter: CreateStarter,
}

impl CreateMenuItem {
    /// `id`, the filename it suggests, and what goes in it.
    pub fn new(
        id: impl Into<String>,
        stem: impl Into<String>,
        extension: impl Into<String>,
        starter: impl Fn(bool) -> String + Send + Sync + 'static,
    ) -> Self {
        let id = id.into();
        Self {
            label_key: format!("assets.new.{id}"),
            label_fallback: id.clone(),
            subtitle_key: format!("assets.new.{id}_sub"),
            subtitle_fallback: String::new(),
            id,
            stem: stem.into(),
            extension: extension.into(),
            icon: "file".to_string(),
            folder: "assets".to_string(),
            attaches: false,
            starter: Box::new(starter),
        }
    }

    pub fn label(mut self, key: impl Into<String>, fallback: impl Into<String>) -> Self {
        self.label_key = key.into();
        self.label_fallback = fallback.into();
        self
    }

    pub fn subtitle(mut self, key: impl Into<String>, fallback: impl Into<String>) -> Self {
        self.subtitle_key = key.into();
        self.subtitle_fallback = fallback.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// Where the hierarchy's Attach overlay points by default.
    pub fn folder(mut self, folder: impl Into<String>) -> Self {
        self.folder = folder.into();
        self
    }

    /// Offer this in the hierarchy's Attach menu as well as the Assets panel.
    pub fn attaches(mut self) -> Self {
        self.attaches = true;
        self
    }

    /// The label, resolved now.
    pub fn label_text(&self) -> String {
        crate::lang::t_or(&self.label_key, &self.label_fallback)
    }

    /// The subtitle, resolved now, falling back to the label rather than to
    /// nothing: an entry with a blank second line reads as a rendering fault.
    pub fn subtitle_text(&self) -> String {
        if self.subtitle_fallback.is_empty() {
            crate::lang::t_or(&self.subtitle_key, &self.label_text())
        } else {
            crate::lang::t_or(&self.subtitle_key, &self.subtitle_fallback)
        }
    }

    /// `new_script.lua`.
    pub fn filename(&self) -> String {
        format!("{}.{}", self.stem, self.extension)
    }
}

/// Everything registered, in registration order.
#[derive(Resource, Default)]
pub struct CreateMenuRegistry(Vec<CreateMenuItem>);

impl CreateMenuRegistry {
    pub fn items(&self) -> &[CreateMenuItem] {
        &self.0
    }

    /// Only the entries the hierarchy offers on an entity.
    pub fn attachable(&self) -> impl Iterator<Item = &CreateMenuItem> {
        self.0.iter().filter(|i| i.attaches)
    }

    pub fn get(&self, id: &str) -> Option<&CreateMenuItem> {
        self.0.iter().find(|i| i.id == id)
    }
}

pub trait RegisterCreateMenuItem {
    /// Add (or replace, by id) an entry in the create-new menus.
    fn register_create_menu_item(&mut self, item: CreateMenuItem) -> &mut Self;
}

impl RegisterCreateMenuItem for App {
    fn register_create_menu_item(&mut self, item: CreateMenuItem) -> &mut Self {
        let mut registry = self
            .world_mut()
            .get_resource_or_insert_with(CreateMenuRegistry::default);
        registry.0.retain(|i| i.id != item.id);
        registry.0.push(item);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> CreateMenuItem {
        CreateMenuItem::new(id, "new_thing", "thing", |_| String::new())
    }

    /// Registering the same id twice replaces rather than stacks, so a plugin
    /// that registers on every load does not fill the menu with duplicates.
    #[test]
    fn re_registering_an_id_replaces_it() {
        let mut app = App::new();
        app.register_create_menu_item(item("lua").label("k", "First"));
        app.register_create_menu_item(item("lua").label("k", "Second"));
        let reg = app.world().resource::<CreateMenuRegistry>();
        assert_eq!(reg.items().len(), 1);
        assert_eq!(reg.get("lua").unwrap().label_fallback, "Second");
    }

    /// The hierarchy asks for a subset. A material has no entity to attach to.
    #[test]
    fn only_attachable_items_reach_the_hierarchy() {
        let mut app = App::new();
        app.register_create_menu_item(item("lua").attaches());
        app.register_create_menu_item(item("material"));
        let reg = app.world().resource::<CreateMenuRegistry>();
        let ids: Vec<&str> = reg.attachable().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["lua"]);
    }

    /// The hierarchy pre-creates this folder, so a default that pointed at the
    /// project root would litter it. `assets` is the one directory every project
    /// already has.
    #[test]
    fn the_default_folder_is_assets() {
        assert_eq!(item("lua").folder, "assets");
        assert_eq!(item("lua").folder("scripts").folder, "scripts");
    }

    #[test]
    fn filename_is_stem_and_extension() {
        assert_eq!(item("lua").filename(), "new_thing.thing");
    }

    /// A blank subtitle falls back to the label. An entry with an empty second
    /// line reads as a rendering fault rather than as "no subtitle".
    #[test]
    fn a_missing_subtitle_falls_back_to_the_label() {
        let i = item("lua").label("nonexistent.key", "Lua Script");
        assert_eq!(i.subtitle_text(), "Lua Script");
    }
}
