//! Building one component section, and keeping its body's contents in step with
//! whether it is open.
//!
//! A collapsed section builds **nothing**. That is not an optimisation detail —
//! `bevy_ui` does not prune hidden subtrees from its per-frame walk, so a hidden
//! row pays full layout every frame forever. The recipe for filling the body is
//! parked on the body entity as [`SectionBodySpec`] and run on expand.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor_framework::NativeInspectorDrawer;
use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::widgets::{toggle_switch, Section};

use super::cull::SectionCull;
use super::fields::build_field_row;
use super::spec::{comp_name_loc, tracked_read, FieldSpec, SectionSpec};
use super::undo::EnableToggleCmd;
use super::{empty_label, phosphor_glyph, GetFn, InspectorSectionHeader, Mutate, SetFn};

#[derive(Component)]
pub(super) struct RemoveBtn {
    pub(super) remove_fn: Mutate,
    pub(super) entity: Entity,
    /// Component type id (short name), so undo can reflect-restore the removed
    /// component's captured value.
    pub(super) type_id: &'static str,
}

#[derive(Component)]
pub(crate) struct LockBtn {
    pub(crate) entity: Entity,
}

/// Marks a `FieldType::Button` widget so [`super::systems::field_button_click`]
/// runs its action.
#[derive(Component)]
pub(super) struct FieldButton {
    pub(super) set_fn: SetFn,
    pub(super) entity: Entity,
}

/// Marks a per-field reset button so [`super::systems::reset_click`] writes the
/// field's default.
#[derive(Component)]
pub(super) struct ResetBtn {
    pub(super) get_fn: GetFn,
    pub(super) set_fn: SetFn,
    pub(super) entity: Entity,
    pub(super) field_name: &'static str,
}

/// Marks a per-field "add keyframe" button. Carries the reflection path the
/// timeline editor matches against the open clip's tracks (see
/// [`super::systems::add_keyframe_click`]).
#[derive(Component)]
pub(super) struct AddKeyframeBtn {
    pub(super) entity: Entity,
    pub(super) component: String,
    pub(super) field: String,
}

/// The recipe for (re)filling a section body, parked on the body entity itself.
///
/// Sections are built collapsed-and-empty and filled on expand, so this has to
/// survive on the body across collapse/expand cycles — it's the only record of
/// how to rebuild rows that were thrown away. `filled` tracks whether the body
/// currently holds its rows, so [`reconcile_section_bodies`] can tell "just
/// expanded" from "already done" without diffing children every frame.
#[derive(Component)]
pub(super) struct SectionBodySpec {
    pub(super) fields: Vec<FieldSpec>,
    pub(super) entity: Entity,
    pub(super) type_id: &'static str,
    pub(super) native_drawer: Option<NativeInspectorDrawer>,
    pub(super) custom: bool,
    pub(super) filled: bool,
}

/// Build the declarative field rows for a section body (shared by the initial
/// build and the fill-on-expand path so the two can't drift — notably the stripe
/// colour, which is derived from row index).
fn fill_section_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    fields: &[FieldSpec],
    entity: Entity,
    type_id: &'static str,
    body: Entity,
) {
    for (i, field) in fields.iter().enumerate() {
        let r = build_field_row(commands, fonts, field, entity, type_id);
        commands
            .entity(r)
            .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(i)));
        commands.entity(body).add_child(r);
    }
}

/// Keep each section body's contents in sync with its header's open flag: fill
/// on expand, throw the rows away on collapse.
///
/// Reconciliation rather than click handling, deliberately — `set_section_open`
/// (expand/collapse-all, and the expand-default policy) moves sections without
/// any click, and observing the resulting *state* covers every path at once.
///
/// Exclusive because native drawers are `fn(&mut World, Entity) -> Entity`.
pub(super) fn reconcile_section_bodies(
    world: &mut World,
    // Scoped to inspector headers: `Section` is a shared ember widget, so a bare
    // `&Section` query would walk every collapsible section in the editor.
    mut headers: Local<QueryState<&Section, With<InspectorSectionHeader>>>,
) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };

    // (body, open) for every section whose body disagrees with its header.
    //
    // "Should hold rows" is the header's open flag AND the body being on screen
    // — see [`super::cull::cull_offscreen_sections`]. Folding culling in here
    // rather than giving it its own despawn path means there is still exactly one
    // place that builds and one that throws away, so the two can't drift.
    let mut todo: Vec<(Entity, bool)> = Vec::new();
    for sec in headers.iter(world) {
        let body = sec.body();
        let culled = world.get::<SectionCull>(body).is_some_and(|c| c.culled);
        let want = sec.is_open() && !culled;
        if let Some(spec) = world.get::<SectionBodySpec>(body) {
            if spec.filled != want {
                todo.push((body, want));
            }
        }
    }
    if todo.is_empty() {
        return;
    }

    for (body, open) in todo {
        let Some(spec) = world.get::<SectionBodySpec>(body) else {
            continue;
        };
        let (fields, ent, type_id, drawer, custom) = (
            spec.fields.clone(),
            spec.entity,
            spec.type_id,
            spec.native_drawer,
            spec.custom,
        );
        // A despawned inspected entity outlives its rows here (selection can
        // change in the same frame a section is toggled) — skip rather than
        // build rows whose accessors would miss.
        if open && world.get_entity(ent).is_err() {
            continue;
        }

        let existing: Vec<Entity> = world
            .get::<Children>(body)
            .map(|ch| ch.iter().collect())
            .unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for child in existing {
                // try_despawn: a row's own binding may have retired it already.
                commands.entity(child).try_despawn();
            }
            if open {
                if drawer.is_some() {
                    // filled below, after the queue applies (needs &mut World)
                } else if custom {
                    let note =
                        empty_label(&mut commands, &fonts, &renzora::lang::t("inspector.custom_pending"));
                    commands.entity(body).add_child(note);
                } else {
                    fill_section_rows(&mut commands, &fonts, &fields, ent, type_id, body);
                }
            }
        }
        queue.apply(world);

        if open {
            if let Some(drawer) = drawer {
                let content = drawer(world, ent);
                if let Ok(mut em) = world.get_entity_mut(body) {
                    em.add_child(content);
                }
            }
        }
        if let Some(mut spec) = world.get_mut::<SectionBodySpec>(body) {
            spec.filled = open;
        }
    }
}

pub(super) fn build_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    sec: &SectionSpec,
    entity: Entity,
) -> (Entity, Entity) {
    // Compose the shared ember section (caret · accent icon · title + colored
    // header + ember-owned collapse); override the body padding to the inspector's
    // tighter spacing and add the lock/enable/trash affordances to the header.
    // `sec.title` stays the English identity (sort priority, collapse-state key);
    // localize only the displayed string.
    let sec_title = comp_name_loc(sec.title);
    let (root, header, body) = renzora_ember::widgets::section_with_header_open(
        commands,
        fonts,
        sec.icon,
        &sec_title,
        sec.accent,
        sec.header_bg,
        sec.open,
    );
    commands.entity(header).insert(InspectorSectionHeader {
        type_id: sec.type_id,
        header_bg: sec.header_bg,
    });
    // Compact the shared section for the inspector: kill the widget's 8px
    // bottom margin + header↔body gap so component cards stack flush, and
    // tighten the header's vertical padding. (Full `Node` overrides — mirror
    // the widget's other layout fields when changing them.)
    commands.entity(root).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        ..default()
    });
    commands.entity(header).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    commands.entity(body).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(4.0)),
        // Preserve the collapsed state `section_with_header_open` encoded in the
        // body's `display`; a bare `Node` would default to `Flex` and show a
        // start-collapsed section, desyncing it from its `Section.open` flag (the
        // first collapse click would then no-op).
        display: if sec.open { Display::Flex } else { Display::None },
        ..default()
    });
    // A COLLAPSED section builds nothing — it records how to fill itself and
    // `reconcile_section_bodies` does so when (if) the user expands it.
    //
    // Collapsing is not enough on its own: `section_with_header_open` sets the
    // body to `Display::None`, and `bevy_ui` does NOT prune hidden subtrees from
    // its per-frame walk — `compute_hidden_layout` clears the cache and recurses,
    // so a hidden row is *never* cached and pays full layout every frame, forever.
    // An entity with a dozen components and two sections open was laying out every
    // row of the other ten. Native drawers were worse: `drawer(world, ent)` ran and
    // built its whole content for a section nobody could see.
    commands.entity(body).insert((
        SectionBodySpec {
            fields: sec.fields.clone(),
            entity,
            type_id: sec.type_id,
            native_drawer: sec.native_drawer,
            custom: sec.custom,
            filled: sec.open,
        },
        // Starts unmeasured, so a freshly built section is never culled before
        // it has a height to reserve. See `SectionCull::placeholder_h`.
        SectionCull::default(),
    ));
    if !sec.open {
        // nothing built — `reconcile_section_bodies` fills it on expand
    } else if sec.native_drawer.is_some() {
        // Body is filled by the registered native drawer once the build queue
        // has applied (it needs exclusive &mut World). See `rebuild_inspector`.
    } else if sec.custom {
        let note = empty_label(commands, fonts, &renzora::lang::t("inspector.custom_pending"));
        commands.entity(body).add_child(note);
    } else {
        fill_section_rows(commands, fonts, &sec.fields, entity, sec.type_id, body);
    }

    // Header affordances: a spacer pushes the optional lock / enable / trash to
    // the right of the title.
    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
        .id();
    // (The inspector lock used to hang off the "ID" section header. It belongs
    // to the entity rather than to any one component, so it moved to the entity
    // header with the rest of the identity controls — see `build_lock_button`.)
    let mut extra = vec![spacer];
    if let Some((_, set_enabled)) = sec.enable.clone() {
        let sw = toggle_switch(commands, sec.enabled_now);
        // Block the press from bubbling to the section header behind it, so
        // flipping the enable switch doesn't also collapse/expand the section
        // (same reason the lock/trash glyphs above set FocusPolicy::Block).
        commands.entity(sw).insert(FocusPolicy::Block);
        let g = sec.enable.clone().unwrap().0;
        let sec_cid = sec.cid;
        bind_2way(
            commands,
            sw,
            move |w| tracked_read(w, entity, sec_cid, |world| g(world, entity)),
            move |w, v: &bool| {
                let target = *v;
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(EnableToggleCmd { entity, set_enabled: set_enabled.clone(), target }),
                );
            },
        );
        extra.push(sw);
    }
    // Scripts and Material hide the header trash: both manage their own
    // contents (per-script remove; the material drawer's own binding controls),
    // so a whole-component delete here is a one-click data-loss hazard. Their
    // registry `remove_fn` stays — it's also the undo half of Add Component.
    let hide_trash = matches!(sec.type_id, "script_component" | "material_ref");
    if let (Some(remove_fn), false) = (sec.remove_fn.clone(), hide_trash) {
        let trash = phosphor_glyph(commands, fonts, "trash", renzora_ember::theme::text_muted(), 13.0);
        commands.entity(trash).insert((
            Interaction::default(),
            FocusPolicy::Block,
            RemoveBtn {
                remove_fn,
                entity,
                type_id: sec.type_id,
            },
        ));
        extra.push(trash);
    }
    commands.entity(header).add_children(&extra);

    (root, body)
}
