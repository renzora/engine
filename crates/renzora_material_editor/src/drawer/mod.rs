//! The Material component's inspector drawer.
//!
//! Three stacked pieces, in the order an artist reaches for them:
//!
//! 1. **The material slot** ([`slot`]) — which `.material` the entity uses: a
//!    preview square, a two-line picker field (name over folder) that opens a
//!    grid of material previews, and a whole-card drop target. The field *is* the
//!    picker, which is why there's no separate "browse" button; and the picker
//!    shows pictures rather than a text list, because a material is a thing you
//!    recognise by looking at it.
//! 2. **Texture slots** ([`textures`]) — one row per PBR channel. Dropping an
//!    image on a row wires it into the material graph: the sampler node is
//!    created, connected to the matching output pin, and the material recompiled
//!    and saved. Dropping a *set* of images on the material slot above routes
//!    each one by its filename (`rock_normal.png` → Normal, `rock_ORM.png` → all
//!    three packed channels). This is the whole point of the drawer — the common
//!    case is six PNGs and a mesh, and it should not require opening the node
//!    editor at all.
//! 3. **Overrides** ([`overrides`]) — for a derived (instance) material, the
//!    master's named parameters. Texture slots are hidden there: the graph belongs
//!    to the master, and editing it from an instance would change every sibling.
//!
//! Neither section stores anything of its own. Texture slots are a *view* of
//! the graph via [`renzora_shader::material::texture_slots`], so a drop here and
//! a wire dragged in the graph editor cannot disagree. Overrides live in the
//! `.material` instance file (not ECS data), so they're loaded into [`MatCache`]
//! on (entity, path) change; param widgets edit the cache and
//! [`overrides::flush_overrides`] writes it back + invalidates the resolver.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_editor_framework::{AppEditorExt, SplashState};
use renzora_ember::reactive::Rx;

use renzora_shader::material::codegen::{MaterialParam, ParamKind};
use renzora_shader::material::graph::MaterialGraph;
use renzora_shader::material::instance::MaterialInstance;
use renzora_shader::material::material_ref::{MaterialRef, ParamValue};

pub(crate) mod build;
pub(crate) mod create;
pub(crate) mod drop;
pub(crate) mod index;
pub(crate) mod overrides;
pub(crate) mod picker;
pub(crate) mod slot;
pub(crate) mod textures;

pub struct MaterialDrawer;

impl Plugin for MaterialDrawer {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatCache>();
        app.init_resource::<MatPickerFilter>();
        app.init_resource::<index::MaterialIndex>();
        app.init_resource::<TexSlotsExpanded>();
        app.register_native_inspector_ui("material_ref", build::material_drawer_root);
        app.add_systems(
            Update,
            (
                build::rebuild_material,
                // Only ticks while a picker popup exists. The rows themselves are
                // a keyed list driven by `MaterialIndex.generation`, so a walk
                // that lands here is picked up by the next snapshot.
                index::refresh_material_index.run_if(any_with_component::<picker::MatPickerPanel>),
                overrides::flush_overrides,
                drop::mat_slot_drop,
                drop::mat_slot_drop_highlight,
                drop::mat_edit_click,
                create::mat_create_click,
                create::mat_create_focus,
                create::mat_create_overlay_buttons,
                drop::mat_clear_click,
                picker::mat_picker_toggle,
                picker::mat_picker_select,
                overrides::mat_revert_click,
                textures::tex_slot_drop,
                textures::tex_slot_highlight,
                textures::tex_slot_browse,
                textures::tex_slot_mute,
                textures::tex_slot_clear,
                textures::tex_slots_expand,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State ────────────────────────────────────────────────────────────────────

/// Loaded `.material` for the inspected entity — the drawer's working copy of
/// whichever of the two file shapes `path` turned out to be. Reloaded on
/// (entity, path, [`rev`](Self::rev)) change; overrides are flushed to disk on
/// edit.
#[derive(Resource, Default)]
pub(crate) struct MatCache {
    pub(super) entity: Option<Entity>,
    pub(super) path: String,
    pub(super) instance_abs: PathBuf,
    pub(super) instance: Option<MaterialInstance>,
    pub(super) params: Vec<MaterialParam>,
    pub(super) dirty: bool,
    /// The graph, when `path` is a master rather than a derived instance. The
    /// texture slots read their state from here; `None` hides them.
    pub(super) graph: Option<MaterialGraph>,
    /// Bumped by every texture-slot edit. Folded into the drawer's rebuild
    /// signature so a drop that rewrote the graph re-reads it — `.material`
    /// files are read with raw `std::fs`, so there is no asset event to react
    /// to, and without this the row would keep showing the old texture.
    pub(super) rev: u64,
}

/// Search text for the material picker popup. No dirty counter: the rows are a
/// keyed list whose token reads `text` directly, so typing re-snapshots and
/// reconciles rather than rebuilding the popup.
#[derive(Resource, Default)]
pub(crate) struct MatPickerFilter {
    pub(super) text: String,
}

/// Whether the drawer is showing every texture channel or just Base Color.
///
/// Seven channel rows are ~230 px of drawer, and on a material with one map
/// bound six of them say "Drop texture" — enough to push the parameters and
/// every component below Material off the bottom of the panel. Collapsed is the
/// default: Base Color is the one channel almost every material has, and the
/// rest are one click away.
///
/// A resource rather than per-drawer state because [`MatCache`] is already
/// single-entity — only one Material drawer is ever built — and folding the flag
/// into the rebuild signature is what makes the click take effect.
#[derive(Resource, Default)]
pub(crate) struct TexSlotsExpanded(pub(super) bool);

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(super) struct MatRoot {
    pub(super) entity: Entity,
    pub(super) sig: Option<u64>,
}
#[derive(Component)]
pub(super) struct MatDropZone {
    pub(super) entity: Entity,
}
#[derive(Component)]
pub(super) struct MatEditBtn {
    pub(super) entity: Entity,
}
/// "New material": writes a fresh `.material` and binds it, *replacing* whatever
/// the mesh pointed at. Distinct from the drop path's
/// [`ensure_material`](drop::ensure_material), which keeps an existing material —
/// here the click itself is the request for a new one, so a mesh sharing a
/// material with five others can be given its own.
#[derive(Component)]
pub(super) struct MatCreateBtn {
    pub(super) entity: Entity,
}
#[derive(Component)]
pub(super) struct MatClearBtn {
    pub(super) entity: Entity,
}
#[derive(Component)]
pub(super) struct MatRevertBtn {
    pub(super) name: String,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub(super) fn material_path(w: &Rx, entity: Entity) -> String {
    w.get::<MaterialRef>(entity).map(|m| m.0.clone()).unwrap_or_default()
}

pub(super) fn material_abs(w: &Rx, path: &str) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    w.get_resource::<CurrentProject>().map(|p| p.resolve_path(path))
}

pub(super) fn sig_of(entity: Entity, path: &str, rev: u64, expanded: bool) -> u64 {
    let mut h = DefaultHasher::new();
    entity.hash(&mut h);
    path.hash(&mut h);
    rev.hash(&mut h);
    expanded.hash(&mut h);
    h.finish()
}

/// Current override value for a param (override if present, else master default).
pub(super) fn ov_get(w: &Rx, name: &str, kind: ParamKind, default_pin_param: &ParamValue) -> ParamValue {
    if let Some(cache) = w.get_resource::<MatCache>() {
        if let Some(inst) = &cache.instance {
            if let Some(v) = inst.overrides.get(name) {
                return v.clone();
            }
        }
    }
    let _ = kind;
    default_pin_param.clone()
}

pub(super) fn ov_set(w: &mut World, name: &str, v: ParamValue) {
    if let Some(mut cache) = w.get_resource_mut::<MatCache>() {
        if let Some(inst) = &mut cache.instance {
            inst.overrides.insert(name.to_string(), v);
            cache.dirty = true;
        }
    }
}
