//! The signature that decides whether to rebuild, the exclusive rebuild itself,
//! and the fixed top bar above the component list.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor_framework::{
    EditorSelection, EditorSettings, FieldType, InspectorRegistry, NativeInspectorDrawer,
};
use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::bind_display;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{text_input, EmberTextInput};

use super::collect::collect_sections;
use super::section::build_section;
use super::{
    empty_label, inspected_entity, phosphor_glyph, ExpandAllButton, ExpandAllGlyph, InspectorFilter,
    InspectorRoot, InspectorState,
};

/// The fixed top bar: the Add Component button + the component-filter text input
/// + the expand/collapse-all toggle. Hidden when nothing is selected.
pub(super) fn build_top_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let add_btn = add_bar(commands, fonts);
    let input = text_input(commands, &fonts.ui, &renzora::lang::t("inspector.filter_placeholder"), "");
    commands.entity(input).insert((
        InspectorFilter,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    // Expand / collapse-all toggle: forces every section open or closed for the
    // current view. Its glyph reflects the live state — "expand" when anything
    // could still open, "collapse" once everything is forced open.
    let expand_btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(26.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ExpandAllButton,
            Name::new("inspector-expand-all"),
        ))
        .id();
    let glyph = phosphor_glyph(
        commands,
        fonts,
        "arrows-out-line-vertical",
        renzora_ember::theme::text_muted(),
        15.0,
    );
    // `sync_expand_glyph` flips this between expand/collapse as sections change.
    commands.entity(glyph).insert(ExpandAllGlyph);
    commands.entity(expand_btn).add_child(glyph);

    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("inspector-top-bar"),
        ))
        .id();
    commands.entity(bar).add_children(&[add_btn, input, expand_btn]);
    bind_display(commands, bar, |w| inspected_entity(w).is_some());
    bar
}

#[derive(Component)]
pub(super) struct AddButton;

fn add_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // A themed ember button (Styled(Role::Button)) — picks up Theme.button +
    // hover/press states, and is editable under "Button" in the Theme editor.
    let btn = renzora_ember::widgets::icon_label_button(commands, fonts, "puzzle-piece", &renzora::lang::t("inspector.add_component"));
    commands.entity(btn).insert((
        AddButton,
        // Sits in the top bar beside the filter input, so it sizes to its own
        // label and refuses to shrink — the input takes the slack. The theme
        // fills padding/radius/colors.
        Node {
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
        Name::new("add-component"),
    ));
    btn
}

/// Sync the filter input's text into state (lowercased) so `collect_sections`
/// and the rebuild signature pick it up.
pub(super) fn inspector_filter_sync(
    input: Query<&EmberTextInput, With<InspectorFilter>>,
    mut state: ResMut<InspectorState>,
) {
    for inp in &input {
        let v = inp.value.to_lowercase();
        if state.filter != v {
            state.filter = v;
        }
    }
}

/// `container_q` is a `Local` rather than a fresh `world.query_filtered(..)` per
/// call: this system runs every frame the Inspector tab is active, and building a
/// `QueryState` each time forces `update_archetypes` down its
/// from-generation-zero full-scan branch. `With<T>` goes through `and_with` and
/// never populates `FilteredAccess::required`, so there is no cheap path — it
/// rescans every archetype in the world, every frame, to find one entity.
pub(super) fn rebuild_inspector(
    world: &mut World,
    mut container_q: Local<QueryState<Entity, With<InspectorRoot>>>,
) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Drop a stale lock, then resolve the inspected entity (lock wins).
    {
        let locked = world.resource::<InspectorState>().locked;
        if let Some(e) = locked {
            if world.get_entity(e).is_err() {
                world.resource_mut::<InspectorState>().locked = None;
            }
        }
    }
    let locked = world.resource::<InspectorState>().locked;
    let entity = locked.or_else(|| {
        world
            .get_resource::<EditorSelection>()
            .and_then(|s| s.get())
    });

    let Some(container) = container_q.iter(world).next() else {
        return;
    };

    let sig = inspector_signature(&Rx::new(&*world), container, entity, locked.is_some());
    if world.resource::<InspectorState>().sig == Some(sig) {
        return;
    }

    let sections = collect_sections(&Rx::new(&*world), entity);
    let filter_active = !world.resource::<InspectorState>().filter.is_empty();
    let existing: Vec<Entity> = world
        .get::<Children>(container)
        .map(|ch| ch.iter().collect())
        .unwrap_or_default();

    let header_host = {
        let mut hq = world.query_filtered::<Entity, With<crate::entity_header::EntityHeaderHost>>();
        hq.iter(world).next()
    };
    let header_host_children: Vec<Entity> = header_host
        .and_then(|h| world.get::<Children>(h).map(|ch| ch.iter().collect()))
        .unwrap_or_default();
    // Read before `Commands` borrows the world: the eye is only built for an
    // entity that has something to hide.
    let header_has_visibility = entity
        .map(|e| crate::entity_header::has_visibility(world, e))
        .unwrap_or(false);
    // Native-drawer sections: (body, drawer, entity) — filled after the queue
    // applies, since drawers need exclusive &mut World.
    let mut native_pending: Vec<(Entity, NativeInspectorDrawer, Entity)> = Vec::new();

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for child in existing {
            commands.entity(child).despawn();
        }

        // The entity header's bindings capture the inspected entity, so it is
        // rebuilt on every signature change rather than kept and re-bound.
        for child in &header_host_children {
            commands.entity(*child).despawn();
        }
        if let (Some(host), Some(entity)) = (header_host, entity) {
            let kids = crate::entity_header::build_entity_header(
                &mut commands,
                &fonts,
                entity,
                header_has_visibility,
            );
            commands.entity(host).add_children(&kids);
        }
        match entity {
            None => {
                let l = empty_label(&mut commands, &fonts, &renzora::lang::t("inspector.no_selection"));
                commands.entity(container).add_child(l);
            }
            Some(entity) => {
                if sections.is_empty() {
                    let msg = if filter_active {
                        renzora::lang::t("inspector.no_match")
                    } else {
                        renzora::lang::t("inspector.no_components")
                    };
                    let l = empty_label(&mut commands, &fonts, &msg);
                    commands.entity(container).add_child(l);
                }
                for sec in sections.iter() {
                    let (root, body) = build_section(&mut commands, &fonts, sec, entity);
                    commands.entity(container).add_child(root);
                    // Only an OPEN section's drawer runs here; a collapsed one is
                    // left to `reconcile_section_bodies` to run if it's expanded.
                    if let (Some(drawer), true) = (sec.native_drawer, sec.open) {
                        native_pending.push((body, drawer, entity));
                    }
                }
            }
        }
    }
    queue.apply(world);

    // Run each native drawer (exclusive World) and parent its content under the
    // section body.
    for (body, drawer, ent) in native_pending {
        let content = drawer(world, ent);
        if let Ok(mut em) = world.get_entity_mut(body) {
            em.add_child(content);
        }
    }

    world.resource_mut::<InspectorState>().sig = Some(sig);
}

/// Known gap, deliberately not closed: a field's *visibility* depends on its
/// `get_fn` returning `Some` (see `collect_sections`), and that predicate is not
/// hashed here — so a field that appears or disappears without any other input
/// changing leaves a stale row.
///
/// Not fixed because the cure is worse: folding it in means calling `get_fn` for
/// every field of every present component **every frame**, before the early-out,
/// to guard against something only three `get_fn`s in the entire workspace can
/// even express (the rest are unconditional). Per-section hashing would make it
/// cheap — it would only re-read the fields of one section — so this belongs with
/// that work rather than as a standalone per-frame cost.
fn inspector_signature(
    world: &Rx,
    container: Entity,
    entity: Option<Entity>,
    locked: bool,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    container.to_bits().hash(&mut h);
    locked.hash(&mut h);
    if let Some(s) = world.get_resource::<InspectorState>() {
        s.filter.hash(&mut h);
    }
    // Changing the default-expand policy re-applies it to the current view, so
    // it forces a rebuild.
    if let Some(s) = world.get_resource::<EditorSettings>() {
        (s.inspector_expand_default as u8).hash(&mut h);
    }
    match entity {
        Some(e) => {
            1u8.hash(&mut h);
            e.to_bits().hash(&mut h);
            if let Some(reg) = world.get_resource::<InspectorRegistry>() {
                for entry in reg.iter() {
                    if (entry.has_fn)(world.untracked(), e) {
                        entry.type_id.hash(&mut h);
                        // Presence-toggled sections (their enable switch
                        // inserts/removes the underlying component, e.g. 2D
                        // Lighting on a camera) change their rows without
                        // changing the section set — fold the enabled bit in
                        // so flipping the switch rebuilds the body.
                        if let Some(is_enabled) = entry.is_enabled_fn {
                            is_enabled(world.untracked(), e).hash(&mut h);
                        }
                        // A `DynamicEnum` field's options are computed from the
                        // world at build time, so a *mutation* that grows/shrinks
                        // the list (e.g. appending a sprite sheet) wouldn't
                        // otherwise change the signature — leaving a stale option
                        // list and an out-of-range selection (blank dropdown).
                        // Fold the options in so the list rebuilds when it changes.
                        for field in &entry.fields {
                            if let FieldType::DynamicEnum { options } = field.field_type {
                                for opt in options(world.untracked(), e) {
                                    opt.hash(&mut h);
                                }
                            }
                        }
                    }
                }
            }
        }
        None => 0u8.hash(&mut h),
    }
    h.finish()
}
