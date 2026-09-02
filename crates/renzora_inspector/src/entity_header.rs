//! The inspector's entity header — the fixed row above the component list that
//! carries everything belonging to the *entity* rather than to any component on
//! it: its icon, its id, its hierarchy label colour, its visibility, and the
//! inspector lock.
//!
//! Why these left the component list. All five used to render as registry
//! sections ("ID" with two field rows, "Visibility" with one), which put them in
//! a list whose whole grammar is add / remove / collapse / enable — and not one
//! of them can be added or removed. Two collapsible cards and four label columns
//! were spending about seventy vertical pixels to expose three controls, on a
//! panel where vertical space is the scarce resource. As one row they cost
//! twenty-eight, and they read as what they are: the identity of the thing you
//! selected, not components it happens to have.
//!
//! Everything here is rebuilt by `rebuild_inspector` into a stable host node
//! ([`EntityHeaderHost`]), the same pattern as the component rail — the widgets'
//! bindings capture the inspected `Entity`, so they cannot outlive a selection
//! change, but the host they hang under can and does.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use renzora_editor_framework::{
    entity_icon_name, ComponentIconRegistry, EntityIcon, EntityLabelColor, FieldValue,
    ENTITY_ICON_CHOICES,
};
use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{bind_text_input, screen_menu_flip, text_input, MenuAction};

use crate::panel::section::LockBtn;
use crate::panel::{c, inspected_entity, phosphor_glyph, record_field_change, GetFn, SetFn};

/// Tint for an "on" header affordance (locked, or explicitly hidden) — the same
/// blue the lock glyph used on the old ID section header.
const ACTIVE_TINT: (u8, u8, u8) = (120, 170, 255);

/// Stable host for the entity header. Repopulated by `rebuild_inspector`;
/// hidden outright when nothing is selected.
#[derive(Component)]
pub(crate) struct EntityHeaderHost;

/// The icon button at the head of the row. Pressing it opens the icon grid.
#[derive(Component)]
pub(crate) struct EntityIconBtn {
    entity: Entity,
}

/// The eye toggle. A separate marker from the icon button so a press on one
/// can never be read as a press on the other.
#[derive(Component)]
pub(crate) struct EntityVisibilityBtn {
    entity: Entity,
}

// ── Field accessors ──────────────────────────────────────────────────────────
//
// Plain `fn(&World, Entity)` pairs rather than closures, so they coerce into the
// `GetFn`/`SetFn` the undo recorder wants and each edit lands on the undo stack
// exactly like an inspector field row does.

/// The entity's id. Sanitized and de-duplicated on write — this is the rule the
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

/// Read visibility through the viewport gate.
///
/// `ViewportGateHidden` means the scene is force-hidden because no viewport
/// panel is showing, and it stashes the *authored* value. Reading `Visibility`
/// directly in that state reports every entity in the scene as hidden — which
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
/// whole inspector — including this header — so tracking it would buy nothing
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

// ── Build ────────────────────────────────────────────────────────────────────

/// The stable host node, built once when the panel is registered.
pub(crate) fn build_entity_header_host(commands: &mut Commands) -> Entity {
    let host = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(2.0), Val::Px(5.0)),
                flex_shrink: 0.0,
                // A rule under the row, not a filled bar: the header is the
                // identity of what follows, so it should close off the fixed
                // chrome above rather than read as one more card.
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(c(renzora_ember::theme::divider())),
            EntityHeaderHost,
            Name::new("inspector-entity-header"),
        ))
        .id();
    bind_display(commands, host, |w| inspected_entity(w).is_some());
    host
}

/// Fill the header for `entity`. Returns the children for the caller to parent
/// under the host (which it has just emptied).
///
/// `has_visibility` comes from the caller because this side of `rebuild_inspector`
/// only holds `Commands` — the world is borrowed mutably by then.
pub(crate) fn build_entity_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    has_visibility: bool,
) -> Vec<Entity> {
    let mut row = vec![
        icon_button(commands, fonts, entity),
        id_input(commands, fonts, entity),
        label_color_swatch(commands, entity),
    ];
    // Only entities that can actually be hidden get an eye. A `Visibility`-less
    // entity with a permanently greyed-out toggle would just be a question.
    if has_visibility {
        row.push(visibility_button(commands, fonts, entity));
    }
    row.push(lock_button(commands, fonts, entity));
    row
}

/// Whether `entity` has anything the eye toggle could act on.
pub(crate) fn has_visibility(world: &World, entity: Entity) -> bool {
    world.get::<Visibility>(entity).is_some()
        || world
            .get::<renzora::core::ViewportGateHidden>(entity)
            .is_some()
}

/// The icon trigger: the entity's current glyph in a pressable box.
fn icon_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(26.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c(renzora_ember::theme::tab_active())),
            Interaction::default(),
            FocusPolicy::Block,
            // The grid is aimed from the button's own rect, recovered from the
            // cursor and its normalized position inside the button — the same
            // scheme `icon_menu_button` uses.
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            EntityIconBtn { entity },
            Name::new("entity-icon-button"),
        ))
        .id();
    let glyph = phosphor_glyph(
        commands,
        fonts,
        "circle",
        renzora_ember::theme::text_primary(),
        14.0,
    );
    bind_text(commands, glyph, move |w| {
        renzora_ember::phosphor_map::icon_glyph(effective_icon(w, entity))
            .unwrap_or('\u{E4C6}')
            .to_string()
    });
    commands.entity(btn).add_child(glyph);
    btn
}

/// The id field. Takes the row's spare width — it is the one control here whose
/// content has no fixed size.
fn id_input(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let ti = text_input(commands, &fonts.ui, "—", "");
    commands.entity(ti).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    bind_text_input(
        commands,
        ti,
        move |w| match w.get::<Name>(entity) {
            Some(n) => n.as_str().to_string(),
            None => String::new(),
        },
        move |w, v: String| {
            record_field_change(
                w,
                entity,
                "ID",
                get(id_get),
                set(id_set),
                FieldValue::String(v),
            )
        },
    );
    ti
}

fn label_color_swatch(commands: &mut Commands, entity: Entity) -> Entity {
    renzora_ember::inspector::color_swatch_field_right(
        commands,
        move |w| match w.get::<EntityLabelColor>(entity) {
            Some(lc) => [
                lc.0[0] as f32 / 255.0,
                lc.0[1] as f32 / 255.0,
                lc.0[2] as f32 / 255.0,
            ],
            None => [220.0 / 255.0, 222.0 / 255.0, 228.0 / 255.0],
        },
        move |w, rgb: [f32; 3]| {
            record_field_change(
                w,
                entity,
                "Label Color",
                get(label_color_get),
                set(label_color_set),
                FieldValue::Color(rgb),
            )
        },
    )
}

/// The eye. Hidden entities get the crossed-out glyph in the active tint, so a
/// hidden selection is visible at a glance rather than only on close reading.
fn visibility_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let eye = phosphor_glyph(
        commands,
        fonts,
        "eye",
        renzora_ember::theme::text_muted(),
        15.0,
    );
    commands.entity(eye).insert((
        Interaction::default(),
        FocusPolicy::Block,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        EntityVisibilityBtn { entity },
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
        c(if visible_now(w) {
            renzora_ember::theme::text_muted()
        } else {
            ACTIVE_TINT
        })
    });
    eye
}

/// The inspector lock — pin the panel to this entity so clicking elsewhere in
/// the viewport doesn't swap what you are editing.
///
/// The glyph is bound rather than baked. `NativeInspectorState::locked` reaches
/// the UI only through the inspector's global rebuild signature, so a baked
/// glyph works by accident today; under a granular rebuild the state would flip
/// with nothing on screen changing, and the button would look broken.
fn lock_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let lock = phosphor_glyph(
        commands,
        fonts,
        "lock-simple-open",
        renzora_ember::theme::text_muted(),
        14.0,
    );
    commands.entity(lock).insert((
        Interaction::default(),
        FocusPolicy::Block,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        LockBtn { entity },
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
    bind_text_color(commands, lock, move |w| {
        c(if locked_now(w) {
            ACTIVE_TINT
        } else {
            renzora_ember::theme::text_muted()
        })
    });
    lock
}

// ── Interaction ──────────────────────────────────────────────────────────────

/// Click the eye → flip visibility, undoably.
pub(crate) fn entity_visibility_click(
    q: Query<(&Interaction, &EntityVisibilityBtn), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
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

/// Press the icon button → open the icon grid under it.
///
/// A `screen_menu_flip` root rather than a child-anchored panel: bevy_ui clips
/// absolutely-positioned children by every scrolling ancestor, and a 250 px grid
/// hanging off a button near the inspector's left edge would be cut off by the
/// panel. Screen menus are never clipped and clamp themselves on-screen.
pub(crate) fn entity_icon_menu_open(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    buttons: Query<
        (
            &Interaction,
            &EntityIconBtn,
            &RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, btn, rcp, cn)) = buttons.iter().find(|(i, ..)| **i == Interaction::Pressed) else {
        return;
    };
    let Some((cursor, win_h)) = windows
        .iter()
        .find_map(|w| w.cursor_position().map(|c| (c, w.height())))
    else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let content = screen_menu_flip(
        &mut commands,
        top_left.x.max(0.0),
        top_left.y + size.y + 2.0,
        win_h,
    );
    let entity = btn.entity;

    // "Auto" first, as a full-width row: it is the only choice that isn't an
    // icon, and it is the one people reach for to undo an experiment.
    let auto = renzora_ember::widgets::menu_item(
        &mut commands,
        &fonts,
        "arrow-counter-clockwise",
        &renzora::lang::t("inspector.entity_icon_auto"),
        move |w: &mut World| {
            record_field_change(
                w,
                entity,
                "Icon",
                get(icon_get),
                set(icon_set),
                FieldValue::String(String::new()),
            );
        },
    );
    commands.entity(content).add_child(auto);

    let grid = commands
        .spawn((
            Node {
                width: Val::Px(GRID_COLS as f32 * CELL + 2.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            Name::new("entity-icon-grid"),
        ))
        .id();
    let cells: Vec<Entity> = ENTITY_ICON_CHOICES
        .iter()
        .map(|(name, label)| icon_cell(&mut commands, &fonts, entity, name, label))
        .collect();
    commands.entity(grid).add_children(&cells);
    commands.entity(content).add_child(grid);
}

/// Grid geometry. Eight columns is what makes the curated set read as six themed
/// bands (see `ENTITY_ICON_CHOICES`) rather than an undifferentiated blob.
const GRID_COLS: usize = 8;
const CELL: f32 = 30.0;

/// One pickable icon in the grid. Carries a [`MenuAction`], so the shared
/// screen-menu systems run the pick and close the menu — the grid needs no
/// click handling of its own.
fn icon_cell(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    name: &'static str,
    label: &str,
) -> Entity {
    let cell = commands
        .spawn((
            Node {
                width: Val::Px(CELL),
                height: Val::Px(CELL),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(label),
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
    renzora_ember::reactive::tracked::bind_bg(commands, cell, move |w| {
        match w.get::<Interaction>(cell) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                c(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        }
    });
    // The currently-chosen icon reads as chosen, so reopening the grid shows you
    // where you are instead of making you remember.
    let glyph = phosphor_glyph(commands, fonts, name, renzora_ember::theme::text_muted(), 17.0);
    bind_text_color(commands, glyph, move |w| {
        c(if effective_icon(w, entity) == name {
            ACTIVE_TINT
        } else {
            renzora_ember::theme::text_muted()
        })
    });
    commands.entity(cell).add_child(glyph);
    cell
}
