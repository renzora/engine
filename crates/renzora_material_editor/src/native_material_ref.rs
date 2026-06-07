//! Bevy-native (ember) port of the egui Material component inspector
//! (`material_custom_ui`). Faithfully mirrors the Unreal-style slot row
//! (thumbnail · name-picker · browse · open · clear · whole-row drop) plus the
//! derived-material **Overrides** editor.
//!
//! The overrides live in a `.material` instance file on disk (not ECS data), so
//! they're loaded into [`MatCache`] on (entity, path) change; param widgets edit
//! the cache and [`flush_overrides`] writes it back + invalidates the resolver.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;

use renzora::core::CurrentProject;
use renzora_editor_framework::{
    open_asset_tab, AppEditorExt, AssetDragPayload, DocTabKind, MaterialThumbnailRegistry, SplashState,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_row, inspector_stripe};
use renzora_ember::reactive::{bind_2way, bind_with};
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};
use renzora_ember::widgets::{checkbox, drag_value, scroll_area, text_input, Popup};

use renzora_shader::material::codegen::{MaterialParam, ParamKind};
use renzora_shader::material::instance::{read_master_parameters, MaterialInstance};
use renzora_shader::material::material_ref::{MaterialRef, ParamValue};
use renzora_shader::material::resolver::{MaterialCache, MaterialResolved};

use crate::material_inspector::{
    default_param_value, find_material_files, pin_to_param, IMAGE_EXTENSIONS,
};

pub struct NativeMaterialRef;

impl Plugin for NativeMaterialRef {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatCache>();
        app.init_resource::<MatPickerFilter>();
        app.register_native_inspector_ui("material_ref", material_native);
        app.add_systems(
            Update,
            (
                rebuild_material,
                rebuild_picker,
                flush_overrides,
                mat_slot_drop,
                mat_slot_drop_highlight,
                mat_edit_click,
                mat_clear_click,
                mat_picker_select,
                mat_revert_click,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State ────────────────────────────────────────────────────────────────────

/// Loaded `.material` instance for the inspected entity (the overrides editor's
/// working copy). Reloaded on (entity, path) change; flushed to disk on edit.
#[derive(Resource, Default)]
struct MatCache {
    entity: Option<Entity>,
    path: String,
    instance_abs: PathBuf,
    instance: Option<MaterialInstance>,
    params: Vec<MaterialParam>,
    dirty: bool,
}

/// Search text for the material picker popup; `sig` bumps to drive a list rebuild.
#[derive(Resource, Default)]
struct MatPickerFilter {
    text: String,
    sig: u64,
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
#[derive(Component)]
struct MatPickerPanel {
    entity: Entity,
    sig: Option<u64>,
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

fn material_path(w: &World, entity: Entity) -> String {
    w.get::<MaterialRef>(entity).map(|m| m.0.clone()).unwrap_or_default()
}

fn material_abs(w: &World, path: &str) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    w.get_resource::<CurrentProject>().map(|p| p.resolve_path(path))
}

fn sig_of(entity: Entity, path: &str) -> u64 {
    let mut h = DefaultHasher::new();
    entity.hash(&mut h);
    path.hash(&mut h);
    h.finish()
}

/// Current override value for a param (override if present, else master default).
fn ov_get(w: &World, name: &str, kind: ParamKind, default_pin_param: &ParamValue) -> ParamValue {
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
fn load_cache(world: &mut World, entity: Entity, path: &str) {
    let mut instance = None;
    let mut params = Vec::new();
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
        cache.dirty = false;
    }
}

fn rebuild_material(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut q = world.query::<(Entity, &MatRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let path = material_path(world, entity);
        let sig = sig_of(entity, &path);
        if old_sig == Some(sig) {
            continue;
        }
        load_cache(world, entity, &path);
        // Request the current material's thumbnail.
        if let Some(abs) = material_abs(world, &path) {
            if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                reg.request(abs);
            }
        }
        let params = world.get_resource::<MatCache>().map(|c| c.params.clone()).unwrap_or_default();

        let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            build_body(&mut commands, &fonts, root, entity, &path, &params);
        }
        queue.apply(world);
        if let Some(mut mr) = world.get_mut::<MatRoot>(root) {
            mr.sig = Some(sig);
        }
        // Force the picker to repopulate for the new selection.
        if let Some(mut f) = world.get_resource_mut::<MatPickerFilter>() {
            f.sig = f.sig.wrapping_add(1);
        }
    }
}

fn build_body(commands: &mut Commands, fonts: &EmberFonts, root: Entity, entity: Entity, path: &str, params: &[MaterialParam]) {
    let mut children: Vec<Entity> = Vec::new();

    // ── Slot row ──
    children.push(build_slot(commands, fonts, entity, path));

    // ── Overrides ──
    if !params.is_empty() {
        children.push(overrides_header(commands, fonts));
        for (i, param) in params.iter().enumerate() {
            let row = param_row(commands, fonts, param);
            commands.entity(row).insert(BackgroundColor(inspector_stripe(i)));
            children.push(row);
        }
    }

    commands.entity(root).add_children(&children);
}

// ── Slot row ─────────────────────────────────────────────────────────────────

fn build_slot(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, path: &str) -> Entity {
    let has_mat = !path.is_empty();
    let label = if has_mat {
        std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or(path).to_string()
    } else {
        "None".to_string()
    };

    // Whole-row drop zone (material + image extensions).
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Stretch, padding: UiRect::all(Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            RelativeCursorPosition::default(),
            MatDropZone { entity },
            Name::new("material-slot"),
        ))
        .id();

    // Thumbnail (ImageNode bound to the registry handle).
    let thumb = commands
        .spawn((
            Node { width: Val::Px(48.0), height: Val::Px(48.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb((14, 14, 18))),
            ImageNode::new(Handle::default()),
            Name::new("material-thumb"),
        ))
        .id();
    bind_with(
        commands,
        thumb,
        move |w| {
            let path = material_path(w, entity);
            material_abs(w, &path).and_then(|abs| w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(&abs)))
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
            MatPickerPanel { entity, sig: None },
            Name::new("material-picker-popup"),
        ))
        .id();

    // Name button = popup trigger.
    let name_btn = commands
        .spawn((
            Node { position_type: PositionType::Relative, width: Val::Percent(100.0), height: Val::Px(22.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb((40, 40, 48))),
            Interaction::default(),
            Popup::new(panel),
            Name::new("material-name"),
        ))
        .id();
    let name_text = commands
        .spawn((Text::new(format!("{}  \u{25BE}", label)), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::ui::FocusPolicy::Pass))
        .id();
    commands.entity(name_btn).add_children(&[name_text, panel]);

    // Action row: browse / open / clear.
    let actions = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), align_items: AlignItems::Center, ..default() })
        .id();
    let browse = icon_btn(commands, fonts, "folder-open");
    commands.entity(browse).insert(Popup::new(panel));
    let edit = icon_btn(commands, fonts, "pencil-simple");
    commands.entity(edit).insert(MatEditBtn { entity });
    let clear = icon_btn(commands, fonts, "arrow-counter-clockwise");
    commands.entity(clear).insert(MatClearBtn { entity });
    commands.entity(actions).add_children(&[browse, edit, clear]);

    commands.entity(col).add_children(&[name_btn, actions]);
    commands.entity(row).add_children(&[thumb, col]);
    row
}

fn icon_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Px(22.0), height: Val::Px(18.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb((40, 40, 48))),
            Interaction::default(),
            Name::new("material-icon-btn"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    commands.entity(btn).add_child(ic);
    btn
}

// ── Picker popup (rebuilt on filter change) ──────────────────────────────────

fn rebuild_picker(world: &mut World) {
    let filter_sig = world.get_resource::<MatPickerFilter>().map(|f| f.sig).unwrap_or(0);
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    // The picker container is the popup panel itself; locate panels needing a refill.
    let mut q = world.query::<(Entity, &MatPickerPanel)>();
    let panels: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(e, p)| (e, p.entity, p.sig)).collect();
    for (panel, entity, old_sig) in panels {
        if old_sig == Some(filter_sig) {
            continue;
        }
        rebuild_one_picker(world, &fonts, panel, entity, filter_sig);
    }
}

fn rebuild_one_picker(world: &mut World, fonts: &EmberFonts, panel: Entity, entity: Entity, sig: u64) {
    let query = world.get_resource::<MatPickerFilter>().map(|f| f.text.clone()).unwrap_or_default();
    let current_path = material_path(world, entity);
    let root = world.get_resource::<CurrentProject>().map(|p| p.path.clone());
    let materials = match &root {
        Some(r) => find_material_files(r),
        None => Vec::new(),
    };
    let lower = query.trim().to_ascii_lowercase();
    let filtered: Vec<(String, String)> = materials
        .into_iter()
        .filter(|(rel, _)| lower.is_empty() || rel.to_ascii_lowercase().contains(&lower))
        .take(200)
        .collect();

    let existing: Vec<Entity> = world.get::<Children>(panel).map(|c| c.iter().collect()).unwrap_or_default();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        // Keep only the search box (the first child); despawn the previous list.
        for (i, ch) in existing.iter().enumerate() {
            if i == 0 {
                continue;
            }
            commands.entity(*ch).despawn();
        }
        if existing.is_empty() {
            // First build — create the search field.
            let search = text_input(&mut commands, &fonts.ui, "Search materials…", "");
            bind_search(&mut commands, search);
            commands.entity(panel).add_child(search);
        }
        // Build the list.
        let list = commands
            .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() })
            .id();
        if filtered.is_empty() {
            let empty = commands
                .spawn((Text::new("No matches"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
                .id();
            commands.entity(list).add_child(empty);
        } else {
            for (rel, abs) in &filtered {
                let item = picker_item(&mut commands, fonts, entity, rel, abs, rel.as_str() == current_path.as_str());
                commands.entity(list).add_child(item);
            }
        }
        let scroll = scroll_area(&mut commands, list, 280.0);
        commands.entity(panel).add_child(scroll);
    }
    queue.apply(world);
    if let Some(mut p) = world.get_mut::<MatPickerPanel>(panel) {
        p.sig = Some(sig);
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
                f.sig = f.sig.wrapping_add(1);
            }
        },
    );
}

// ── Overrides ────────────────────────────────────────────────────────────────

fn overrides_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let h = commands
        .spawn(Node { margin: UiRect { top: Val::Px(8.0), bottom: Val::Px(2.0), ..default() }, ..default() })
        .id();
    let t = commands
        .spawn((Text::new("Overrides"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(h).add_child(t);
    h
}

fn param_row(commands: &mut Commands, fonts: &EmberFonts, param: &MaterialParam) -> Entity {
    let name = param.name.clone();
    let kind = param.kind;
    let default_param = pin_to_param(&param.default).unwrap_or(default_param_value(kind));

    let ctrl = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();

    let editor = build_param_editor(commands, fonts, name.clone(), kind, default_param);
    let revert = icon_btn(commands, fonts, "arrow-counter-clockwise");
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
                move |w| match ov_get(w, &n1, kind, &d1) {
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
                move |w| matches!(ov_get(w, &n1, kind, &d1), ParamValue::Bool(true)),
                move |w, v: &bool| ov_set(w, &name, ParamValue::Bool(*v)),
            );
            cb
        }
        ParamKind::Color => {
            let n1 = name.clone();
            let d1 = default_param.clone();
            color_field_rgba(
                commands,
                move |w| match ov_get(w, &n1, kind, &d1) {
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
                    move |w| vec_component(&ov_get(w, &n1, kind, &d1), i),
                    move |w, v: &f32| {
                        let cur = ov_get(w, &n2, kind2, &default_param_value(kind2));
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
        let dropped = payload.path.clone();
        let entity = zone.entity;
        commands.queue(move |w: &mut World| apply_drop(w, entity, dropped));
        break;
    }
}

fn apply_drop(world: &mut World, entity: Entity, dropped: PathBuf) {
    let ext = dropped.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "material" {
        let mat_path = world
            .get_resource::<CurrentProject>()
            .map(|p| p.make_asset_relative(&dropped))
            .unwrap_or_else(|| dropped.to_string_lossy().to_string());
        bind_material(world, entity, mat_path);
    } else if IMAGE_EXTENSIONS.iter().any(|e| ext == *e) {
        create_material_from_image(world, entity, dropped);
    }
}

fn bind_material(world: &mut World, entity: Entity, mat_path: String) {
    world.entity_mut(entity).remove::<MaterialResolved>();
    if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
        mr.0 = mat_path;
    } else {
        world.entity_mut(entity).insert(MaterialRef(mat_path));
    }
}

/// Auto-create a one-texture `.material` from a dropped image and bind it
/// (mirrors the egui drawer's image-drop path).
fn create_material_from_image(world: &mut World, entity: Entity, dropped: PathBuf) {
    let (tex_path, mat_save_dir) = {
        let project = world.get_resource::<CurrentProject>();
        let tex = project.map(|p| p.make_asset_relative(&dropped)).unwrap_or_else(|| dropped.to_string_lossy().to_string());
        let dir = project.map(|p| p.path.join("materials")).unwrap_or_else(|| PathBuf::from("."));
        (tex, dir)
    };
    let mat_name = dropped.file_stem().and_then(|s| s.to_str()).unwrap_or("material").to_string();

    let mut graph = renzora_shader::material::graph::MaterialGraph::new(&mat_name, renzora_shader::material::graph::MaterialDomain::Surface);
    let tex_id = graph.add_node("texture/sample", [-200.0, 0.0]);
    if let Some(node) = graph.get_node_mut(tex_id) {
        node.input_values.insert("texture".to_string(), renzora_shader::material::graph::PinValue::TexturePath(tex_path));
    }
    let Some(output) = graph.output_node() else { return };
    let output_id = output.id;
    graph.connect(tex_id, "color", output_id, "base_color");

    let _ = std::fs::create_dir_all(&mat_save_dir);
    let mat_file = mat_save_dir.join(format!("{}.material", mat_name));
    if let Some(project_root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) {
        let mut graph = graph;
        if let Ok((json, _errors)) = renzora_shader::material::precompiled::save_compiled_and_serialize(&mut graph, &project_root, &mat_file) {
            let _ = std::fs::write(&mat_file, &json);
        }
    }
    let mat_asset_path = world
        .get_resource::<CurrentProject>()
        .map(|p| p.make_asset_relative(&mat_file))
        .unwrap_or_else(|| mat_file.to_string_lossy().to_string());
    bind_material(world, entity, mat_asset_path);
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
            let path = material_path(w, e);
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
