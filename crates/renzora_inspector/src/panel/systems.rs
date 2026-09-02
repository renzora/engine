//! Every click the panel answers, the expand/collapse bookkeeping, and the Add
//! Component overlay.

use bevy::prelude::*;

use renzora_editor_framework::{
    EditorCommands, EditorSelection, EditorSettings, FieldValue, InspectorExpandDefault,
};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{set_section_open, Section};

use super::rebuild::AddButton;
use super::section::{AddKeyframeBtn, FieldButton, LockBtn, RemoveBtn, ResetBtn};
use super::undo::{AddComponentCmd, AddPluginComponentCmd, RemoveComponentCmd};
use super::{
    policy_open, record_field_change, InspectorRoot, InspectorSectionHeader, InspectorSectionsOpen,
    InspectorState, Mutate,
};

pub(super) fn remove_click(
    q: Query<(&Interaction, &RemoveBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (remove_fn, entity, type_id) = (btn.remove_fn.clone(), btn.entity, btn.type_id);
        cmds.push(move |w: &mut World| {
            let ctx = renzora_undo::active_context(w);
            renzora_undo::execute(
                w,
                ctx,
                Box::new(RemoveComponentCmd {
                    entity,
                    type_id,
                    remove_fn,
                    captured: None,
                }),
            );
        });
    }
}

pub(super) fn lock_click(
    q: Query<(&Interaction, &LockBtn), Changed<Interaction>>,
    mut state: ResMut<InspectorState>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.locked = if state.locked == Some(btn.entity) {
            None
        } else {
            Some(btn.entity)
        };
    }
}

/// Expand/collapse-all button: drives the live section headers directly (no
/// rebuild, so it's instant and can't flicker). Smart toggle — if *any* section
/// is collapsed, open them all; otherwise collapse them all.
pub(super) fn expand_all_click(
    q: Query<&Interaction, (With<super::ExpandAllButton>, Changed<Interaction>)>,
    mut sections: Query<&mut Section, With<InspectorSectionHeader>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let target_open = sections.iter().any(|s| !s.is_open());
    for mut sec in &mut sections {
        if sec.is_open() != target_open {
            set_section_open(&mut sec, target_open, &mut nodes, &mut texts);
        }
    }
}

/// Zebra-stripe collapsed component headers: a closed section's header takes
/// the odd/even row colour (by its position in the component list) so the
/// flush-stacked collapsed cards read as distinct rows; an open header keeps
/// its per-category colour. Runs off the live [`Section`] flag, so it tracks
/// header clicks and the expand/collapse-all button without a rebuild.
pub(super) fn stripe_collapsed_headers(
    root: Query<&Children, With<InspectorRoot>>,
    sections: Query<&Children>,
    mut headers: Query<(&Section, &InspectorSectionHeader, &mut BackgroundColor)>,
) {
    // Derive the stripe index from the LIVE child order rather than a baked-in
    // position. Position is presentation, not identity — storing it on the header
    // made inserting a section near the top look like a content change for
    // everything below it, which would force needless rebuilds once the section
    // list is reconciled rather than rewritten.
    let Ok(children) = root.single() else {
        return;
    };
    for (i, section_root) in children.iter().enumerate() {
        // A section's header is its first child (see `build_section`).
        let Some(header) = sections.get(section_root).ok().and_then(|c| c.iter().next()) else {
            continue;
        };
        let Ok((sec, hdr, mut bg)) = headers.get_mut(header) else {
            continue;
        };
        let want = if sec.is_open() {
            renzora_ember::theme::rgb(hdr.header_bg)
        } else {
            renzora_ember::inspector::inspector_stripe(i)
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Persist a section's collapse state under its component type whenever the user
/// toggles it, so it survives rebuilds and follows the user across selections.
/// Mirrors `scripts.rs`'s `remember_script_sections`, keyed by type rather than
/// by `(entity, script_id)` — see [`InspectorSectionsOpen`].
pub(super) fn remember_inspector_sections(
    changed: Query<(&Section, &InspectorSectionHeader), Changed<Section>>,
    mut open: ResMut<InspectorSectionsOpen>,
) {
    for (sec, hdr) in &changed {
        open.0.insert(hdr.type_id, sec.is_open());
    }
}

/// Keep the Inspector Expand Default setting authoritative.
///
/// Without this the setting is unreachable once the user has toggled anything:
/// remembered per-type state would always win, and simply clearing the map is not
/// enough either — a rebuild is not guaranteed, so the live sections would keep
/// their old state. Wipe the memory *and* drive the live `Section`s, the same way
/// `expand_all_click` does.
pub(super) fn apply_expand_policy_change(
    settings: Res<EditorSettings>,
    mut open: ResMut<InspectorSectionsOpen>,
    mut sections: Query<(&mut Section, &InspectorSectionHeader)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
    mut last: Local<Option<InspectorExpandDefault>>,
) {
    let policy = settings.inspector_expand_default;
    if *last == Some(policy) {
        return;
    }
    // Skip the first observation: that is startup, not a user change, and the
    // sections built since already honour the policy.
    let first_run = last.is_none();
    *last = Some(policy);
    if first_run {
        return;
    }

    open.0.clear();
    for (mut sec, hdr) in &mut sections {
        let want = policy_open(policy, hdr.type_id);
        if sec.is_open() != want {
            set_section_open(&mut sec, want, &mut nodes, &mut texts);
        }
    }
}

/// Keep the expand-all button's glyph reflecting the current state: a "collapse"
/// icon once every section is open, an "expand" icon otherwise.
pub(super) fn sync_expand_glyph(
    sections: Query<&Section, With<InspectorSectionHeader>>,
    mut glyph: Query<&mut Text, With<super::ExpandAllGlyph>>,
) {
    // No sections (nothing selected) → leave it on the default "expand" glyph.
    let all_open = !sections.is_empty() && sections.iter().all(|s| s.is_open());
    let name = if all_open {
        "arrows-in-line-vertical"
    } else {
        "arrows-out-line-vertical"
    };
    let Some(g) = renzora_ember::font::icon_glyph(name) else {
        return;
    };
    let g = g.to_string();
    for mut t in &mut glyph {
        if t.0 != g {
            t.0 = g.clone();
        }
    }
}

pub(super) fn add_button_click(
    q: Query<&Interaction, (With<AddButton>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        cmds.push(open_add_component);
    }
}

/// Run a `FieldType::Button`'s action when its widget is pressed. The set_fn is
/// invoked with `FieldValue::Bool(true)` as the "pressed" signal.
pub(super) fn field_button_click(
    q: Query<(&Interaction, &FieldButton), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (set_fn, entity) = (btn.set_fn.clone(), btn.entity);
        cmds.push(move |w: &mut World| set_fn(w, entity, FieldValue::Bool(true)));
    }
}

/// Reset a field to its default when its reset button is pressed. We read the
/// current value first only to recover the `FieldValue` variant, then write the
/// matching `type_default()` back through the field's own `set_fn`.
pub(super) fn reset_click(
    q: Query<(&Interaction, &ResetBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (get_fn, set_fn, entity, name) = (btn.get_fn.clone(), btn.set_fn.clone(), btn.entity, btn.field_name);
        cmds.push(move |w: &mut World| {
            if let Some(cur) = get_fn(w, entity) {
                record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), cur.type_default());
            }
        });
    }
}

/// Queue a keyframe-add when a field's keyframe button is pressed. The timeline
/// editor drains [`renzora::KeyframeRequests`] and keys the entity's live value
/// at the playhead onto the matching track (the undo is recorded there).
pub(super) fn add_keyframe_click(
    q: Query<(&Interaction, &AddKeyframeBtn), Changed<Interaction>>,
    reqs: Option<ResMut<renzora::KeyframeRequests>>,
) {
    let Some(mut reqs) = reqs else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        reqs.push(btn.entity, btn.component.clone(), btn.field.clone());
    }
}

/// Open the shared ember search overlay listing every addable component that the
/// inspected entity doesn't already have.
fn open_add_component(world: &mut World) {
    use bevy::ecs::world::CommandQueue;

    let entity = {
        let st = world.resource::<InspectorState>();
        st.locked
            .or_else(|| world.get_resource::<EditorSelection>().and_then(|s| s.get()))
    };
    let Some(entity) = entity else {
        return;
    };
    // Snapshot the registry (copying fn ptrs + &'static metadata) so the
    // has_fn / overlay build don't alias the registry borrow.
    type Spec = (
        &'static str,
        &'static str,
        &'static str,
        fn(&World, Entity) -> bool,
        fn(&mut World, Entity),
        Option<fn(&mut World, Entity)>,
    );
    let specs: Vec<Spec> = world
        .get_resource::<renzora_editor_framework::InspectorRegistry>()
        .map(|reg| {
            reg.iter()
                .filter_map(|e| {
                    e.add_fn
                        .map(|af| (e.display_name, e.icon, e.category, e.has_fn, af, e.remove_fn))
                })
                .collect()
        })
        .unwrap_or_default();

    // Per-camera effects only render on a `Camera3d`: the curated `"camera"`
    // image-quality set (tonemapping, exposure, bloom, DOF, AA, …) and the open
    // `"post_process"` shader effects (rain, glitch, CRT, … — they carry an
    // `extract_component_filter(With<Camera3d>)`). Offer them only when a camera
    // is selected, so they don't show on a cube where they'd silently do nothing.
    let is_camera = world.get::<Camera3d>(entity).is_some();

    let mut entries: Vec<renzora_ember::widgets::SearchEntry> = Vec::new();
    for (label, icon, category, has_fn, add_fn, remove_fn) in specs {
        if has_fn(world, entity) {
            continue; // already present
        }
        if matches!(category, "camera" | "post_process") && !is_camera {
            continue; // per-camera effect on a non-camera entity
        }
        entries.push(renzora_ember::widgets::SearchEntry::new(
            icon,
            label,
            category,
            move |w: &mut World| {
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(AddComponentCmd {
                        entity,
                        // The Add Component overlay is fed purely from the
                        // hand-written registry, so these are still plain fn
                        // pointers; they coerce here.
                        add_fn: std::sync::Arc::new(add_fn),
                        remove_fn: remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
                    }),
                );
            },
        ));
    }

    // NOTE: Add Component is deliberately NOT fed from reflection.
    //
    // Inferring addability from `#[reflect(Default)]` was tried and reverted. It
    // is technically correct and practically useless: every ecosystem crate
    // registers its internals, so the menu filled with `AngularDamping`,
    // `CenterOfMass`, `ColliderConstructorHierarchy` — twice each, because
    // avian2d and avian3d both register a type of that name.
    //
    // The deeper reason is a vocabulary mismatch, not a filtering problem.
    // Reflection enumerates *components*; this menu offers *features*. One
    // feature ("Vignette") is a plugin that may own several components, and no
    // amount of per-component metadata reconstructs that grouping. Whatever
    // replaces the registry here has to be declared at plugin level.

    // Plugin components. Injected here rather than through `InspectorRegistry`
    // because `SearchEntry` takes a CLOSURE — so the component id can be captured
    // — whereas `InspectorEntry` is built from bare `fn` pointers that have
    // nowhere to put it.
    let plugin_specs: Vec<(String, bevy::ecs::component::ComponentId, Vec<u8>)> = world
        .get_resource::<renzora_plugin::host::PluginComponentSchemas>()
        .map(|s| {
            s.0.iter()
                // A resource is global — there is no entity to add it to.
                .filter(|i| !i.is_resource)
                .map(|i| (i.display_name.clone(), i.id, i.default_value.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (label, component, default_value) in plugin_specs {
        // Already present — nothing to add.
        if world.get_entity(entity).is_ok_and(|e| e.contains_id(component)) {
            continue;
        }
        let default_value = if default_value.is_empty() {
            // The plugin supplied no default. Zeroed is the only option left, and
            // is at least a valid instance for any POD component.
            let size = world
                .components()
                .get_info(component)
                .map(|i| i.layout().size())
                .unwrap_or(0);
            vec![0u8; size]
        } else {
            default_value
        };
        entries.push(renzora_ember::widgets::SearchEntry::new(
            "puzzle-piece",
            &label,
            "plugin",
            move |w: &mut World| {
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(AddPluginComponentCmd {
                        entity,
                        component,
                        default_value: default_value.clone(),
                    }),
                );
            },
        ));
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        renzora_ember::widgets::search_overlay(&mut commands, &fonts, &renzora::lang::t("inspector.add_component"), entries);
    }
    queue.apply(world);
}
