//! The inspector's entity header: the fixed row at the top of the panel that
//! carries everything belonging to the *entity* rather than to any component on
//! it: its icon and hierarchy label colour, its name, what kind of thing it is,
//! its visibility, and the inspector lock.
//!
//! Why these left the component list. All of them used to render as registry
//! sections ("ID" with two field rows, "Visibility" with one), which put them in
//! a list whose whole grammar is add / remove / collapse / enable, and not one
//! of them can be added or removed. As one row they read as what they are: the
//! identity of the thing you selected, not components it happens to have.
//!
//! The row leads with a 44px tile: the entity's icon on a wash of its label
//! colour. Clicking it opens one popover for both, the colour swatches above
//! the icon grid. The icon button and the colour dot used to be two separate
//! controls a few pixels apart, each opening its own picker, and the dot was
//! the smallest thing in the row. The name is plain text until clicked, when it
//! becomes the input that edits it: a boxed input there all the time read as a
//! form field, which is the one thing a header is not.
//!
//! Everything here is rebuilt by `rebuild_inspector` into a stable host node
//! ([`EntityHeaderHost`]), the same pattern as the page rail: the widgets'
//! bindings capture the inspected `Entity`, so they cannot outlive a selection
//! change, but the host they hang under can and does.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::{
    entity_icon_name, ComponentIconRegistry, EntityIcon, EntityLabelColor, FieldValue,
    ENTITY_ICON_CHOICES,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::{bind_bg, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, mix, panel_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{text_input, EmberTextInput, MenuAction};
use renzora_ember::UiInteraction;

use crate::panel::section::LockBtn;
use crate::panel::{c, phosphor_glyph, record_field_change, GetFn, SetFn};

/// Tint for the eye while the entity is hidden.
const ACTIVE_TINT: (u8, u8, u8) = (120, 170, 255);

/// Tint for the lock while the Inspector is pinned to this entity.
const LOCKED_TINT: (u8, u8, u8) = (248, 113, 113);

/// The icon tile's size and corner radius.
const TILE: f32 = 44.0;
const TILE_RADIUS: f32 = 10.0;

/// The colours the popover offers, in its swatch order. The same hues the
/// editor's categories use, so a label colour sits comfortably beside them.
const SWATCHES: [((u8, u8, u8), &str); 8] = [
    ((96, 165, 250), "Blue"),
    ((180, 156, 255), "Purple"),
    ((244, 114, 182), "Pink"),
    ((248, 113, 113), "Red"),
    ((255, 155, 115), "Orange"),
    ((255, 210, 120), "Yellow"),
    ((110, 231, 160), "Green"),
    ((45, 212, 191), "Teal"),
];

/// Stable host for the entity header. Repopulated by `rebuild_inspector`;
/// hidden outright when nothing is selected.
#[derive(Component)]
pub(crate) struct EntityHeaderHost;

/// The icon tile at the head of the row. Pressing it opens the icon and
/// colour popover.
#[derive(Component)]
pub(crate) struct EntityIconBtn {
    entity: Entity,
}

/// The eye toggle. A separate marker from the icon tile so a press on one can
/// never be read as a press on the other.
#[derive(Component)]
pub(crate) struct EntityVisibilityBtn {
    entity: Entity,
}

/// The name's input, always on screen. `editing` is set from the frame the
/// input takes focus until the edit is committed or cancelled.
#[derive(Component)]
pub(crate) struct EntityNameEdit {
    entity: Entity,
    editing: bool,
}

/// The rainbow swatch: shows or hides the full colour picker under the row.
#[derive(Component)]
pub(crate) struct CustomColourBtn {
    panel: Entity,
}

/// The popover's icon search box.
#[derive(Component)]
pub(crate) struct IconSearchInput;

/// One icon in the popover's grid, matched against the search box.
#[derive(Component)]
pub(crate) struct IconCell {
    search: Entity,
    label_lc: String,
}

// ── Field accessors ──────────────────────────────────────────────────────────
//
// Plain `fn(&World, Entity)` pairs rather than closures, so they coerce into the
// `GetFn`/`SetFn` the undo recorder wants and each edit lands on the undo stack
// exactly like an inspector field row does.

/// The entity's id. Sanitized and de-duplicated on write: this is the rule the
/// old "ID" field owned, and it is the reason typing here can't produce two
/// entities that scripts cannot tell apart.
fn id_get(world: &World, entity: Entity) -> Option<FieldValue> {
    world
        .get::<Name>(entity)
        .map(|n| FieldValue::String(n.as_str().to_string()))
}

fn id_set(world: &mut World, entity: Entity, val: FieldValue) {
    if let FieldValue::String(v) = val {
        let id = renzora::unique_entity_name(world, &v, entity);
        if let Some(mut n) = world.get_mut::<Name>(entity) {
            *n = Name::new(id);
        }
    }
}

fn label_color_get(world: &World, entity: Entity) -> Option<FieldValue> {
    let col = world
        .get::<EntityLabelColor>(entity)
        .map(|lc| lc.0)
        .unwrap_or([220, 222, 228]);
    Some(FieldValue::Color([
        col[0] as f32 / 255.0,
        col[1] as f32 / 255.0,
        col[2] as f32 / 255.0,
    ]))
}

fn label_color_set(world: &mut World, entity: Entity, val: FieldValue) {
    if let FieldValue::Color([r, g, b]) = val {
        let color = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8];
        world.entity_mut(entity).insert(EntityLabelColor(color));
    }
}

/// The label colour as `"#rrggbb"`, or `""` when the entity has none. The
/// swatches edit through this pair rather than [`label_color_get`], whose
/// absent case reads as a colour: undoing "None" through that one would put
/// back the default grey instead of no colour at all.
fn label_color_opt_get(world: &World, entity: Entity) -> Option<FieldValue> {
    Some(FieldValue::String(
        world
            .get::<EntityLabelColor>(entity)
            .map(|lc| format!("#{:02x}{:02x}{:02x}", lc.0[0], lc.0[1], lc.0[2]))
            .unwrap_or_default(),
    ))
}

fn label_color_opt_set(world: &mut World, entity: Entity, val: FieldValue) {
    let FieldValue::String(s) = val else { return };
    let Ok(mut em) = world.get_entity_mut(entity) else {
        return;
    };
    match parse_hex(&s) {
        Some(col) => {
            em.insert(EntityLabelColor(col));
        }
        None => {
            em.remove::<EntityLabelColor>();
        }
    }
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Read visibility through the viewport gate.
///
/// `ViewportGateHidden` means the scene is force-hidden because no viewport
/// panel is showing, and it stashes the *authored* value. Reading `Visibility`
/// directly in that state reports every entity in the scene as hidden, which
/// the old Visibility section did, so in a viewport-less layout its checkbox was
/// unanimously wrong and un-fixable by clicking it.
fn visibility_get(world: &World, entity: Entity) -> Option<FieldValue> {
    if let Some(gate) = world.get::<renzora::core::ViewportGateHidden>(entity) {
        return Some(FieldValue::Bool(gate.0 != Visibility::Hidden));
    }
    world
        .get::<Visibility>(entity)
        .map(|v| FieldValue::Bool(matches!(*v, Visibility::Inherited | Visibility::Visible)))
}

fn visibility_set(world: &mut World, entity: Entity, val: FieldValue) {
    let FieldValue::Bool(v) = val else { return };
    let next = if v {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    // Gated: write the stash, so the gate restores the user's choice when a
    // viewport comes back rather than overwriting it with the pre-gate value.
    if let Some(mut gate) = world.get_mut::<renzora::core::ViewportGateHidden>(entity) {
        gate.0 = next;
        return;
    }
    if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
        *vis = next;
    }
}

fn icon_get(world: &World, entity: Entity) -> Option<FieldValue> {
    Some(FieldValue::String(
        world
            .get::<EntityIcon>(entity)
            .map(|i| i.0.clone())
            .unwrap_or_default(),
    ))
}

/// An empty name *removes* the override rather than storing `""`, so "Auto"
/// leaves no trace in the saved scene and the entity goes back to being
/// described by its components.
fn icon_set(world: &mut World, entity: Entity, val: FieldValue) {
    let FieldValue::String(name) = val else { return };
    let Ok(mut em) = world.get_entity_mut(entity) else {
        return;
    };
    match entity_icon_name(&name) {
        Some(resolved) => {
            em.insert(EntityIcon(resolved.to_string()));
        }
        None => {
            em.remove::<EntityIcon>();
        }
    }
}

fn get(f: fn(&World, Entity) -> Option<FieldValue>) -> GetFn {
    std::sync::Arc::new(f)
}

fn set(f: fn(&mut World, Entity, FieldValue)) -> SetFn {
    std::sync::Arc::new(f)
}

/// The icon this entity draws right now: its override if it has a usable one,
/// otherwise whatever its components say it is.
///
/// Only the override is tracked reactively. The archetype half is read
/// untracked because a component appearing or disappearing already rebuilds the
/// whole inspector (this header included), so tracking it would buy nothing
/// and cost a dependency on every registered icon component.
fn effective_icon(rx: &Rx, entity: Entity) -> &'static str {
    if let Some(name) = rx
        .get::<EntityIcon>(entity)
        .and_then(|i| entity_icon_name(&i.0))
    {
        return name;
    }
    let world = rx.untracked();
    world
        .get_resource::<ComponentIconRegistry>()
        .and_then(|reg| reg.entity_icon(world, entity))
        .map(|(icon, _)| icon)
        .unwrap_or("circle")
}

/// The colour the tile is washed with: the entity's label colour, or the
/// theme accent when it has none.
fn tile_hue(rx: &Rx, entity: Entity) -> (u8, u8, u8) {
    rx.get::<EntityLabelColor>(entity)
        .map(|lc| (lc.0[0], lc.0[1], lc.0[2]))
        .unwrap_or_else(accent)
}

/// The line under the name: what kind of thing the entity is, and the model
/// file it came from when it is a model or a part of one.
///
/// The kind is the hierarchy's own type label for it (Camera, Mesh, Light),
/// so the two panels describe an entity the same way. The source is found by
/// walking up to the nearest model root, which is the one place an imported
/// model records its file: a part three levels inside a model shows that
/// model, which is the question a long list of near-identical part names
/// leaves you asking.
pub(crate) fn entity_subtitle(world: &World, entity: Entity) -> String {
    let kind = if world.get::<renzora::MeshInstanceData>(entity).is_some() {
        renzora::lang::t_or("inspector.kind.model", "Model")
    } else {
        world
            .get_resource::<ComponentIconRegistry>()
            .and_then(|reg| reg.entity_type_name(world, entity))
            .map(|k| k.to_string())
            .unwrap_or_else(|| renzora::lang::t_or("inspector.kind.empty", "Empty"))
    };
    let mut source = None;
    let mut cur = Some(entity);
    // Bounded: a hierarchy deeper than this is not a model's.
    for _ in 0..64 {
        let Some(e) = cur else { break };
        if let Some(m) = world.get::<renzora::MeshInstanceData>(e) {
            source = m.model_path.clone();
            break;
        }
        cur = world.get::<ChildOf>(e).map(|p| p.parent());
    }
    match source {
        Some(path) => format!("{kind} \u{00B7} {path}"),
        None => kind,
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

/// The stable host node, built once when the panel is registered.
pub(crate) fn build_entity_header_host(commands: &mut Commands) -> Entity {
    let host = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::new(Val::Px(12.0), Val::Px(10.0), Val::Px(12.0), Val::Px(10.0)),
                flex_shrink: 0.0,
                ..default()
            },
            EntityHeaderHost,
            Name::new("inspector-entity-header"),
        ))
        .id();
    // Always shown: with nothing selected it names the scene instead (see
    // `panel::scene_page`).
    host
}

/// Fill the header for `entity`. Returns the children for the caller to parent
/// under the host (which it has just emptied).
///
/// `has_visibility` and `subtitle` come from the caller because this side of
/// `rebuild_inspector` only holds `Commands`: the world is borrowed mutably by
/// then.
pub(crate) fn build_entity_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    has_visibility: bool,
    subtitle: &str,
) -> Vec<Entity> {
    vec![
        icon_tile(commands, fonts, entity),
        name_column(commands, fonts, entity, subtitle, has_visibility),
    ]
}

/// Whether `entity` has anything the eye toggle could act on.
pub(crate) fn has_visibility(world: &World, entity: Entity) -> bool {
    world.get::<Visibility>(entity).is_some()
        || world
            .get::<renzora::core::ViewportGateHidden>(entity)
            .is_some()
}

/// The icon tile: the entity's glyph on a wash of its label colour, with a
/// small pencil badge on its corner saying it can be changed.
fn icon_tile(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let tile = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(TILE),
                height: Val::Px(TILE),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(TILE_RADIUS)),
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(0.17)),
            UiInteraction::default(),
            BLOCK,
            // The popover is aimed from the tile's own rect, recovered from the
            // cursor and its normalized position inside the tile.
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or(
                "inspector.icon_and_colour",
                "Icon and colour",
            )),
            EntityIconBtn { entity },
            Name::new("entity-icon-tile"),
        ))
        .id();
    bind_bg(commands, tile, move |w| {
        let hovered = matches!(
            w.get::<UiInteraction>(tile),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        );
        rgb(tile_hue(w, entity)).with_alpha(if hovered { 0.26 } else { 0.17 })
    });
    let glyph = phosphor_glyph(commands, fonts, "circle", accent(), 26.0);
    bind_text(commands, glyph, move |w| {
        renzora_ember::phosphor_map::icon_glyph(effective_icon(w, entity))
            .unwrap_or('\u{E4C6}')
            .to_string()
    });
    bind_text_color(commands, glyph, move |w| rgb(tile_hue(w, entity)));

    let badge = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-4.0),
                bottom: Val::Px(-4.0),
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(mix(panel_bg(), text_primary(), 0.16)),
            BorderColor::all(rgb(panel_bg())),
            PASS_THROUGH,
            Name::new("entity-icon-badge"),
        ))
        .id();
    let pencil = phosphor_glyph(commands, fonts, "pencil-simple", text_primary(), 9.0);
    commands.entity(badge).add_child(pencil);
    commands.entity(tile).add_children(&[glyph, badge]);
    tile
}

/// The name in an always-visible input, with the kind and source line under
/// it.
///
/// It was plain text that turned into the input on click. A name that only
/// looks editable once you have found out it is gave no hint it could be
/// renamed there, and every other editor's inspector leads with a name field;
/// a box that is always there says so at a glance.
///
/// The eye and lock sit on the input's own line. As siblings of the whole
/// column they were centred on the input and subtitle together, so they hung
/// half a line below the field they act alongside.
fn name_column(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    subtitle: &str,
    has_visibility: bool,
) -> Entity {
    let col = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("entity-name-column"),
        ))
        .id();

    let input = text_input(
        commands,
        &fonts.ui,
        &renzora::lang::t_or("inspector.entity_name_placeholder", "Name"),
        "",
    );
    // `PAD_X` horizontally: the caret and click-to-caret position from it, so
    // any other padding puts the caret off the text (see its doc).
    // The text inside clips to the box (ember's text input keeps it in its own
    // clipping node), so a long generated name never runs under the eye.
    commands.entity(input).insert((
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(0.0),
            height: Val::Px(26.0),
            padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(3.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        EntityNameEdit { entity, editing: false },
        Name::new("entity-name"),
    ));

    // 13 lands on the 11px rung, two steps under the name.
    let sub = commands
        .spawn((
            Text::new(subtitle),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
            Name::new("entity-subtitle"),
        ))
        .id();
    renzora_ember::widgets::ellipsize(commands, sub, subtitle, None);

    let line = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("entity-name-line"),
        ))
        .id();
    let mut line_kids = vec![input];
    // Only entities that can actually be hidden get an eye. A `Visibility`-less
    // entity with a permanently greyed-out toggle would just be a question.
    if has_visibility {
        line_kids.push(visibility_button(commands, fonts, entity));
    }
    line_kids.push(lock_button(commands, fonts, entity));
    commands.entity(line).add_children(&line_kids);

    commands.entity(col).add_children(&[line, sub]);
    col
}

/// The eye. Hidden entities get the crossed-out glyph in the active tint, so a
/// hidden selection is visible at a glance rather than only on close reading.
fn visibility_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let eye = phosphor_glyph(commands, fonts, "eye", text_muted(), 16.0);
    commands.entity(eye).insert((
        UiInteraction::default(),
        BLOCK,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        EntityVisibilityBtn { entity },
        Node { padding: UiRect::all(Val::Px(3.0)), ..default() },
    ));
    // Reads the same gate-aware accessor the setter writes through, so the glyph
    // can never disagree with what a click would do.
    let visible_now = move |w: &Rx| {
        w.get::<renzora::core::ViewportGateHidden>(entity)
            .map(|g| g.0 != Visibility::Hidden)
            .or_else(|| w.get::<Visibility>(entity).map(|v| *v != Visibility::Hidden))
            .unwrap_or(true)
    };
    bind_text(commands, eye, move |w| {
        let name = if visible_now(w) { "eye" } else { "eye-slash" };
        renzora_ember::phosphor_map::icon_glyph(name)
            .unwrap_or('\u{E4C6}')
            .to_string()
    });
    bind_text_color(commands, eye, move |w| {
        c(if visible_now(w) { text_muted() } else { ACTIVE_TINT })
    });
    eye
}

/// The inspector lock: pin the panel to this entity so clicking elsewhere in
/// the viewport doesn't swap what you are editing.
///
/// The glyph is bound rather than baked. `NativeInspectorState::locked` reaches
/// the UI only through the inspector's global rebuild signature, so a baked
/// glyph works by accident today; under a granular rebuild the state would flip
/// with nothing on screen changing, and the button would look broken.
fn lock_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let lock = phosphor_glyph(commands, fonts, "lock-simple-open", text_muted(), 16.0);
    commands.entity(lock).insert((
        UiInteraction::default(),
        BLOCK,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        LockBtn { entity },
        Node { padding: UiRect::all(Val::Px(3.0)), ..default() },
    ));
    let locked_now = move |w: &Rx| {
        w.get_resource::<crate::panel::InspectorState>()
            .and_then(|s| s.locked)
            == Some(entity)
    };
    bind_text(commands, lock, move |w| {
        let name = if locked_now(w) {
            "lock-simple"
        } else {
            "lock-simple-open"
        };
        renzora_ember::phosphor_map::icon_glyph(name)
            .unwrap_or('\u{E4C6}')
            .to_string()
    });
    // Red while locked, not the eye's blue: a pinned Inspector ignores your
    // selection, which is the thing to notice, and blue read as "on" for an
    // ordinary toggle.
    bind_text_color(commands, lock, move |w| {
        c(if locked_now(w) { LOCKED_TINT } else { text_muted() })
    });
    lock
}

// ── Interaction ──────────────────────────────────────────────────────────────

/// Click the eye → flip visibility, undoably.
pub(crate) fn entity_visibility_click(
    q: Query<(&UiInteraction, &EntityVisibilityBtn), Changed<UiInteraction>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let entity = btn.entity;
        commands.queue(move |world: &mut World| {
            let visible = matches!(visibility_get(world, entity), Some(FieldValue::Bool(true)));
            record_field_change(
                world,
                entity,
                "Visible",
                get(visibility_get),
                set(visibility_set),
                FieldValue::Bool(!visible),
            );
        });
    }
}

/// Drive the name field. While it is not being edited it follows the entity's
/// `Name`, so a rename from the Hierarchy or a script shows up. Focusing it
/// starts an edit; Enter or focus moving elsewhere commits it as one undoable
/// rename, and Escape puts the current name back.
///
/// One input exists at a time and the follow step writes only when the name
/// actually differs, so the per-frame cost is a string compare.
pub(crate) fn entity_name_edit(
    keys: Res<ButtonInput<KeyCode>>,
    mut edits: Query<(&mut EntityNameEdit, &mut EmberTextInput)>,
    names: Query<&Name>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
) {
    for (mut edit, mut inp) in &mut edits {
        let current = names.get(edit.entity).map(|n| n.as_str().to_string()).unwrap_or_default();
        if !edit.editing {
            if inp.focused {
                edit.editing = true;
            } else {
                if inp.value != current {
                    show_name(&mut inp, &mut texts, current);
                }
                continue;
            }
        }
        let cancel = keys.just_pressed(KeyCode::Escape);
        let commit = !cancel && (keys.just_pressed(KeyCode::Enter) || !inp.focused);
        if !cancel && !commit {
            continue;
        }
        if commit {
            let value = inp.value.trim().to_string();
            if !value.is_empty() && value != current {
                let entity = edit.entity;
                commands.queue(move |w: &mut World| {
                    record_field_change(w, entity, "ID", get(id_get), set(id_set), FieldValue::String(value));
                });
            } else {
                // An empty or unchanged name is not a rename: show the real one.
                show_name(&mut inp, &mut texts, current);
            }
        } else {
            show_name(&mut inp, &mut texts, current);
        }
        inp.focused = false;
        inp.select_all = false;
        edit.editing = false;
    }
}

/// Put `name` in the name field: its value, its caret and the text it draws.
fn show_name(inp: &mut EmberTextInput, texts: &mut Query<&mut Text>, name: String) {
    inp.caret_index = name.chars().count();
    if let Ok(mut t) = texts.get_mut(inp.text_entity) {
        t.0 = name.clone();
    }
    inp.value = name;
}

/// Press the icon tile → open the icon and colour popover under it.
///
/// A screen menu root rather than a child-anchored panel: bevy_ui clips
/// absolutely-positioned children by every scrolling ancestor, and a 300 px
/// card hanging off a tile near the inspector's edge would be cut off by the
/// panel. Screen menus are never clipped and clamp themselves on-screen.
pub(crate) fn entity_icon_menu_open(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    buttons: Query<
        (
            &UiInteraction,
            &EntityIconBtn,
            &RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        Changed<UiInteraction>,
    >,
    labels: Query<&EntityLabelColor>,
    icons: Query<&EntityIcon>,
    windows: Query<&Window>,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, btn, rcp, cn)) = buttons.iter().find(|(i, ..)| **i == UiInteraction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let rect = renzora_ember::widgets::trigger_rect(cursor, rcp, cn);
    let (content, card) = renzora_ember::widgets::screen_menu_parts(
        &mut commands,
        rect.min.x.max(0.0),
        rect.max.y + 4.0,
    );
    commands.entity(card).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Px(POPOVER_W);
        n.padding = UiRect::all(Val::Px(10.0));
        n.border_radius = BorderRadius::all(Val::Px(10.0));
    });
    let entity = btn.entity;
    let current = labels.get(entity).ok().map(|lc| (lc.0[0], lc.0[1], lc.0[2]));
    let hue = current.unwrap_or_else(accent);
    let chosen_icon = icons.get(entity).ok().and_then(|i| entity_icon_name(&i.0));

    let col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("entity-icon-popover"),
        ))
        .id();
    let colour_title = section_title(&mut commands, &fonts, &renzora::lang::t_or("inspector.colour", "Colour"));
    let (swatch_row, picker_panel) = colour_row(&mut commands, &fonts, entity, current);
    let divider = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), ..default() },
            BackgroundColor(c(renzora_ember::theme::divider())),
        ))
        .id();
    let icon_head = icon_heading(&mut commands, &fonts, entity);
    let search = text_input(
        &mut commands,
        &fonts.ui,
        &renzora::lang::t_or("inspector.search_icons", "Search icons"),
        "",
    );
    commands.entity(search).insert((
        IconSearchInput,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            height: Val::Px(26.0),
            padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(3.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
    ));
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(CELL_GAP),
                row_gap: Val::Px(CELL_GAP),
                ..default()
            },
            Name::new("entity-icon-grid"),
        ))
        .id();
    let cells: Vec<Entity> = ENTITY_ICON_CHOICES
        .iter()
        .map(|(name, label)| {
            icon_cell(&mut commands, &fonts, entity, name, label, search, hue, chosen_icon == Some(*name))
        })
        .collect();
    commands.entity(grid).add_children(&cells);
    commands
        .entity(col)
        .add_children(&[colour_title, swatch_row, picker_panel, divider, icon_head, grid, search]);
    commands.entity(content).add_child(col);
}

/// The popover's width; the grid's seven 34 px cells and their gaps fit it.
const POPOVER_W: f32 = 300.0;
const CELL: f32 = 34.0;
const CELL_GAP: f32 = 4.0;
const SWATCH: f32 = 22.0;

fn section_title(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

/// The swatch row (None, the presets, the custom rainbow) and the full colour
/// picker the rainbow shows under it.
fn colour_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    current: Option<(u8, u8, u8)>,
) -> (Entity, Entity) {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("entity-colour-row"),
        ))
        .id();
    let mut kids = Vec::new();

    // None: back to the default look, an outline with a slash through it.
    let none = swatch_base(commands, renzora::lang::t_or("inspector.colour_none", "No colour"), current.is_none());
    commands.entity(none).insert((
        BackgroundColor(Color::NONE),
        MenuAction(Box::new(move |w: &mut World| {
            record_field_change(
                w,
                entity,
                "Label Color",
                get(label_color_opt_get),
                set(label_color_opt_set),
                FieldValue::String(String::new()),
            );
        })),
    ));
    if current.is_some() {
        commands.entity(none).insert(BorderColor::all(rgb(text_muted())));
    }
    let slash = icon_text(commands, &fonts.phosphor, "prohibit", text_muted(), 13.0);
    commands.entity(slash).insert(PASS_THROUGH);
    commands.entity(none).add_child(slash);
    kids.push(none);

    for (hue, name) in SWATCHES {
        let s = swatch_base(commands, renzora::lang::t_or(&format!("inspector.colour.{}", name.to_lowercase()), name), current == Some(hue));
        let hex = format!("#{:02x}{:02x}{:02x}", hue.0, hue.1, hue.2);
        commands.entity(s).insert((
            BackgroundColor(rgb(hue)),
            MenuAction(Box::new(move |w: &mut World| {
                record_field_change(
                    w,
                    entity,
                    "Label Color",
                    get(label_color_opt_get),
                    set(label_color_opt_set),
                    FieldValue::String(hex.clone()),
                );
            })),
        ));
        kids.push(s);
    }

    // Custom: the full picker, inline under the row rather than in a popup of
    // its own, so a press on it is still inside this popover and keeps it open.
    let picker = renzora_ember::widgets::hsv_picker(commands, 0.0, 0.0, 0.5);
    renzora_ember::widgets::bind_hsv_picker(
        commands,
        picker,
        move |w| match w.get::<EntityLabelColor>(entity) {
            Some(lc) => [lc.0[0] as f32 / 255.0, lc.0[1] as f32 / 255.0, lc.0[2] as f32 / 255.0],
            None => [220.0 / 255.0, 222.0 / 255.0, 228.0 / 255.0],
        },
        move |w, rgb: [f32; 3]| {
            record_field_change(w, entity, "Label Color", get(label_color_get), set(label_color_set), FieldValue::Color(rgb))
        },
    );
    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::vertical(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            Name::new("entity-colour-picker"),
        ))
        .id();
    commands.entity(panel).add_child(picker);
    let custom = swatch_base(commands, renzora::lang::t_or("inspector.colour_custom", "Custom colour"), false);
    commands.entity(custom).insert((
        renzora_ember::widgets::linear_gradient(
            90.0,
            [
                Color::srgb(0.97, 0.44, 0.44),
                Color::srgb(1.0, 0.82, 0.47),
                Color::srgb(0.43, 0.91, 0.63),
                Color::srgb(0.38, 0.65, 0.98),
                Color::srgb(0.71, 0.61, 1.0),
            ],
        ),
        CustomColourBtn { panel },
    ));
    kids.push(custom);
    commands.entity(row).add_children(&kids);
    (row, panel)
}

/// A round 22 px swatch with a hover cursor and a tooltip. `selected` rings
/// it, which is how the popover says which colour the entity has now.
fn swatch_base(commands: &mut Commands, tooltip: String, selected: bool) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(SWATCH),
                height: Val::Px(SWATCH),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                border_radius: BorderRadius::all(Val::Px(SWATCH * 0.5)),
                ..default()
            },
            BorderColor::all(if selected { rgb(text_primary()) } else { Color::NONE }),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            Name::new("entity-colour-swatch"),
        ))
        .id()
}

/// "Icon" with the Auto button at the other end of the line.
fn icon_heading(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();
    let title = section_title(commands, fonts, &renzora::lang::t_or("inspector.icon", "Icon"));
    let auto = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("inspector.entity_icon_auto")),
            MenuAction(Box::new(move |w: &mut World| {
                record_field_change(w, entity, "Icon", get(icon_get), set(icon_set), FieldValue::String(String::new()));
            })),
            Name::new("entity-icon-auto"),
        ))
        .id();
    bind_bg(commands, auto, move |w| match w.get::<UiInteraction>(auto) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => c(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let auto_text = commands
        .spawn((
            Text::new(renzora::lang::t_or("inspector.icon_auto_short", "Auto")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(auto).add_child(auto_text);
    commands.entity(row).add_children(&[title, auto]);
    row
}

/// One pickable icon in the grid. Carries a [`MenuAction`], so the shared
/// screen-menu systems run the pick and close the popover. The current icon
/// sits on a wash of the entity's colour, so reopening the grid shows you
/// where you are instead of making you remember.
#[allow(clippy::too_many_arguments)]
fn icon_cell(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    name: &'static str,
    label: &str,
    search: Entity,
    hue: (u8, u8, u8),
    chosen: bool,
) -> Entity {
    let cell = commands
        .spawn((
            Node {
                width: Val::Px(CELL),
                height: Val::Px(CELL),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(label),
            IconCell { search, label_lc: format!("{} {}", label.to_lowercase(), name) },
            MenuAction(Box::new(move |w: &mut World| {
                record_field_change(
                    w,
                    entity,
                    "Icon",
                    get(icon_get),
                    set(icon_set),
                    FieldValue::String(name.to_string()),
                );
            })),
            Name::new("entity-icon-cell"),
        ))
        .id();
    bind_bg(commands, cell, move |w| {
        let hovered = matches!(
            w.get::<UiInteraction>(cell),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        );
        match (chosen, hovered) {
            (true, _) => rgb(hue).with_alpha(if hovered { 0.28 } else { 0.2 }),
            (false, true) => c(renzora_ember::theme::hover_bg()),
            (false, false) => Color::NONE,
        }
    });
    let glyph = phosphor_glyph(
        commands,
        fonts,
        name,
        if chosen { hue } else { (196, 196, 206) },
        20.0,
    );
    commands.entity(cell).add_child(glyph);
    cell
}

/// The rainbow swatch: show or hide the full colour picker under the row.
pub(crate) fn entity_custom_colour_toggle(
    q: Query<(&UiInteraction, &CustomColourBtn), Changed<UiInteraction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        if let Ok(mut n) = nodes.get_mut(btn.panel) {
            n.display = if n.display == Display::None {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

/// Filter the icon grid by the popover's search box, only when what was typed
/// changes.
pub(crate) fn entity_icon_search(
    inputs: Query<(Entity, Ref<EmberTextInput>), With<IconSearchInput>>,
    mut cells: Query<(&IconCell, &mut Node)>,
) {
    for (input, inp) in &inputs {
        if !inp.is_changed() {
            continue;
        }
        let q = inp.value.trim().to_lowercase();
        for (cell, mut node) in &mut cells {
            if cell.search != input {
                continue;
            }
            let want = if q.is_empty() || cell.label_lc.contains(&q) {
                Display::Flex
            } else {
                Display::None
            };
            if node.display != want {
                node.display = want;
            }
        }
    }
}
