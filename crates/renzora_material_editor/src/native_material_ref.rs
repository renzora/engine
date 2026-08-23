//! The Material component's inspector drawer.
//!
//! Three stacked pieces, in the order an artist reaches for them:
//!
//! 1. **The material slot** — which `.material` the entity uses: thumbnail,
//!    name picker, browse / open-in-editor / clear, and a whole-row drop target.
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
use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, IoTaskPool, Task};
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;

use renzora::core::CurrentProject;
use renzora_editor_framework::{
    open_asset_tab, AppEditorExt, AssetDragPayload, DocTabKind, MaterialThumbnailRegistry, SplashState,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_row, inspector_stripe};
use renzora_ember::reactive::{KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_with};
use renzora_ember::virtual_scroll::{virtual_scroll_versioned, VirtualMetrics};
use renzora_ember::theme::{
    accent, border, card_bg, faint_bg, hover_bg, placeholder, popup_bg, rgb, text_muted,
    text_primary,
};
use renzora_ember::widgets::{
    checkbox, drag_value, scroll_area, text_input, HoverTint, HoverTooltip, Popup,
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
                mat_clear_click,
                mat_picker_select,
                mat_revert_click,
                tex_slot_drop,
                tex_slot_highlight,
                tex_slot_browse,
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
#[derive(Component)]
struct MatClearBtn {
    entity: Entity,
}
/// Marks a picker popup panel. Purely a marker now — the rows are a keyed list
/// that captures its inspected entity directly, so nothing needs to look it up
/// off the panel. Kept because `refresh_material_index` gates on its presence
/// (no popup built → never walk the project).
#[derive(Component)]
struct MatPickerPanel;
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
    /// Preview handle for `texture`. Held by the row's `ImageNode`, so the image
    /// stays loaded for as long as the row is on screen.
    thumb: Option<Handle<Image>>,
}

/// Read every slot's current texture off the cached master graph.
///
/// Returns the full set of (possibly empty) slots for a mesh with no material
/// at all — those empty rows are the affordance, and dropping on one creates
/// the material. Returns nothing only for a derived instance, whose graph
/// belongs to the master and must not be edited from here.
fn slot_states(world: &World) -> Vec<SlotState> {
    let Some(cache) = world.get_resource::<MatCache>() else { return Vec::new() };
    if cache.instance.is_some() {
        return Vec::new();
    }
    let graph = cache.graph.as_ref();
    let assets = world.get_resource::<AssetServer>();
    TEXTURE_SLOTS
        .iter()
        .map(|slot| {
            let texture = graph.and_then(|g| texture_slots::slot_texture(g, slot));
            let thumb = texture
                .as_ref()
                .and_then(|p| assets.map(|a| a.load::<Image>(p.clone())));
            SlotState { slot, texture, thumb }
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
    if !slots.is_empty() {
        children.push(section_header(commands, fonts, "Textures", "Drop images here — they're wired into the material graph"));
        for state in slots {
            children.push(texture_slot_row(commands, fonts, entity, state));
        }
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

// ── Slot row ─────────────────────────────────────────────────────────────────

fn build_slot(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, path: &str) -> Entity {
    let has_mat = !path.is_empty();
    let label = if has_mat {
        std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or(path).to_string()
    } else {
        "No material".to_string()
    };
    let folder = std::path::Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Whole-row drop zone (material + image extensions).
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), align_items: AlignItems::Center, padding: UiRect::all(Val::Px(6.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(Color::NONE),
            RelativeCursorPosition::default(),
            MatDropZone { entity },
            Name::new("material-slot"),
        ))
        .id();

    // Thumbnail (ImageNode bound to the registry handle).
    let thumb = commands
        .spawn((
            Node { width: Val::Px(46.0), height: Val::Px(46.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(faint_bg())),
            ImageNode::new(Handle::default()),
            Name::new("material-thumb"),
        ))
        .id();
    bind_with(
        commands,
        thumb,
        move |w| {
            let path = material_path(&Rx::new(w.untracked()), entity);
            material_abs(&Rx::new(w.untracked()), &path).and_then(|abs| w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs)))
        },
        |w, e, h: &Option<Handle<Image>>| {
            if let Some(mut img) = w.get_mut::<ImageNode>(e) {
                img.image = h.clone().unwrap_or_default();
            }
        },
    );

    // Right column: name picker (Popup trigger) + action icons.
    let col = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();

    // Picker popup panel (filled by rebuild_picker), anchored under the name row.
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                width: Val::Px(260.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb((24, 24, 30))),
            BorderColor::all(rgb((70, 70, 82))),
            GlobalZIndex(1000),
            MatPickerPanel,
            Name::new("material-picker-popup"),
        ))
        .id();

    // Popup shell, built ONCE here rather than refilled per keystroke. The rows
    // are registered on the inner `list` node, so the search box below is never
    // despawned and keeps focus + in-progress text across every filter change.
    let search = text_input(commands, &fonts.ui, "Search materials…", "");
    bind_search(commands, search);
    let list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    register_picker_rows(commands, list, entity);
    let scroll = scroll_area(commands, list, PICKER_VIEWPORT_H);
    commands.entity(panel).add_children(&[search, scroll]);

    // Name button = popup trigger. The caret is its own icon rather than a
    // glyph appended to the label, so a long material name truncates against
    // the button edge instead of pushing the affordance out of view.
    let name_btn = commands
        .spawn((
            // No `overflow: clip` here: the picker popup is a child of this
            // button (it anchors to it), and clipping the button would clip the
            // popup away entirely. The label clips inside its own wrapper.
            Node { position_type: PositionType::Relative, width: Val::Percent(100.0), height: Val::Px(24.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(7.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            HoverTint::solid(rgb(popup_bg()), rgb(hover_bg()), rgb(hover_bg())),
            Interaction::default(),
            Popup::new(panel),
            HoverTooltip::new("Pick a material"),
            Name::new("material-name"),
        ))
        .id();
    let name_text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(if has_mat { text_primary() } else { placeholder() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    // A clip wrapper, because `Overflow::clip` clips a node's *children*: on the
    // text node itself a long material name would spill over the caret.
    let name_clip = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(name_clip).add_child(name_text);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(name_btn).add_children(&[name_clip, caret, panel]);

    // Folder line + action row: browse / open / clear.
    let actions = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), align_items: AlignItems::Center, ..default() })
        .id();
    if let Some(folder) = folder {
        let path_text = commands
            .spawn((
                Text::new(folder),
                ui_font(&fonts.ui, 9.0),
                TextColor(rgb(placeholder())),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        let path_clip = commands
            .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), align_items: AlignItems::Center, overflow: Overflow::clip(), ..default() })
            .id();
        commands.entity(path_clip).add_child(path_text);
        commands.entity(actions).add_child(path_clip);
    } else {
        let spacer = commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).id();
        commands.entity(actions).add_child(spacer);
    }
    let browse = icon_btn(commands, fonts, "folder-open", "Browse project materials");
    commands.entity(browse).insert(Popup::new(panel));
    let edit = icon_btn(commands, fonts, "pencil-simple", "Open in the material editor");
    commands.entity(edit).insert(MatEditBtn { entity });
    let clear = icon_btn(commands, fonts, "x", "Remove this material");
    commands.entity(clear).insert(MatClearBtn { entity });
    commands.entity(actions).add_children(&[browse, edit, clear]);

    commands.entity(col).add_children(&[name_btn, actions]);
    commands.entity(row).add_children(&[thumb, col]);
    row
}

fn icon_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, tooltip: &str) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Px(22.0), height: Val::Px(20.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            HoverTint::solid(Color::NONE, rgb(hover_bg()), rgb(accent()).with_alpha(0.35)),
            Interaction::default(),
            // Block, or the press also lands on whatever sits under the button —
            // for the per-slot clear that is the slot row itself, which would
            // open a file dialog on the same click that emptied the slot.
            bevy::ui::FocusPolicy::Block,
            HoverTooltip::new(tooltip),
            Name::new("material-icon-btn"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(btn).add_child(ic);
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
            HoverTooltip::new(format!("Drop an image to wire it into the material's {} input", state.slot.label.to_ascii_lowercase())),
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
        commands.entity(preview).insert(ImageNode::new(thumb.clone()));
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
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let value = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(if filled { text_muted() } else { placeholder() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(text_col).add_children(&[label, value]);
    commands.entity(row).add_children(&[preview, text_col]);

    if filled {
        let clear = icon_btn(commands, fonts, "x", "Clear this texture");
        commands.entity(clear).insert(TexSlotClearBtn { entity, slot: state.slot });
        commands.entity(row).add_child(clear);
    }
    row
}

// ── Picker popup rows (a windowed keyed list, registered once) ───────────────

/// Popup viewport height, and the row stride `picker_item` produces (26 px tall +
/// 1 px `row_gap`). Used to seed [`VirtualMetrics`] so the list is windowed from
/// the very first frame instead of building every row until a real measurement
/// lands — that first-frame burst is the thing this whole change removes.
const PICKER_VIEWPORT_H: f32 = 280.0;
const PICKER_ROW_H: f32 = 27.0;

/// Most rows the picker will ever offer. The search box is the way to reach the
/// rest; building thousands would defeat the windowing on first open.
const PICKER_MAX_ROWS: usize = 200;

/// Register the result rows as a windowed keyed list.
///
/// Called **once**, when the popup shell is built — never per frame and never per
/// keystroke. Registered on the inner `list` node rather than the panel, so
/// reconciling rows can never touch the search box: `run_keyed_lists` calls
/// `replace_children` on its container, which would otherwise blow the input away
/// (and with it the user's focus and half-typed query) on every keystroke.
fn register_picker_rows(commands: &mut Commands, list: Entity, entity: Entity) {
    virtual_scroll_versioned(
        commands,
        list,
        4,
        // Dirty token: re-snapshot only when the query text, the cached index, or
        // this entity's assigned material actually changes. `virtual_scroll_versioned`
        // folds the scroll window in on top of this, so scrolling still re-windows.
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
    commands.entity(list).insert(VirtualMetrics {
        offset: 0.0,
        viewport_h: PICKER_VIEWPORT_H,
        row_h: PICKER_ROW_H,
        columns: 1,
        measured: true,
    });
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
    // Filter by reference and clone only the survivors — the cached index can be
    // far larger than the rows the popup shows.
    let rows: Vec<(String, String, bool)> = materials
        .iter()
        .filter(|(rel, _)| lower.is_empty() || rel.to_ascii_lowercase().contains(&lower))
        .take(PICKER_MAX_ROWS)
        .map(|(rel, abs)| {
            let is_current = rel.as_str() == current_path.as_str();
            (rel.clone(), abs.clone(), is_current)
        })
        .collect();

    if rows.is_empty() {
        // NB: deliberately not `u64::MAX` / `u64::MAX - 1` — `virtual_scroll`
        // reserves those as its spacer keys, and colliding would make the empty
        // row and a spacer alias each other.
        let mut k = DefaultHasher::new();
        "\u{0}<no-matches>".hash(&mut k);
        return KeyedSnapshot {
            items: vec![(k.finish(), 0)],
            build: Box::new(|c: &mut Commands, f: &EmberFonts, _| {
                c.spawn((
                    Text::new("No matches"),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                ))
                .id()
            }),
        };
    }

    // Key = the project-relative path: stable identity that survives filtering, so
    // narrowing the search keeps surviving rows AND their thumbnail bindings.
    // Hash = only what is baked into the row at build time. The thumbnail
    // `Handle<Image>` is deliberately excluded — it arrives via the row's own
    // `bind_with`, and hashing it would make every thumbnail that resolves
    // despawn and rebuild its row.
    let items: Vec<(u64, u64)> = rows
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

    KeyedSnapshot {
        items,
        build: Box::new(move |c: &mut Commands, f: &EmberFonts, i: usize| {
            let (rel, abs, is_current) = &rows[i];
            picker_item(c, f, entity, rel, abs, *is_current)
        }),
    }
}

fn picker_item(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, rel: &str, abs: &str, is_current: bool) -> Entity {
    let path = std::path::Path::new(rel);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(rel).to_string();
    let parent = path.parent().and_then(|p| p.to_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    let item = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(if is_current { rgb(accent()).with_alpha(0.18) } else { Color::NONE }),
            Interaction::default(),
            MatPickerItem { entity, rel: rel.to_string() },
            Name::new("material-picker-item"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node { width: Val::Px(18.0), height: Val::Px(18.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() },
            BackgroundColor(rgb((14, 14, 18))),
            ImageNode::new(Handle::default()),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let abs_pb = PathBuf::from(abs);
    bind_with(
        commands,
        thumb,
        move |w| w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs_pb)),
        |w, e, h: &Option<Handle<Image>>| {
            if let Some(mut img) = w.get_mut::<ImageNode>(e) {
                img.image = h.clone().unwrap_or_default();
            }
        },
    );
    let name_color = if is_current { accent() } else { text_primary() };
    let text_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, ..default() })
        .id();
    let name = commands
        .spawn((Text::new(stem), ui_font(&fonts.ui, 11.0), TextColor(rgb(name_color)), bevy::ui::FocusPolicy::Pass))
        .id();
    commands.entity(text_col).add_child(name);
    if let Some(parent) = parent {
        let p = commands
            .spawn((Text::new(parent), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted())), bevy::ui::FocusPolicy::Pass))
            .id();
        commands.entity(text_col).add_child(p);
    }
    commands.entity(item).add_children(&[thumb, text_col]);
    item
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
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone())?;

    // Name it after the mesh so the file is findable later; fall back to a
    // generic name for an unnamed entity.
    let base = world
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| "Material".to_string());
    let safe: String = base
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();

    let dir = project_root.join("materials");
    let _ = std::fs::create_dir_all(&dir);
    // Never write over a material that already exists — two meshes called
    // "Cube" must not end up silently sharing (and overwriting) one file.
    let mut fs_path = dir.join(format!("{safe}.material"));
    let mut n = 1;
    while fs_path.exists() {
        fs_path = dir.join(format!("{safe}_{n}.material"));
        n += 1;
    }

    let asset_path = renzora_shader::material::precompiled::project_relative(&project_root, &fs_path);
    let mut graph = MaterialGraph::new(&safe, MaterialDomain::Surface);
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
        commands.queue(move |w: &mut World| bind_material(w, e, rel));
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
