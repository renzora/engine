//! Pointer state for UI nodes: hovered, pressed, or neither, from Bevy's
//! picking.
//!
//! Bevy 0.20 deprecated `bevy_ui::Interaction`, and with it the focus system
//! that computed it, in favour of picking: the `Hovered` component and the
//! `Pressed` marker. Ember moved over whole. This module is what the rest of
//! ember and the editor build on:
//!
//! - [`UiInteraction`] is the one state a widget reads: `None`, `Hovered` or
//!   `Pressed`. It is derived each frame from `Hovered` and `Pressed`, so
//!   `Changed<UiInteraction>` still says "the pointer state of this widget
//!   moved", which `Hovered` and `Pressed` cannot say on their own (a removed
//!   marker is not a change a query filter can see).
//! - A widget opts in by carrying `UiInteraction`, which brings `Hovered`.
//!
//! # Hovered
//!
//! `Hovered` is CSS `:hover`: true while the pointer is over the node *or any
//! of its descendants*. So a row stays hovered while the pointer is on a
//! button inside it, which the old focus system did not do (the button
//! blocked the row).
//!
//! # Pressed
//!
//! Bevy only inserts `Pressed` for its own `ui_widgets::Button`, so ember does
//! it for every interactive node: a primary-button `PointerPress` on a node
//! with a `UiInteraction` marks it pressed. Only a direct hit counts, not one
//! bubbled up from a child: picking delivers the press to every node under
//! the pointer down to the first that blocks, which is exactly the set the
//! old focus system pressed, and a child that blocks its parent keeps the
//! press to itself as it always did. The press is cleared at the end of the
//! frame the button is released (in `Last`, never earlier), so a click that
//! goes down and up between two frames is still seen by every `Update` system
//! once, as it was before.
//!
//! # Blocking: `Pickable`, not `FocusPolicy`
//!
//! Picking ignores `FocusPolicy`. Left to Bevy, a node without a `Pickable`
//! blocks everything under it, which is the opposite of the old default
//! (every node carried `FocusPolicy::Pass` unless told to block), and the
//! whole editor is laid out on that default: labels over the viewport,
//! decorative layers, drag ghosts and their labels. So this plugin makes every
//! UI node, and every text span, start out as [`PASS_THROUGH`]; a node that
//! should stop the pointer says so with [`BLOCK`], just as it used to with
//! `FocusPolicy::Block`. `Pickable::IGNORE` takes a node out of picking
//! altogether.
//!
//! The default changes one thing that is not UI: a game's UI panel no longer
//! stops a click from reaching a 3D object behind it unless it blocks.
//!
//! # Writing the state directly
//!
//! A system may set `UiInteraction::Pressed` itself to press a button from
//! the keyboard or a gamepad (Enter submitting a form). The write stands for
//! the rest of that frame and is replaced by the pointer's state at the start
//! of the next, exactly as with the old `Interaction`.

use bevy::picking::events::PointerPress;
use bevy::picking::hover::{Hovered, PickingInteraction};
use bevy::picking::pointer::PointerButton;
use bevy::picking::{Pickable, PickingSystems};
use bevy::prelude::*;
use bevy::ui::{InteractionDisabled, Pressed, UiSystems};

/// A UI node's pointer state. See the module docs.
#[derive(Component, Copy, Clone, Eq, PartialEq, Debug, Default, Reflect)]
#[reflect(Component, Default, PartialEq, Debug, Clone)]
#[require(Hovered)]
pub enum UiInteraction {
    /// The primary button went down on this node and has not been released.
    Pressed,
    /// The pointer is over this node or one of its descendants.
    Hovered,
    /// Neither.
    #[default]
    None,
}

/// Hit-testable, but lets whatever is underneath be hit too: what the old
/// `FocusPolicy::Pass` meant. For a node that should react to the pointer
/// itself and still not block, or a label that should not hide its
/// siblings.
pub const PASS_THROUGH: Pickable = Pickable { should_block_lower: false, is_hoverable: true };

/// Hit-testable and blocks what is underneath: what the old
/// `FocusPolicy::Block` meant. UI nodes start as [`PASS_THROUGH`] (see the
/// module docs), so a node that should stop the pointer carries this.
pub const BLOCK: Pickable = Pickable { should_block_lower: true, is_hoverable: true };

/// The set the state is derived in. Inside `UiSystems::Focus`, so a system
/// ordered after that set (as many were, after the old focus system) reads
/// the new state without being told about this one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiInteractionSystems;

pub struct UiInteractionPlugin;

impl Plugin for UiInteractionPlugin {
    fn build(&self, app: &mut App) {
        // The old default, pass-through, for every UI node and text span; see
        // the module docs. Bevy only accepts this before the first node is
        // spawned, which is why it is here, at plugin build.
        let world = app.world_mut();
        for result in [
            world.try_register_required_components_with::<Node, Pickable>(|| PASS_THROUGH),
            world.try_register_required_components_with::<bevy::text::TextSpan, Pickable>(|| PASS_THROUGH),
        ] {
            if let Err(e) = result {
                warn!("UI nodes keep Bevy's blocking picking default: {e}");
            }
        }

        app.register_type::<UiInteraction>()
            .add_observer(press_on_pointer_press)
            .configure_sets(
                PreUpdate,
                UiInteractionSystems.in_set(UiSystems::Focus).after(PickingSystems::Last),
            )
            .add_systems(PreUpdate, derive_ui_interaction.in_set(UiInteractionSystems))
            .add_systems(Last, release_presses);
    }
}

/// Mark an interactive node pressed when a primary press hits it directly.
///
/// `PointerPress` fires on every entity in the hover map (everything under
/// the pointer down to the first blocker) and then bubbles up each one's
/// parents. Only the direct hits are pressed; the bubbled copies are left
/// alone (and left to propagate, for other observers), so a button that
/// blocks its row presses the button and not the row.
fn press_on_pointer_press(
    press: On<PointerPress>,
    interactive: Query<Has<InteractionDisabled>, With<UiInteraction>>,
    mut commands: Commands,
) {
    if press.button != PointerButton::Primary || press.original_event_target() != press.entity {
        return;
    }
    if let Ok(false) = interactive.get(press.entity) {
        commands.entity(press.entity).try_insert(Pressed);
    }
}

/// Recompute every [`UiInteraction`] from `Hovered` and `Pressed`.
///
/// Every frame rather than on change, because a system may have written the
/// state directly last frame (see the module docs) and that has to be undone.
/// `set_if_neq`, so an unchanged state never reads as changed.
///
/// Hover is `Hovered` *or* `PickingInteraction`. `Hovered` follows the mouse
/// only; `PickingInteraction` (which Bevy keeps on every directly hit entity)
/// covers every pointer, which is how a world-space UI panel, driven by a
/// game's own ray pointers, still shows hover.
fn derive_ui_interaction(
    mut nodes: Query<(
        &mut UiInteraction,
        &Hovered,
        Option<&PickingInteraction>,
        Has<Pressed>,
        Option<&InheritedVisibility>,
    )>,
) {
    for (mut state, hovered, picking, pressed, visibility) in &mut nodes {
        let hovered_by_any = hovered.get()
            || picking.is_some_and(|p| *p != PickingInteraction::None);
        let next = if visibility.is_some_and(|v| !v.get()) {
            // A hidden node is not interactable, and must not stay stuck
            // pressed or hovered from before it was hidden.
            UiInteraction::None
        } else if pressed {
            UiInteraction::Pressed
        } else if hovered_by_any {
            UiInteraction::Hovered
        } else {
            UiInteraction::None
        };
        state.set_if_neq(next);
    }
}

/// Clear every press once the primary button is up.
///
/// In `Last`, after everything in `Update` has seen the press, and judged by
/// the button's state rather than the release event: the release may happen
/// over a different node, or outside the window, and the press has to end
/// either way.
fn release_presses(
    mouse: Res<ButtonInput<MouseButton>>,
    pressed: Query<Entity, (With<Pressed>, With<UiInteraction>)>,
    mut commands: Commands,
) {
    if mouse.pressed(MouseButton::Left) {
        return;
    }
    for entity in &pressed {
        commands.entity(entity).try_remove::<Pressed>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .configure_sets(PreUpdate, (UiSystems::Focus, PickingSystems::Last))
            .add_plugins(UiInteractionPlugin);
        app
    }

    #[test]
    fn the_state_follows_hovered_and_pressed() {
        let mut app = app();
        let e = app.world_mut().spawn(UiInteraction::default()).id();
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::None);

        app.world_mut().entity_mut(e).insert(Hovered(true));
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::Hovered);

        // Held down: the press survives the frame.
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.world_mut().entity_mut(e).insert(Pressed);
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::Pressed);
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::Pressed);

        // Released: cleared at the end of the frame, hovered again the next.
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
        app.update();
        assert!(app.world().get::<Pressed>(e).is_none());
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::Hovered);
    }

    /// A press that goes down and up between two frames is still seen once.
    #[test]
    fn a_click_inside_one_frame_is_still_a_press() {
        let mut app = app();
        let e = app.world_mut().spawn((UiInteraction::default(), Hovered(true))).id();
        app.update();
        // Mouse already up again, as if pressed and released within the frame.
        app.world_mut().entity_mut(e).insert(Pressed);
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::Pressed);
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::Hovered);
    }

    /// A direct write stands for a frame and is then replaced, as the old
    /// `Interaction` was.
    #[test]
    fn a_written_state_is_replaced_next_frame() {
        let mut app = app();
        let e = app.world_mut().spawn(UiInteraction::default()).id();
        app.update();
        *app.world_mut().get_mut::<UiInteraction>(e).unwrap() = UiInteraction::Pressed;
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::None);
    }

    /// The old default: a node lets the pointer through unless it says
    /// otherwise, and saying otherwise wins.
    #[test]
    fn nodes_pass_through_unless_told_to_block() {
        let mut app = app();
        let plain = app.world_mut().spawn(Node::default()).id();
        let blocking = app.world_mut().spawn((Node::default(), BLOCK)).id();
        let blocking_first = app.world_mut().spawn((BLOCK, Node::default())).id();
        let blocks = |app: &App, e| app.world().get::<Pickable>(e).unwrap().should_block_lower;
        assert!(!blocks(&app, plain));
        assert!(blocks(&app, blocking));
        assert!(blocks(&app, blocking_first));
    }

    #[cfg(feature = "game_ui")]
    #[test]
    fn a_game_ui_button_blocks_whatever_the_bundle_order() {
        use crate::game_ui::components::UiButton;
        let mut app = app();
        let a = app.world_mut().spawn((Node::default(), UiButton)).id();
        let b = app.world_mut().spawn((UiButton, Node::default())).id();
        app.world_mut().flush();
        for e in [a, b] {
            assert!(app.world().get::<Pickable>(e).unwrap().should_block_lower);
        }
    }

    #[test]
    fn a_hidden_node_is_not_interactive() {
        let mut app = app();
        let e = app
            .world_mut()
            .spawn((UiInteraction::default(), Hovered(true), InheritedVisibility::HIDDEN))
            .id();
        app.update();
        assert_eq!(*app.world().get::<UiInteraction>(e).unwrap(), UiInteraction::None);
    }
}
