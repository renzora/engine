//! Drive UI focus and activation from a gamepad.
//!
//! A game that ships to a controller has to be playable without a cursor, and
//! nothing in bevy_ui moves a selection around for you — `Interaction` is
//! written from the pointer and from nothing else, so on a pad every button in
//! every menu is dead. This module supplies the missing half: a focused entity,
//! directional movement between focusable nodes, and a button that activates
//! the focused one.
//!
//! # It works through `Interaction`, not around it
//!
//! Focus is published by writing `Interaction::Hovered` on the focused node,
//! and activation by writing `Interaction::Pressed` for a single frame. That is
//! deliberate. Every existing consumer — the `on_press` bridge in
//! [`super::interactions`], hover styling, `interaction_style_system` — already
//! watches `Interaction` and needs no knowledge that a pad exists. A parallel
//! "gamepad focus" channel would have meant teaching each of them a second way
//! to be triggered, and any that were missed would work with a mouse and be
//! dead on a pad, which is exactly the bug this module exists to remove.
//!
//! Bevy's own `ui_focus_system` runs in `PreUpdate` and these systems run in
//! `Update`, so on a frame where both write, the pad's value is the one that
//! survives.
//!
//! # Handing control back to the mouse
//!
//! Focus is only asserted while the pad is the thing being used. Moving the
//! mouse drops it, so a player who reaches for the mouse mid-menu doesn't fight
//! a stuck highlight, and pressing a direction takes it back.

use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

/// How far the stick must be pushed before it counts as a direction. Below
/// this a resting stick's drift would walk the selection on its own.
const STICK_DEAD_ZONE: f32 = 0.5;

/// Seconds before a held direction starts repeating, and between repeats after
/// that. Without the first delay a single press skips two or three items;
/// without the second, navigating a long list means tapping.
const REPEAT_DELAY: f32 = 0.40;
const REPEAT_RATE: f32 = 0.12;

/// A step off-axis costs this much more than a step along it when scoring
/// candidates. Pure axis distance picks whatever is nearest the pointer's line
/// even when it is far to the side; weighting the perpendicular offset keeps a
/// press of "down" inside the column the player is looking at.
const OFF_AXIS_PENALTY: f32 = 2.0;

/// Which way a navigation step went.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

impl NavDir {
    /// Unit vector in UI space, where y grows **downward**.
    fn as_vec(self) -> Vec2 {
        match self {
            NavDir::Up => Vec2::new(0.0, -1.0),
            NavDir::Down => Vec2::new(0.0, 1.0),
            NavDir::Left => Vec2::new(-1.0, 0.0),
            NavDir::Right => Vec2::new(1.0, 0.0),
        }
    }
}

/// Gamepad-driven UI focus.
#[derive(Resource, Debug)]
pub struct UiGamepadNav {
    /// The focused node, while a pad is driving. `None` means no gamepad focus
    /// is being asserted and the mouse owns `Interaction` as usual.
    pub focused: Option<Entity>,
    /// Master switch. Turn off for a screen that reads the pad directly (a
    /// gameplay HUD) so the two don't both act on the same press.
    pub enabled: bool,
    /// Counts down to the next repeat while a direction is held.
    repeat_timer: f32,
    /// The direction currently held, so releasing it resets the repeat delay.
    held: Option<NavDir>,
    /// Set on the frame a press was published, so the next frame can release
    /// it. A press that was never released reads as a button held forever.
    pressed_last_frame: Option<Entity>,
}

impl Default for UiGamepadNav {
    fn default() -> Self {
        Self {
            focused: None,
            enabled: true,
            repeat_timer: 0.0,
            held: None,
            pressed_last_frame: None,
        }
    }
}

/// One focusable candidate, reduced to the rectangle the scoring needs.
struct Candidate {
    entity: Entity,
    center: Vec2,
}

/// Read the pads: the direction being asked for, and whether activate fired.
fn read_pads(pads: &Query<&Gamepad>) -> (Option<NavDir>, bool) {
    let mut dir = None;
    let mut activate = false;

    for pad in pads.iter() {
        if pad.just_pressed(GamepadButton::South) {
            activate = true;
        }

        // D-pad first — it is unambiguous, so a player using it never has a
        // resting stick argue with it.
        let d = if pad.pressed(GamepadButton::DPadUp) {
            Some(NavDir::Up)
        } else if pad.pressed(GamepadButton::DPadDown) {
            Some(NavDir::Down)
        } else if pad.pressed(GamepadButton::DPadLeft) {
            Some(NavDir::Left)
        } else if pad.pressed(GamepadButton::DPadRight) {
            Some(NavDir::Right)
        } else {
            let x = pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
            let y = pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
            // Whichever axis is pushed further wins, so a diagonal resolves to
            // one step instead of cancelling out or firing both.
            if x.abs() > y.abs() && x.abs() > STICK_DEAD_ZONE {
                Some(if x > 0.0 { NavDir::Right } else { NavDir::Left })
            } else if y.abs() > STICK_DEAD_ZONE {
                // Stick y is up-positive; UI y grows downward.
                Some(if y > 0.0 { NavDir::Up } else { NavDir::Down })
            } else {
                None
            }
        };
        if d.is_some() {
            dir = d;
        }
    }
    (dir, activate)
}

/// Pick the best candidate in `dir` from `from`, or `None` if nothing lies
/// that way.
fn best_in_direction(from: Vec2, dir: NavDir, candidates: &[Candidate]) -> Option<Entity> {
    let axis = dir.as_vec();
    let perp = Vec2::new(axis.y, axis.x).abs();

    candidates
        .iter()
        .filter_map(|c| {
            let delta = c.center - from;
            let along = delta.dot(axis);
            // Must actually be in that direction. The small floor rejects
            // nodes sharing a centre line, which would otherwise score 0 and
            // win every time.
            if along <= 1.0 {
                return None;
            }
            let off = (delta * perp).length();
            Some((c.entity, along + off * OFF_AXIS_PENALTY))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// Move focus, publish it as `Interaction`, and turn the activate button into a
/// one-frame `Interaction::Pressed`.
pub fn gamepad_ui_nav(
    time: Res<Time>,
    pads: Query<&Gamepad>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut nav: ResMut<UiGamepadNav>,
    mut focusables: Query<
        (Entity, &ComputedNode, &UiGlobalTransform, &mut Interaction),
        With<Button>,
    >,
) {
    // Release last frame's synthetic press before anything else, so a held
    // button reads as one press rather than as never having been let go.
    if let Some(entity) = nav.pressed_last_frame.take() {
        if let Ok((_, _, _, mut interaction)) = focusables.get_mut(entity) {
            if *interaction == Interaction::Pressed {
                *interaction = Interaction::Hovered;
            }
        }
    }

    if !nav.enabled {
        nav.focused = None;
        return;
    }

    let (dir, activate) = read_pads(&pads);

    // The mouse moving means the player has switched input; stop asserting a
    // highlight they are no longer steering. Drained either way so a stale
    // backlog can't drop focus later.
    let mouse_moved = mouse_motion.read().any(|m| m.delta != Vec2::ZERO);
    if mouse_moved && dir.is_none() && !activate {
        nav.focused = None;
        nav.held = None;
        return;
    }

    // Repeat gating: a fresh direction acts at once, a held one waits out the
    // delay and then repeats at a steady rate.
    let step = match dir {
        None => {
            nav.held = None;
            nav.repeat_timer = 0.0;
            None
        }
        Some(d) if nav.held != Some(d) => {
            nav.held = Some(d);
            nav.repeat_timer = REPEAT_DELAY;
            Some(d)
        }
        Some(d) => {
            nav.repeat_timer -= time.delta_secs();
            if nav.repeat_timer <= 0.0 {
                nav.repeat_timer = REPEAT_RATE;
                Some(d)
            } else {
                None
            }
        }
    };

    if step.is_none() && !activate && nav.focused.is_none() {
        return;
    }

    // Collect what can be focused right now. A zero-sized node is laid out to
    // nothing — `display: none`, a collapsed panel, a menu that is closed — and
    // must not be reachable, or focus vanishes into invisible entries.
    let candidates: Vec<Candidate> = focusables
        .iter()
        .filter(|(_, computed, _, _)| {
            let s = computed.size();
            s.x > 0.0 && s.y > 0.0
        })
        .map(|(entity, _, transform, _)| Candidate {
            entity,
            center: transform.translation,
        })
        .collect();

    if candidates.is_empty() {
        nav.focused = None;
        return;
    }

    // A focus pointing at something despawned or now hidden is stale.
    if let Some(f) = nav.focused {
        if !candidates.iter().any(|c| c.entity == f) {
            nav.focused = None;
        }
    }

    if let Some(dir) = step {
        nav.focused = match nav.focused {
            // First input on a pad: start at the top-left rather than at
            // whatever the query happened to yield first, so the entry point
            // is the same every time.
            None => candidates
                .iter()
                .min_by(|a, b| {
                    (a.center.y, a.center.x)
                        .partial_cmp(&(b.center.y, b.center.x))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|c| c.entity),
            Some(current) => {
                let from = candidates
                    .iter()
                    .find(|c| c.entity == current)
                    .map(|c| c.center)
                    .unwrap_or(Vec2::ZERO);
                // Nothing that way — keep the current selection rather than
                // clearing it, so pushing against the end of a list is inert
                // instead of dropping focus entirely.
                best_in_direction(from, dir, &candidates).or(Some(current))
            }
        };
    }

    let Some(focused) = nav.focused else {
        return;
    };

    // Publish focus as hover, and clear hover from everything else we own so a
    // node the mouse left behind doesn't stay lit.
    for (entity, _, _, mut interaction) in &mut focusables {
        if entity == focused {
            if activate {
                if *interaction != Interaction::Pressed {
                    *interaction = Interaction::Pressed;
                }
            } else if *interaction != Interaction::Hovered {
                *interaction = Interaction::Hovered;
            }
        } else if *interaction != Interaction::None {
            *interaction = Interaction::None;
        }
    }

    if activate {
        nav.pressed_last_frame = Some(focused);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<UiGamepadNav>();
    app.add_systems(Update, gamepad_ui_nav);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid test entity index")
    }

    fn c(entity_index: u32, x: f32, y: f32) -> Candidate {
        Candidate {
            entity: ent(entity_index),
            center: Vec2::new(x, y),
        }
    }

    /// A vertical list is the common case: down goes to the next row, up to the
    /// previous, and neither reaches past its neighbour.
    #[test]
    fn a_vertical_list_steps_one_row_at_a_time() {
        let list = vec![c(0, 100.0, 0.0), c(1, 100.0, 50.0), c(2, 100.0, 100.0)];
        assert_eq!(
            best_in_direction(Vec2::new(100.0, 0.0), NavDir::Down, &list),
            Some(ent(1))
        );
        assert_eq!(
            best_in_direction(Vec2::new(100.0, 100.0), NavDir::Up, &list),
            Some(ent(1))
        );
    }

    /// Pushing past the end has nothing to find. The caller keeps the current
    /// selection in that case; what matters here is that it isn't handed a
    /// wrong answer.
    #[test]
    fn the_end_of_a_list_has_nothing_further() {
        let list = vec![c(0, 100.0, 0.0), c(1, 100.0, 50.0)];
        assert_eq!(
            best_in_direction(Vec2::new(100.0, 50.0), NavDir::Down, &list),
            None
        );
    }

    /// The off-axis penalty is the whole reason navigation feels right: a
    /// button slightly below but far to the side must lose to one squarely
    /// below, even though the sideways one is closer as the crow flies.
    #[test]
    fn a_square_hit_beats_a_nearer_one_off_to_the_side() {
        let list = vec![
            c(0, 100.0, 60.0),  // directly below, 60 away
            c(1, 145.0, 30.0),  // closer in raw distance, well off to the side
        ];
        assert_eq!(
            best_in_direction(Vec2::new(100.0, 0.0), NavDir::Down, &list),
            Some(ent(0))
        );
    }

    /// Nodes sharing a centre line score zero along the axis and would win
    /// every comparison, so a step must never resolve to one.
    #[test]
    fn a_node_on_the_same_line_is_not_a_step() {
        let list = vec![c(0, 100.0, 0.0), c(1, 100.0, 40.0)];
        assert_eq!(
            best_in_direction(Vec2::new(100.0, 0.0), NavDir::Down, &list),
            Some(ent(1))
        );
        // Left, where only a same-line node exists, finds nothing.
        assert_eq!(
            best_in_direction(Vec2::new(100.0, 0.0), NavDir::Left, &list),
            None
        );
    }

    /// UI y grows downward, so "up" must decrease y. Getting this inverted is
    /// the classic bug and it is invisible in a symmetric layout.
    #[test]
    fn up_is_negative_y_in_ui_space() {
        assert_eq!(NavDir::Up.as_vec(), Vec2::new(0.0, -1.0));
        assert_eq!(NavDir::Down.as_vec(), Vec2::new(0.0, 1.0));
    }

    #[test]
    fn a_horizontal_row_steps_sideways() {
        let row = vec![c(0, 0.0, 50.0), c(1, 60.0, 50.0), c(2, 120.0, 50.0)];
        assert_eq!(
            best_in_direction(Vec2::new(0.0, 50.0), NavDir::Right, &row),
            Some(ent(1))
        );
        assert_eq!(
            best_in_direction(Vec2::new(120.0, 50.0), NavDir::Left, &row),
            Some(ent(1))
        );
    }
}
