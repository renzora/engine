//! The "a resize gesture is in flight" flag, shared by every panel that has to
//! ignore the press that started it.
//!
//! A resize handle is always **bigger than the seam it sizes** so it stays
//! grabbable — a dock divider's line is 1px but its grab strip is 11px, the
//! bottom panel's grip band straddles its own top edge, a window's edge zones
//! lie over whatever is docked against the perimeter. That overhang is the
//! point, and it is also why a press meant for a handle keeps landing on the
//! content underneath it: any consumer that decides "was this press mine?"
//! **geometrically** — `RelativeCursorPosition::cursor_over`, or a
//! cursor-inside-my-rect test — is pure geometry and knows nothing about what
//! is drawn on top. It is what made dragging the hierarchy panel's edge divider
//! sweep-select the rows the drag passed (GH #81), and what made dragging the
//! global bottom panel upward arm the viewport's selection box: the grip
//! overhangs the viewport by half its height, so the viewport read the press as
//! landing in the scene.
//!
//! The flag lives here, in the contract crate, rather than beside the handles
//! that raise it, because its readers span crates that share nothing else —
//! `renzora_ember` raises it, and `renzora_gizmo` (which links no UI crate) is
//! one of the consumers that must obey it.
//!
//! [`ResizeBusy`] is raised by `renzora_ember::resize`, which owns the
//! `ResizeHandle` marker and the `PreUpdate` refresh; see that module for how
//! a handle also blocks the focus walk so `Interaction`-based consumers never
//! see the press at all.

use bevy::prelude::*;

/// True from the moment the left button goes down on a resize handle until it
/// is released — the whole gesture, not just the press frame.
///
/// Anything that acts on a left press it resolved geometrically (clear the
/// selection, arm a rubber band, start a drag) must skip that press while this
/// is set; see the module docs for why `cursor_over` can't tell a handle press
/// apart from a press on the content the handle overhangs.
///
/// Refreshed in `PreUpdate` after the pointer state settles, so every `Update`
/// reader sees the correct value on the press frame itself whatever order the
/// systems run in. That ordering is the reason consumers read *this* rather
/// than some downstream "is the viewport hovered" mirror recomputed in
/// `Update`: a mirror can be a frame late, and the frame it is late for is the
/// only one that matters.
#[derive(Resource, Default)]
pub struct ResizeBusy(pub bool);

impl ResizeBusy {
    /// Whether a resize gesture is currently in flight.
    pub fn active(&self) -> bool {
        self.0
    }
}

/// Read the flag from an optional resource — the shape consumers outside the
/// editor's UI crates use, since the resource only exists once
/// `renzora_ember`'s plugin has registered it.
pub fn resize_in_flight(busy: &Option<Res<ResizeBusy>>) -> bool {
    busy.as_ref().is_some_and(|b| b.active())
}

/// Run condition: skip the whole system while a resize gesture is in flight.
///
/// The alternative to [`resize_in_flight`] for systems that do nothing but act
/// on a press — and for the ones already sitting on Bevy's 16-parameter limit,
/// where one more `Res` would stop them being systems at all.
pub fn not_resizing(busy: Option<Res<ResizeBusy>>) -> bool {
    !busy.is_some_and(|b| b.active())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn run(flag: Option<bool>) -> (bool, bool) {
        let mut world = World::new();
        if let Some(v) = flag {
            world.insert_resource(ResizeBusy(v));
        }
        let guarded = world.run_system_once(not_resizing).unwrap();
        // `resize_in_flight` takes the same optional resource the run condition
        // does, so exercise it through a system too.
        let in_flight = world
            .run_system_once(|busy: Option<Res<ResizeBusy>>| resize_in_flight(&busy))
            .unwrap();
        (guarded, in_flight)
    }

    /// The polarity that matters most: with no resource — a runtime with no
    /// editor UI — the guard must let presses through. Inverting it would
    /// disable every guarded gesture everywhere, silently.
    #[test]
    fn absent_resource_reads_as_not_resizing() {
        assert_eq!(run(None), (true, false));
    }

    #[test]
    fn tracks_the_flag_when_present() {
        assert_eq!(run(Some(false)), (true, false));
        assert_eq!(run(Some(true)), (false, true));
    }
}
