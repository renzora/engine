//! "Animate" on a node's value field: right-click it to key that pin over the
//! effect's time in the Timeline.
//!
//! Ember's node graph view has no context menu for a pin: a right-click on a
//! port disconnects its wires, and one over a node opens nothing. The value
//! fields, though, are this crate's own widgets ([`crate::pin_editors`]), so
//! each carries an [`AnimatablePin`] and this module watches for a right-click
//! on one. That keeps the gesture where the value is, which is where people
//! look for it, without teaching the shared graph widget about particle
//! tracks.
//!
//! A keyed field is washed in the accent colour, because its inline value is
//! no longer what plays: the track overrides it, and a field that looked
//! untouched would invite edits that do nothing.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_item, screen_menu};
use renzora_particles::asset::NodeId;
use renzora_particles::params::{find_track, new_pin_track, ParamTarget};

use crate::state::VfxEditorState;
use crate::timeline::{focus_timeline, same_path, TimelineLink};

/// A value field for input `pin` of `node` in emitter `emitter`'s graph.
#[derive(Component, Clone, Copy)]
pub struct AnimatablePin {
    pub emitter: usize,
    pub node: NodeId,
    pub pin: &'static str,
}

fn target(pin: &AnimatablePin) -> ParamTarget {
    ParamTarget::Pin { node: pin.node, pin: pin.pin.to_string() }
}

/// Whether the open effect keys `pin`.
pub fn is_keyed(state: &VfxEditorState, pin: &AnimatablePin) -> bool {
    find_track(&state.doc, pin.emitter, &target(pin)).is_some_and(|i| !state.doc.tracks[i].keys.is_empty())
}

/// Right-click (press and release without a drag, so a right-drag still pans
/// the graph) on a value field: Animate, or Show in Timeline and Remove
/// Animation once it is keyed.
#[allow(clippy::too_many_arguments)]
pub fn pin_context_menu(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cells: Query<(&RelativeCursorPosition, &AnimatablePin)>,
    state: Res<VfxEditorState>,
    fonts: Option<Res<EmberFonts>>,
    mut press: Local<Option<Vec2>>,
    mut commands: Commands,
) {
    let cursor = windows.iter().find_map(|w| w.cursor_position());
    if buttons.just_pressed(MouseButton::Right) {
        *press = cursor;
        return;
    }
    if !buttons.just_released(MouseButton::Right) {
        return;
    }
    let (Some(start), Some(at), Some(fonts)) = (press.take(), cursor, fonts) else { return };
    if (at - start).length() > 5.0 {
        return;
    }
    let Some(pin) = cells.iter().find(|(rcp, _)| rcp.cursor_over).map(|(_, p)| *p) else { return };
    if state.load_error.is_some() || state.path.is_none() {
        return;
    }
    let menu = screen_menu(&mut commands, at.x, at.y);
    let items = if is_keyed(&state, &pin) {
        vec![
            menu_item(&mut commands, &fonts, "film-strip", "Show in Timeline", move |w| animate(w, pin)),
            menu_item(&mut commands, &fonts, "x", "Remove Animation", move |w| remove(w, pin)),
        ]
    } else {
        vec![menu_item(&mut commands, &fonts, "diamond", "Animate", move |w| animate(w, pin))]
    };
    commands.entity(menu).add_children(&items);
}

/// Give `pin` a track (one key, at the timeline's playhead when the timeline
/// is on this effect, holding the value the field shows) unless it has one,
/// then bring the Timeline forward with the track selected.
pub fn animate(world: &mut World, pin: AnimatablePin) {
    let Some(path) = world.get_resource::<VfxEditorState>().and_then(|s| s.path.clone()) else { return };
    let time = world
        .get_resource::<TimelineLink>()
        .and_then(|l| l.shown.as_ref())
        .filter(|(p, _)| same_path(p, &path))
        .map(|(_, t)| *t)
        .unwrap_or(0.0);
    let index = {
        let mut st = world.resource_mut::<VfxEditorState>();
        match find_track(&st.doc, pin.emitter, &target(&pin)) {
            Some(i) => {
                if st.doc.tracks[i].keys.is_empty() {
                    if let Some(fresh) = new_pin_track(&st.doc, pin.emitter, pin.node, pin.pin, time) {
                        st.doc.tracks[i].keys = fresh.keys;
                    }
                }
                Some(i)
            }
            None => new_pin_track(&st.doc, pin.emitter, pin.node, pin.pin, time).map(|track| {
                st.doc.tracks.push(track);
                st.doc.tracks.len() - 1
            }),
        }
    };
    let Some(index) = index else { return };
    if let Some(mut link) = world.get_resource_mut::<TimelineLink>() {
        link.focus = Some((path, index));
    }
    focus_timeline(world);
}

/// Drop `pin`'s track: the field's inline value plays again.
fn remove(world: &mut World, pin: AnimatablePin) {
    let t = target(&pin);
    if let Some(mut st) = world.get_resource_mut::<VfxEditorState>() {
        st.doc.tracks.retain(|tr| !(tr.emitter == pin.emitter && tr.target == t));
    }
}
