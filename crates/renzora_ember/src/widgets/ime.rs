//! IME composition (Korean Hangul, Japanese, Chinese) for the text inputs.
//!
//! Bevy only delivers [`Ime`] messages to a window whose `Window::ime_enabled`
//! is set, and nothing used to set it, so a Korean user could not type a single
//! syllable: the IME never reached the engine. This module owns that switch and
//! the in-progress composition; the edits themselves stay in
//! [`super::text_input::text_input_type`], which reads keyboard and IME input
//! from one ordered stream.
//!
//! **The composition is display only.** While the IME is composing, the
//! focused input shows the preedit spliced in where it will land, but its
//! `value` does not change until the IME commits. Writing the preedit into
//! `value` would hand half-built syllables to every binding, validator and
//! undo stack that reads it, and a cancelled composition would have to be
//! taken back out of all of them.
//!
//! One composition exists at a time, because only one text input holds focus.
//! It records its owner because focus can move before the IME commits: like an
//! OS text field, the field being left keeps what was being composed in it,
//! and none of it lands in the field that took focus.
//!
//! The game UI's markup `<input>` (`markup::input_field`) applies the same
//! rules to its own focus. It shares only the window switch with this module,
//! and the candidate-window placement in [`place_ime`].
//!
//! wasm is not covered: winit's IME switch and candidate area are no-ops in a
//! browser, so Hangul there needs a DOM text-input bridge.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

/// Take back the `just_pressed` of every key the IME consumed this frame.
///
/// `ButtonInput<KeyCode>` follows the physical key, so the Enter that confirms
/// a Japanese clause or the Space that ends a Hangul syllable reads as
/// `just_pressed` like any other, and every handler that acts on Enter or
/// Escape through it (an inline rename, the search list, a viewport shortcut)
/// would fire mid-composition. The IME's own keys arrive as [`Key::Process`],
/// so clearing them here, once, right after Bevy's input pass, fixes all of
/// them rather than each handler separately. The key still reads as held.
pub(crate) fn release_ime_consumed_keys(
    mut events: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    for ev in events.read() {
        if ev.state == ButtonState::Pressed && matches!(ev.logical_key, Key::Process) {
            keys.clear_just_pressed(ev.key_code);
        }
    }
}

use super::code_editor::CodeEditor;
use super::text_input::{display_for, insert_committed, EmberTextInput, SingleLineInput};
use crate::markup::input_field::UiFocus;
use crate::theme::rgb;

/// The IME composition in progress, if any.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct ImeComposition {
    /// The text input the composition belongs to.
    pub owner: Option<Entity>,
    /// The preedit text the IME is building (a Hangul syllable in progress).
    pub text: String,
    /// The end of the span the IME has highlighted inside `text` (a clause it
    /// is converting), as a char index. `None` draws the caret at the end of
    /// `text`: the IME hid it, or reported only an insertion point.
    pub cursor: Option<usize>,
    /// Set when focus left a field mid-composition and the syllable was
    /// committed there on the IME's behalf: the commit the IME may still flush
    /// for it is a duplicate, and belongs to no field. Cleared once the IME
    /// reports the composition over or a new one starts, so it can never eat a
    /// later, wanted commit.
    pub discard_commit: bool,
}

impl ImeComposition {
    /// Whether `e` is showing a non-empty composition right now.
    pub fn is_composing(&self, e: Entity) -> bool {
        self.owner == Some(e) && !self.text.is_empty()
    }

    /// Drop the preedit text, keeping the owner. winit sends an empty preedit
    /// right before the commit of a finished syllable, so the owner has to
    /// survive it for the commit to land.
    pub fn clear_text(&mut self) {
        if !self.text.is_empty() || self.cursor.is_some() {
            self.text.clear();
            self.cursor = None;
        }
    }
}

/// Char index of byte offset `b` in `s`. winit reports the preedit caret in
/// UTF-8 bytes, but every caret in this crate counts chars, and one Hangul
/// syllable is three bytes. A `b` inside a char lands after that char.
pub(crate) fn byte_to_char(s: &str, b: usize) -> usize {
    s.char_indices().take_while(|(i, _)| *i < b).count()
}

/// The string the input shows while composing, and the char index where the
/// preedit starts in it.
///
/// The preedit takes the place of whatever is selected, because that is what
/// the commit will replace: showing it beside a still-visible selection would
/// preview the wrong result.
pub(crate) fn compose(inp: &EmberTextInput, single_line: bool, preedit: &str) -> (String, usize) {
    if inp.select_all {
        return (preedit.to_string(), 0);
    }
    if !single_line {
        // The textarea edits at its end (it has no caret index of its own).
        let start = inp.value.chars().count();
        return (format!("{}{}", inp.value, preedit), start);
    }
    let len = inp.value.chars().count();
    let (a, b) = inp
        .selection_range()
        .unwrap_or((inp.caret_index.min(len), inp.caret_index.min(len)));
    let head: String = inp.value.chars().take(a).collect();
    let tail: String = inp.value.chars().skip(b).collect();
    (format!("{head}{preedit}{tail}"), a)
}

/// Char index of the caret inside the composed display string.
pub(crate) fn composed_caret(comp: &ImeComposition, start: usize) -> usize {
    let n = comp.text.chars().count();
    start + comp.cursor.unwrap_or(n).min(n)
}

/// Allow the IME on every window while a text input is focused, and on none
/// otherwise; finish a composition whose owner lost focus.
///
/// IME permission is not the same thing as keyboard capture: viewport
/// shortcuts already back off through `ui_wants_keyboard` while a field is
/// focused, and this leaves that alone. Every window rather than the one
/// holding the field, because a UI node does not know its window, and allowing
/// the IME on a window nobody is typing into changes nothing for it.
///
/// Leaving a field mid-composition commits the syllable into it, which is what
/// an OS text field does. It is committed here rather than by waiting for the
/// IME, because whether an IME flushes its composition when it loses the
/// window differs from one IME to the next, and a syllable that silently
/// vanished on a click would read as a lost keystroke.
///
/// Moving focus from one field to another leaves the IME allowed on the same
/// window, so the OS never hears about it and would carry the syllable into
/// the next field as well. Withdrawing permission for one frame is the only
/// lever winit offers to end that composition; the duplicate commit the IME
/// may flush in response is discarded.
///
/// The markup `<input>` of a game UI ([`UiFocus`]) and the code editor keep
/// their own focus and composition, but the window switch is one switch: it is
/// decided here for all three, or each would turn the IME off every frame
/// another wants it. Likewise the duplicate flush: it reaches whichever input
/// took focus, of any of the three kinds, so ending a composition arms every
/// kind's discard, and each disarms on the IME's own end of the composition.
pub(crate) fn ime_focus_sync(
    mut inputs: Query<(Entity, &mut EmberTextInput, Has<SingleLineInput>)>,
    mut comp: ResMut<ImeComposition>,
    mut markup: Option<ResMut<UiFocus>>,
    mut code: Query<&mut CodeEditor>,
    mut windows: Query<&mut Window>,
    mut last: Local<Option<bool>>,
) {
    let focused = inputs.iter().find(|(_, i, _)| i.focused).map(|(e, _, _)| e);
    let mut bounce = false;
    if let Some(owner) = comp.owner {
        if focused != Some(owner) {
            if comp.is_composing(owner) {
                // A field despawned mid-composition has nowhere to keep it.
                if let Ok((_, mut inp, single)) = inputs.get_mut(owner) {
                    insert_committed(&mut inp, single, &comp.text);
                }
                bounce = true;
            }
            comp.owner = None;
            comp.clear_text();
        }
    }
    let markup_focused = markup.as_ref().is_some_and(|f| f.focused.is_some());
    if let Some(f) = markup.as_mut().filter(|f| f.ime_bounce) {
        f.ime_bounce = false;
        bounce = true;
    }
    let mut code_focused = false;
    for mut ed in &mut code {
        code_focused |= ed.wants_ime();
        // Checked through a shared borrow first: a `&mut` call on every editor
        // every frame would mark them all changed.
        if ed.has_stranded_preedit() {
            bounce |= ed.finish_composition();
        }
    }
    if bounce {
        comp.discard_commit = true;
        if let Some(f) = markup.as_mut() {
            f.discard_commit = true;
        }
        for mut ed in &mut code {
            ed.discard_next_commit();
        }
    }
    let want = (focused.is_some() || markup_focused || code_focused) && !bounce;
    // Written only when ember's own answer changes (or a window is new), never
    // every frame: a game script or plugin with a text box of its own may turn
    // the IME on while no ember input is focused, and an every-frame write
    // would switch it straight back off.
    let changed = *last != Some(want);
    *last = Some(want);
    for mut w in &mut windows {
        if (changed || w.is_added()) && w.ime_enabled != want {
            w.ime_enabled = want;
        }
    }
}

/// Put the IME's candidate window under the focused input's caret.
///
/// `Window::ime_position` is in logical px from the window's client-area
/// origin. The caret's `UiGlobalTransform` is its centre in physical px, so
/// the bottom-left corner is derived from the physical size and the whole point
/// divided by the window's scale factor. `ComputedNode::inverse_scale_factor`
/// would be the wrong divisor: it folds in `UiScale`, which is part of where
/// the caret really is on screen.
///
/// Written to the OS-focused window, which is the one receiving the keys. It
/// reads the transform the previous frame laid out; a caret one frame behind is
/// not visible in a candidate window.
pub(crate) fn ime_position_sync(
    inputs: Query<&EmberTextInput>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<&mut Window>,
) {
    let Some(inp) = inputs.iter().find(|i| i.focused) else {
        return;
    };
    let Ok((cn, xf)) = nodes.get(inp.caret) else {
        return;
    };
    let size = cn.size();
    let centre = xf.affine().translation;
    place_ime(&mut windows, centre + Vec2::new(-size.x * 0.5, size.y * 0.5));
}

/// Put the IME's candidate window at `physical`, a point in physical px from
/// the window's client-area origin, on the OS-focused window. Shared with the
/// markup `<input>`, which places it at the end of its own text.
pub(crate) fn place_ime(windows: &mut Query<&mut Window>, physical: Vec2) {
    for mut w in windows {
        if !w.focused {
            continue;
        }
        let pos = physical / w.scale_factor().max(f32::EPSILON);
        if w.ime_position.distance_squared(pos) > 0.25 {
            w.ime_position = pos;
        }
    }
}

/// Draw the composition into its owner's text, and put the plain value back
/// when the composition ends or moves.
///
/// Runs after [`super::text_input::text_input_sync`], which redraws a changed
/// input from its `value` alone and would otherwise wipe the preedit off the
/// screen on the frame a syllable is committed beside a new one.
pub(crate) fn ime_display(
    comp: Res<ImeComposition>,
    inputs: Query<(&EmberTextInput, Has<SingleLineInput>)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    mut shown: Local<Option<Entity>>,
) {
    let active = comp.owner.filter(|e| comp.is_composing(*e));
    if let Some(prev) = *shown {
        if Some(prev) != active {
            if let Ok((inp, _)) = inputs.get(prev) {
                let (disp, col) = display_for(&inp.value, &inp.placeholder, inp.password);
                write_text(&mut texts, inp.text_entity, disp, col);
            }
        }
    }
    *shown = active;
    let Some(owner) = active else { return };
    let Ok((inp, single)) = inputs.get(owner) else {
        return;
    };
    let (composed, _) = compose(inp, single, &comp.text);
    let (disp, col) = display_for(&composed, &inp.placeholder, inp.password);
    write_text(&mut texts, inp.text_entity, disp, col);
}

/// Write `disp` in colour `col` to text node `e`, touching only what differs:
/// this runs every frame of a composition, and an unconditional write would
/// mark the text changed and relayout it each time.
fn write_text(
    texts: &mut Query<(&mut Text, &mut TextColor)>,
    e: Entity,
    disp: String,
    col: (u8, u8, u8),
) {
    if let Ok((mut t, mut c)) = texts.get_mut(e) {
        if t.0 != disp {
            t.0 = disp;
        }
        let col = rgb(col);
        if c.0 != col {
            c.0 = col;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::ButtonState;
    use bevy::window::{Ime, WindowEvent};

    use crate::widgets::form::{form_enter_submit, EmberForm};
    use crate::widgets::text_input::text_input_type;
    use crate::UiInteraction;

    fn press(logical: Key, code: KeyCode) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: logical,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    /// An Enter the IME consumed is no longer `just_pressed`, so a rename or
    /// search that acts on Enter does not fire mid-composition; a plain Enter
    /// still is.
    #[test]
    fn a_key_the_ime_consumed_is_not_just_pressed() {
        let mut world = World::new();
        world.init_resource::<Messages<KeyboardInput>>();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Enter);
        keys.press(KeyCode::Space);
        world.insert_resource(keys);
        world.write_message(press(Key::Process, KeyCode::Enter));
        world.write_message(press(Key::Space, KeyCode::Space));
        world.run_system_once(release_ime_consumed_keys).unwrap();
        let keys = world.resource::<ButtonInput<KeyCode>>();
        assert!(!keys.just_pressed(KeyCode::Enter));
        assert!(keys.pressed(KeyCode::Enter));
        assert!(keys.just_pressed(KeyCode::Space));
    }

    /// A focused input holding `value`, with the caret at char `caret`.
    fn input(value: &str, caret: usize) -> EmberTextInput {
        EmberTextInput {
            value: value.to_string(),
            focused: true,
            text_entity: Entity::PLACEHOLDER,
            placeholder: String::new(),
            caret: Entity::PLACEHOLDER,
            password: false,
            select_all: false,
            caret_index: caret,
            advance: 0.0,
            offsets: Vec::new(),
            sel_anchor: None,
            lead: 0.0,
        }
    }

    /// A world holding one focused single-line input showing `value`, with the
    /// caret at its end.
    fn typing_world(value: &str) -> (World, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<Messages<WindowEvent>>();
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ImeComposition>();
        let text = world.spawn((Text::new(value), TextColor::default())).id();
        let mut inp = input(value, value.chars().count());
        inp.text_entity = text;
        let e = world.spawn((inp, SingleLineInput)).id();
        (world, e, text)
    }

    /// Apply `events` as one frame. The queue is cleared first: every
    /// `run_system_once` starts a fresh reader, which would replay earlier
    /// frames' messages as well.
    fn send(world: &mut World, events: impl IntoIterator<Item = WindowEvent>) {
        world.resource_mut::<Messages<WindowEvent>>().clear();
        for ev in events {
            world.write_message(ev);
        }
        world.run_system_once(text_input_type).unwrap();
    }

    fn value(world: &World, e: Entity) -> &str {
        &world.get::<EmberTextInput>(e).unwrap().value
    }

    #[test]
    fn a_commit_is_inserted_at_the_caret() {
        let (mut world, e, _) = typing_world("가");
        send(
            &mut world,
            [WindowEvent::Ime(Ime::Commit {
                window: Entity::PLACEHOLDER,
                value: "나".into(),
            })],
        );
        assert_eq!(value(&world, e), "가나");
        assert_eq!(world.get::<EmberTextInput>(e).unwrap().caret_index, 2);
    }

    #[test]
    fn a_key_the_ime_consumed_edits_nothing() {
        let (mut world, e, _) = typing_world("가");
        // The Backspace that removes a jamo from a syllable still in the IME.
        send(
            &mut world,
            [WindowEvent::KeyboardInput(press(
                Key::Process,
                KeyCode::Backspace,
            ))],
        );
        assert_eq!(value(&world, e), "가");
    }

    #[test]
    fn a_preedit_is_shown_but_not_typed() {
        let (mut world, e, text) = typing_world("가");
        send(
            &mut world,
            [WindowEvent::Ime(Ime::Preedit {
                window: Entity::PLACEHOLDER,
                value: "ㄴ".into(),
                cursor: Some((3, 3)),
            })],
        );
        assert_eq!(value(&world, e), "가");
        let comp = world.resource::<ImeComposition>();
        assert!(comp.is_composing(e));
        world.run_system_once(ime_display).unwrap();
        assert_eq!(world.get::<Text>(text).unwrap().0, "가ㄴ");
    }

    fn preedit(world: &mut World, value: &str, cursor: (usize, usize)) {
        send(
            world,
            [WindowEvent::Ime(Ime::Preedit {
                window: Entity::PLACEHOLDER,
                value: value.into(),
                cursor: Some(cursor),
            })],
        );
    }

    /// The Microsoft Korean IME puts its caret at 0, before the syllable it is
    /// building. Regression guard: the caret was drawn one cell behind.
    #[test]
    fn a_collapsed_ime_caret_sits_after_the_syllable() {
        let (mut world, e, _) = typing_world("가");
        preedit(&mut world, "나", (0, 0));
        let comp = world.resource::<ImeComposition>();
        assert_eq!(comp.cursor, None);
        let inp = world.get::<EmberTextInput>(e).unwrap();
        assert_eq!(composed_caret(comp, compose(inp, true, &comp.text).1), 2);
    }

    #[test]
    fn a_highlighted_clause_puts_the_caret_at_its_end() {
        let (mut world, _, _) = typing_world("");
        // "한글" with the first syllable (bytes 0..3) under conversion.
        preedit(&mut world, "한글", (0, 3));
        assert_eq!(world.resource::<ImeComposition>().cursor, Some(1));
    }

    /// Clicking another field mid-syllable keeps the syllable where it was
    /// typed, as an OS text field does, and the IME's own flush of it lands
    /// nowhere.
    #[test]
    fn leaving_a_field_mid_composition_commits_it_there() {
        let (mut world, e, _) = typing_world("가");
        preedit(&mut world, "한", (0, 0));
        let other = world.spawn(input("", 0)).id();
        world.get_mut::<EmberTextInput>(e).unwrap().focused = false;
        world.run_system_once(ime_focus_sync).unwrap();
        assert_eq!(value(&world, e), "가한");
        assert!(!world.resource::<ImeComposition>().is_composing(e));
        send(
            &mut world,
            [
                WindowEvent::Ime(Ime::Commit {
                    window: Entity::PLACEHOLDER,
                    value: "한".into(),
                }),
                WindowEvent::Ime(Ime::Disabled {
                    window: Entity::PLACEHOLDER,
                }),
            ],
        );
        assert_eq!(value(&world, e), "가한");
        assert_eq!(value(&world, other), "");
        // Past `Disabled` the guard is down, so the next commit is typed.
        send(
            &mut world,
            [WindowEvent::Ime(Ime::Commit {
                window: Entity::PLACEHOLDER,
                value: "글".into(),
            })],
        );
        assert_eq!(value(&world, other), "글");
    }

    /// A form whose input is focused, and its submit button.
    fn form_world() -> (World, Entity) {
        let (mut world, input, _) = typing_world("가");
        let submit = world.spawn(UiInteraction::None).id();
        world.spawn(EmberForm { submit }).add_child(input);
        (world, submit)
    }

    fn press_enter(world: &mut World, logical: Key) {
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        world.write_message(press(logical, KeyCode::Enter));
        world.run_system_once(form_enter_submit).unwrap();
    }

    #[test]
    fn enter_submits_a_form() {
        let (mut world, submit) = form_world();
        press_enter(&mut world, Key::Enter);
        assert_eq!(
            *world.get::<UiInteraction>(submit).unwrap(),
            UiInteraction::Pressed
        );
    }

    #[test]
    fn an_enter_the_ime_consumed_does_not_submit() {
        let (mut world, submit) = form_world();
        press_enter(&mut world, Key::Process);
        assert_eq!(
            *world.get::<UiInteraction>(submit).unwrap(),
            UiInteraction::None
        );
    }

    #[test]
    fn byte_offsets_become_char_indices() {
        // Each Hangul syllable is three UTF-8 bytes.
        assert_eq!(byte_to_char("한글", 0), 0);
        assert_eq!(byte_to_char("한글", 3), 1);
        assert_eq!(byte_to_char("한글", 6), 2);
        assert_eq!(byte_to_char("a한", 4), 2);
    }

    #[test]
    fn preedit_lands_at_the_caret() {
        let inp = input("가다", 1);
        assert_eq!(compose(&inp, true, "나"), ("가나다".to_string(), 1));
    }

    #[test]
    fn preedit_replaces_a_drag_selection() {
        let mut inp = input("abcd", 3);
        inp.sel_anchor = Some(1);
        assert_eq!(compose(&inp, true, "한"), ("a한d".to_string(), 1));
    }

    #[test]
    fn preedit_replaces_select_all() {
        let mut inp = input("old", 3);
        inp.select_all = true;
        assert_eq!(compose(&inp, true, "한"), ("한".to_string(), 0));
    }

    #[test]
    fn textarea_preedit_appends() {
        let inp = input("줄\n", 0);
        assert_eq!(compose(&inp, false, "한"), ("줄\n한".to_string(), 2));
    }

    #[test]
    fn composed_caret_defaults_to_the_preedit_end() {
        let comp = ImeComposition {
            text: "한".into(),
            ..Default::default()
        };
        assert_eq!(composed_caret(&comp, 2), 3);
        let comp = ImeComposition {
            cursor: Some(0),
            ..comp
        };
        assert_eq!(composed_caret(&comp, 2), 2);
    }
}
