//! Markup interaction → script `on_ui` bridge.
//!
//! The loader (`loader.rs`) attaches bevy_hui's `OnUiPress` / `OnUiEnter` /
//! `OnUiExit` components to entities whose markup uses `on_press="..."` etc.
//! This module owns the bevy_ui-side: it watches `Changed<Interaction>` and
//! turns each transition into a [`renzora::UiCallback`] queued for
//! [`renzora::ScriptUiInbox`], which `renzora_scripting` drains every frame
//! and dispatches to every script's `on_ui(name, args, entity)` hook.
//!
//! No Rust-side function registry yet — every `on_press` name routes to
//! scripts. If/when we want Rust handlers, the dispatch site is right here.

use bevy::prelude::*;
use bevy_hui::prelude::{OnUiEnter, OnUiExit, OnUiPress};
use renzora::{ScriptUiInbox, UiCallback};
use std::collections::HashMap;

/// Forward each Pressed / Hovered / None interaction transition to scripts
/// via [`ScriptUiInbox`]. Press fires `on_press`, hover fires `on_enter`,
/// leaving hover fires `on_exit` — matching bevy_hui's original semantics.
pub fn forward_ui_interactions(
    interactions: Query<
        (
            Entity,
            &Interaction,
            Option<&OnUiPress>,
            Option<&OnUiEnter>,
            Option<&OnUiExit>,
        ),
        Changed<Interaction>,
    >,
    mut inbox: ResMut<ScriptUiInbox>,
) {
    for (entity, interaction, press, enter, exit) in &interactions {
        let names: Option<&Vec<String>> = match interaction {
            Interaction::Pressed => press.map(|p| &p.0),
            Interaction::Hovered => enter.map(|e| &e.0),
            Interaction::None => exit.map(|e| &e.0),
        };
        let Some(names) = names else { continue };
        for name in names {
            inbox.pending.push(UiCallback {
                name: name.clone(),
                args: HashMap::new(),
                entity_bits: entity.to_bits(),
            });
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, forward_ui_interactions);
}
