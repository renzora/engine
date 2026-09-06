//! The base-colour row: what an untextured primitive is coloured by.
//!
//! # Why this lives in the Material drawer
//!
//! `MeshColor` is not a material, and a primitive that has one has no
//! `.material` at all — the engine builds it a `StandardMaterial` from the
//! blockout grid tinted by that colour. But "what colour is this thing" is the
//! question the material slot has just answered with *No material*, and sending
//! someone to a separate component for the answer would be sending them away
//! from the place they correctly looked first.
//!
//! It is also why the row disappears the moment a real material is assigned:
//! from then on the material owns the surface, and a tint control that no
//! longer tinted anything would be worse than no control.

use bevy::prelude::*;

use renzora::core::MeshColor;
use renzora_ember::font::EmberFonts;
use renzora_ember::inspector::{color_field_rgba, inspector_row};
use renzora_ember::reactive::Rx;

/// The colour an entity shows when it has neither a `MeshColor` nor a material.
///
/// Matches `rehydrate_meshes`, which falls back to white for a primitive with no
/// stored colour. Reading a different default here would make the swatch
/// disagree with the object beside it.
const UNSET: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

pub(super) fn build_tint_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
) -> Entity {
    let field = color_field_rgba(
        commands,
        move |w: &Rx| {
            w.get::<MeshColor>(entity)
                .map(|c| {
                    let c = c.0.to_srgba();
                    [c.red, c.green, c.blue, c.alpha]
                })
                .unwrap_or(UNSET)
        },
        move |w: &mut World, rgba: [f32; 4]| {
            let colour = Color::srgba(rgba[0], rgba[1], rgba[2], rgba[3]);
            // Insert rather than assume: a primitive spawned before this existed,
            // or one whose colour was never set, has no `MeshColor` to write to,
            // and refusing to colour it would make the row look broken on exactly
            // the objects most likely to need it.
            match w.get_mut::<MeshColor>(entity) {
                Some(mut existing) => existing.0 = colour,
                None => {
                    if let Ok(mut e) = w.get_entity_mut(entity) {
                        e.insert(MeshColor(colour));
                    }
                }
            }
        },
    );
    inspector_row(commands, &fonts.ui, "Base Color", field)
}
