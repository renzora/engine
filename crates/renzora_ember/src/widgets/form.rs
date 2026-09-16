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
//! - **Tab / Shift+Tab** moves focus to the next/previous visible field, and
//!   accepts what was typed into the one it leaves.
//!
//! Tab also works *without* the marker: it falls back to cycling fields under
//! the smallest ancestor that contains at least two of them, so multi-field
//! groups get sensible tabbing for free.
//!
//! A "field" here is a text input *or* a numeric [`drag_value`](super::drag_value).
//! The two kinds interleave in tree order, so tabbing runs down a panel row by
//! row whichever kind each row happens to be, and a Vec3 row tabs X, Y, Z.

use bevy::prelude::*;

use crate::reactive::Bound;
use crate::style::{Styled, WidgetState};
use super::drag_value::{self, DragRange, DragSnap, EmberDragValue};
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

/// Every widget Tab stops on: a text input (single line or textarea) or a
/// numeric drag-value field.
type IsField = Or<(With<EmberTextInput>, With<EmberDragValue>)>;

/// Depth-first, tree-order list of the fields under `root`, skipping
/// `Display::None` subtrees (hidden views toggled by `bind_display` must not
/// steal a Tab stop).
///
/// `nodes` is the same `&mut Node` query the focus step writes through, read
/// here through its read-only view: a system may not hold both a `&Node` and a
/// `&mut Node` query.
fn collect_fields(
    root: Entity,
    children: &Query<&Children>,
    nodes: &Query<&mut Node>,
    is_field: &Query<(), IsField>,
    out: &mut Vec<Entity>,
) {
    if nodes.get(root).is_ok_and(|n| n.display == Display::None) {
        return;
    }
    if is_field.contains(root) {
        out.push(root);
    }
    if let Ok(kids) = children.get(root) {
        for kid in kids.iter() {
            collect_fields(kid, children, nodes, is_field, out);
        }
    }
}

/// True when the field, or anything it hangs under, is `Display::None`: its
/// panel is stashed and the focus flag it still carries is stale (the same
/// case [`form_enter_submit`] refuses to submit on). Nothing is committed or
/// focused on such a field's behalf.
fn is_hidden(mut e: Entity, parents: &Query<&ChildOf>, nodes: &Query<&mut Node>) -> bool {
    loop {
        if nodes.get(e).is_ok_and(|n| n.display == Display::None) {
            return true;
        }
        let Ok(c) = parents.get(e) else { return false };
        e = c.parent();
    }
}

/// Tab / Shift+Tab while a field is focused → **accept what was typed into it**
/// and focus the next / previous field in the form (wrapping). The scope is the
/// nearest [`EmberForm`] ancestor, or failing that the smallest ancestor subtree
/// containing at least two fields. Tabbing into a field selects its content,
/// like an OS text field.
///
/// A text input publishes every keystroke to its bound state, so for those
/// "accept" is already true and Tab only has to move. A numeric field does not:
/// it holds typed digits in a buffer until something commits them, and until
/// this handler existed only `Enter` or a click away would. Tabbing out of one
/// dropped the edit and snapped the number back to its old value, which is what
/// issue #88 reported. Both kinds now end an edit the same way, so the two can
/// be tabbed through interchangeably.
#[allow(clippy::type_complexity)]
pub(crate) fn form_tab_focus(
    keys: Res<ButtonInput<KeyCode>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    is_field: Query<(), IsField>,
    forms: Query<(), With<EmberForm>>,
    mut inputs: Query<(Entity, &mut EmberTextInput, &mut Styled), Without<EmberDragValue>>,
    mut drags: Query<
        (
            Entity,
            &mut EmberDragValue,
            &mut Bound<f32>,
            Option<&DragRange>,
            Option<&DragSnap>,
            Option<&mut Styled>,
            &mut crate::cursor_icon::HoverCursor,
        ),
        Without<EmberTextInput>,
    >,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    // The focused field: a text input holding the caret, or a numeric field
    // being typed into (a drag-value carries its own edit flag rather than
    // sharing `EmberTextInput`).
    let Some(current) = inputs
        .iter()
        .find(|(_, i, _)| i.focused)
        .map(|(e, _, _)| e)
        .or_else(|| {
            drags
                .iter()
                .find(|(_, dv, ..)| dv.is_editing())
                .map(|(e, ..)| e)
        })
    else {
        return;
    };
    if is_hidden(current, &parents, &nodes) {
        return;
    }

    // Scope: nearest EmberForm ancestor wins; otherwise the smallest ancestor
    // whose subtree holds ≥2 fields (so ungrouped field pairs still tab).
    let mut ordered = Vec::new();
    let mut e = current;
    while let Ok(c) = parents.get(e) {
        e = c.parent();
        if forms.contains(e) {
            ordered.clear();
            collect_fields(e, &children, &nodes, &is_field, &mut ordered);
            break;
        }
        if ordered.len() < 2 {
            ordered.clear();
            collect_fields(e, &children, &nodes, &is_field, &mut ordered);
        }
    }
    let target = match ordered.iter().position(|&e| e == current) {
        Some(pos) if ordered.len() >= 2 => {
            let back = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
            let step = if back { ordered.len() - 1 } else { 1 };
            Some(ordered[(pos + step) % ordered.len()])
        }
        // A field with nowhere to tab to still accepts its edit below: the point
        // of the key is leaving the field, and it has been left either way.
        _ => None,
    };

    if let Ok((_, mut dv, mut bound, range, snap, mut styled, mut cursor)) = drags.get_mut(current)
    {
        if dv.is_editing() {
            drag_value::tab_commit(
                &mut dv,
                &mut bound,
                range,
                snap,
                styled.as_deref_mut(),
                &mut cursor,
                &mut texts,
                &mut nodes,
            );
        }
    }

    let Some(target) = target else {
        return;
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

    if let Ok((_, mut dv, bound, range, _, mut styled, mut cursor)) = drags.get_mut(target) {
        if !dv.is_editing() {
            let value = bound.0;
            drag_value::tab_focus(
                &mut dv,
                value,
                range.is_some(),
                styled.as_deref_mut(),
                &mut cursor,
                &mut texts,
                &mut nodes,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::drag_value::drag_value;
    use bevy::text::FontSource;

    /// The entities a test's fields were spawned as, in tree order.
    #[derive(Resource)]
    struct Fields(Vec<Entity>);

    /// `count` numeric fields sharing one parent: the shape of an inspector
    /// Vec3 row (three drag-values under the row's value column).
    fn app_with_fields(count: usize) -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, form_tab_focus);
        let mut commands = app.world_mut().commands();
        let font = FontSource::default();
        let fields: Vec<Entity> = (0..count)
            .map(|i| drag_value(&mut commands, &font, "X", (230, 90, 90), i as f32, 0.1))
            .collect();
        let row = commands.spawn(Node::default()).id();
        commands.entity(row).add_children(&fields);
        commands.insert_resource(Fields(fields));
        app.world_mut().flush();
        app
    }

    fn fields(app: &App) -> Vec<Entity> {
        app.world().resource::<Fields>().0.clone()
    }

    fn press_tab(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Tab);
        app.update();
    }

    fn edit(app: &mut App, field: Entity, typed: &str) {
        app.world_mut()
            .get_mut::<EmberDragValue>(field)
            .expect("field exists")
            .begin_edit_for_test(typed);
    }

    fn value(app: &App, field: Entity) -> f32 {
        app.world().get::<Bound<f32>>(field).expect("field exists").0
    }

    fn editing(app: &App, field: Entity) -> bool {
        app.world()
            .get::<EmberDragValue>(field)
            .expect("field exists")
            .is_editing()
    }

    /// The issue: a number typed into a field and tabbed out of was thrown
    /// away. Tab has to accept it, exactly as Enter does.
    #[test]
    fn tab_accepts_the_number_being_typed() {
        let mut app = app_with_fields(3);
        let f = fields(&app);
        edit(&mut app, f[0], "5");
        press_tab(&mut app);
        assert_eq!(value(&app, f[0]), 5.0);
    }

    /// Having accepted it, Tab moves on: an inspector Vec3 row is typed
    /// X → Y → Z without touching the mouse.
    #[test]
    fn tab_moves_to_the_next_field() {
        let mut app = app_with_fields(3);
        let f = fields(&app);
        edit(&mut app, f[0], "5");
        press_tab(&mut app);
        assert!(!editing(&app, f[0]));
        assert!(editing(&app, f[1]));
    }

    /// Shift+Tab walks back, and the field it leaves is accepted the same way.
    #[test]
    fn shift_tab_walks_back_and_wraps() {
        let mut app = app_with_fields(3);
        let f = fields(&app);
        edit(&mut app, f[0], "5");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        press_tab(&mut app);
        assert_eq!(value(&app, f[0]), 5.0);
        assert!(editing(&app, f[2]));
    }

    /// A field with nowhere to tab to still accepts what was typed: the key
    /// means "I'm done here", and it is true whether or not focus lands
    /// anywhere afterwards.
    #[test]
    fn a_lone_field_still_accepts_its_edit() {
        let mut app = app_with_fields(1);
        let f = fields(&app);
        edit(&mut app, f[0], "5");
        press_tab(&mut app);
        assert_eq!(value(&app, f[0]), 5.0);
        assert!(!editing(&app, f[0]));
    }

    /// A stashed panel's field keeps its edit flag, so Tab pressed somewhere
    /// else entirely would find it and commit a value nobody is looking at.
    #[test]
    fn a_hidden_field_is_left_alone() {
        let mut app = app_with_fields(2);
        let f = fields(&app);
        edit(&mut app, f[0], "5");
        app.world_mut()
            .get_mut::<Node>(f[0])
            .expect("field exists")
            .display = Display::None;
        press_tab(&mut app);
        assert_eq!(value(&app, f[0]), 0.0);
        assert!(!editing(&app, f[1]));
    }

    /// A field nobody is typing into must not be disturbed: Tab with no
    /// focused field at all is somebody else's key.
    #[test]
    fn tab_with_nothing_focused_changes_nothing() {
        let mut app = app_with_fields(2);
        let f = fields(&app);
        press_tab(&mut app);
        assert_eq!(value(&app, f[0]), 0.0);
        assert!(!editing(&app, f[0]));
        assert!(!editing(&app, f[1]));
    }
}
