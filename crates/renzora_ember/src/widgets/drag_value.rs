//! Drag-value — a scrubbable numeric field (drag horizontally to change).
//!
//! Beyond scrubbing, it behaves like Godot's SpinBox:
//!  * a thin slider line at the bottom shows the value within its [`DragRange`]
//!    (only drawn when a range is set), and
//!  * a click (press without a drag) enters keyboard-edit mode — type a number,
//!    `Enter`/click-away to commit, `Esc` to cancel.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::reactive::Bound;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

use super::common::{format_num, text_node};

#[derive(Component)]
pub(crate) struct EmberDragValue {
    step: f32,
    text: Entity,
    /// Bottom slider track (the faint full-width line); `None` for the flat
    /// variant. Shown only while a [`DragRange`] is present.
    track: Option<Entity>,
    /// The filled portion of the bottom slider (accent color); its width tracks
    /// the value's fraction within the range.
    fill: Option<Entity>,
    /// The round grabber riding the track at the value's fraction.
    handle: Option<Entity>,
    last_x: Option<f32>,
    /// Cursor X at press start — used to tell a click (edit) from a drag (scrub).
    press_x: Option<f32>,
    /// Whether the current press has moved past the click threshold.
    moved: bool,
    /// The current press began on the bottom slider rail, so it sets the value
    /// absolutely from the cursor's position across the track (a fast min→max
    /// sweep) instead of the number area's fine relative scrub.
    rail_drag: bool,
    /// Keyboard-edit mode: the field shows `buffer` and accepts typed digits.
    editing: bool,
    buffer: String,
    /// Just entered edit by a click — the whole value reads as "selected", so the
    /// first keystroke (or Delete/Backspace) replaces it wholesale (Godot-style).
    select_all: bool,
    /// Full-field highlight shown while `select_all` (the "everything selected"
    /// look); `None` for the flat variant.
    highlight: Option<Entity>,
    /// A thin vertical-line text caret, shown while editing (and not selected) so
    /// an emptied field reads as a cursor rather than a lone axis label.
    caret: Entity,
}

/// Optional inclusive clamp for a [`drag_value`]. Insert alongside the widget
/// to bound its scrub range (matches egui's `DragValue::range`). When present it
/// also lights up the bottom slider line (Godot-style).
#[derive(Component, Clone, Copy)]
pub struct DragRange {
    pub min: f32,
    pub max: f32,
}

/// How far (px) the cursor may travel during a press and still count as a click
/// (which enters keyboard edit) rather than a drag (which scrubs).
const CLICK_SLOP: f32 = 3.0;
/// Bottom band (logical px) of a boxed field that counts as the slider rail — a
/// press here drives the value absolutely (the quick min→max sweep).
const RAIL_PX: f32 = 6.0;

/// Settings for the drag-value widget. `rail_quick_drag` toggles the boxed
/// field's bottom-rail "sweep": when on, a press on the bottom slider rail sets
/// the value absolutely (fast min→max) instead of the fine relative scrub the
/// number area does. On by default; the editor exposes a Settings toggle that
/// drives this resource.
#[derive(Resource)]
pub struct DragValueConfig {
    pub rail_quick_drag: bool,
}

impl Default for DragValueConfig {
    fn default() -> Self {
        Self { rail_quick_drag: true }
    }
}

/// Map a press's normalized X within the box to a range value, accounting for
/// the track's 4px side insets. Returns `None` for a degenerate width.
fn rail_value(norm_x: f32, computed: &bevy::ui::ComputedNode, r: &DragRange) -> Option<f32> {
    let w = computed.size().x * computed.inverse_scale_factor();
    if w <= 8.0 {
        return None;
    }
    let f = (((norm_x * w) - 4.0) / (w - 8.0)).clamp(0.0, 1.0);
    Some(r.min + f * (r.max - r.min))
}

/// Set true for the frame the wheel turns *and* a value field owns the gesture,
/// so the scroll area beneath swallows the wheel. Read by
/// [`super::scroll_area::scroll_wheel`].
#[derive(Resource, Default)]
pub(crate) struct WheelOverDragValue(pub bool);

/// True whenever any [`drag_value`] field is in keyboard-edit mode (the user
/// clicked it and is typing a number).
///
/// A drag-value is its own widget and does **not** share [`super::EmberTextInput`],
/// so the editor's input-focus tracker — which otherwise only watches text
/// inputs — can't see that a numeric field is being typed into. Without this,
/// typing digits into an inspector field also fires global keyboard shortcuts
/// (the numpad camera view-angle keys, gizmo G/R/S, …). The focus tracker reads
/// this resource and treats an editing drag-value as "keyboard is taken".
#[derive(Resource, Default)]
pub struct AnyDragValueEditing(pub bool);

/// Keep [`AnyDragValueEditing`] in sync with whether any field is editing.
pub(crate) fn track_drag_value_editing(
    values: Query<&EmberDragValue>,
    mut editing: ResMut<AnyDragValueEditing>,
) {
    let any = values.iter().any(|dv| dv.editing);
    if editing.0 != any {
        editing.0 = any;
    }
}

/// Tracks which "thing" the current scroll gesture belongs to. A gesture is owned
/// by a value field only if the cursor was over one when it *started*; mid-gesture
/// the owner sticks, so a panel scroll that drifts across a field keeps scrolling
/// the panel instead of snagging on the field.
#[derive(Resource, Default)]
pub(crate) struct WheelGesture {
    /// `Time::elapsed_secs` of the last wheel event.
    last_t: f32,
    /// Whether the in-progress gesture started over a value field.
    field_owned: bool,
}

/// Idle gap (seconds) between wheel events that ends a gesture. The next event
/// past this gap re-decides ownership from where the cursor is then.
const GESTURE_GAP: f32 = 0.18;

/// A scrubbable numeric field with the standard inset (dark box + border) look.
/// `axis` is an optional colored prefix (e.g. "X").
///
/// The live value lives in `Bound<f32>`, so `bind_2way` can drive it both ways;
/// insert a [`DragRange`] to clamp the scrub and show the bottom slider line.
pub fn drag_value(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    axis: &str,
    axis_color: (u8, u8, u8),
    value: f32,
    step: f32,
) -> Entity {
    drag_value_impl(commands, font, axis, axis_color, value, step, false)
}

/// Like [`drag_value`] but *flat*: no inset box, no border, transparent
/// background — for inline use on a toolbar pill where the number should blend
/// into the surrounding fill rather than read as its own dark field.
pub fn drag_value_flat(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    axis: &str,
    axis_color: (u8, u8, u8),
    value: f32,
    step: f32,
) -> Entity {
    drag_value_impl(commands, font, axis, axis_color, value, step, true)
}

#[allow(clippy::too_many_arguments)]
fn drag_value_impl(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    axis: &str,
    axis_color: (u8, u8, u8),
    value: f32,
    step: f32,
    flat: bool,
) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(if flat { 44.0 } else { 58.0 }),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                border: if flat {
                    UiRect::all(Val::Px(0.0))
                } else {
                    UiRect::all(Val::Px(1.0))
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                // No clip: the round grabber rides the rail and hangs slightly
                // below the box, so it must be free to overflow.
                ..default()
            },
            BackgroundColor(if flat { Color::NONE } else { rgb(popup_bg()) }),
            Interaction::default(),
            // Lets the drag system tell a press on the bottom slider rail (an
            // absolute min→max sweep) from one on the number area (fine scrub).
            bevy::ui::RelativeCursorPosition::default(),
            // Scrub affordance on hover; flips to the text I-beam while editing.
            crate::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
            Name::new("drag-value"),
        ))
        .id();
    // The boxed look is owned by the `Styled` style system (so it repaints with
    // the theme); the flat variant deliberately opts out so nothing repaints a
    // background over it.
    if !flat {
        commands
            .entity(box_e)
            .insert((BorderColor::all(rgb(border())), Styled::new(Role::Input)));
    }
    let text = text_node(commands, font, &format_num(value), 12.0, text_primary());

    // Godot-style bottom slider: a faint track inset off the rounded corners,
    // an accent fill, and a round grabber riding the fill's end. Only the boxed
    // variant draws it; it's hidden until a `DragRange` is seen.
    let (track, fill, handle) = if flat {
        (None, None, None)
    } else {
        let fill_e = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    height: Val::Percent(100.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("drag-value-fill"),
            ))
            .id();
        let handle_e = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    // Center an 8px circle on the 2px rail (rail center sits 1px
                    // up): vertically bottom = 1 - 4, horizontally margin -4.
                    bottom: Val::Px(-3.0),
                    margin: UiRect::left(Val::Px(-4.0)),
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                BorderColor::all(rgb(text_primary())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("drag-value-handle"),
            ))
            .id();
        let track_e = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    // Inset off the box's 4px corner radius so the rail and
                    // grabber clear the rounded corners.
                    left: Val::Px(4.0),
                    right: Val::Px(4.0),
                    bottom: Val::Px(0.0),
                    height: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(rgb(border()).with_alpha(0.6)),
                bevy::ui::FocusPolicy::Pass,
                Name::new("drag-value-track"),
            ))
            .id();
        commands.entity(track_e).add_children(&[fill_e, handle_e]);
        (Some(track_e), Some(fill_e), Some(handle_e))
    };

    // Full-field selection highlight (behind the text), shown only while the
    // freshly-clicked value reads as "selected". Hidden by default.
    let highlight = if flat {
        None
    } else {
        Some(
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        display: Display::None,
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(accent()).with_alpha(0.35)),
                    bevy::ui::FocusPolicy::Pass,
                    Name::new("drag-value-highlight"),
                ))
                .id(),
        )
    };

    // A thin vertical-line caret to the right of the value while editing.
    let caret = commands
        .spawn((
            Node {
                width: Val::Px(2.0),
                height: Val::Px(14.0),
                margin: UiRect::left(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("drag-value-caret"),
        ))
        .id();

    let mut kids = Vec::new();
    // Highlight first so it paints behind the number and the slider rail.
    if let Some(h) = highlight {
        kids.push(h);
    }
    if !axis.is_empty() {
        kids.push(text_node(commands, font, axis, 11.0, axis_color));
    }
    kids.push(text);
    kids.push(caret);
    if let Some(track_e) = track {
        kids.push(track_e);
    }
    commands.entity(box_e).insert((
        EmberDragValue {
            step,
            text,
            track,
            fill,
            handle,
            last_x: None,
            press_x: None,
            moved: false,
            rail_drag: false,
            editing: false,
            buffer: String::new(),
            select_all: false,
            highlight,
            caret,
        },
        Bound::<f32>(value),
    ));
    commands.entity(box_e).add_children(&kids);
    box_e
}

/// A full-precision, round-trippable string for the edit buffer (so committing
/// an untouched field doesn't snap it to the 1-decimal display rounding).
fn edit_string(v: f32) -> String {
    format!("{v}")
}

/// Parse the edit buffer into a (range-clamped) value.
fn parse_commit(buffer: &str, range: Option<&DragRange>) -> Option<f32> {
    let mut v: f32 = buffer.trim().parse().ok()?;
    if let Some(r) = range {
        v = v.clamp(r.min, r.max);
    }
    Some(v)
}

/// Value change per wheel notch: 1% of the range for a bounded field, else a
/// step scaled off the drag speed (so the notch is always perceptible).
fn wheel_step(step: f32, range: Option<&DragRange>) -> f32 {
    match range {
        Some(r) => ((r.max - r.min) / 100.0).max(1e-4),
        None => (step * 10.0).max(0.001),
    }
}

/// Hover over a field → highlight its border (accent) and let **Shift+wheel**
/// nudge the value. A plain wheel is left for the panel scrollbar: scrolling past
/// a field would otherwise snag on it and change the value, which reads as the
/// field "fighting" the scroll. Requiring Shift makes the panel always win and
/// the scrub an explicit, opt-in gesture. The scrub is swallowed from the panel
/// (via [`WheelOverDragValue`]) only while a field actually owns the gesture —
/// i.e. a Shift+scroll that began over a field; mid-gesture the owner sticks so a
/// drift across another field doesn't hand off.
pub(crate) fn drag_value_scroll(
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut capture: ResMut<WheelOverDragValue>,
    mut gesture: ResMut<WheelGesture>,
    mut values: Query<(
        &Interaction,
        &EmberDragValue,
        &mut Bound<f32>,
        Option<&DragRange>,
        Option<&mut Styled>,
    )>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }

    // Pass 1: hover → accent-border highlight; note whether any field is hovered.
    let mut over_any = false;
    for (interaction, dv, _bound, _range, styled) in &mut values {
        let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        if hovered {
            over_any = true;
        }
        // A hovered (or dragging) field lights the accent border — unless it's
        // mid-edit, which already owns the Active state.
        if !dv.editing {
            if let Some(mut s) = styled {
                let want = if hovered {
                    WidgetState::Active
                } else {
                    WidgetState::Normal
                };
                if s.state != want {
                    s.state = want;
                }
            }
        }
    }

    // Decide whether a field owns this wheel gesture (sticky for the gesture's
    // duration; re-decided only after an idle gap). A field can only own the
    // gesture while Shift is held — without it the wheel belongs to the panel.
    let mut field_scrub = false;
    if dy != 0.0 {
        let now = time.elapsed_secs();
        if now - gesture.last_t > GESTURE_GAP {
            gesture.field_owned = shift && over_any; // new gesture: owner = where it began
        }
        gesture.last_t = now;
        field_scrub = shift && gesture.field_owned && over_any;
    }

    // Pass 2: only if a field owns the gesture, scrub the hovered field(s).
    if field_scrub {
        for (interaction, dv, mut bound, range, _) in &mut values {
            let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
            if hovered && !dv.editing {
                let mut v = bound.0 + dy * wheel_step(dv.step, range);
                if let Some(r) = range {
                    v = v.clamp(r.min, r.max);
                }
                if v != bound.0 {
                    bound.0 = v;
                }
            }
        }
    }

    capture.0 = field_scrub;
}

/// Drag → update the model (`Bound<f32>`, clamped by an optional [`DragRange`]).
/// A press that never moves past [`CLICK_SLOP`] is a click and enters edit mode.
pub(crate) fn drag_value_drag(
    windows: Query<&Window>,
    config: Option<Res<DragValueConfig>>,
    mut values: Query<(
        &Interaction,
        &mut EmberDragValue,
        &mut Bound<f32>,
        Option<&DragRange>,
        Option<&mut Styled>,
        &mut crate::cursor_icon::HoverCursor,
        &bevy::ui::RelativeCursorPosition,
        &bevy::ui::ComputedNode,
    )>,
) {
    let cursor_x = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|p| p.x);
    // On unless a Settings toggle turns it off.
    let rail_enabled = config.as_ref().map(|c| c.rail_quick_drag).unwrap_or(true);
    for (interaction, mut dv, mut bound, range, styled, mut cursor, rel, computed) in &mut values {
        if dv.editing {
            continue;
        }
        if *interaction == Interaction::Pressed {
            match dv.last_x {
                None => {
                    dv.last_x = cursor_x;
                    dv.press_x = cursor_x;
                    dv.moved = false;
                    // Rail sweep: a boxed, ranged field whose press lands in the
                    // bottom rail band drives the value absolutely from here on.
                    dv.rail_drag = false;
                    if rail_enabled && dv.handle.is_some() {
                        if let (Some(r), Some(norm)) = (range, rel.normalized) {
                            let h = computed.size().y * computed.inverse_scale_factor();
                            if (1.0 - norm.y) * h <= RAIL_PX {
                                dv.rail_drag = true;
                                dv.moved = true; // acts immediately; never edits
                                if let Some(v) = rail_value(norm.x, computed, r) {
                                    if v != bound.0 {
                                        bound.0 = v;
                                    }
                                }
                            }
                        }
                    }
                }
                Some(last) => {
                    if let Some(cx) = cursor_x {
                        if dv.rail_drag {
                            // Absolute: track the cursor across the rail (clamped
                            // to min/max even when dragged past the box edges).
                            if let (Some(r), Some(norm)) = (range, rel.normalized) {
                                if let Some(v) = rail_value(norm.x, computed, r) {
                                    if v != bound.0 {
                                        bound.0 = v;
                                    }
                                }
                            }
                            dv.last_x = Some(cx);
                        } else {
                            if let Some(px) = dv.press_x {
                                if (cx - px).abs() > CLICK_SLOP {
                                    dv.moved = true;
                                }
                            }
                            if dv.moved {
                                let delta = cx - last;
                                if delta != 0.0 {
                                    let mut v = bound.0 + delta * dv.step;
                                    if let Some(r) = range {
                                        v = v.clamp(r.min, r.max);
                                    }
                                    if v != bound.0 {
                                        bound.0 = v;
                                    }
                                }
                            }
                            dv.last_x = Some(cx);
                        }
                    }
                }
            }
        } else if dv.last_x.is_some() {
            // Released. A press that never moved (and wasn't a rail sweep) is a
            // click → enter edit mode with the whole value selected.
            if !dv.moved && !dv.rail_drag {
                dv.editing = true;
                dv.buffer = edit_string(bound.0);
                dv.select_all = true;
                cursor.0 = SystemCursorIcon::Text;
                if let Some(mut s) = styled {
                    s.state = WidgetState::Active;
                }
            }
            dv.last_x = None;
            dv.press_x = None;
            dv.moved = false;
            dv.rail_drag = false;
        }
    }
}

/// What a frame of input did to an editing field.
enum EditAction {
    None,
    Live,
    Commit,
    Cancel,
}

/// Keyboard edit: type into the focused field; `Enter`/click-away commits,
/// `Esc` cancels.
pub(crate) fn drag_value_edit(
    mut keys: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut values: Query<(
        &Interaction,
        &mut EmberDragValue,
        &mut Bound<f32>,
        Option<&DragRange>,
        Option<&mut Styled>,
        &mut crate::cursor_icon::HoverCursor,
    )>,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node>,
) {
    // Collect this frame's key presses once (the query is iterated per-field).
    let mut typed: Vec<Key> = Vec::new();
    for ev in keys.read() {
        if ev.state == ButtonState::Pressed {
            typed.push(ev.logical_key.clone());
        }
    }
    let clicked = mouse.just_pressed(MouseButton::Left);

    for (interaction, mut dv, mut bound, range, styled, mut cursor) in &mut values {
        if !dv.editing {
            continue;
        }

        // A press anywhere but this field commits and blurs (off-click dismiss).
        let action = if clicked && *interaction != Interaction::Pressed {
            EditAction::Commit
        } else {
            let mut a = EditAction::None;
            for key in &typed {
                match key {
                    Key::Character(s) => {
                        // First keystroke over a fully-selected value replaces it.
                        if dv.select_all {
                            dv.buffer.clear();
                            dv.select_all = false;
                        }
                        for c in s.chars() {
                            if c.is_ascii_digit()
                                || matches!(c, '.' | '-' | '+' | 'e' | 'E')
                            {
                                dv.buffer.push(c);
                            }
                        }
                        a = EditAction::Live;
                    }
                    // Delete clears the field; Backspace clears it when everything
                    // is selected, else removes the last character.
                    Key::Delete => {
                        dv.buffer.clear();
                        dv.select_all = false;
                        a = EditAction::Live;
                    }
                    Key::Backspace => {
                        if dv.select_all {
                            dv.buffer.clear();
                            dv.select_all = false;
                        } else {
                            dv.buffer.pop();
                        }
                        a = EditAction::Live;
                    }
                    Key::Enter => {
                        a = EditAction::Commit;
                        break;
                    }
                    Key::Escape => {
                        a = EditAction::Cancel;
                        break;
                    }
                    _ => {}
                }
            }
            a
        };

        match action {
            EditAction::None => {}
            EditAction::Live => {
                let buf = dv.buffer.clone();
                if let Ok(mut t) = texts.get_mut(dv.text) {
                    *t = Text::new(buf);
                }
            }
            EditAction::Commit | EditAction::Cancel => {
                if matches!(action, EditAction::Commit) {
                    if let Some(v) = parse_commit(&dv.buffer, range) {
                        if v != bound.0 {
                            bound.0 = v;
                        }
                    }
                }
                dv.editing = false;
                dv.select_all = false;
                dv.buffer.clear();
                cursor.0 = SystemCursorIcon::EwResize;
                if let Some(mut s) = styled {
                    s.state = WidgetState::Normal;
                }
                let val = bound.0;
                if let Ok(mut t) = texts.get_mut(dv.text) {
                    *t = Text::new(format_num(val));
                }
            }
        }

        // Sync the full-field selection highlight + the caret to the final state.
        // While everything is selected the highlight shows (no caret); once the
        // selection is replaced the caret takes over.
        if let Some(h) = dv.highlight {
            if let Ok(mut n) = nodes.get_mut(h) {
                let d = if dv.editing && dv.select_all {
                    Display::Flex
                } else {
                    Display::None
                };
                if n.display != d {
                    n.display = d;
                }
            }
        }
        if let Ok(mut n) = nodes.get_mut(dv.caret) {
            let d = if dv.editing && !dv.select_all {
                Display::Flex
            } else {
                Display::None
            };
            if n.display != d {
                n.display = d;
            }
        }
    }
}

/// Show the round grabber only when rail-sweep is enabled (and the field has a
/// range to sweep) — with the setting off the handle is non-functional, so it's
/// hidden, leaving just the fill line as a value indicator. Cheap, guarded
/// writes; runs every frame so both the live toggle and freshly-built fields
/// pick up the current setting.
pub(crate) fn drag_value_handle_vis(
    config: Option<Res<DragValueConfig>>,
    values: Query<(&EmberDragValue, Option<&DragRange>)>,
    mut nodes: Query<&mut Node>,
) {
    let on = config.as_ref().map(|c| c.rail_quick_drag).unwrap_or(true);
    for (dv, range) in &values {
        if let Some(handle) = dv.handle {
            if let Ok(mut n) = nodes.get_mut(handle) {
                let want = if on && range.is_some() {
                    Display::Flex
                } else {
                    Display::None
                };
                if n.display != want {
                    n.display = want;
                }
            }
        }
    }
}

/// Model (`Bound<f32>`) → displayed text + the bottom slider line (drag or
/// external `bind_2way` push). The slider line shows only for ranged fields.
pub(crate) fn drag_value_apply(
    values: Query<(&EmberDragValue, &Bound<f32>, Option<&DragRange>), Changed<Bound<f32>>>,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node>,
) {
    for (dv, b, range) in &values {
        // While typing, the text mirrors the buffer (owned by the edit system).
        if !dv.editing {
            if let Ok(mut text) = texts.get_mut(dv.text) {
                *text = Text::new(format_num(b.0));
            }
        }
        if let (Some(track), Some(fill), Some(handle)) = (dv.track, dv.fill, dv.handle) {
            match range {
                Some(r) => {
                    if let Ok(mut n) = nodes.get_mut(track) {
                        n.display = Display::Flex;
                    }
                    let f = ((b.0 - r.min) / (r.max - r.min).max(1e-4)).clamp(0.0, 1.0);
                    if let Ok(mut n) = nodes.get_mut(fill) {
                        n.width = Val::Percent(f * 100.0);
                    }
                    if let Ok(mut n) = nodes.get_mut(handle) {
                        n.left = Val::Percent(f * 100.0);
                    }
                }
                None => {
                    if let Ok(mut n) = nodes.get_mut(track) {
                        n.display = Display::None;
                    }
                }
            }
        }
    }
}
