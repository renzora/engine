//! One row per field: the label column, the widget for its
//! [`FieldKind`](super::spec::FieldKind), and the reset / keyframe affordances
//! beside it.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor_framework::FieldValue;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text};
use renzora_ember::widgets::{
    bind_text_input, drag_value, dropdown, text_input, toggle_switch, DragRange,
};

use super::assets::build_asset_field;
use super::section::{AddKeyframeBtn, FieldButton, ResetBtn};
use super::spec::{field_label_loc, format_value, tracked_read, FieldInit, FieldKind, FieldSpec};
use super::{c, phosphor_glyph, record_field_change, GetFn, SetFn};

pub(super) fn build_field_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
    type_id: &'static str,
) -> Entity {
    // The field's control(s) sit in a value container, then the shared
    // `inspector_row` adds a left-aligned label column — so declarative fields
    // and native drawers (which also use `inspector_row`) line up identically.
    let value = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("field-value"),
        ))
        .id();
    build_field_value(commands, fonts, field, entity, value);
    // Growable controls (drag values, text inputs, dropdowns, asset slots) are
    // stretched by `fill_control` inside `build_field_value`, which pushes the
    // trailing keyframe/reset buttons to the row's right edge. Controls with an
    // intrinsic size (toggle, color swatch, read-only text) can't stretch, so a
    // spacer absorbs the free width instead — the buttons stay pinned right at
    // a fixed size either way, however the panel is resized.
    if matches!(
        field.kind,
        FieldKind::Bool | FieldKind::Color | FieldKind::ColorRgba | FieldKind::ReadOnly
    ) {
        let spacer = commands
            .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
            .id();
        commands.entity(value).add_child(spacer);
    }
    // A per-field "add keyframe" affordance, left of the reset button. Reactively
    // hidden unless the timeline has a clip open with a bound track for this
    // property (see `build_add_keyframe_button`); pressing it keys the live value.
    if let Some((component, field_path)) = field_anim_path(type_id, field.name, field.kind) {
        let kf = build_add_keyframe_button(commands, fonts, entity, component, field_path);
        commands.entity(value).add_child(kf);
    }
    // A per-field "reset to default" affordance, right of the editable widget(s).
    // Skipped for kinds that have no value to reset (action buttons, read-only
    // text) — resetting those would be meaningless.
    if field_is_resettable(field.kind) {
        let reset = build_reset_button(commands, fonts, field.name, field.get_fn.clone(), field.set_fn.clone(), entity);
        commands.entity(value).add_child(reset);
    }
    let label = field_label_loc(field.name);
    renzora_ember::inspector::inspector_row(commands, &fonts.ui, &label, value)
}

/// Whether a field carries an editable value worth a reset button. `Button` is a
/// fire-and-forget action and `ReadOnly` can't be edited, so neither gets one.
fn field_is_resettable(kind: FieldKind) -> bool {
    !matches!(kind, FieldKind::Button { .. } | FieldKind::ReadOnly)
}

/// A small icon button that resets a field to its type-appropriate default
/// (via [`FieldValue::type_default`]). Reads the field's current value only to
/// learn its `FieldValue` variant, then writes the matching default back; the
/// field's two-way binding refreshes the widget on the next frame.
fn build_reset_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field_name: &'static str,
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ResetBtn { get_fn, set_fn, entity, field_name },
            Name::new("field-reset"),
        ))
        .id();
    let glyph = phosphor_glyph(
        commands,
        fonts,
        "arrow-counter-clockwise",
        renzora_ember::theme::text_muted(),
        11.0,
    );
    commands.entity(btn).add_child(glyph);
    btn
}

/// Guess the `(component, field)` reflection path an inspector row animates, for
/// matching against the open clip's property tracks. `type_id` is already the
/// reflected component short-name; the field path is the display name reversed to
/// snake_case (the `Inspectable` derive title-cases the field ident, so
/// lowercasing + underscoring recovers it) — except Transform, whose hand-written
/// labels ("Position") differ from the animated channels ("translation"). Returns
/// `None` for non-animatable kinds (text/asset/enum/button/read-only). Wrong
/// guesses are harmless: they just never match a track, so no button shows.
fn field_anim_path(type_id: &str, field_name: &str, kind: FieldKind) -> Option<(String, String)> {
    if !matches!(
        kind,
        FieldKind::Float { .. }
            | FieldKind::Int { .. }
            | FieldKind::Vec3 { .. }
            | FieldKind::Bool
            | FieldKind::Color
            | FieldKind::ColorRgba
            | FieldKind::DynamicEnum
    ) {
        return None;
    }
    // The "Sprite Image" section aggregates fields that animate *different*
    // components than its `type_id`: the `Image` dropdown → `SpriteImages.index`
    // (switchable sheet), and the merged-in grid → `SpriteSheet.{h,v}frames` /
    // `frame`. Map them explicitly (as with Transform). The single-image asset
    // slot is `Asset` kind and already bailed above as non-animatable.
    if type_id == "sprite_image" {
        match field_name {
            "Image" => return Some(("SpriteImages".to_string(), "index".to_string())),
            "H Frames" => return Some(("SpriteSheet".to_string(), "hframes".to_string())),
            "V Frames" => return Some(("SpriteSheet".to_string(), "vframes".to_string())),
            "Frame" => return Some(("SpriteSheet".to_string(), "frame".to_string())),
            _ => {}
        }
    }
    let field = if type_id == "transform" {
        match field_name {
            "Position" => "translation",
            "Rotation" => "rotation",
            "Scale" => "scale",
            _ => return None,
        }
        .to_string()
    } else {
        field_name.trim().to_lowercase().replace(' ', "_")
    };
    Some((type_id.to_string(), field))
}

/// A small per-field "add keyframe" button (a keyframe diamond, matching the
/// timeline's add-key glyph). Hidden by default and shown reactively while the
/// timeline has a clip open on the inspected entity — see
/// [`renzora::ActiveTimeline::animates`]. Pressing it queues a
/// [`renzora::KeyframeRequests`] entry that the timeline editor keys at the
/// playhead from the entity's live value, creating the track first if this field
/// isn't animated yet.
fn build_add_keyframe_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    component: String,
    field: String,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                // Start hidden; `bind_display` reveals it on the next reaction
                // frame if the timeline is animating this entity (avoids a
                // one-frame flash on rows built while no clip is open).
                display: Display::None,
                ..default()
            },
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            AddKeyframeBtn { entity, component, field },
            Name::new("field-add-keyframe"),
        ))
        .id();
    bind_display(commands, btn, move |w| {
        w.get_resource::<renzora::ActiveTimeline>()
            .is_some_and(|t| t.animates(entity))
    });
    // Amber diamond — the timeline's keyframe color, so the affordance reads as
    // "add a keyframe" rather than another neutral inspector control.
    let glyph = phosphor_glyph(commands, fonts, "diamond", (230, 170, 90), 11.0);
    commands.entity(btn).add_child(glyph);
    btn
}

fn build_field_value(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
    value_parent: Entity,
) {
    // Which component this field reads, for the dependency the value closures
    // declare — see [`tracked_read`].
    let cid = field.cid;
    match field.kind {
        FieldKind::Float { speed, min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, speed.max(0.001));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_2way(
                commands,
                dv,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Float(*v)),
            );
            renzora_ember::inspector::fill_control(commands, dv);
            commands.entity(value_parent).add_child(dv);
        }
        FieldKind::Int { min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            // Quarter-unit-per-pixel scrub (4 px per whole step) with the model
            // snapped to integers — the snap is what stops the rounded set_fn
            // read-back from fighting the drag.
            let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, 0.25);
            commands.entity(dv).insert(renzora_ember::widgets::DragSnap(1.0));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_2way(
                commands,
                dv,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Float(*v)),
            );
            renzora_ember::inspector::fill_control(commands, dv);
            commands.entity(value_parent).add_child(dv);
        }
        FieldKind::Vec3 { speed } => {
            let init = if let FieldInit::Vec3(a) = field.init {
                a
            } else {
                [0.0; 3]
            };
            const AXES: [(&str, (u8, u8, u8)); 3] = [
                ("X", (230, 90, 90)),
                ("Y", (130, 200, 90)),
                ("Z", (90, 150, 230)),
            ];
            for (i, (axis, color)) in AXES.iter().enumerate() {
                let dv = drag_value(commands, &fonts.ui, axis, *color, init[i], speed.max(0.001));
                let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
                let get_r = get_fn.clone();
                bind_2way(
                    commands,
                    dv,
                    move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                        Some(FieldValue::Vec3(a)) => a[i],
                        _ => 0.0,
                    },
                    move |w, v: &f32| {
                        if let Some(FieldValue::Vec3(mut a)) = get_fn(w, entity) {
                            a[i] = *v;
                            record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Vec3(a));
                        }
                    },
                );
                renzora_ember::inspector::fill_control(commands, dv);
                commands.entity(value_parent).add_child(dv);
            }
        }
        FieldKind::Bool => {
            let init = matches!(field.init, FieldInit::Bool(true));
            let sw = toggle_switch(commands, init);
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_2way(
                commands,
                sw,
                move |w| matches!(tracked_read(w, entity, cid, |world| get_r(world, entity)), Some(FieldValue::Bool(true))),
                move |w, v: &bool| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Bool(*v)),
            );
            commands.entity(value_parent).add_child(sw);
        }
        FieldKind::Color => {
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            let editor = renzora_ember::inspector::color_field(
                commands,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Color(c)) => c,
                    _ => [0.0; 3],
                },
                move |w, rgb: [f32; 3]| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Color(rgb)),
            );
            commands.entity(value_parent).add_child(editor);
        }
        FieldKind::ColorRgba => {
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            let editor = renzora_ember::inspector::color_field_rgba(
                commands,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::ColorRgba(c)) => c,
                    _ => [0.0; 4],
                },
                move |w, rgba: [f32; 4]| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::ColorRgba(rgba)),
            );
            commands.entity(value_parent).add_child(editor);
        }
        FieldKind::Text => {
            let init = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let ti = text_input(commands, &fonts.ui, "—", &init);
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_text_input(
                commands,
                ti,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::String(s)) => s,
                    _ => String::new(),
                },
                move |w, v: String| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::String(v)),
            );
            renzora_ember::inspector::fill_control(commands, ti);
            commands.entity(value_parent).add_child(ti);
        }
        FieldKind::Enum { options } => {
            // Use the shared ember `dropdown` (position-aware — flips up near a
            // panel/window bottom) rather than a bespoke inspector popup, so enum
            // fields get the same behaviour as every other dropdown.
            let refs: Vec<&str> = options.to_vec();
            let cur = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let sel = options.iter().position(|o| *o == cur).unwrap_or(0);
            let dd = dropdown(commands, fonts, &refs, sel);
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            // The dropdown works in option indices; the field stores an enum
            // string, so translate both ways.
            bind_2way(
                commands,
                dd,
                move |w| {
                    let cur = match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                        Some(FieldValue::Enum(s)) => s,
                        _ => String::new(),
                    };
                    options.iter().position(|o| *o == cur).unwrap_or(0)
                },
                move |w, i: &usize| {
                    if let Some(opt) = options.get(*i) {
                        record_field_change(
                            w,
                            entity,
                            name,
                            get_fn.clone(),
                            set_fn.clone(),
                            FieldValue::Enum((*opt).to_string()),
                        );
                    }
                },
            );
            renzora_ember::inspector::fill_control(commands, dd);
            commands.entity(value_parent).add_child(dd);
        }
        FieldKind::DynamicEnum => {
            let (options, selected) = if let FieldInit::DynEnum(ref o, s) = field.init {
                (o.clone(), s)
            } else {
                (Vec::new(), 0)
            };
            let refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            let sel = selected.min(refs.len().saturating_sub(1));
            let dd = dropdown(commands, fonts, &refs, sel);
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            // The value is the selected index; two-way bind so a keyframed /
            // externally-changed index updates the shown option and vice versa.
            bind_2way(
                commands,
                dd,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Float(v)) => v.round().max(0.0) as usize,
                    _ => 0,
                },
                move |w, i: &usize| {
                    record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Float(*i as f32));
                },
            );
            renzora_ember::inspector::fill_control(commands, dd);
            commands.entity(value_parent).add_child(dd);
        }
        FieldKind::Asset => {
            let f = build_asset_field(
                commands,
                fonts,
                entity,
                field.name,
                field.get_fn.clone(),
                field.set_fn.clone(),
                field.extensions.clone(),
                field.create_fn.clone(),
            );
            commands.entity(value_parent).add_child(f);
        }
        FieldKind::Button { icon } => {
            let btn_label = field_label_loc(field.name);
            let btn = renzora_ember::widgets::icon_label_button(commands, fonts, icon, &btn_label);
            commands.entity(btn).insert((
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                FieldButton {
                    set_fn: field.set_fn.clone(),
                    entity,
                },
            ));
            commands.entity(value_parent).add_child(btn);
        }
        FieldKind::ReadOnly => {
            let text = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let t = commands
                .spawn((
                    Text::new(text),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(c(renzora_ember::theme::text_muted())),
                ))
                .id();
            // `ReadOnly` was the ONE field kind with no binding: its value was
            // formatted once at `collect_sections` time and baked into the `Text`.
            // It only *appeared* to stay fresh because the inspector's global
            // signature rebuilds the whole panel so often — accidental
            // reactivity, not real. Anything whose displayed value changes without
            // the component set changing (a mesh's vertex count, a camera's
            // computed projection, a resolved asset path) was already able to go
            // stale, and would freeze outright once rebuilds become granular.
            //
            // `ReadOnly` is also the catch-all arm of `#[derive(Inspectable)]`
            // (`renzora_macros/src/inspectable.rs`), so this is the common case for
            // any field type the derive can't infer — not a corner.
            //
            // One-way `bind_text`: there is no editing to conflict with, so unlike
            // the `bind_2way` fields there's no focus or in-progress drag to
            // destroy by writing to it.
            let get = field.get_fn.clone();
            bind_text(commands, t, move |w| format_value((get)(w.untracked(), entity).as_ref()));
            commands.entity(value_parent).add_child(t);
        }
    }
}
