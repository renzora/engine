//! The animated "glow box" that frames the on-screen element the current step
//! wants the user to interact with (the workspace ribbon, the dock area, the
//! theme menu — see [`crate::steps::highlight_for`]).
//!
//! A single persistent, **click-through** box (`FocusPolicy::Pass`) is moved over
//! the target node's screen rect each frame and its border pulses. The target is
//! found by bevy_ui `Name`; its logical rect comes from `ComputedNode` (size) +
//! `GlobalTransform` (center) — the shell compares UI `GlobalTransform.translation`
//! directly against the logical cursor, so that translation is in logical px.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, FocusPolicy, UiGlobalTransform};

use renzora::core::{EditorLocked, HideInHierarchy};
use renzora_ember::theme::accent;

use crate::state::TutorialState;
use crate::steps::{highlight_for, STEPS};

/// The single reusable highlight box (hidden when the step has no target).
#[derive(Component)]
pub struct HighlightBox;

/// Spawn the (initially hidden) highlight box. Click-through so it never blocks
/// the element it frames.
pub fn spawn_box(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            FocusPolicy::Pass,
            GlobalZIndex(8150),
            HideInHierarchy,
            EditorLocked,
            HighlightBox,
            Name::new("tutorial-highlight"),
        ))
        .id()
}

/// Track the current step's target node each frame: position the box over it and
/// pulse its border (width + alpha) for an animated glow. Hides the box when the
/// step has no target or the target isn't on screen.
pub fn update_highlight(
    time: Res<Time>,
    state: Res<TutorialState>,
    // UI nodes position via `UiGlobalTransform` (physical px), NOT the regular
    // `GlobalTransform` — using the latter matched nothing, so the box never
    // showed. `ComputedNode` gives size + the inverse scale factor to logical px.
    targets: Query<(&Name, &ComputedNode, &UiGlobalTransform)>,
    mut boxes: Query<(&mut Node, &mut BorderColor), With<HighlightBox>>,
) {
    let Ok((mut node, mut border)) = boxes.single_mut() else {
        return;
    };

    let target_name = (state.active && state.current < STEPS.len())
        .then(|| highlight_for(STEPS[state.current].kind))
        .flatten();
    let Some(target_name) = target_name else {
        node.display = Display::None;
        return;
    };

    // Find the named node's logical rect, skipping zero-sized (collapsed/hidden)
    // matches. `UiGlobalTransform.translation` + `ComputedNode.size()` are both
    // physical px; multiply by the inverse scale factor for logical (Val::Px).
    let mut rect = None;
    for (name, cn, ugt) in &targets {
        if name.as_str() == target_name {
            let isf = cn.inverse_scale_factor();
            let size = cn.size() * isf;
            if size.x < 1.0 || size.y < 1.0 {
                continue;
            }
            let top_left = (ugt.translation - cn.size() * 0.5) * isf;
            rect = Some((top_left, size));
            break;
        }
    }
    let Some((top_left, size)) = rect else {
        node.display = Display::None;
        return;
    };

    const PAD: f32 = 4.0;
    node.display = Display::Flex;
    node.left = Val::Px(top_left.x - PAD);
    node.top = Val::Px(top_left.y - PAD);
    node.width = Val::Px(size.x + PAD * 2.0);
    node.height = Val::Px(size.y + PAD * 2.0);

    // Animated glow: pulse border width + alpha together.
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 3.5).sin();
    node.border = UiRect::all(Val::Px(2.0 + 2.5 * pulse));
    let (r, g, b) = accent();
    *border = BorderColor::all(Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        0.5 + 0.45 * pulse,
    ));
}
