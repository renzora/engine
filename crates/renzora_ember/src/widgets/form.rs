//! Form behaviors — Tab focus cycling + Enter-to-submit.
//!
//! Any container holding text inputs and a submit button becomes a "form" by
//! inserting [`EmberForm`] on it. That one marker buys both behaviors with no
//! per-panel keyboard code:
//!
//! - **Enter** in a focused single-line input simulates a click on the form's
//!   submit button, so whatever `Changed<Interaction>` handler the panel
//!   already has for that button fires unchanged. (Textareas keep Enter as a
//!   literal newline — only single-line fields submit.)
//! - **Tab / Shift+Tab** moves focus to the next/previous visible input.
//!
//! Tab also works *without* the marker: it falls back to cycling inputs under
//! the smallest ancestor that contains at least two of them, so multi-field
//! groups get sensible tabbing for free.

use bevy::prelude::*;

use crate::style::WidgetState;
use super::text_input::{EmberTextInput, SingleLineInput};

/// Marks a container as a form and names its submit button. Pressing Enter in
/// any focused single-line input inside the container "clicks" `submit`.
#[derive(Component)]
pub struct EmberForm {
    /// The button whose `Interaction` is driven to `Pressed` on Enter.
    pub submit: Entity,
}

/// Enter in a focused single-line input → press the nearest enclosing
/// [`EmberForm`]'s submit button.
///
/// Runs in `PreUpdate` after `UiSystems::Focus` (which has just settled this
/// frame's real `Interaction` values): the simulated `Pressed` then survives
/// the whole `Update` schedule, so every panel's `Changed<Interaction>` click
/// handler observes it regardless of system order. Next frame the focus system
/// resets the button to `None` as usual.
pub(crate) fn form_enter_submit(
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<(Entity, &EmberTextInput), With<SingleLineInput>>,
    parents: Query<&ChildOf>,
    nodes: Query<&Node>,
    forms: Query<&EmberForm>,
    mut interactions: Query<&mut Interaction>,
) {
    if !keys.just_pressed(KeyCode::Enter) && !keys.just_pressed(KeyCode::NumpadEnter) {
        return;
    }
    let Some((focused, _)) = inputs.iter().find(|(_, i)| i.focused) else {
        return;
    };
    let mut e = focused;
    loop {
        // A hidden ancestor means the input's panel/view is stashed but its
        // focus flag went stale — don't submit an invisible form.
        if nodes.get(e).is_ok_and(|n| n.display == Display::None) {
            return;
        }
        if let Ok(form) = forms.get(e) {
            if let Ok(mut i) = interactions.get_mut(form.submit) {
                *i = Interaction::Pressed;
            }
            return;
        }
        let Ok(c) = parents.get(e) else { return };
        e = c.parent();
    }
}

/// Depth-first, tree-order list of text-input boxes under `root`, skipping
/// `Display::None` subtrees (hidden views toggled by `bind_display` must not
/// steal a Tab stop).
fn collect_inputs(
    root: Entity,
    children: &Query<&Children>,
    nodes: &Query<&Node>,
    is_input: &Query<(), With<EmberTextInput>>,
    out: &mut Vec<Entity>,
) {
    if nodes.get(root).is_ok_and(|n| n.display == Display::None) {
        return;
    }
    if is_input.contains(root) {
        out.push(root);
    }
    if let Ok(kids) = children.get(root) {
        for kid in kids.iter() {
            collect_inputs(kid, children, nodes, is_input, out);
        }
    }
}

/// Tab / Shift+Tab while an input is focused → focus the next / previous
/// input in the form (wrapping). The scope is the nearest [`EmberForm`]
/// ancestor, or failing that the smallest ancestor subtree containing at
/// least two inputs. Tabbing into a field selects its content, like an OS
/// text field.
#[allow(clippy::type_complexity)]
pub(crate) fn form_tab_focus(
    keys: Res<ButtonInput<KeyCode>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    nodes: Query<&Node>,
    is_input: Query<(), With<EmberTextInput>>,
    forms: Query<(), With<EmberForm>>,
    mut inputs: Query<(Entity, &mut EmberTextInput, &mut crate::style::Styled)>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let Some(current) = inputs
        .iter()
        .find(|(_, i, _)| i.focused)
        .map(|(e, _, _)| e)
    else {
        return;
    };

    // Scope: nearest EmberForm ancestor wins; otherwise the smallest ancestor
    // whose subtree holds ≥2 inputs (so ungrouped field pairs still tab).
    let mut ordered = Vec::new();
    let mut e = current;
    while let Ok(c) = parents.get(e) {
        e = c.parent();
        if forms.contains(e) {
            ordered.clear();
            collect_inputs(e, &children, &nodes, &is_input, &mut ordered);
            break;
        }
        if ordered.len() < 2 {
            ordered.clear();
            collect_inputs(e, &children, &nodes, &is_input, &mut ordered);
        }
    }
    let Some(pos) = ordered.iter().position(|&e| e == current) else {
        return;
    };
    if ordered.len() < 2 {
        return;
    }
    let back = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let target = if back {
        ordered[(pos + ordered.len() - 1) % ordered.len()]
    } else {
        ordered[(pos + 1) % ordered.len()]
    };

    for (e, mut inp, mut styled) in &mut inputs {
        let focus = e == target;
        if inp.focused != focus {
            inp.focused = focus;
            styled.state = if focus { WidgetState::Active } else { WidgetState::Normal };
        }
        if focus {
            // Select the tabbed-into value (OS convention) so typing replaces it.
            inp.select_all = !inp.value.is_empty();
            inp.caret_index = inp.value.chars().count();
        }
        inp.sel_anchor = None;
    }
}
