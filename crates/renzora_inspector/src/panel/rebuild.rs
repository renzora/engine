//! The signature that decides whether to rebuild, the exclusive rebuild itself,
//! and the fixed top bar above the component list.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor_framework::{
    EditorSelection, EditorSettings, FieldType, InspectorRegistry, NativeInspectorDrawer,
};
use renzora_ember::font::EmberFonts;
use renzora_ember::interaction::BLOCK;
use renzora_ember::reactive::tracked::bind_display;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{search_input, EmberTextInput};
use renzora_ember::UiInteraction;

use super::collect::collect_sections;
use super::pages::{self, RailSelection};
use super::section::build_section;
use super::{
    empty_label, inspected_entity, phosphor_glyph, ExpandAllButton, ExpandAllGlyph, InspectorFilter,
    InspectorRoot, InspectorState,
};

/// Height shared by everything in the toolbar row, so the filter, the button
/// and the toggle line up top and bottom.
const TOOLBAR_H: f32 = 28.0;

/// The fixed toolbar over the card list, beside the page rail: Add, then the
/// component filter with its magnifier inside, then the expand/collapse-all
/// toggle. Hidden when nothing is selected.
///
/// Add leads the row, solid accent and labelled. It sat on the right as a bare
/// "+" in an early layout, and on its own a "+" by a search box read as "add a
/// search"; at the head of the row with a word on it, it says what it is. The
/// full "Add Component" is in its tooltip and the picker's title, where there
/// is room for it.
pub(super) fn build_top_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let add_btn = add_bar(commands, fonts);
    let input = search_input(commands, fonts, &renzora::lang::t("inspector.filter_placeholder"), "");
    commands.entity(input).insert((
        InspectorFilter,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            height: Val::Px(TOOLBAR_H),
            // `search_input` puts its magnifier in this left padding; the
            // caret relies on the text starting exactly there.
            padding: UiRect::new(
                Val::Px(renzora_ember::widgets::PAD_X + renzora_ember::widgets::SEARCH_LEAD),
                Val::Px(renzora_ember::widgets::PAD_X),
                Val::Px(4.0),
                Val::Px(4.0),
            ),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(7.0)),
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
                width: Val::Px(TOOLBAR_H),
                height: Val::Px(TOOLBAR_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            UiInteraction::default(),
            BLOCK,
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
                padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(8.0), Val::Px(4.0)),
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

/// The solid accent "+ Add" button. Not a themed `Styled` button: those take
/// the theme's neutral button colours, and this is the one filled control in
/// the panel, the thing to press to put something new on the entity.
fn add_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    use renzora_ember::theme::{accent, mix, rgb, text_primary};
    let btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                height: Val::Px(TOOLBAR_H),
                padding: UiRect::horizontal(Val::Px(11.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            UiInteraction::default(),
            BLOCK,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("inspector.add_component")),
            AddButton,
            Name::new("add-component"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => mix(accent(), text_primary(), 0.12),
        _ => rgb(accent()),
    });
    let plus = phosphor_glyph(commands, fonts, "plus", (255, 255, 255), 13.0);
    let label = commands
        .spawn((
            Text::new(renzora::lang::t_or("inspector.add_short", "Add")),
            renzora_ember::font::ui_font(&fonts.ui, 13.0),
            TextColor(Color::WHITE),
            bevy::text::TextLayout::no_wrap(),
            renzora_ember::interaction::PASS_THROUGH,
        ))
        .id();
    commands.entity(btn).add_children(&[plus, label]);
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
    // Both cached for the same reason as `container_q`; only read while
    // nothing is selected, for the Scene page.
    mut env_q: Local<QueryState<Entity, With<renzora::WorldEnvironment>>>,
    mut cam_q: Local<QueryState<&Name, With<renzora::core::DefaultCamera>>>,
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

    let (world_env, start_camera) = if entity.is_none() {
        (
            env_q.iter(world).next(),
            cam_q.iter(world).next().map(|n| n.as_str().to_string()),
        )
    } else {
        (None, None)
    };
    let sig = inspector_signature(
        &Rx::new(&*world),
        container,
        entity,
        locked.is_some(),
        (world_env, start_camera.as_deref()),
    );
    if world.resource::<InspectorState>().sig == Some(sig) {
        return;
    }
    // The Scene page's facts and the world environment's cards, read now
    // while the world is still free to borrow.
    let scene_facts = entity
        .is_none()
        .then(|| super::scene_page::scene_facts(world, start_camera.clone()));
    let env_sections: Vec<_> = match world_env.filter(|_| entity.is_none()) {
        Some(env) => {
            use renzora::inspector_pages::InspectorPage;
            let rx = Rx::new(&*world);
            let mut out = collect_sections(&rx, Some(env), Some(InspectorPage::Environment));
            out.extend(collect_sections(&rx, Some(env), Some(InspectorPage::Lighting)));
            out
        }
        None => Vec::new(),
    };
    let theme = world
        .get_resource::<renzora_theme::ThemeManager>()
        .map(|t| t.active_theme.clone());

    let filter = world.resource::<InspectorState>().filter.clone();
    let filter_active = !filter.is_empty();
    // Which pages the entity has, and the one to show. A filter searches every
    // page, so it shows matching cards from all of them whatever is picked.
    let page_entries = entity
        .map(|e| pages::entity_pages(world, e, &filter))
        .unwrap_or_default();
    let shown = pages::effective_selection(world.resource::<InspectorState>().rail, &page_entries);
    world.resource_mut::<InspectorState>().shown = shown;
    let sections = match (filter_active, shown) {
        (true, _) | (false, RailSelection::All) => collect_sections(&Rx::new(&*world), entity, None),
        (false, RailSelection::Page(p)) => collect_sections(&Rx::new(&*world), entity, Some(p)),
        (false, RailSelection::Info | RailSelection::Relations) => Vec::new(),
    };
    let facts = match (entity, filter_active, shown) {
        (Some(e), false, RailSelection::Info | RailSelection::Relations) => Some(pages::entity_facts(world, e)),
        _ => None,
    };
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
    let rail_host = {
        let mut rq = world.query_filtered::<Entity, With<pages::RailHost>>();
        rq.iter(world).next()
    };
    let rail_host_children: Vec<Entity> = rail_host
        .and_then(|h| world.get::<Children>(h).map(|ch| ch.iter().collect()))
        .unwrap_or_default();
    // Read before `Commands` borrows the world: the eye is only built for an
    // entity that has something to hide, and the subtitle names what it is.
    let header_has_visibility = entity
        .map(|e| crate::entity_header::has_visibility(world, e))
        .unwrap_or(false);
    let header_subtitle = entity
        .map(|e| crate::entity_header::entity_subtitle(world, e))
        .unwrap_or_default();
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
        if let Some(host) = header_host {
            let kids = match (entity, &scene_facts) {
                (Some(entity), _) => crate::entity_header::build_entity_header(
                    &mut commands,
                    &fonts,
                    entity,
                    header_has_visibility,
                    &header_subtitle,
                ),
                (None, Some(facts)) => super::scene_page::build_scene_header(&mut commands, &fonts, facts),
                (None, None) => Vec::new(),
            };
            commands.entity(host).add_children(&kids);
        }
        for child in &rail_host_children {
            commands.entity(*child).despawn();
        }
        if let (Some(host), Some(_)) = (rail_host, entity) {
            let kids = pages::build_rail(&mut commands, &fonts, &page_entries, shown, filter_active);
            commands.entity(host).add_children(&kids);
        }
        match entity {
            None => {
                use super::scene_page as sp;
                let theme = theme.as_ref();
                let mut kids = vec![sp::group_heading(
                    &mut commands,
                    &fonts,
                    &renzora::lang::t_or("inspector.scene.world", "World"),
                )];
                match world_env {
                    // The world environment entity's own cards, editing it
                    // directly, as if it were selected.
                    Some(env) => {
                        for sec in env_sections.iter() {
                            let (root, body) = build_section(&mut commands, &fonts, sec, env);
                            kids.push(root);
                            if let (Some(drawer), true) = (sec.native_drawer, sec.open) {
                                native_pending.push((body, drawer, env));
                            }
                        }
                    }
                    None => kids.push(sp::build_no_world(&mut commands, &fonts, theme)),
                }
                if let Some(facts) = &scene_facts {
                    kids.push(sp::group_heading(
                        &mut commands,
                        &fonts,
                        &renzora::lang::t_or("inspector.scene.scene", "Scene"),
                    ));
                    kids.push(sp::build_game_card(&mut commands, &fonts, facts, theme));
                }
                kids.push(sp::build_stats(&mut commands, &fonts));
                kids.push(sp::build_hint(&mut commands, &fonts));
                commands.entity(container).add_children(&kids);
            }
            Some(_) if facts.is_some() => {
                let facts = facts.as_ref().expect("checked by the guard");
                let kids = if shown == RailSelection::Info {
                    pages::build_info_page(&mut commands, &fonts, facts)
                } else {
                    pages::build_relations_page(&mut commands, &fonts, facts)
                };
                commands.entity(container).add_children(&kids);
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
    scene: (Option<Entity>, Option<&str>),
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    container.to_bits().hash(&mut h);
    locked.hash(&mut h);
    if let Some(s) = world.get_resource::<InspectorState>() {
        s.filter.hash(&mut h);
        // The page picked on the rail. What it resolves to also depends on the
        // entity's component set, which is hashed below.
        s.rail.hash(&mut h);
    }
    // Changing the default-expand policy re-applies it to the current view, so
    // it forces a rebuild.
    if let Some(s) = world.get_resource::<EditorSettings>() {
        (s.inspector_expand_default as u8).hash(&mut h);
    }
    // The graphics-quality tier decides which sections wear the gate warning, and
    // it changes without the component set changing at all — so without this the
    // warning would only appear on the next reselect, which is precisely when the
    // user has stopped wondering why the effect vanished.
    if let Some(q) = world.get_resource::<renzora::ResolvedGraphicsQuality>() {
        (q.0 as u8).hash(&mut h);
    }
    // A section dropped somewhere new changes nothing about the *set* of
    // components, so the drag records a revision and this is what turns it into
    // a rebuilt (and therefore reordered) panel.
    if let Some(o) = world.get_resource::<super::order::InspectorSectionOrder>() {
        o.revision.hash(&mut h);
    }
    match entity {
        Some(e) => {
            1u8.hash(&mut h);
            hash_entity(world, e, &mut h);
        }
        // The Scene page: what it names, plus the world environment entity's
        // components, since its cards are that entity's.
        None => {
            0u8.hash(&mut h);
            super::scene_page::hash_scene(world.untracked(), scene, &mut h);
            if let Some(env) = scene.0 {
                hash_entity(world, env, &mut h);
            }
        }
    }
    h.finish()
}

/// An entity's identity and component set, as far as the cards it gets.
fn hash_entity(world: &Rx, e: Entity, h: &mut impl Hasher) {
    e.to_bits().hash(h);
    if let Some(reg) = world.get_resource::<InspectorRegistry>() {
        for entry in reg.iter() {
            if (entry.has_fn)(world.untracked(), e) {
                entry.type_id.hash(h);
                // Presence-toggled sections (their enable switch
                // inserts/removes the underlying component, e.g. 2D
                // Lighting on a camera) change their rows without
                // changing the section set, so fold the enabled bit in
                // so flipping the switch rebuilds the body.
                if let Some(is_enabled) = entry.is_enabled_fn {
                    is_enabled(world.untracked(), e).hash(h);
                }
                // A `DynamicEnum` field's options are computed from the
                // world at build time, so a *mutation* that grows/shrinks
                // the list (e.g. appending a sprite sheet) wouldn't
                // otherwise change the signature, leaving a stale option
                // list and an out-of-range selection (blank dropdown).
                // Fold the options in so the list rebuilds when it changes.
                for field in &entry.fields {
                    if let FieldType::DynamicEnum { options } = field.field_type {
                        for opt in options(world.untracked(), e) {
                            opt.hash(h);
                        }
                    }
                }
            }
        }
    }
}
