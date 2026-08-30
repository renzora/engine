use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use renzora_editor_framework::{
    ComponentIconRegistry, EditorLocked, EntityIcon, EntityLabelColor, HideInHierarchy,
    HierarchyFilter, HierarchyOrder,
};

/// A node in the entity tree, built from ECS data. Cached in
/// [`HierarchyTreeCache`] and only rebuilt when the tree actually changes
/// (names, hierarchy, visibility, etc.) — see
/// [`crate::cache::mark_hierarchy_dirty`] / [`crate::cache::update_hierarchy_cache`].
#[derive(Clone)]
pub struct EntityNode {
    pub entity: Entity,
    pub name: String,
    pub icon: &'static str,
    pub icon_color: [u8; 3],
    pub children: Vec<EntityNode>,
    /// Effective label color for rendering — the entity's own color, or the
    /// nearest ancestor's color if this entity hasn't been assigned one.
    pub label_color: Option<[u8; 3]>,
    pub is_visible: bool,
    pub is_locked: bool,
    pub is_default_camera: bool,
    /// Authored-asset badges shown next to the eye/lock toggles: a script (a
    /// `ScriptComponent` entry that isn't a blueprint), a blueprint (a
    /// `.blueprint` script entry), and/or a material (`MaterialRef`). They let
    /// you spot what rides on an entity without opening the inspector.
    pub has_script: bool,
    /// A sound this entity owns is currently audible. Purely live state — it is
    /// not part of the scene, and it changes as often as playback does.
    pub is_emitting: bool,
    pub has_blueprint: bool,
    pub has_material: bool,
    /// Registered type label from `ComponentIconRegistry`, or `None` when the
    /// entity didn't match any registered icon entry. Used by the hierarchy's
    /// "filter by type" UI — `None` is grouped under "Other".
    pub type_name: Option<&'static str>,
}

/// The order in which entities first showed up in the tree — the fallback sort
/// for root entities that carry no explicit [`HierarchyOrder`].
///
/// Roots used to tiebreak on `Entity` itself, which reads like "spawn order" and
/// isn't. `Entity`'s `Ord` compares `to_bits()`, and `to_bits()` puts the
/// *generation* in the high 32 bits — so it sorts by generation first and index
/// second. Index slots are recycled, and the editor recycles constantly (every
/// tooltip, reactive row and status-bar readout it spawns and despawns frees
/// one), so a shape spawned now typically lands in a recycled slot with a high
/// generation and a low index. Two shapes added back to back could therefore
/// appear anywhere relative to each other and to everything already in the
/// scene, and the list visibly reshuffled as unrelated entities came and went.
///
/// A first-seen counter is the honest version of what the `Entity` tiebreak was
/// reaching for. It only ever grows, so a new root always lands at the bottom,
/// which is where you look for the thing you just added.
#[derive(Resource, Default)]
pub struct HierarchySpawnSeq {
    seq: HashMap<Entity, u64>,
    next: u64,
}

impl HierarchySpawnSeq {
    /// Number the entities we haven't seen before and forget the ones that have
    /// gone away. `present` is the *unfiltered* candidate set: numbering only
    /// what survives the hierarchy filter would renumber everything else the
    /// moment the filter cleared, which is exactly the reshuffle this exists to
    /// prevent.
    fn sync(&mut self, present: &[Entity]) {
        let live: HashSet<Entity> = present.iter().copied().collect();
        let mut fresh: Vec<Entity> = present
            .iter()
            .copied()
            .filter(|e| !self.seq.contains_key(e))
            .collect();
        // Several entities can appear in one rebuild (a scene load, a dropped
        // model). Query iteration is archetype-ordered and therefore not stable
        // across frames, so number them by allocation index — the closest thing
        // to spawn order the ECS still remembers once generations are out of the
        // picture.
        fresh.sort_by_key(|e| e.index_u32());
        for e in fresh {
            self.seq.insert(e, self.next);
            self.next += 1;
        }
        self.seq.retain(|e, _| live.contains(e));
    }

    /// This entity's first-seen number. `u64::MAX` for anything not numbered
    /// yet, which sorts it last — the same place a brand-new root belongs.
    fn of(&self, entity: Entity) -> u64 {
        self.seq.get(&entity).copied().unwrap_or(u64::MAX)
    }
}

/// A query filter for "this entity is a candidate for the hierarchy tree".
///
/// **This is the editor-chrome boundary, and it is deliberately a query filter
/// rather than a per-entity check.** The rule is: an entity is scene content
/// unless it is a `bevy_ui` node, and a `bevy_ui` node is scene content only if
/// it is authored game UI (`UiCanvas`/`UiWidget`).
///
/// Expressing it as a filter is the entire point. Bevy resolves `With`/`Without`
/// once per *archetype*, so the ~1500 editor-chrome UI nodes are skipped by
/// never being visited. This used to be four `world.get::<T>(entity)` random
/// lookups per named entity — plus a `world.get::<Name>` on every entity in the
/// world, ~3000 of them, just to find the named ones — all of it repeated on
/// every rebuild, and the rebuild runs up to 10x/sec in an exclusive system.
/// The information was always there in the archetype; we were re-deriving it per
/// entity.
///
/// `HideInHierarchy` and `Gamepad` fold in for free for the same reason.
type HierarchyCandidate = (
    Without<HideInHierarchy>,
    Without<bevy::input::gamepad::Gamepad>,
    Or<(
        Without<bevy::ui::Node>,
        With<renzora_ember::game_ui::UiCanvas>,
        With<renzora_ember::game_ui::UiWidget>,
    )>,
);

/// Whether the tree should show the nodes a `.html` template built.
///
/// Only when it is scoped to `UiCanvas` — i.e. the UI workspace, where the
/// hierarchy *is* the UI tree. Everywhere else these stay hidden, and that is
/// not a preference: a template is rebuilt from its file on every load and
/// hot-reload, so a scene tree offering them would let you select and reorder
/// things the next rebuild silently discards.
///
/// The scene **saver** is unaffected either way — it cascades `HideInHierarchy`
/// through `has_hidden_ancestor` on its own path, which is what stops a
/// template being serialised into the `.bsn` and then thrown away on load.
pub fn ui_scoped_for(filter: Option<&HierarchyFilter>) -> bool {
    matches!(
        filter,
        Some(HierarchyFilter::OnlyWithComponents(names))
            if names.contains(&"UiCanvas")
    )
}

pub fn ui_scoped(world: &World) -> bool {
    ui_scoped_for(world.get_resource::<HierarchyFilter>())
}

/// A display name for a markup node.
///
/// `name="..."` in the template becomes a Bevy `Name`, so a node the author
/// bothered to name says so. The rest are anonymous, and a template is mostly
/// anonymous `<node>`s — so fall back to the tag, inferred from the components
/// the loader attached. An entity id would be technically accurate and useless.
fn markup_label(world: &World, entity: Entity) -> String {
    if let Some(name) = world.get::<Name>(entity) {
        return name.as_str().to_string();
    }
    let has = |f: fn(&World, Entity) -> bool| f(world, entity);
    if has(|w, e| w.get::<bevy::ui::widget::Text>(e).is_some()) {
        // The text itself, trimmed — far more use than the word "text" six
        // times in a row. Interpolation is already resolved by this point, so
        // this is what is actually on screen.
        if let Some(t) = world.get::<bevy::ui::widget::Text>(entity) {
            let s = t.0.trim();
            if !s.is_empty() {
                let clipped: String = s.chars().take(24).collect();
                return if clipped.len() < s.len() {
                    format!("\"{clipped}…\"")
                } else {
                    format!("\"{clipped}\"")
                };
            }
        }
        return "text".to_string();
    }
    if has(|w, e| w.get::<bevy::ui::widget::Button>(e).is_some()) {
        return "button".to_string();
    }
    if has(|w, e| w.get::<bevy::ui::widget::ImageNode>(e).is_some()) {
        return "image".to_string();
    }
    "node".to_string()
}

/// Build the entity tree from the world.
///
/// Takes `&mut World` only to build the candidate `QueryState` (which registers
/// the component ids it needs); everything after that is read-only.
pub fn build_entity_tree(world: &mut World, spawn_seq: &mut HierarchySpawnSeq) -> Vec<EntityNode> {
    let mut candidates = world.query_filtered::<(Entity, &Name), HierarchyCandidate>();
    // Markup nodes, revealed only under a UI-scoped filter — see
    // `ui_scoped`. Built unconditionally because a `QueryState` has to
    // register its component ids while the borrow is still mutable; iterating it
    // is what's conditional.
    let mut markup = world
        .query_filtered::<Entity, (With<renzora_ember::markup::provenance::MarkupSource>, With<bevy::ui::Node>)>();
    let show_markup = ui_scoped(world);
    let world: &World = world;

    // One row per entity, as `(entity, label)`. Two sources, one body: markup
    // nodes are excluded from `HierarchyCandidate` at the archetype level (they
    // carry `HideInHierarchy`, and most have no `Name`), so they cannot come out
    // of the same query — but everything downstream treats them identically.
    let mut rows: Vec<(Entity, String)> = candidates
        .iter(world)
        .map(|(e, n)| (e, n.as_str().to_string()))
        .collect();
    if show_markup {
        rows.extend(markup.iter(world).map(|e| (e, markup_label(world, e))));
    }

    // Number every candidate *before* the filters below prune any — see
    // `HierarchySpawnSeq::sync`.
    let all_candidates: Vec<Entity> = rows.iter().map(|(e, _)| *e).collect();
    spawn_seq.sync(&all_candidates);
    // Resolve hierarchy filter — map component type names to ComponentIds.
    let resolve_ids = |names: &Vec<&'static str>| -> Vec<bevy::ecs::component::ComponentId> {
        let Some(registry) = world.get_resource::<AppTypeRegistry>() else {
            return Vec::new();
        };
        let registry = registry.read();
        names
            .iter()
            .filter_map(|name| {
                let reg = registry.iter().find(|r| {
                    let table = r.type_info().type_path_table();
                    table.short_path() == *name || (table.ident() == Some(*name))
                })?;
                world.components().get_id(reg.type_id())
            })
            .collect()
    };
    let (include_ids, exclude_ids): (Vec<_>, Vec<_>) = match world.get_resource::<HierarchyFilter>()
    {
        Some(HierarchyFilter::OnlyWithComponents(names)) => (resolve_ids(names), Vec::new()),
        Some(HierarchyFilter::ExcludeDescendantsOf(names)) => (Vec::new(), resolve_ids(names)),
        _ => (Vec::new(), Vec::new()),
    };
    let filter_component_ids = include_ids;

    struct Entry {
        entity: Entity,
        name: String,
        icon: &'static str,
        color: [u8; 3],
        parent: Option<Entity>,
        label_color: Option<[u8; 3]>,
        is_visible: bool,
        is_locked: bool,
        is_default_camera: bool,
        has_script: bool,
        is_emitting: bool,
        has_blueprint: bool,
        has_material: bool,
        type_name: Option<&'static str>,
    }

    let mut entries: Vec<Entry> = Vec::new();
    let mut named_entities: HashSet<Entity> = HashSet::new();

    {
        for (entity, name) in rows.iter().map(|(e, n)| (*e, n)) {
            // Apply component filter: skip entities unless they or an ancestor
            // have one of the required components (so children of matching
            // entities still appear in the hierarchy).
            if !filter_component_ids.is_empty() {
                let mut found = false;
                let mut check = entity;
                loop {
                    let er = world.entity(check);
                    if filter_component_ids.iter().any(|id| er.contains_id(*id)) {
                        found = true;
                        break;
                    }
                    match world.get::<ChildOf>(check) {
                        Some(c) => check = c.parent(),
                        None => break,
                    }
                }
                if !found {
                    continue;
                }
            }
            if !exclude_ids.is_empty() {
                let mut excluded = false;
                let mut check = entity;
                loop {
                    let er = world.entity(check);
                    if exclude_ids.iter().any(|id| er.contains_id(*id)) {
                        excluded = true;
                        break;
                    }
                    match world.get::<ChildOf>(check) {
                        Some(c) => check = c.parent(),
                        None => break,
                    }
                }
                if excluded {
                    continue;
                }
            }
            // `HideInHierarchy`, the editor's own bevy_ui chrome, and gamepad
            // device entities are all excluded by `HierarchyCandidate` at the
            // archetype level — see its doc comment for why that matters.
            //
            // Skip children of gamepad entities (axis/button sub-entities)
            // and any entity whose name indicates it's a system gamepad device.
            if let Some(child_of) = world.get::<ChildOf>(entity) {
                if world
                    .get::<bevy::input::gamepad::Gamepad>(child_of.parent())
                    .is_some()
                {
                    continue;
                }
            }
            // Children of `HideInHierarchy` parents are NOT auto-hidden — that
            // lets us mark GLTF wrapper nodes (`SceneRoot`, `RootNode.NNN`)
            // hidden so the dropped model's mesh entities promote into the
            // model root rather than appearing under invisible plumbing.
            // Callers that genuinely want to hide a whole subtree mark each
            // descendant individually (see `studio_preview` for the pattern).
            let name_str = name.clone();
            let (icon, color) = entity_icon(world, entity);
            let type_name = world
                .get_resource::<ComponentIconRegistry>()
                .and_then(|reg| reg.entity_type_name(world, entity));
            let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
            let label_color = world.get::<EntityLabelColor>(entity).map(|c| c.0);
            // While the viewport gate has the scene force-hidden (no viewport
            // panel visible), the eye icon shows the *authored* visibility the
            // gate stashed, not the temporary `Hidden` override.
            let is_visible = world
                .get::<renzora::core::ViewportGateHidden>(entity)
                .map(|g| g.0 != Visibility::Hidden)
                .or_else(|| {
                    world
                        .get::<Visibility>(entity)
                        .map(|v| *v != Visibility::Hidden)
                })
                .unwrap_or(true);
            let is_locked = world.get::<EditorLocked>(entity).is_some();
            let is_default_camera = world.get::<renzora::core::DefaultCamera>(entity).is_some();

            // Asset badges. A `.blueprint` script entry is a blueprint; any other
            // entry (a `.lua`/`.rhai` file or a registered `script_id`) is a
            // plain script, so an entity can legitimately show both badges.
            let is_blueprint = |e: &renzora_scripting::ScriptEntry| {
                e.script_path
                    .as_ref()
                    .and_then(|p| p.extension())
                    .is_some_and(|x| x.eq_ignore_ascii_case("blueprint"))
            };
            let (has_script, has_blueprint) = world
                .get::<renzora_scripting::ScriptComponent>(entity)
                .map(|sc| {
                    (
                        sc.scripts.iter().any(|e| !is_blueprint(e)),
                        sc.scripts.iter().any(is_blueprint),
                    )
                })
                .unwrap_or((false, false));
            let has_material = world.get::<renzora::core::MaterialRef>(entity).is_some();
            // One bit, read through the contract crate's marker — the hierarchy
            // links no audio crate to ask this. See `renzora::AudioEmitting`.
            let is_emitting = world.get::<renzora::AudioEmitting>(entity).is_some();

            named_entities.insert(entity);
            entries.push(Entry {
                entity,
                name: name_str,
                icon,
                color,
                parent,
                label_color,
                is_visible,
                is_locked,
                is_default_camera,
                has_script,
                has_blueprint,
                has_material,
                is_emitting,
                type_name,
            });
        }
    }

    let mut children_map: HashMap<Entity, Vec<usize>> = HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        // Walk up the ancestor chain to find the nearest named parent.
        // This handles unnamed intermediaries (e.g. SceneRoot entities in GLTF
        // hierarchies) by reparenting children to the closest visible ancestor.
        let mut resolved_parent = None;
        if let Some(mut p) = entry.parent {
            loop {
                if named_entities.contains(&p) {
                    resolved_parent = Some(p);
                    break;
                }
                match world.get::<ChildOf>(p) {
                    Some(child_of) => p = child_of.parent(),
                    None => break,
                }
            }
        }
        match resolved_parent {
            Some(p) => {
                children_map.entry(p).or_default().push(i);
            }
            None => {
                root_indices.push(i);
            }
        }
    }

    // Sort root entities by an explicit `HierarchyOrder` (written by a
    // drag-reorder) and fall back to the order they entered the tree, so a
    // freshly spawned root lands at the bottom. The fallback used to be `Entity`
    // itself, which sorts generation-first — see [`HierarchySpawnSeq`].
    root_indices.sort_by_key(|&idx| {
        let entity = entries[idx].entity;
        let order = world
            .get::<HierarchyOrder>(entity)
            .map(|h| h.0)
            .unwrap_or(u32::MAX);
        (order, spawn_seq.of(entity))
    });

    // Sort children by a key that's deterministic even when entries were
    // promoted through a hidden ancestor (e.g. a `RootNode_2` wrapper).
    //
    // The sort key for each entry is a path of positions: starting from the
    // entry, walk toward the resolved parent and collect the entry's index
    // inside each direct parent's `Children` component along the way. This
    // preserves the GLB-authored order even when intermediate wrappers are
    // hidden, and is stable across archetype iteration order changes (which
    // shift every frame in play mode and would otherwise scramble promoted
    // siblings here).
    let position_in_parent = |entity: Entity, parent: Entity, world: &World| -> usize {
        world
            .get::<Children>(parent)
            .and_then(|children| children.iter().position(|c| c == entity))
            .unwrap_or(usize::MAX)
    };

    let chain_key = |idx: usize, resolved_parent: Entity, world: &World| -> Vec<usize> {
        let entity = entries[idx].entity;
        let mut path = Vec::new();
        let mut current = entity;
        while current != resolved_parent {
            let Some(direct_parent) = world.get::<ChildOf>(current).map(|c| c.parent()) else {
                break;
            };
            path.push(position_in_parent(current, direct_parent, world));
            current = direct_parent;
        }
        path.reverse();
        path
    };

    for (parent_entity, child_indices) in &mut children_map {
        let parent = *parent_entity;
        // Decorate-sort-undecorate so we don't recompute the chain on every
        // comparison. Tiebreak by Entity for determinism when keys collide
        // (shouldn't happen for valid hierarchies, but cheap insurance).
        let mut keyed: Vec<(Vec<usize>, Entity, usize)> = child_indices
            .iter()
            .map(|&idx| (chain_key(idx, parent, world), entries[idx].entity, idx))
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        *child_indices = keyed.into_iter().map(|(_, _, idx)| idx).collect();
    }

    fn build_node(
        index: usize,
        entries: &[Entry],
        children_map: &HashMap<Entity, Vec<usize>>,
        inherited_label_color: Option<[u8; 3]>,
    ) -> EntityNode {
        let entry = &entries[index];
        // An explicit color on this entity wins; otherwise inherit from the
        // nearest ancestor that had one. Children further down inherit the
        // effective color so the cascade continues through subtrees.
        let effective_label_color = entry.label_color.or(inherited_label_color);
        let mut children = Vec::new();

        if let Some(child_indices) = children_map.get(&entry.entity) {
            for &ci in child_indices {
                children.push(build_node(ci, entries, children_map, effective_label_color));
            }
        }

        let final_icon = if !children.is_empty() && entry.icon == "circle" {
            "folder"
        } else {
            entry.icon
        };
        let final_color = if !children.is_empty() && entry.icon == "circle" {
            [170, 175, 190]
        } else {
            entry.color
        };

        EntityNode {
            entity: entry.entity,
            name: entry.name.clone(),
            icon: final_icon,
            icon_color: final_color,
            children,
            label_color: effective_label_color,
            is_visible: entry.is_visible,
            is_locked: entry.is_locked,
            is_default_camera: entry.is_default_camera,
            has_script: entry.has_script,
            is_emitting: entry.is_emitting,
            has_blueprint: entry.has_blueprint,
            has_material: entry.has_material,
            type_name: entry.type_name,
        }
    }

    root_indices
        .iter()
        .map(|&i| build_node(i, &entries, &children_map, None))
        .collect()
}

/// Detect an icon and color for an entity using the `ComponentIconRegistry`.
/// Falls back to a generic circle icon if no match is found.
///
/// An [`EntityIcon`] override replaces the *glyph* only — the colour keeps
/// coming from the registry, so an overridden light still reads as a light in
/// the tree's colour language and the override says what kind of one it is.
fn entity_icon(world: &World, entity: Entity) -> (&'static str, [u8; 3]) {
    let (icon, color) = world
        .get_resource::<ComponentIconRegistry>()
        .and_then(|registry| registry.entity_icon(world, entity))
        .unwrap_or(("circle", [150, 150, 165]));
    match world.get::<EntityIcon>(entity) {
        // Resolved through the curated table rather than used raw: the icon
        // name has to outlive the `EntityNode` that carries it, and a name the
        // font has no glyph for would draw an empty box instead of falling back.
        Some(o) => (renzora_editor_framework::entity_icon_name(&o.0).unwrap_or(icon), color),
        None => (icon, color),
    }
}

/// Filter the tree to only include nodes whose name matches the search.
pub fn filter_tree(nodes: Vec<EntityNode>, search: &str) -> Vec<EntityNode> {
    let lower = search.to_lowercase();
    nodes
        .into_iter()
        .filter_map(|node| filter_node(node, &lower))
        .collect()
}

fn filter_node(node: EntityNode, search: &str) -> Option<EntityNode> {
    let name_matches = node.name.to_lowercase().contains(search);
    let filtered_children: Vec<EntityNode> = node
        .children
        .into_iter()
        .filter_map(|child| filter_node(child, search))
        .collect();

    if name_matches || !filtered_children.is_empty() {
        Some(EntityNode {
            children: filtered_children,
            ..node
        })
    } else {
        None
    }
}

/// Filter the tree to only include nodes whose type label is in `allowed`,
/// or whose descendants are. `None`-typed nodes match the sentinel
/// `"__other__"` so the popup can offer an "Other" toggle for entities that
/// don't match any registered type.
pub fn filter_tree_by_type(
    nodes: Vec<EntityNode>,
    allowed: &std::collections::HashSet<&'static str>,
) -> Vec<EntityNode> {
    nodes
        .into_iter()
        .filter_map(|node| filter_node_by_type(node, allowed))
        .collect()
}

fn filter_node_by_type(
    node: EntityNode,
    allowed: &std::collections::HashSet<&'static str>,
) -> Option<EntityNode> {
    let key = node.type_name.unwrap_or("__other__");
    let type_matches = allowed.contains(key);
    let filtered_children: Vec<EntityNode> = node
        .children
        .into_iter()
        .filter_map(|child| filter_node_by_type(child, allowed))
        .collect();

    if type_matches || !filtered_children.is_empty() {
        Some(EntityNode {
            children: filtered_children,
            ..node
        })
    } else {
        None
    }
}
