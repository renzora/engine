//! The Material component's inspector drawer.
//!
//! Three stacked pieces, in the order an artist reaches for them:
//!
//! 1. **The material slot** — which `.material` the entity uses: a card holding
//!    a preview square, a two-line picker field (name over folder) that opens a
//!    grid of material previews, an action row, and a whole-card drop target.
//!    The field *is* the picker, which is why there's no separate "browse"
//!    button any more; and the picker shows pictures rather than a text list,
//!    because a material is a thing you recognise by looking at it.
//! 2. **Texture slots** — one row per PBR channel (base color, normal,
//!    roughness, metallic, AO, emissive). Dropping an image on a row wires it
//!    into the material graph: the sampler node is created, connected to the
//!    matching output pin, and the material recompiled and saved. Dropping a
//!    *set* of images on the material slot above routes each one by its filename
//!    (`rock_normal.png` → Normal, `rock_ORM.png` → all three packed channels).
//!    This is the whole point of the drawer — the common case is six PNGs and a
//!    mesh, and it should not require opening the node editor at all.
//! 3. **Overrides** — for a derived (instance) material, the master's named
//!    parameters. Texture slots are hidden there: the graph belongs to the
//!    master, and editing it from an instance would change every sibling.
//!
//! Neither section stores anything of its own. Texture slots are a *view* of
//! the graph via [`renzora_shader::material::texture_slots`], so a drop here and
//! a wire dragged in the graph editor cannot disagree. Overrides live in the
//! `.material` instance file (not ECS data), so they're loaded into [`MatCache`]
//! on (entity, path) change; param widgets edit the cache and
//! [`flush_overrides`] writes it back + invalidates the resolver.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, IoTaskPool, Task};
use bevy::ui::widget::ImageNode;
use bevy::ui::{FlexWrap, RelativeCursorPosition};

use renzora::core::CurrentProject;
use renzora_editor_framework::{
    open_asset_tab, AppEditorExt, AssetDragPayload, DocTabKind, MaterialThumbnailRegistry, SplashState,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_row, inspector_stripe};
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_with, keyed_list_tokened};
use renzora_ember::theme::{
    accent, border, faint_bg, hover_bg, placeholder, popup_bg, rgb, section_bg, text_muted,
    text_primary,
};
use renzora_ember::widgets::{
    button, checkbox, drag_value, folder_new_button, folder_picker, overlay_sized, text_input, EmberForm,
    EmberTextInput, FolderPick, HoverTint, HoverTooltip,
};

use renzora_shader::material::codegen::{MaterialParam, ParamKind};
use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
use renzora_shader::material::instance::{read_master_parameters, MaterialInstance};
use renzora_shader::material::material_ref::{MaterialRef, ParamValue};
use renzora_shader::material::resolver::{MaterialCache, MaterialResolved};
use renzora_shader::material::texture_slots::{self, TextureSlot, TEXTURE_SLOTS};

use crate::material_inspector::{
    default_param_value, find_material_files, pin_to_param, IMAGE_EXTENSIONS,
};

pub struct NativeMaterialRef;

impl Plugin for NativeMaterialRef {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatCache>();
        app.init_resource::<MatPickerFilter>();
        app.init_resource::<MaterialIndex>();
        app.register_native_inspector_ui("material_ref", material_native);
        app.add_systems(
            Update,
            (
                rebuild_material,
                // Only ticks while a picker popup exists. The rows themselves are
                // a keyed list driven by `MaterialIndex.generation`, so a walk
                // that lands here is picked up by the next snapshot.
                refresh_material_index.run_if(any_with_component::<MatPickerPanel>),
                flush_overrides,
                mat_slot_drop,
                mat_slot_drop_highlight,
                mat_edit_click,
                mat_create_click,
                mat_create_focus,
                mat_create_overlay_buttons,
                mat_clear_click,
                mat_picker_toggle,
                mat_picker_select,
                mat_revert_click,
                tex_slot_drop,
                tex_slot_highlight,
                tex_slot_browse,
                tex_slot_mute,
                tex_slot_clear,
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
struct MatCache {
    entity: Option<Entity>,
    path: String,
    instance_abs: PathBuf,
    instance: Option<MaterialInstance>,
    params: Vec<MaterialParam>,
    dirty: bool,
    /// The graph, when `path` is a master rather than a derived instance. The
    /// texture slots read their state from here; `None` hides them.
    graph: Option<MaterialGraph>,
    /// Bumped by every texture-slot edit. Folded into the drawer's rebuild
    /// signature so a drop that rewrote the graph re-reads it — `.material`
    /// files are read with raw `std::fs`, so there is no asset event to react
    /// to, and without this the row would keep showing the old texture.
    rev: u64,
}

/// Search text for the material picker popup. No dirty counter: the rows are a
/// keyed list whose token reads `text` directly, so typing re-snapshots and
/// reconciles rather than rebuilding the popup.
#[derive(Resource, Default)]
struct MatPickerFilter {
    text: String,
}

/// Cached list of the project's `.material` files, feeding the picker popup.
///
/// The scan is a recursive `read_dir` walk of the project (see
/// [`find_material_files`]). It used to run inline in `rebuild_one_picker`, which
/// rebuilds on **every keystroke** in the picker's search box — so typing one
/// character walked the whole project. Profiling put that path at 13.9 ms in a
/// single frame. The walk now runs on the IO task pool and publishes here.
///
/// Same shape and same reasoning as `renzora_inspector`'s `ScriptIndex`: there is
/// no file-watch signal to hook (`.material` files are read with raw `std::fs`,
/// never through the `AssetServer`), so a slow throttle catches files created by
/// anything other than the editor itself.
#[derive(Resource, Default)]
struct MaterialIndex {
    /// Last completed scan: `(project-relative path, absolute path)`, sorted.
    /// `Arc` so the picker snapshots it without cloning every entry per rebuild.
    materials: Arc<Vec<(String, String)>>,
    /// Bumped only when `materials` actually changes content. The picker's keyed
    /// list folds this into its dirty token, so a periodic rescan that finds
    /// nothing new re-snapshots nothing.
    generation: u64,
    /// Project root the cached scan came from; a change rescans immediately.
    root: Option<PathBuf>,
    /// `Time::elapsed_secs()` when the in-flight walk *started*, so a slow walk
    /// can't immediately trigger the next one. Wall-clock rather than an
    /// accumulated delta because this system is gated on a popup being open.
    last_scan: Option<f32>,
    /// The walk in flight. Never dropped to "cancel" — dropping a bevy `Task`
    /// cancels the work — it is held until `poll_once` yields.
    task: Option<Task<Vec<(String, String)>>>,
}

/// How often to re-walk for `.material` files created by something other than the
/// editor. Matches `ScriptIndex`'s throttle; the popup is short-lived, so in
/// practice this is one walk per time it is opened.
const MATERIAL_SCAN_THROTTLE: f32 = 3.0;

/// Land a finished walk and start a new one when the project changed or the
/// throttle elapsed. Bumps [`MatPickerFilter::sig`] only when the file set
/// actually changed, so a rescan that finds nothing new rebuilds nothing.
fn refresh_material_index(
    mut index: ResMut<MaterialIndex>,
    project: Option<Res<CurrentProject>>,
    time: Res<Time>,
) {
    // Bind the poll result before touching `index.task` again — folding this into
    // the `if let` keeps the `as_mut()` borrow alive across the body.
    let finished = index.task.as_mut().and_then(|t| block_on(poll_once(t)));
    if let Some(materials) = finished {
        index.task = None;
        // Only republish when the set really changed: the generation bump makes
        // the picker re-snapshot, and a rebuilt row loses its thumbnail binding,
        // so a periodic no-op rescan must not churn the list under the user.
        if materials != *index.materials {
            index.materials = Arc::new(materials);
            index.generation = index.generation.wrapping_add(1);
        }
    }

    let Some(project) = project else { return };
    if index.task.is_some() {
        return;
    }

    let now = time.elapsed_secs();
    let root_changed = index.root.as_deref() != Some(project.path.as_path());
    let stale = index.last_scan.is_none_or(|t| now - t >= MATERIAL_SCAN_THROTTLE);
    if !root_changed && !stale {
        return;
    }
    if root_changed {
        index.root = Some(project.path.clone());
        index.materials = Arc::new(Vec::new());
    }
    index.last_scan = Some(now);

    let root = project.path.clone();
    index.task = Some(IoTaskPool::get().spawn(async move { find_material_files(&root) }));
}

#[derive(Component)]
struct MatRoot {
    entity: Entity,
    sig: Option<u64>,
}
#[derive(Component)]
struct MatDropZone {
    entity: Entity,
}
#[derive(Component)]
struct MatEditBtn {
    entity: Entity,
}
/// "New material": writes a fresh `.material` and binds it, *replacing* whatever
/// the mesh pointed at. Distinct from the drop path's [`ensure_material`], which
/// keeps an existing material — here the click itself is the request for a new
/// one, so a mesh sharing a material with five others can be given its own.
#[derive(Component)]
struct MatCreateBtn {
    entity: Entity,
}
#[derive(Component)]
struct MatClearBtn {
    entity: Entity,
}
/// Marks a picker tray. Purely a marker — the tiles are a keyed list that
/// captures its inspected entity directly, so nothing needs to look it up off
/// the tray. Kept because `refresh_material_index` gates on its presence (no
/// tray built → never walk the project) and [`close_pickers`] finds trays by it.
#[derive(Component)]
struct MatPickerPanel;

/// The field that slides its picker tray open, and the two things it drives.
///
/// Deliberately not ember's [`Popup`](renzora_ember::widgets::Popup): that's for
/// panels that *float*, and its positioning system pins one with `top: 100%`,
/// which on an in-flow node offsets it by its own height rather than anchoring
/// it under the trigger.
#[derive(Component)]
struct MatPickerToggle {
    /// The inspected entity, so opening the tray can hide *its* texture rows and
    /// not another drawer's.
    entity: Entity,
    panel: Entity,
    caret: Entity,
}
#[derive(Component)]
struct MatPickerItem {
    entity: Entity,
    rel: String,
}
#[derive(Component)]
struct MatRevertBtn {
    name: String,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn material_path(w: &Rx, entity: Entity) -> String {
    w.get::<MaterialRef>(entity).map(|m| m.0.clone()).unwrap_or_default()
}

fn material_abs(w: &Rx, path: &str) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    w.get_resource::<CurrentProject>().map(|p| p.resolve_path(path))
}

fn sig_of(entity: Entity, path: &str, rev: u64) -> u64 {
    let mut h = DefaultHasher::new();
    entity.hash(&mut h);
    path.hash(&mut h);
    rev.hash(&mut h);
    h.finish()
}

/// Current override value for a param (override if present, else master default).
fn ov_get(w: &Rx, name: &str, kind: ParamKind, default_pin_param: &ParamValue) -> ParamValue {
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

fn ov_set(w: &mut World, name: &str, v: ParamValue) {
    if let Some(mut cache) = w.get_resource_mut::<MatCache>() {
        if let Some(inst) = &mut cache.instance {
            inst.overrides.insert(name.to_string(), v);
            cache.dirty = true;
        }
    }
}

// ── Drawer root + rebuild ────────────────────────────────────────────────────

fn material_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(2.0)), ..default() },
            MatRoot { entity, sig: None },
            Name::new("material-ref-inspector-root"),
        ))
        .id()
}

/// Reload [`MatCache`] from disk for the given entity + path.
///
/// The two `.material` shapes are told apart by which one parses: a derived
/// instance has a `master` field and no nodes, a master graph the reverse, so
/// neither ever deserializes as the other.
fn load_cache(world: &mut World, entity: Entity, path: &str) {
    let mut instance = None;
    let mut params = Vec::new();
    let mut graph = None;
    let mut instance_abs = PathBuf::new();
    if let Some(project) = world.get_resource::<CurrentProject>() {
        if !path.is_empty() {
            instance_abs = project.resolve_path(path);
            if let Ok(content) = std::fs::read_to_string(&instance_abs) {
                if let Ok(inst) = serde_json::from_str::<MaterialInstance>(&content) {
                    if !inst.master.is_empty() {
                        let master_abs = project.resolve_path(&inst.master);
                        params = read_master_parameters(&master_abs).unwrap_or_default();
                    }
                    instance = Some(inst);
                } else {
                    graph = serde_json::from_str::<MaterialGraph>(&content).ok();
                }
            }
        }
    }
    if let Some(mut cache) = world.get_resource_mut::<MatCache>() {
        cache.entity = Some(entity);
        cache.path = path.to_string();
        cache.instance_abs = instance_abs;
        cache.instance = instance;
        cache.params = params;
        cache.graph = graph;
        cache.dirty = false;
    }
}

fn rebuild_material(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut q = world.query::<(Entity, &MatRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let path = material_path(&Rx::new(&*world), entity);
        let rev = world.get_resource::<MatCache>().map(|c| c.rev).unwrap_or(0);
        let sig = sig_of(entity, &path, rev);
        if old_sig == Some(sig) {
            continue;
        }
        load_cache(world, entity, &path);
        // Request the current material's thumbnail.
        if let Some(abs) = material_abs(&Rx::new(&*world), &path) {
            if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                reg.request(abs);
            }
        }
        let params = world.get_resource::<MatCache>().map(|c| c.params.clone()).unwrap_or_default();
        let slots = slot_states(world);

        let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            build_body(&mut commands, &fonts, root, entity, &path, &params, &slots);
        }
        queue.apply(world);
        if let Some(mut mr) = world.get_mut::<MatRoot>(root) {
            mr.sig = Some(sig);
        }
        // No picker poke needed: its keyed list folds `material_path(entity)`
        // into its dirty token, so the new selection re-snapshots on its own.
    }
}

/// One texture-slot row's state, resolved from the cached graph before the
/// build so the row builder needs no world access.
struct SlotState {
    slot: &'static TextureSlot,
    /// Asset-relative texture path currently wired into the slot.
    texture: Option<String>,
    /// Wired, but not applied to the mesh — see
    /// [`renzora_shader::material::texture_slots::set_slot_muted`].
    muted: bool,
    /// Preview handle for `texture`. Held by the row's `ImageNode`, so the image
    /// stays loaded for as long as the row is on screen.
    thumb: Option<Handle<Image>>,
}

/// Read every slot's current texture off the cached master graph.
///
/// Returns nothing in two cases. A mesh with **no material at all**: six empty
/// channel rows are six things you can't meaningfully do yet, and they buried
/// the one thing you can — the picker and the New-material button at the top.
/// (Dropping images still works: the material *slot* routes a whole texture set
/// by filename and mints a material to hold it, so the affordance those rows
/// used to be is still there, one target up.) And a **derived instance**, whose
/// graph belongs to the master and must not be edited from here.
fn slot_states(world: &World) -> Vec<SlotState> {
    let Some(cache) = world.get_resource::<MatCache>() else { return Vec::new() };
    if cache.instance.is_some() || cache.path.is_empty() {
        return Vec::new();
    }
    let graph = cache.graph.as_ref();
    let assets = world.get_resource::<AssetServer>();
    TEXTURE_SLOTS
        .iter()
        .map(|slot| {
            let texture = graph.and_then(|g| texture_slots::slot_texture(g, slot));
            let muted = graph.is_some_and(|g| texture_slots::slot_muted(g, slot));
            let thumb = texture
                .as_ref()
                .and_then(|p| assets.map(|a| a.load::<Image>(p.clone())));
            SlotState { slot, texture, muted, thumb }
        })
        .collect()
}

fn build_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    root: Entity,
    entity: Entity,
    path: &str,
    params: &[MaterialParam],
    slots: &[SlotState],
) {
    let mut children: Vec<Entity> = Vec::new();

    // ── Material slot ──
    children.push(build_slot(commands, fonts, entity, path));

    // ── Texture slots ──
    //
    // No section header: each row already names its channel, so a heading plus a
    // sentence of instructions above six self-explanatory rows was two lines of
    // chrome saying what the rows say themselves.
    for (i, state) in slots.iter().enumerate() {
        let row = texture_slot_row(commands, fonts, entity, state);
        if i == 0 {
            // The only spacing the block needs, now that nothing announces it.
            commands.entity(row).entry::<Node>().and_modify(|mut n| {
                n.margin.top = Val::Px(8.0);
            });
        }
        children.push(row);
    }

    // ── Overrides ──
    if !params.is_empty() {
        children.push(section_header(commands, fonts, "Overrides", "Parameters this instance overrides on its master"));
        for (i, param) in params.iter().enumerate() {
            let row = param_row(commands, fonts, param);
            commands.entity(row).insert(BackgroundColor(inspector_stripe(i)));
            children.push(row);
        }
    }

    commands.entity(root).add_children(&children);
}

/// A section divider: a small label over a hairline, with the hint that follows
/// it kept muted so the eye lands on the rows, not the prose.
fn section_header(commands: &mut Commands, fonts: &EmberFonts, title: &str, hint: &str) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                margin: UiRect { top: Val::Px(10.0), bottom: Val::Px(4.0), ..default() },
                padding: UiRect { top: Val::Px(6.0), ..default() },
                border: UiRect { top: Val::Px(1.0), ..default() },
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("material-section-header"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(title), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    let h = commands
        .spawn((Text::new(hint), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
        .id();
    commands.entity(wrap).add_children(&[t, h]);
    wrap
}

// ── Material slot ────────────────────────────────────────────────────────────

/// Side of the slot's preview square. The field is given the same height, so the
/// two line up as one band with the action row tucked underneath.
const SLOT_PREVIEW: f32 = 40.0;

/// A material preview square: the rendered thumbnail when one exists, a muted
/// sphere glyph when it doesn't. Returns `(square, fallback_glyph)`; feed both
/// to [`bind_preview`].
///
/// The fallback earns its keep. A `.material` thumbnail is a separate one-shot
/// render that may not have landed yet — and never lands for a material that
/// fails to compile — and an `ImageNode` holding a default handle draws
/// *nothing*, so the old slot was a flat dark hole for most of the time it was
/// on screen. A framed square with a glyph in it reads as "preview pending"
/// rather than "broken", which matters far more now that the picker is a grid
/// of these.
fn preview_square(
    commands: &mut Commands,
    fonts: &EmberFonts,
    size: f32,
    radius: f32,
    glyph_size: f32,
) -> (Entity, Entity) {
    let square = commands
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(radius)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border()).with_alpha(0.55)),
            ImageNode::new(Handle::default()),
            bevy::ui::FocusPolicy::Pass,
            Name::new("material-preview"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, "sphere", placeholder(), glyph_size);
    commands.entity(glyph).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(square).add_child(glyph);
    (square, glyph)
}

/// Point a [`preview_square`] at whatever thumbnail `thumb` resolves to, hiding
/// the fallback glyph exactly while an image is bound (otherwise the glyph would
/// keep drawing on top of the render once it arrives).
fn bind_preview<F>(commands: &mut Commands, square: Entity, glyph: Entity, thumb: F)
where
    F: for<'w> Fn(&Rx<'w>) -> Option<Handle<Image>> + Send + Sync + 'static,
{
    bind_with(commands, square, thumb, move |w, e, h: &Option<Handle<Image>>| {
        if let Some(mut img) = w.get_mut::<ImageNode>(e) {
            img.image = h.clone().unwrap_or_default();
        }
        if let Some(mut n) = w.get_mut::<Node>(glyph) {
            let want = if h.is_some() { Display::None } else { Display::Flex };
            if n.display != want {
                n.display = want;
            }
        }
    });
}

fn build_slot(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, path: &str) -> Entity {
    let has_mat = !path.is_empty();
    let name = if has_mat {
        std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or(path).to_string()
    } else {
        "No material".to_string()
    };
    // Second line of the field. For a bound material it's where the file lives;
    // for an empty slot it's the instruction, because telling you what to do
    // with it is the only job an empty slot has.
    let sub = if has_mat {
        std::path::Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "project root".to_string())
    } else {
        "Click to pick, or drop a .material".to_string()
    };

    // The slot is a column: the header (preview + field + actions) with the
    // picker tray under it. The tray is **in flow**, so opening it pushes the
    // texture slots down rather than floating over them — the drawer stays one
    // readable column instead of growing a second layer.
    let slot = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    // The header, and the drop zone (material + image extensions). No card fill
    // behind it: the field and the action chips carry their own surfaces, and a
    // filled box around them was a third nested rectangle saying nothing. The
    // transparent border stays because `mat_slot_drop_highlight` accents it
    // while a compatible file is dragged over.
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            RelativeCursorPosition::default(),
            MatDropZone { entity },
            Name::new("material-slot"),
        ))
        .id();

    let (thumb, thumb_glyph) = preview_square(commands, fonts, SLOT_PREVIEW, 5.0, 17.0);
    bind_preview(commands, thumb, thumb_glyph, move |w| {
        let path = material_path(&Rx::new(w.untracked()), entity);
        material_abs(&Rx::new(w.untracked()), &path)
            .and_then(|abs| w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs)))
    });

    // Right column: the picker field over the action row.
    let col = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })
        .id();

    let panel = build_picker_panel(commands, fonts, entity);

    // The field: name over folder, caret on the right, and the whole thing is
    // the picker trigger. It replaces a one-line button, a loose folder caption
    // and a "browse" icon that opened this same list — three pieces of chrome
    // all answering "which material is this".
    let field = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(SLOT_PREVIEW),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            HoverTint::solid(rgb(popup_bg()), rgb(hover_bg()), rgb(hover_bg())),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            // No tooltip: a field with a caret on it already reads as a picker,
            // and this one is big enough to hover by accident on the way to the
            // action chips under it.
            Name::new("material-name"),
        ))
        .id();
    // A clip wrapper, because `Overflow::clip` clips a node's *children*: on the
    // text nodes themselves a long material name would spill over the caret.
    let text_col = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: Val::Px(1.0), overflow: Overflow::clip(), ..default() },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let name_text = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(if has_mat { text_primary() } else { placeholder() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let sub_text = commands
        .spawn((
            Text::new(sub),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(placeholder())),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(text_col).add_children(&[name_text, sub_text]);
    // The caret is repointed (not respawned) by `mat_picker_toggle`, so it also
    // reports the tray's state instead of permanently promising "down".
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(field).insert(MatPickerToggle { entity, panel, caret });

    // Edit and remove live *inside* the field, just left of the caret, rather
    // than on a row of their own underneath it. They act on the material the
    // field names, so that's where they belong — and the row they used to sit on
    // was a second line of chrome for two glyphs.
    //
    // Nesting them in the picker's own trigger is safe because `chip_btn` blocks
    // focus: a press on a chip doesn't fall through to the field behind it, so
    // clicking ✕ doesn't also slide the tray open. Same mechanism the
    // texture-slot clear already relies on inside its row.
    let mut field_kids = vec![text_col];
    if has_mat {
        let edit = icon_btn(commands, fonts, "pencil-simple", "Open in the material editor");
        commands.entity(edit).insert(MatEditBtn { entity });
        let clear = icon_btn(commands, fonts, "x", "Remove this material");
        commands.entity(clear).insert(MatClearBtn { entity });
        field_kids.extend_from_slice(&[edit, clear]);
    }
    field_kids.push(caret);
    commands.entity(field).add_children(&field_kids);

    let mut col_kids = vec![field];
    if !has_mat {
        // An empty slot has exactly one sensible move, so it gets exactly one
        // button — and this one keeps its label, because there's no material to
        // reason from and a lone "+" would be a guess.
        let actions = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Center, ..default() })
            .id();
        let create = chip_btn(commands, fonts, "plus", Some("New material"), None, rgb(section_bg()), text_primary());
        commands.entity(create).insert(MatCreateBtn { entity });
        commands.entity(actions).add_child(create);
        col_kids.push(actions);
    }

    commands.entity(col).add_children(&col_kids);
    commands.entity(row).add_children(&[thumb, col]);
    commands.entity(slot).add_children(&[row, panel]);
    slot
}

/// A bare glyph button — the field's edit/remove, the texture-slot clear and the
/// override revert. Each sits *inside* something that already has a surface, so
/// it stays transparent until hovered.
fn icon_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, tooltip: &str) -> Entity {
    chip_btn(commands, fonts, icon, None, Some(tooltip), Color::NONE, text_muted())
}

/// A small button: a glyph, optionally a label, optionally a tooltip.
///
/// `tooltip` is optional because a *labelled* button doesn't want one — the
/// label already says it, and a bubble repeating the word under the cursor is
/// noise.
fn chip_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: Option<&str>,
    tooltip: Option<&str>,
    base: Color,
    fg: (u8, u8, u8),
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(22.0),
                min_width: Val::Px(24.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::horizontal(Val::Px(if label.is_some() { 8.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(base),
            HoverTint::solid(base, rgb(hover_bg()), rgb(accent()).with_alpha(0.35)),
            Interaction::default(),
            // Block, or the press also lands on whatever sits under the button —
            // for the per-slot clear that is the slot row itself, which would
            // open a file dialog on the same click that emptied the slot, and
            // for the field's edit/remove it is the picker trigger.
            bevy::ui::FocusPolicy::Block,
            Name::new("material-icon-btn"),
        ))
        .id();
    if let Some(tooltip) = tooltip {
        commands.entity(btn).insert(HoverTooltip::new(tooltip));
    }
    let ic = icon_text(commands, &fonts.phosphor, icon, fg, 12.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(btn).add_child(ic);
    if let Some(label) = label {
        let text = commands
            .spawn((
                Text::new(label),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(fg)),
                bevy::text::TextLayout::no_wrap(),
                bevy::ui::FocusPolicy::Pass,
            ))
            .id();
        commands.entity(btn).add_child(text);
    }
    btn
}

// ── Texture slots ────────────────────────────────────────────────────────────

/// Marks a texture-slot row as a drop target for one PBR channel.
#[derive(Component)]
struct TexSlotZone {
    entity: Entity,
    slot: &'static TextureSlot,
}

#[derive(Component)]
struct TexSlotClearBtn {
    entity: Entity,
    slot: &'static TextureSlot,
}

/// The eye on a filled texture row: applies or un-applies that channel without
/// touching the texture. Carries the state it was built in, so the click knows
/// which way to flip — the row is rebuilt from the graph afterwards, so it can't
/// drift.
#[derive(Component)]
struct TexSlotMuteBtn {
    entity: Entity,
    slot: &'static TextureSlot,
    muted: bool,
}

/// One channel row: preview · label · texture name · clear.
///
/// The whole row is the drop target (not just the thumbnail) — the row is what
/// the eye reads as "the Normal slot", and a 34 px square is a small thing to
/// ask someone to hit with a dragged file.
fn texture_slot_row(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, state: &SlotState) -> Entity {
    let filled = state.texture.is_some();
    let name = state
        .texture
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Drop texture".to_string());

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            HoverTint::solid(Color::NONE, rgb(hover_bg()), rgb(hover_bg())),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            RelativeCursorPosition::default(),
            TexSlotZone { entity, slot: state.slot },
            // No tooltip here: the row already spells out its channel in the
            // label, and six rows that each pop a sentence on hover turn a
            // glance down the list into a wall of bubbles.
            Name::new("material-texture-slot"),
        ))
        .id();

    // Preview: the texture when one is bound, the channel's icon when not, so
    // an empty set still reads as six labelled places to drop something.
    let preview = commands
        .spawn((
            Node { width: Val::Px(34.0), height: Val::Px(34.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb(faint_bg())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("material-texture-thumb"),
        ))
        .id();
    if let Some(thumb) = &state.thumb {
        let mut image = ImageNode::new(thumb.clone());
        if state.muted {
            // Faded rather than hidden: the texture is still *assigned*, and a
            // row that emptied itself would be indistinguishable from one you'd
            // actually cleared.
            image.color = Color::WHITE.with_alpha(0.25);
        }
        commands.entity(preview).insert(image);
    } else {
        let ic = icon_text(commands, &fonts.phosphor, state.slot.icon, placeholder(), 14.0);
        commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
        commands.entity(preview).add_child(ic);
    }

    let text_col = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: Val::Px(1.0), overflow: Overflow::clip(), ..default() },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(state.slot.label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(if state.muted { placeholder() } else { text_primary() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let value = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(if filled && !state.muted { text_muted() } else { placeholder() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(text_col).add_children(&[label, value]);
    commands.entity(row).add_children(&[preview, text_col]);

    if filled {
        // The eye turns the channel off *on the mesh* without giving the texture
        // up; the ✕ beside it is the one that actually unwires it. Two very
        // different answers to "I don't want to see this right now", and only
        // one of them is reversible.
        let mute = icon_btn(
            commands,
            fonts,
            if state.muted { "eye-slash" } else { "eye" },
            if state.muted { "Apply this texture again" } else { "Turn this texture off on the mesh" },
        );
        commands.entity(mute).insert(TexSlotMuteBtn {
            entity,
            slot: state.slot,
            muted: state.muted,
        });
        let clear = icon_btn(commands, fonts, "x", "Clear this texture");
        commands.entity(clear).insert(TexSlotClearBtn { entity, slot: state.slot });
        commands.entity(row).add_children(&[mute, clear]);
    }
    row
}

// ── Picker tray (a keyed grid, registered once) ──────────────────────────────

/// Tile metrics. A `.material` is a *picture*, so the picker shows pictures: a
/// wrapping grid of preview tiles instead of the old text rows, which packed an
/// 11px name and a 9px folder into a 26px row and collided.
///
/// The tile is a fixed width and the grid wraps, so the layout re-flows with the
/// inspector — a wide dock simply fits more per row.
const TILE_W: f32 = 78.0;
const TILE_GAP: f32 = 6.0;
/// 3px padding + 72px preview + 3px gap + 13px label + 3px padding.
const TILE_H: f32 = 94.0;

/// Most tiles the tray will ever show.
///
/// This is a *hard* cap, not a window: the tray has no scroll area of its own,
/// so what it builds is what it is tall enough for. That is the point — the tray
/// lives inside the inspector, which already scrolls, and nesting a second
/// scrollbar a few pixels from the panel's own read as a mistake before it read
/// as a control. Twelve is four rows at the usual three columns: enough to
/// recognise a material by sight, small enough that the drawer below stays
/// reachable. Anything past it is reached by typing, and
/// [`picker_note`] says so rather than letting the rest vanish silently.
const PICKER_MAX_ROWS: usize = 12;

/// Build the picker tray: a search box over a grid of previews.
///
/// It's an ordinary in-flow node that starts hidden, **not** a `Popup` — an
/// overlay would have to float above the drawer, and ember's `popup_position`
/// pins a panel with `top: 100%`, which on a node that isn't absolutely
/// positioned offsets it by its own height instead of anchoring it. Opening the
/// tray simply makes the drawer taller and slides the texture slots down.
///
/// It carries no surface of its own either. A filled, bordered tray inside the
/// inspector's own filled, bordered panel was a box in a box; the search field
/// is the only thing here that needs an edge, so it's the only thing that has
/// one, and the tiles sit directly on the drawer.
///
/// Built **once**, with the slot — never refilled per keystroke. The tiles are
/// registered on the inner `grid` node rather than on the tray, so reconciling
/// them can never touch the search box: `run_keyed_lists` calls
/// `replace_children` on its container, which would otherwise blow the input away
/// (and with it the focus and the half-typed query) on every keystroke.
fn build_picker_panel(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::top(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            bevy::ui::FocusPolicy::Block,
            MatPickerPanel,
            Name::new("material-picker-tray"),
        ))
        .id();

    // The search row *is* the search box: the glyph sits inside the same
    // bordered surface as the text, so there's one edge here rather than an
    // input box nested in a header strip drawing a second one beside it.
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Name::new("material-picker-search"),
        ))
        .id();
    let glass = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 12.0);
    commands.entity(glass).insert(bevy::ui::FocusPolicy::Pass);
    let search = text_input(commands, &fonts.ui, "Search materials…", "");
    commands
        .entity(search)
        .insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)))
        .entry::<Node>()
        .and_modify(|mut n| {
            n.flex_grow = 1.0;
            n.min_width = Val::Px(0.0);
        });
    bind_search(commands, search);
    commands.entity(header).add_children(&[glass, search]);

    // Wrapping grid, sitting straight on the drawer. The vertical gap is the
    // tile's own bottom margin rather than the container's `row_gap` so the last
    // row doesn't leave a hanging gap above the note under it.
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(TILE_GAP),
            ..default()
        })
        .id();
    register_picker_rows(commands, grid, entity);
    commands.entity(panel).add_children(&[header, grid]);
    panel
}

/// Register the tiles as a keyed list.
///
/// Plain rather than virtualized: [`PICKER_MAX_ROWS`] is the whole list now, and
/// windowing twelve tiles would be bookkeeping in exchange for nothing.
fn register_picker_rows(commands: &mut Commands, list: Entity, entity: Entity) {
    keyed_list_tokened(
        commands,
        list,
        // Dirty token: re-snapshot only when the query text, the cached index, or
        // this entity's assigned material actually changes.
        move |w: &Rx| {
            let mut h = DefaultHasher::new();
            w.get_resource::<MatPickerFilter>()
                .map(|f| f.text.as_str())
                .unwrap_or("")
                .hash(&mut h);
            w.get_resource::<MaterialIndex>().map(|i| i.generation).unwrap_or(0).hash(&mut h);
            material_path(&Rx::new(w.untracked()), entity).hash(&mut h);
            h.finish()
        },
        move |w: &Rx| picker_snapshot(&Rx::new(w.untracked()), entity),
    );
}

/// This frame's filtered row set. Cheap: an `Arc` clone plus a substring test per
/// candidate; no filesystem access (see [`MaterialIndex`]).
fn picker_snapshot(w: &Rx, entity: Entity) -> KeyedSnapshot {
    let query = w.get_resource::<MatPickerFilter>().map(|f| f.text.clone()).unwrap_or_default();
    let current_path = material_path(w, entity);
    let materials = w
        .get_resource::<MaterialIndex>()
        .map(|i| i.materials.clone())
        .unwrap_or_default();
    let lower = query.trim().to_ascii_lowercase();
    // Count every match, then keep only the first [`PICKER_MAX_ROWS`]: the total
    // is what the truncation note reports, and without it a cap of twelve would
    // quietly claim the project has twelve materials.
    let matched: Vec<&(String, String)> = materials
        .iter()
        .filter(|(rel, _)| lower.is_empty() || rel.to_ascii_lowercase().contains(&lower))
        .collect();
    let total = matched.len();
    let rows: Vec<(String, String, bool)> = matched
        .into_iter()
        .take(PICKER_MAX_ROWS)
        .map(|(rel, abs)| {
            let is_current = rel.as_str() == current_path.as_str();
            (rel.clone(), abs.clone(), is_current)
        })
        .collect();

    if rows.is_empty() {
        let mut k = DefaultHasher::new();
        "\u{0}<no-matches>".hash(&mut k);
        return KeyedSnapshot {
            items: vec![(k.finish(), 0)],
            build: Box::new(|c: &mut Commands, f: &EmberFonts, _| {
                picker_note(c, f, "No materials match".to_string(), 48.0)
            }),
        };
    }

    // Key = the project-relative path: stable identity that survives filtering, so
    // narrowing the search keeps surviving rows AND their thumbnail bindings.
    // Hash = only what is baked into the row at build time. The thumbnail
    // `Handle<Image>` is deliberately excluded — it arrives via the row's own
    // `bind_with`, and hashing it would make every thumbnail that resolves
    // despawn and rebuild its row.
    let mut items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(rel, abs, is_current)| {
            let mut k = DefaultHasher::new();
            rel.hash(&mut k);
            let mut h = DefaultHasher::new();
            abs.hash(&mut h);
            is_current.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();

    // One more "row" when the cap bit, hashed on the count so it re-renders as
    // typing narrows the field.
    let shown = rows.len();
    let truncated = total > shown;
    if truncated {
        let mut k = DefaultHasher::new();
        "\u{0}<truncated>".hash(&mut k);
        let mut h = DefaultHasher::new();
        total.hash(&mut h);
        items.push((k.finish(), h.finish()));
    }

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, f: &EmberFonts, i: usize| {
            match rows.get(i) {
                Some((rel, abs, is_current)) => picker_tile(c, f, entity, rel, abs, *is_current),
                None => picker_note(
                    c,
                    f,
                    format!("Showing {shown} of {total} — type to narrow"),
                    22.0,
                ),
            }
        }),
    }
}

/// A full-width line in the grid — the empty state and the truncation note.
///
/// Full width so it takes a row of its own and centres, rather than landing in
/// the first tile's column.
fn picker_note(commands: &mut Commands, fonts: &EmberFonts, message: String, height: f32) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    let text = commands
        .spawn((
            Text::new(message),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(wrap).add_child(text);
    wrap
}

/// One grid tile: a preview square over a clipped name.
///
/// The folder lives in the tooltip rather than on a second line — it only
/// matters when two materials share a name, and paying every tile a line of 9px
/// grey for that case is what made the old rows unreadable.
fn picker_tile(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, rel: &str, abs: &str, is_current: bool) -> Entity {
    let path = std::path::Path::new(rel);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(rel).to_string();
    let parent = path
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("project root");

    // The current material is marked three ways — tinted tile, accented preview
    // border, accented name — because at 78px one of them alone reads as noise.
    let base = if is_current { rgb(accent()).with_alpha(0.20) } else { Color::NONE };
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(TILE_W),
                height: Val::Px(TILE_H),
                margin: UiRect::bottom(Val::Px(TILE_GAP)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(base),
            HoverTint::solid(base, rgb(hover_bg()), rgb(accent()).with_alpha(0.32)),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            HoverTooltip::new(format!("{stem}  ·  {parent}")),
            MatPickerItem { entity, rel: rel.to_string() },
            Name::new("material-picker-tile"),
        ))
        .id();

    let (preview, glyph) = preview_square(commands, fonts, TILE_W - 6.0, 4.0, 22.0);
    if is_current {
        commands.entity(preview).insert(BorderColor::all(rgb(accent())));
    }
    let abs_pb = PathBuf::from(abs);
    // Ask for the thumbnail from the tile's own build. A tile is only built when
    // it scrolls into the window, so opening the picker on a project with
    // hundreds of materials queues renders for the dozen actually on screen
    // rather than for all of them; `request` is a no-op once a path is cached or
    // in flight, so scrolling back over one costs nothing.
    let wanted = abs_pb.clone();
    commands.queue(move |w: &mut World| {
        if let Some(mut reg) = w.get_resource_mut::<MaterialThumbnailRegistry>() {
            reg.request(wanted);
        }
    });
    bind_preview(commands, preview, glyph, move |w| {
        w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs_pb))
    });

    let name = commands
        .spawn((
            Text::new(stem),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(if is_current { accent() } else { text_primary() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    // Clip wrapper again: `Overflow::clip` clips a node's *children*, so a long
    // name has to sit inside something rather than carry the clip itself.
    let name_clip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(13.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(name_clip).add_child(name);
    commands.entity(tile).add_children(&[preview, name_clip]);
    tile
}

fn bind_search(commands: &mut Commands, input: Entity) {
    use renzora_ember::widgets::bind_text_input;
    bind_text_input(
        commands,
        input,
        move |w| w.get_resource::<MatPickerFilter>().map(|f| f.text.clone()).unwrap_or_default(),
        move |w, s: String| {
            if let Some(mut f) = w.get_resource_mut::<MatPickerFilter>() {
                f.text = s;
            }
        },
    );
}

// ── Overrides ────────────────────────────────────────────────────────────────

fn param_row(commands: &mut Commands, fonts: &EmberFonts, param: &MaterialParam) -> Entity {
    let name = param.name.clone();
    let kind = param.kind;
    let default_param = pin_to_param(&param.default).unwrap_or(default_param_value(kind));

    let ctrl = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();

    let editor = build_param_editor(commands, fonts, name.clone(), kind, default_param);
    let revert = icon_btn(commands, fonts, "arrow-counter-clockwise", "Revert to the master's value");
    commands.entity(revert).insert(MatRevertBtn { name: name.clone() });
    commands.entity(ctrl).add_children(&[editor, revert]);

    inspector_row(commands, &fonts.ui, &param.name, ctrl)
}

fn build_param_editor(commands: &mut Commands, fonts: &EmberFonts, name: String, kind: ParamKind, default_param: ParamValue) -> Entity {
    match kind {
        ParamKind::Float => {
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.0, 0.01);
            let (n1, d1) = (name.clone(), default_param.clone());
            bind_2way(
                commands,
                dv,
                move |w| match ov_get(&Rx::new(w.untracked()), &n1, kind, &d1) {
                    ParamValue::Float(f) => f,
                    _ => 0.0,
                },
                move |w, v: &f32| ov_set(w, &name, ParamValue::Float(*v)),
            );
            dv
        }
        ParamKind::Bool => {
            let cb = checkbox(commands, false);
            let (n1, d1) = (name.clone(), default_param.clone());
            bind_2way(
                commands,
                cb,
                move |w| matches!(ov_get(&Rx::new(w.untracked()), &n1, kind, &d1), ParamValue::Bool(true)),
                move |w, v: &bool| ov_set(w, &name, ParamValue::Bool(*v)),
            );
            cb
        }
        ParamKind::Color => {
            let n1 = name.clone();
            let d1 = default_param.clone();
            color_field_rgba(
                commands,
                move |w| match ov_get(&Rx::new(w.untracked()), &n1, kind, &d1) {
                    ParamValue::Color(c) => c,
                    _ => [1.0; 4],
                },
                move |w, a: [f32; 4]| ov_set(w, &name, ParamValue::Color(a)),
            )
        }
        ParamKind::Vec2 | ParamKind::Vec3 | ParamKind::Vec4 => {
            let n = match kind {
                ParamKind::Vec2 => 2,
                ParamKind::Vec3 => 3,
                _ => 4,
            };
            let group = commands
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), flex_grow: 1.0, ..default() })
                .id();
            let axes = ["x", "y", "z", "w"];
            let mut cells = Vec::new();
            for (i, axis) in axes.iter().enumerate().take(n) {
                let dv = drag_value(commands, &fonts.ui, axis, (210, 210, 220), 0.0, 0.01);
                let (n1, d1) = (name.clone(), default_param.clone());
                let (n2, kind2) = (name.clone(), kind);
                bind_2way(
                    commands,
                    dv,
                    move |w| vec_component(&ov_get(&Rx::new(w.untracked()), &n1, kind, &d1), i),
                    move |w, v: &f32| {
                        let cur = ov_get(&Rx::new(&*w), &n2, kind2, &default_param_value(kind2));
                        let updated = set_vec_component(cur, kind2, i, *v);
                        ov_set(w, &n2, updated);
                    },
                );
                cells.push(dv);
            }
            commands.entity(group).add_children(&cells);
            group
        }
    }
}

fn vec_component(v: &ParamValue, i: usize) -> f32 {
    match v {
        ParamValue::Vec2(a) => *a.get(i).unwrap_or(&0.0),
        ParamValue::Vec3(a) => *a.get(i).unwrap_or(&0.0),
        ParamValue::Vec4(a) => *a.get(i).unwrap_or(&0.0),
        _ => 0.0,
    }
}

fn set_vec_component(mut v: ParamValue, kind: ParamKind, i: usize, val: f32) -> ParamValue {
    match (&mut v, kind) {
        (ParamValue::Vec2(a), ParamKind::Vec2) => {
            if i < 2 {
                a[i] = val;
            }
        }
        (ParamValue::Vec3(a), ParamKind::Vec3) => {
            if i < 3 {
                a[i] = val;
            }
        }
        (ParamValue::Vec4(a), ParamKind::Vec4) => {
            if i < 4 {
                a[i] = val;
            }
        }
        _ => {
            // Type drifted (override stored a different kind) — reset to the kind's default.
            let mut d = default_param_value(kind);
            d = set_vec_component(d, kind, i, val);
            return d;
        }
    }
    v
}

// ── Interaction systems ──────────────────────────────────────────────────────

fn mat_slot_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    zones: Query<(&RelativeCursorPosition, &MatDropZone)>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached {
        return;
    }
    let mut exts: Vec<&str> = vec!["material"];
    exts.extend_from_slice(IMAGE_EXTENSIONS);
    if !payload.matches_extensions(&exts) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let dropped = dropped_paths(&payload);
        let entity = zone.entity;
        commands.queue(move |w: &mut World| apply_drop(w, entity, dropped));
        break;
    }
}

/// Every path in the drag. A multi-select drag fills `paths`; older single
/// drags only set `path`.
fn dropped_paths(payload: &AssetDragPayload) -> Vec<PathBuf> {
    if payload.paths.is_empty() {
        vec![payload.path.clone()]
    } else {
        payload.paths.clone()
    }
}

fn is_image(path: &std::path::Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    IMAGE_EXTENSIONS.contains(&ext.as_str())
}

/// Handle a drop on the material slot itself.
///
/// A `.material` binds the material. Images are routed to texture slots by
/// filename, so dragging a whole downloaded texture set onto the row fills the
/// channels in one gesture. A single image whose name says nothing goes to base
/// color — that is what one unlabelled texture nearly always is — but in a
/// multi-file drop the unrecognised ones are left alone rather than fighting
/// over the same slot in whatever order the drag happened to list them.
fn apply_drop(world: &mut World, entity: Entity, dropped: Vec<PathBuf>) {
    if let Some(mat) = dropped.iter().find(|p| {
        p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("material"))
    }) {
        let mat_path = world
            .get_resource::<CurrentProject>()
            .map(|p| p.make_asset_relative(mat))
            .unwrap_or_else(|| mat.to_string_lossy().to_string());
        bind_material(world, entity, mat_path);
        return;
    }

    let images: Vec<PathBuf> = dropped.into_iter().filter(|p| is_image(p)).collect();
    if images.is_empty() {
        return;
    }
    let single = images.len() == 1;
    let routed: Vec<(Vec<&'static TextureSlot>, String)> = images
        .iter()
        .filter_map(|img| {
            let mut slots = texture_slots::guess_slots(img);
            if slots.is_empty() && single {
                slots = texture_slots::slot("base_color").into_iter().collect();
            }
            if slots.is_empty() {
                return None;
            }
            Some((slots, asset_relative(world, img)))
        })
        .collect();
    if routed.is_empty() {
        warn!("[material] dropped images don't name a texture channel; drop them on a slot row instead");
        return;
    }

    slot_edit(world, entity, move |graph| {
        let mut changed = false;
        for (slots, rel) in &routed {
            for slot in slots {
                changed |= texture_slots::set_slot_texture(graph, slot, rel);
            }
        }
        changed
    });
}

/// Project-relative form of a dropped file, which is what a graph stores.
fn asset_relative(world: &World, path: &std::path::Path) -> String {
    world
        .get_resource::<CurrentProject>()
        .map(|p| p.make_asset_relative(path))
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn bind_material(world: &mut World, entity: Entity, mat_path: String) {
    world.entity_mut(entity).remove::<MaterialResolved>();
    if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
        mr.0 = mat_path;
    } else {
        world.entity_mut(entity).insert(MaterialRef(mat_path));
    }
}

/// Edit the entity's material graph, creating a material first if it has none,
/// and re-read the drawer afterwards.
///
/// Every texture-slot change goes through here so a drop behaves the same
/// whether the mesh already had a material or not — an entity with no
/// `MaterialRef` gets a fresh empty graph rather than refusing the drop.
fn slot_edit(world: &mut World, entity: Entity, edit: impl FnOnce(&mut MaterialGraph) -> bool) {
    let Some(path) = ensure_material(world, entity) else { return };
    if crate::edit_material_graph(world, &path, edit) {
        // The drawer re-reads the file on a `rev` change; nothing else would
        // tell it the graph on disk moved under it.
        if let Some(mut cache) = world.get_resource_mut::<MatCache>() {
            cache.rev = cache.rev.wrapping_add(1);
        }
    }
}

/// The entity's material path, creating and binding an empty one if needed.
/// Returns `None` only when there is no project to write into.
fn ensure_material(world: &mut World, entity: Entity) -> Option<String> {
    let existing = material_path(&Rx::new(&*world), entity);
    if !existing.is_empty() {
        return Some(existing);
    }
    create_material(world, entity)
}

/// Write a fresh empty `.material` under `<project>/materials/` and bind it to
/// `entity`, whatever it pointed at before.
///
/// Returns `None` only when there is no project to write into, or the save
/// failed — in which case nothing is bound, so the mesh keeps the material it
/// had rather than losing it to a file that isn't there.
fn create_material(world: &mut World, entity: Entity) -> Option<String> {
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone())?;
    let stem = default_material_stem(world, entity);
    create_material_at(world, entity, &project_root.join(MATERIALS_DIR), &stem)
}

/// Name a new material after the mesh so the file is findable later; a generic
/// name for an unnamed entity. Sanitised, because this becomes a filename.
fn default_material_stem(world: &World, entity: Entity) -> String {
    let base = world
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| "Material".to_string());
    sanitize_stem(&base)
}

fn sanitize_stem(base: &str) -> String {
    base.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

/// Write `<dir>/<stem>.material` (uniquified) and bind it to `entity`.
///
/// Returns `None` when there's no project to write into, or the save failed —
/// in which case nothing is bound, so the mesh keeps whatever it had rather than
/// losing it to a file that isn't there.
fn create_material_at(world: &mut World, entity: Entity, dir: &Path, stem: &str) -> Option<String> {
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone())?;
    let stem = if stem.trim().is_empty() { "Material" } else { stem };
    let _ = std::fs::create_dir_all(dir);
    // Never write over a material that already exists — two meshes called
    // "Cube" must not end up silently sharing (and overwriting) one file.
    let mut fs_path = dir.join(format!("{stem}.material"));
    let mut n = 1;
    while fs_path.exists() {
        fs_path = dir.join(format!("{stem}_{n}.material"));
        n += 1;
    }

    let asset_path = renzora_shader::material::precompiled::project_relative(&project_root, &fs_path);
    let mut graph = MaterialGraph::new(stem, MaterialDomain::Surface);
    if !crate::save_material_graph(world, &asset_path, &mut graph) {
        return None;
    }
    bind_material(world, entity, asset_path.clone());
    Some(asset_path)
}

/// Drop an image on a texture-slot row → wire it into that channel.
fn tex_slot_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    zones: Query<(&RelativeCursorPosition, &TexSlotZone)>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached || !payload.matches_extensions(IMAGE_EXTENSIONS) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        // A row is one channel, so a multi-file drag aimed at it takes the
        // drag's primary file only. Dropping a whole set is what the material
        // row above is for — there the names decide where each file lands.
        let dropped = payload.path.clone();
        let (entity, slot) = (zone.entity, zone.slot);
        commands.queue(move |w: &mut World| {
            let rel = asset_relative(w, &dropped);
            slot_edit(w, entity, move |graph| texture_slots::set_slot_texture(graph, slot, &rel));
        });
        break;
    }
}

/// Accent the row being dragged over, so the target channel is unambiguous
/// before the mouse comes up.
fn tex_slot_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<TexSlotZone>>,
) {
    for (rcp, mut bc) in &mut zones {
        let active = payload
            .as_ref()
            .is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(IMAGE_EXTENSIONS));
        let want = BorderColor::all(if active { rgb(accent()) } else { Color::NONE });
        if *bc != want {
            *bc = want;
        }
    }
}

/// Click a row (with nothing being dragged) → pick a file for that channel.
fn tex_slot_browse(
    q: Query<(&Interaction, &TexSlotZone), Changed<Interaction>>,
    payload: Option<Res<AssetDragPayload>>,
    mut commands: Commands,
) {
    if payload.as_ref().is_some_and(|p| p.is_detached) {
        return;
    }
    for (interaction, zone) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, slot) = (zone.entity, zone.slot);
        #[cfg(not(target_arch = "wasm32"))]
        commands.queue(move |w: &mut World| {
            let Some(file) = rfd::FileDialog::new().add_filter("Image", IMAGE_EXTENSIONS).pick_file() else {
                return;
            };
            let rel = asset_relative(w, &file);
            slot_edit(w, entity, move |graph| texture_slots::set_slot_texture(graph, slot, &rel));
        });
        #[cfg(target_arch = "wasm32")]
        let _ = (entity, slot, &mut commands);
    }
}

/// The eye on a texture row → apply or un-apply that channel on the mesh.
///
/// Routed through `slot_edit` like every other slot change, so the graph is
/// re-saved, recompiled and re-read: the mesh updates immediately and the row
/// rebuilds with the icon the graph now justifies.
fn tex_slot_mute(q: Query<(&Interaction, &TexSlotMuteBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, slot, muted) = (btn.entity, btn.slot, btn.muted);
        commands.queue(move |w: &mut World| {
            slot_edit(w, entity, move |graph| texture_slots::set_slot_muted(graph, slot, !muted));
        });
    }
}

fn tex_slot_clear(q: Query<(&Interaction, &TexSlotClearBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, slot) = (btn.entity, btn.slot);
        commands.queue(move |w: &mut World| {
            slot_edit(w, entity, move |graph| texture_slots::clear_slot(graph, slot));
        });
    }
}

fn mat_slot_drop_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<MatDropZone>>,
) {
    let mut exts: Vec<&str> = vec!["material"];
    exts.extend_from_slice(IMAGE_EXTENSIONS);
    for (rcp, mut bc) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(&exts));
        let want = BorderColor::all(if active { rgb(accent()) } else { Color::NONE });
        if *bc != want {
            *bc = want;
        }
    }
}

fn mat_edit_click(q: Query<(&Interaction, &MatEditBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            let path = material_path(&Rx::new(&*w), e);
            if path.is_empty() {
                return;
            }
            let abs = w.get_resource::<CurrentProject>().map(|p| p.resolve_path(&path)).unwrap_or_else(|| PathBuf::from(&path));
            open_asset_tab(w, &abs, DocTabKind::Material);
        });
    }
}

// ── New-material overlay ─────────────────────────────────────────────────────

/// Conventional home for materials the editor creates, and the seeded
/// destination in the overlay's tree.
const MATERIALS_DIR: &str = "materials";

/// How far below the project root the destination tree walks. Two levels is
/// enough for `materials/` and a category under it without turning the overlay
/// into a file manager.
const MAT_PICKER_DEPTH: usize = 2;

/// The open "New material" overlay.
#[derive(Resource)]
struct PendingMatCreate {
    entity: Entity,
    overlay: Entity,
    name_input: Entity,
    ticks: u8,
}

#[derive(Component)]
struct MatCreateConfirmBtn;
#[derive(Component)]
struct MatCreateCancelBtn;

/// "New material" → ask where it should go.
///
/// It used to write straight into `<project>/materials/` and jump to the
/// Material Editor. Both were assumptions: a project that files materials by
/// area now has to move the file afterwards, and being thrown into a node graph
/// is the wrong answer when what you wanted was a material on this mesh — the
/// texture slots in the drawer are where a new material actually gets filled in.
fn mat_create_click(q: Query<(&Interaction, &MatCreateBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| open_create_overlay(w, e));
    }
}

/// Build the name + destination overlay. Exclusive-world so it can read the
/// project, pre-create the conventional folder and walk the tree in one shot.
fn open_create_overlay(world: &mut World, entity: Entity) {
    if world.contains_resource::<PendingMatCreate>() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let Some(root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    // Pre-create the conventional folder so the default destination is a real
    // row in the tree even on a project that has never had one.
    let default_dest = root.join(MATERIALS_DIR);
    let _ = std::fs::create_dir_all(&default_dest);
    let stem = default_material_stem(world, entity);

    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let (overlay, content) = overlay_sized(&mut commands, &fonts, "New material", 480.0, 440.0, true);

        let name_input = text_input(&mut commands, &fonts.ui, &stem, &stem);
        let name_row = overlay_field(&mut commands, &fonts, "Name", name_input);
        let dest_label = overlay_label(&mut commands, &fonts, "Destination");
        let picker = folder_picker(&mut commands, &fonts, &root, &default_dest, MAT_PICKER_DEPTH);

        let buttons = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .id();
        // New Folder rides in the button row rather than under the tree — one
        // row of controls, not two. It floats at the row's left edge (absolute,
        // out of flow), so Cancel and Create lay out untouched.
        let new_folder = folder_new_button(&mut commands, &fonts, picker);
        let cancel = button(&mut commands, &fonts.ui, "Cancel");
        commands.entity(cancel).insert(MatCreateCancelBtn);
        let confirm = button(&mut commands, &fonts.ui, "Create");
        commands.entity(confirm).insert(MatCreateConfirmBtn);
        commands.entity(buttons).add_children(&[new_folder, cancel, confirm]);

        let body = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    ..default()
                },
                // Enter in the name field = Create. Typing a name and then
                // reaching for the mouse is the one interaction this overlay
                // would otherwise force on every single use.
                EmberForm { submit: confirm },
            ))
            .id();
        commands.entity(body).add_children(&[name_row, dest_label, picker, buttons]);
        commands.entity(content).add_child(body);

        commands.insert_resource(PendingMatCreate { entity, overlay, name_input, ticks: 0 });
    }
    queue.apply(world);
}

/// A labelled row in the overlay: fixed-width caption, control filling the rest.
fn overlay_field(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let caption = commands
        .spawn((
            Node { width: Val::Px(72.0), flex_shrink: 0.0, ..default() },
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(control).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(0.0);
    });
    commands.entity(row).add_children(&[caption, control]);
    row
}

fn overlay_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

/// Focus the name field with its default selected, so the overlay is "type the
/// name, press Enter" with no click first.
///
/// Deliberately a tick late. The overlay is spawned from a button press, and
/// ember's `text_input_focus` blurs every input on any left press that didn't
/// land *on* an input — this field doesn't exist yet when that press is read, so
/// focusing on the opening frame would be undone by whichever order the two
/// systems happen to run in. The next tick has no press to blur against.
fn mat_create_focus(pending: Option<ResMut<PendingMatCreate>>, mut inputs: Query<&mut EmberTextInput>) {
    let Some(mut pending) = pending else { return };
    if pending.ticks > 1 {
        return;
    }
    pending.ticks += 1;
    if pending.ticks != 2 {
        return;
    }
    if let Ok(mut input) = inputs.get_mut(pending.name_input) {
        input.focused = true;
        // Select-all, so the first keystroke replaces the default name rather
        // than prepending to it.
        input.select_all = true;
        input.caret_index = input.value.chars().count();
    }
}

/// Create → write the material into the picked folder and bind it; cancel (or a
/// backdrop/Escape dismiss, which despawns the overlay out from under us) → drop
/// the pending state and leave the mesh alone.
fn mat_create_overlay_buttons(
    confirm: Query<&Interaction, (With<MatCreateConfirmBtn>, Changed<Interaction>)>,
    cancel: Query<&Interaction, (With<MatCreateCancelBtn>, Changed<Interaction>)>,
    pending: Option<Res<PendingMatCreate>>,
    inputs: Query<&EmberTextInput>,
    pick: Res<FolderPick>,
    project: Option<Res<CurrentProject>>,
    nodes: Query<(), With<Node>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };

    // Escape and backdrop clicks are ember's, and they despawn the root without
    // telling us — so a vanished overlay is a cancel.
    if nodes.get(pending.overlay).is_err() {
        commands.remove_resource::<PendingMatCreate>();
        return;
    }
    if cancel.iter().any(|i| *i == Interaction::Pressed) {
        commands.entity(pending.overlay).despawn();
        commands.remove_resource::<PendingMatCreate>();
        return;
    }
    if !confirm.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }

    let typed = inputs.get(pending.name_input).map(|i| i.value.trim().to_string()).unwrap_or_default();
    let stem = sanitize_stem(&typed);
    let Some(root) = project.as_ref().map(|p| p.path.clone()) else { return };
    let dir = pick.path().map(Path::to_path_buf).unwrap_or_else(|| root.join(MATERIALS_DIR));
    let entity = pending.entity;

    commands.entity(pending.overlay).despawn();
    commands.remove_resource::<PendingMatCreate>();
    commands.queue(move |w: &mut World| {
        // The drawer keys off (entity, path, rev); the path changed, so the
        // rebuild picks the new file up — and its texture slots appear — without
        // anything here poking it. No editor tab: filling the material in is
        // what those slots are for.
        create_material_at(w, entity, &dir, &stem);
    });
}

fn mat_clear_click(q: Query<(&Interaction, &MatClearBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            w.entity_mut(e).remove::<MaterialRef>();
            w.entity_mut(e).remove::<MaterialResolved>();
            w.entity_mut(e).remove::<bevy::pbr::MeshMaterial3d<renzora_shader::material::runtime::GraphMaterial>>();
            let default_mat = w.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
            w.entity_mut(e).insert(bevy::pbr::MeshMaterial3d(default_mat));
        });
    }
}

fn mat_picker_select(q: Query<(&Interaction, &MatPickerItem), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, item) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (e, rel) = (item.entity, item.rel.clone());
        commands.queue(move |w: &mut World| {
            bind_material(w, e, rel);
            close_pickers(w);
        });
    }
}

/// The picker field's surface and border for the two tray states.
///
/// Accent-tinted while open, so the field reads as the thing the grid below
/// belongs to rather than as an unrelated control that happens to sit above it.
fn field_colors(open: bool) -> (Color, Color) {
    if open {
        (rgb(accent()).with_alpha(0.22), rgb(accent()))
    } else {
        (rgb(popup_bg()), rgb(border()))
    }
}

/// Click the field → slide its picker tray open or shut.
///
/// Opening it also folds the texture-slot rows away. They're the tallest thing
/// in the drawer and they're about the material you're in the middle of
/// *replacing*, so leaving them there pushed the grid off the bottom of the
/// panel and asked you to scroll past six rows that were on their way out.
fn mat_picker_toggle(
    mut q: Query<
        (&Interaction, &MatPickerToggle, &mut HoverTint, &mut BackgroundColor, &mut BorderColor),
        Changed<Interaction>,
    >,
    tex_rows: Query<(Entity, &TexSlotZone)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, toggle, mut tint, mut bg, mut bc) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let open = nodes.get(toggle.panel).is_ok_and(|n| n.display == Display::None);
        if let Ok(mut node) = nodes.get_mut(toggle.panel) {
            node.display = if open { Display::Flex } else { Display::None };
        }
        set_caret(&mut texts, toggle.caret, open);
        set_texture_rows(&mut nodes, &tex_rows, Some(toggle.entity), !open);

        // `HoverTint.base` too, not just the background: ember's `hover_tint`
        // writes `base` back the moment the pointer leaves, so painting only the
        // background would hold the active colour exactly until you moved the
        // mouse off the field.
        let (fill, edge) = field_colors(open);
        tint.base = fill;
        bg.0 = fill;
        *bc = BorderColor::all(edge);
    }
}

/// Show or hide texture-slot rows — every one when `entity` is `None`, otherwise
/// only the rows belonging to that inspected entity.
fn set_texture_rows(
    nodes: &mut Query<&mut Node>,
    tex_rows: &Query<(Entity, &TexSlotZone)>,
    entity: Option<Entity>,
    visible: bool,
) {
    let want = if visible { Display::Flex } else { Display::None };
    for (row, zone) in tex_rows {
        if entity.is_some_and(|e| e != zone.entity) {
            continue;
        }
        if let Ok(mut node) = nodes.get_mut(row) {
            if node.display != want {
                node.display = want;
            }
        }
    }
}

/// Point a field's caret at the tray's state.
fn set_caret(texts: &mut Query<&mut Text>, caret: Entity, open: bool) {
    set_glyph(texts, caret, if open { "caret-up" } else { "caret-down" });
}

/// Repoint an existing icon entity at another phosphor glyph, by name.
fn set_glyph(texts: &mut Query<&mut Text>, icon: Entity, name: &str) {
    let Some(glyph) = renzora_ember::phosphor_map::icon_glyph(name) else { return };
    if let Ok(mut text) = texts.get_mut(icon) {
        let want = glyph.to_string();
        if text.0 != want {
            *text = Text::new(want);
        }
    }
}


/// Shut every open picker tray and reset its search.
///
/// Picking used to leave the list sitting there — it only closed on an outside
/// click, so choosing a material took two clicks: one to choose, one to get the
/// grid off the drawer. Now that the tray is in flow that second click also cost
/// the texture slots their position on screen, which makes closing it on select
/// non-negotiable rather than merely tidy.
fn close_pickers(world: &mut World) {
    let toggles: Vec<(Entity, Entity, Entity)> = world
        .query::<(Entity, &MatPickerToggle)>()
        .iter(world)
        .map(|(field, t)| (field, t.panel, t.caret))
        .collect();
    let (fill, edge) = field_colors(false);
    for (field, panel, caret) in toggles {
        if let Some(mut node) = world.get_mut::<Node>(panel) {
            if node.display == Display::None {
                continue;
            }
            node.display = Display::None;
        }
        if let Some(glyph) = renzora_ember::phosphor_map::icon_glyph("caret-down") {
            if let Some(mut text) = world.get_mut::<Text>(caret) {
                *text = Text::new(glyph.to_string());
            }
        }
        if let Some(mut tint) = world.get_mut::<HoverTint>(field) {
            tint.base = fill;
        }
        if let Some(mut bg) = world.get_mut::<BackgroundColor>(field) {
            bg.0 = fill;
        }
        if let Some(mut bc) = world.get_mut::<BorderColor>(field) {
            *bc = BorderColor::all(edge);
        }
    }
    // Every tray is shut now, so every texture row is due back — no need to
    // match them up per entity. The rebuild that follows a *changed* material
    // respawns them visible anyway; this covers the case where the pick landed
    // on the material already bound, which changes the drawer's signature not at
    // all and so rebuilds nothing.
    let rows: Vec<Entity> = world
        .query_filtered::<Entity, With<TexSlotZone>>()
        .iter(world)
        .collect();
    for row in rows {
        if let Some(mut node) = world.get_mut::<Node>(row) {
            if node.display != Display::Flex {
                node.display = Display::Flex;
            }
        }
    }
    // Reopening pre-filtered by a query you typed a minute ago reads as "there
    // are only two materials in this project".
    if let Some(mut filter) = world.get_resource_mut::<MatPickerFilter>() {
        if !filter.text.is_empty() {
            filter.text.clear();
        }
    }
}

fn mat_revert_click(q: Query<(&Interaction, &MatRevertBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let name = b.name.clone();
        commands.queue(move |w: &mut World| {
            if let Some(mut cache) = w.get_resource_mut::<MatCache>() {
                if let Some(inst) = &mut cache.instance {
                    if inst.overrides.remove(&name).is_some() {
                        cache.dirty = true;
                    }
                }
            }
        });
    }
}

/// Write the edited overrides back to disk + invalidate the resolver so every
/// entity bound to this `.material` re-renders.
fn flush_overrides(world: &mut World) {
    let dirty = world.get_resource::<MatCache>().map(|c| c.dirty).unwrap_or(false);
    if !dirty {
        return;
    }
    let (instance, instance_abs, asset_path) = {
        let cache = world.resource::<MatCache>();
        (cache.instance.clone(), cache.instance_abs.clone(), cache.path.clone())
    };
    world.resource_mut::<MatCache>().dirty = false;
    let Some(inst) = instance else { return };

    if let Ok(json) = serde_json::to_string_pretty(&inst) {
        if let Err(e) = std::fs::write(&instance_abs, json) {
            bevy::log::warn!("[material] couldn't write {}: {}", instance_abs.display(), e);
            return;
        }
    }
    if let Some(mut cache) = world.get_resource_mut::<MaterialCache>() {
        cache.invalidate(&asset_path);
    }
    let mut to_invalidate: Vec<Entity> = Vec::new();
    let mut q = world.query::<(Entity, &MaterialRef)>();
    for (e, mr) in q.iter(world) {
        if mr.0 == asset_path {
            to_invalidate.push(e);
        }
    }
    for e in to_invalidate {
        world.entity_mut(e).remove::<MaterialResolved>();
    }
}
