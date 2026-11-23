//! Plugin-contributed rows in the Hierarchy's right-click menu.
//!
//! The menu's own rows (Rename, Duplicate, Unparent, Group) apply to any
//! entity. Some actions belong to a crate the Hierarchy does not link:
//! "Partition into cells" is world partition's, and the Hierarchy has no
//! business depending on the scene crate to offer it. So a crate registers the
//! action with a test for which entities it applies to, and the menu shows
//! whatever passes.
//!
//! In the contract crate for the reason [`crate::asset_actions`] is: the two
//! crates that need the type are the Hierarchy and whatever contributes to it,
//! which may be an installed plugin.

use bevy::prelude::*;

/// One registered row.
pub struct EntityAction {
    /// Stable key; registering the same id again replaces the first.
    pub id: String,
    /// Phosphor icon name.
    pub icon: String,
    /// Translation key and the text to fall back on (most plugins ship no
    /// translations).
    pub label_key: String,
    pub label_fallback: String,
    /// Whether to offer the action for the right-clicked entity. The
    /// selection (which the action usually works on) is the editor's
    /// `EditorSelection` resource.
    pub applies: fn(&World, Entity) -> bool,
    /// Runs when picked, with the right-clicked entity. The selection is in
    /// `EditorSelection`, so an action can act on that row alone or, as
    /// Delete does, on the whole selection when the row is part of it.
    pub run: fn(&mut World, Entity),
}

impl EntityAction {
    pub fn new(id: impl Into<String>, run: fn(&mut World, Entity)) -> Self {
        let id = id.into();
        Self {
            label_key: format!("hierarchy.action.{id}"),
            label_fallback: id.clone(),
            id,
            icon: "lightning".to_string(),
            applies: |_, _| true,
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

    pub fn applies_to(mut self, applies: fn(&World, Entity) -> bool) -> Self {
        self.applies = applies;
        self
    }

    pub fn label_text(&self) -> String {
        renzora_lang_api::t_or(&self.label_key, &self.label_fallback)
    }
}

/// Every registered action, in registration order.
#[derive(Resource, Default)]
pub struct EntityActionRegistry(Vec<EntityAction>);

impl EntityActionRegistry {
    pub fn items(&self) -> &[EntityAction] {
        &self.0
    }

    /// The actions offered for `entity`.
    pub fn for_entity<'a>(&'a self, world: &'a World, entity: Entity) -> impl Iterator<Item = &'a EntityAction> + 'a {
        self.0.iter().filter(move |a| (a.applies)(world, entity))
    }
}

pub trait RegisterEntityAction {
    /// Add (or replace, by id) a row in the Hierarchy's right-click menu.
    fn register_entity_action(&mut self, action: EntityAction) -> &mut Self;
}

impl RegisterEntityAction for App {
    fn register_entity_action(&mut self, action: EntityAction) -> &mut Self {
        let mut registry = self
            .world_mut()
            .get_resource_or_insert_with(EntityActionRegistry::default);
        registry.0.retain(|a| a.id != action.id);
        registry.0.push(action);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop(_: &mut World, _: Entity) {}

    #[derive(Component)]
    struct Marked;

    #[test]
    fn an_action_is_offered_only_where_it_applies() {
        let mut app = App::new();
        app.register_entity_action(
            EntityAction::new("marked_only", noop).applies_to(|w, e| w.get::<Marked>(e).is_some()),
        );
        let marked = app.world_mut().spawn(Marked).id();
        let plain = app.world_mut().spawn_empty().id();
        let world = app.world();
        let reg = world.resource::<EntityActionRegistry>();
        assert_eq!(reg.for_entity(world, marked).count(), 1);
        assert_eq!(reg.for_entity(world, plain).count(), 0);
    }

    #[test]
    fn re_registering_an_id_replaces_it() {
        let mut app = App::new();
        app.register_entity_action(EntityAction::new("a", noop).label("k", "One"));
        app.register_entity_action(EntityAction::new("a", noop).label("k", "Two"));
        let reg = app.world().resource::<EntityActionRegistry>();
        assert_eq!(reg.items().len(), 1);
        assert_eq!(reg.items()[0].label_fallback, "Two");
    }
}
