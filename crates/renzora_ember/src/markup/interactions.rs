//! Markup interaction → script `on_ui` bridge.
//!
//! The loader (`loader.rs`) attaches bevy_hui's `OnUiPress` / `OnUiEnter` /
//! `OnUiExit` components to entities whose markup uses `on_press="..."` etc.
//! This module owns the bevy_ui-side: it watches `Changed<UiInteraction>` and
//! turns each transition into a [`renzora::UiCallback`] queued for
//! [`renzora::ScriptUiInbox`], which `renzora_scripting` drains every frame
//! and dispatches to every script's `on_ui(name, args, entity)` hook.
//!
//! One kind of name is handled here rather than by scripts: an animator action
//! (`anim:hero.cheer`, see [`AnimAction`]), so a button can drive an animation
//! state machine with no script at all. Everything else routes to scripts.

use bevy::prelude::*;
use bevy_hui::prelude::{OnUiEnter, OnUiExit, OnUiPress};
use renzora_scripting_api::{ScriptAction, ScriptUiInbox, UiCallback};
use std::collections::HashMap;

use crate::UiInteraction;

/// A button event that drives an animator directly, written in the markup as
/// `anim:<entity>.<parameter>` with an optional `=<value>`:
///
/// - `anim:hero.cheer` fires the `cheer` trigger
/// - `anim:door.open=true` / `=false` sets a bool parameter
/// - `anim:hero.speed=2.5` sets a float parameter
///
/// No commas and no brackets, because bevy_hui splits an event attribute on
/// commas: `on_press="anim:hero.cheer, play_sound"` is two handlers, the
/// animator action and a script callback. The first `.` separates entity from
/// parameter, which is safe because the editor turns punctuation in entity
/// names into `_`.
///
/// It sends the same `ScriptAction`s a script's `set_anim_trigger`,
/// `set_anim_bool` and `set_anim_param` send, so the animation crate handles
/// both identically and this crate needs no dependency on it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimAction {
    pub target: String,
    pub param: String,
    pub value: AnimValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimValue {
    Trigger,
    Bool(bool),
    Float(f32),
}

impl AnimAction {
    /// Parse one event name. `None` for anything that is not an animator
    /// action, which then goes to scripts as before.
    pub fn parse(name: &str) -> Option<Self> {
        let rest = name.trim().strip_prefix("anim:")?;
        let (target, rest) = rest.split_once('.')?;
        let (param, value) = match rest.split_once('=') {
            Some((param, value)) => {
                let value = match value.trim() {
                    "true" => AnimValue::Bool(true),
                    "false" => AnimValue::Bool(false),
                    v => AnimValue::Float(v.parse().ok()?),
                };
                (param, value)
            }
            None => (rest, AnimValue::Trigger),
        };
        let (target, param) = (target.trim(), param.trim());
        if target.is_empty() || param.is_empty() {
            return None;
        }
        Some(Self {
            target: target.to_string(),
            param: param.to_string(),
            value,
        })
    }

    /// The action the animation crate's bridge recognises, aimed at `entity`.
    fn to_script_action(&self, entity: Entity) -> ScriptAction {
        let action = match self.value {
            AnimValue::Trigger => ScriptAction::new("trigger_anim", entity),
            AnimValue::Bool(v) => ScriptAction::new("set_anim_bool", entity).arg("value", v),
            AnimValue::Float(v) => ScriptAction::new("set_anim_param", entity).arg("value", v),
        };
        action.arg("name", self.param.as_str())
    }
}

/// Forward each Pressed / Hovered / None interaction transition: animator
/// actions straight to the animator, every other name to scripts via
/// [`ScriptUiInbox`]. Press fires `on_press`, hover fires `on_enter`, leaving
/// hover fires `on_exit`, matching bevy_hui's original semantics.
pub fn forward_ui_interactions(
    interactions: Query<
        (
            Entity,
            &UiInteraction,
            Option<&OnUiPress>,
            Option<&OnUiEnter>,
            Option<&OnUiExit>,
        ),
        Changed<UiInteraction>,
    >,
    // The animator's owner is a scene object, never a UI node, so UI nodes are
    // left out: a button named like its target must not shadow it.
    named: Query<(Entity, &Name), Without<Node>>,
    mut inbox: ResMut<ScriptUiInbox>,
    mut commands: Commands,
) {
    for (entity, interaction, press, enter, exit) in &interactions {
        let names: Option<&Vec<String>> = match interaction {
            UiInteraction::Pressed => press.map(|p| &p.0),
            UiInteraction::Hovered => enter.map(|e| &e.0),
            UiInteraction::None => exit.map(|e| &e.0),
        };
        let Some(names) = names else { continue };
        for name in names {
            if let Some(anim) = AnimAction::parse(name) {
                match named.iter().find(|(_, n)| n.as_str() == anim.target) {
                    Some((target, _)) => commands.trigger(anim.to_script_action(target)),
                    None => warn!(
                        "[ui] `{}`: no entity named `{}` to drive",
                        name.trim(),
                        anim.target
                    ),
                }
                continue;
            }
            // Trimmed: bevy_hui splits the attribute on commas and keeps the
            // spaces, so `on_press="anim:hero.cheer, press_play"` handed scripts
            // " press_play", which an `if name == "press_play"` never matched.
            inbox.pending.push(UiCallback {
                name: name.trim().to_string(),
                args: HashMap::new(),
                entity_bits: entity.to_bits(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Option<(String, String, AnimValue)> {
        AnimAction::parse(s).map(|a| (a.target, a.param, a.value))
    }

    #[test]
    fn a_bare_parameter_is_a_trigger() {
        assert_eq!(parse("anim:hero.cheer"), Some(("hero".into(), "cheer".into(), AnimValue::Trigger)));
    }

    #[test]
    fn values_set_bools_and_floats() {
        assert_eq!(parse("anim:door.open=true"), Some(("door".into(), "open".into(), AnimValue::Bool(true))));
        assert_eq!(parse("anim:door.open=false"), Some(("door".into(), "open".into(), AnimValue::Bool(false))));
        assert_eq!(parse("anim:hero.speed=2.5"), Some(("hero".into(), "speed".into(), AnimValue::Float(2.5))));
    }

    #[test]
    fn the_space_after_a_comma_in_the_attribute_is_ignored() {
        assert_eq!(parse(" anim:hero.cheer"), Some(("hero".into(), "cheer".into(), AnimValue::Trigger)));
    }

    #[test]
    fn script_callback_names_are_left_for_scripts() {
        assert_eq!(parse("press_play"), None);
        assert_eq!(parse("anim_play"), None);
        assert_eq!(parse("anim:hero"), None, "no parameter");
        assert_eq!(parse("anim:.cheer"), None, "no target");
        assert_eq!(parse("anim:hero.speed=fast"), None, "not a number or bool");
    }

    #[derive(Resource, Default)]
    struct Sent(Vec<(String, Entity)>);

    /// A real press on a button whose `on_press` mixes an animator action and
    /// a script name: the action reaches the named scene entity, the script
    /// name still goes to scripts, and a UI node with the same name is not
    /// mistaken for the target.
    #[test]
    fn a_press_drives_the_named_entity_and_still_tells_scripts() {
        let mut app = App::new();
        app.init_resource::<ScriptUiInbox>()
            .init_resource::<Sent>()
            .add_systems(Update, forward_ui_interactions)
            .add_observer(|t: On<ScriptAction>, mut sent: ResMut<Sent>| {
                sent.0.push((t.event().name.clone(), t.event().entity));
            });
        let hero = app.world_mut().spawn(Name::new("hero")).id();
        app.world_mut().spawn((Name::new("hero"), Node::default()));
        let button = app
            .world_mut()
            .spawn((
                UiInteraction::None,
                OnUiPress(vec!["anim:hero.go".into(), " press_run".into()]),
            ))
            .id();
        app.update();
        app.world_mut().resource_mut::<Sent>().0.clear();
        app.world_mut().resource_mut::<ScriptUiInbox>().pending.clear();

        *app.world_mut().get_mut::<UiInteraction>(button).unwrap() = UiInteraction::Pressed;
        app.update();

        assert_eq!(app.world().resource::<Sent>().0, vec![("trigger_anim".to_string(), hero)]);
        let names: Vec<String> = app
            .world()
            .resource::<ScriptUiInbox>()
            .pending
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, vec!["press_run"], "the space after the comma is not part of the name");
    }

    #[test]
    fn each_kind_sends_the_action_the_animation_bridge_handles() {
        let e = Entity::PLACEHOLDER;
        let names = |s: &str| AnimAction::parse(s).unwrap().to_script_action(e).name;
        assert_eq!(names("anim:a.t"), "trigger_anim");
        assert_eq!(names("anim:a.b=true"), "set_anim_bool");
        assert_eq!(names("anim:a.f=1"), "set_anim_param");
        let action = AnimAction::parse("anim:a.speed=2").unwrap().to_script_action(e);
        assert_eq!(action.entity, e);
        assert!(matches!(
            action.args.get("name"),
            Some(renzora_scripting_api::ScriptActionValue::String(s)) if s == "speed"
        ));
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, forward_ui_interactions);
}
