//! Bevy-native (ember) inspector panel.
//!
//! Registry-driven like the egui inspector: each `InspectorRegistry` entry shows
//! when its `has_fn` matches and renders either declarative `fields` (a
//! `FieldType` + get/set fn-pointers, rendered generically here) or a bespoke
//! `custom_ui_fn` egui closure (placeholder until the bevy_ui drawer contract).
//!
//! `rebuild_inspector` (exclusive) rebuilds sections + rows whenever the
//! selection / locked entity / component set / add-overlay changes (hashed
//! signature, so field-value edits don't trigger a rebuild — those are reactive
//! via `bind_2way`).
//!
//! Layout matches the egui inspector: component sections with a header
//! (caret · icon · title · enable toggle · trash) and field rows with a
//! right-aligned label column + boxed value, alternating row striping.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor::{
    EditorCommands, EditorSelection, FieldType, FieldValue, InspectorRegistry,
    NativeInspectorDrawer, NativeInspectorRegistry,
};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_with};
use renzora_ember::widgets::{drag_value, toggle_switch, DragRange};
use renzora_theme::ThemeManager;

type GetFn = fn(&World, Entity) -> Option<FieldValue>;
type SetFn = fn(&mut World, Entity, FieldValue);
type Pred = fn(&World, Entity) -> bool;
type Mutate = fn(&mut World, Entity);
type SetEnabled = fn(&mut World, Entity, bool);

const TEXT_VALUE: (u8, u8, u8) = (210, 210, 220);
const TEXT_MUTED: (u8, u8, u8) = (150, 150, 162);
const HEADER_BG: (u8, u8, u8) = (44, 44, 54);
const PANEL_DARK: (u8, u8, u8) = (30, 30, 38);
const BORDER: (u8, u8, u8) = (60, 60, 74);

fn c(rgb: (u8, u8, u8)) -> Color {
    Color::srgb_u8(rgb.0, rgb.1, rgb.2)
}

#[derive(Component)]
struct InspectorRoot;

#[derive(Resource, Default)]
struct NativeInspectorState {
    sig: Option<u64>,
    locked: Option<Entity>,
    add_open: bool,
}

pub fn register_native_inspector(app: &mut App) {
    use renzora_editor::SplashState;
    app.init_resource::<NativeInspectorState>();
    app.register_panel_content("inspector", true, |commands, _fonts| {
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(4.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                InspectorRoot,
                Name::new("inspector-root"),
            ))
            .id()
    });
    app.add_systems(
        Update,
        (
            section_collapse,
            remove_click,
            add_button_click,
            add_option_click,
            lock_click,
            enum_toggle,
            enum_option_click,
        )
            .run_if(in_state(SplashState::Editor)),
    );
    app.add_systems(Update, rebuild_inspector.run_if(in_state(SplashState::Editor)));
}

// ── Specs collected (under the exclusive borrow) before building ─────────────

#[derive(Clone, Copy)]
enum FieldKind {
    Float { speed: f32, min: f32, max: f32 },
    Vec3 { speed: f32 },
    Bool,
    Color,
    Enum { options: &'static [&'static str] },
    ReadOnly,
}

enum FieldInit {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Color([f32; 3]),
    Text(String),
}

struct FieldSpec {
    name: &'static str,
    kind: FieldKind,
    get_fn: GetFn,
    set_fn: SetFn,
    init: FieldInit,
}

struct SectionSpec {
    title: &'static str,
    icon: &'static str, // egui_phosphor glyph
    type_id: &'static str,
    custom: bool,
    /// Native (bevy_ui) drawer, if the component registered one. Takes priority
    /// over declarative fields.
    native_drawer: Option<NativeInspectorDrawer>,
    remove_fn: Option<Mutate>,
    enable: Option<(Pred, SetEnabled)>,
    enabled_now: bool,
    /// Category-derived header background + accent (icon tint).
    header_bg: (u8, u8, u8),
    accent: (u8, u8, u8),
    fields: Vec<FieldSpec>,
}

fn c32(col: bevy_egui::egui::Color32) -> (u8, u8, u8) {
    (col.r(), col.g(), col.b())
}

/// Replicates `renzora_ui::category_colors`: maps a component category to its
/// themed (accent, header_bg). So lights get an amber header, environment a
/// blue-grey one, etc. — not all the same.
fn category_rgb(theme: &renzora_theme::Theme, category: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    let s = match category {
        "environment" => &theme.categories.environment,
        "light" | "lighting" => &theme.categories.lighting,
        "camera" => &theme.categories.camera,
        "script" | "scripting" => &theme.categories.scripting,
        "physics" => &theme.categories.physics,
        "plugin" => &theme.categories.plugin,
        "nodes2d" | "nodes_2d" => &theme.categories.nodes_2d,
        "ui" => &theme.categories.ui,
        "rendering" => &theme.categories.rendering,
        "effects" | "particles" => &theme.categories.effects,
        _ => &theme.categories.transform,
    };
    (c32(s.accent.to_color32()), c32(s.header_bg.to_color32()))
}

/// An addable component for the "Add Component" overlay.
struct AddSpec {
    label: &'static str,
    icon: &'static str,
    add_fn: Mutate,
}

// ── Rebuild ──────────────────────────────────────────────────────────────────

fn rebuild_inspector(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Drop a stale lock, then resolve the inspected entity (lock wins).
    {
        let locked = world.resource::<NativeInspectorState>().locked;
        if let Some(e) = locked {
            if world.get_entity(e).is_err() {
                world.resource_mut::<NativeInspectorState>().locked = None;
            }
        }
    }
    let locked = world.resource::<NativeInspectorState>().locked;
    let add_open = world.resource::<NativeInspectorState>().add_open;
    let entity = locked.or_else(|| {
        world
            .get_resource::<EditorSelection>()
            .and_then(|s| s.get())
    });

    let mut cq = world.query_filtered::<Entity, With<InspectorRoot>>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let sig = inspector_signature(world, container, entity, locked.is_some(), add_open);
    if world.resource::<NativeInspectorState>().sig == Some(sig) {
        return;
    }

    let sections = collect_sections(world, entity);
    let adds = if add_open {
        collect_adds(world, entity)
    } else {
        Vec::new()
    };
    let existing: Vec<Entity> = world
        .get::<Children>(container)
        .map(|ch| ch.iter().collect())
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

        // Top toolbar: Add Component button.
        if entity.is_some() {
            let bar = add_bar(&mut commands, &fonts);
            commands.entity(container).add_child(bar);
        }

        match entity {
            None => {
                let l = empty_label(&mut commands, &fonts, "No entity selected");
                commands.entity(container).add_child(l);
            }
            Some(entity) => {
                if add_open {
                    let pop = add_overlay(&mut commands, &fonts, &adds);
                    commands.entity(container).add_child(pop);
                }
                if sections.is_empty() {
                    let l = empty_label(&mut commands, &fonts, "No inspectable components.");
                    commands.entity(container).add_child(l);
                }
                let locked_here = locked == Some(entity);
                for sec in &sections {
                    let (root, body) = build_section(&mut commands, &fonts, sec, entity, locked_here);
                    commands.entity(container).add_child(root);
                    if let Some(drawer) = sec.native_drawer {
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

    world.resource_mut::<NativeInspectorState>().sig = Some(sig);
}

fn inspector_signature(
    world: &World,
    container: Entity,
    entity: Option<Entity>,
    locked: bool,
    add_open: bool,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    container.to_bits().hash(&mut h);
    locked.hash(&mut h);
    add_open.hash(&mut h);
    match entity {
        Some(e) => {
            1u8.hash(&mut h);
            e.to_bits().hash(&mut h);
            if let Some(reg) = world.get_resource::<InspectorRegistry>() {
                for entry in reg.iter() {
                    if (entry.has_fn)(world, e) {
                        entry.type_id.hash(&mut h);
                    }
                }
            }
        }
        None => 0u8.hash(&mut h),
    }
    h.finish()
}

fn collect_sections(world: &World, entity: Option<Entity>) -> Vec<SectionSpec> {
    let Some(entity) = entity else {
        return Vec::new();
    };
    let Some(reg) = world.get_resource::<InspectorRegistry>() else {
        return Vec::new();
    };
    let theme = world.get_resource::<ThemeManager>();
    let native_reg = world.get_resource::<NativeInspectorRegistry>();
    let mut out = Vec::new();
    for entry in reg.iter() {
        if !(entry.has_fn)(world, entity) {
            continue;
        }
        let (accent, header_bg) = theme
            .map(|tm| category_rgb(&tm.active_theme, entry.category))
            .unwrap_or(((120, 140, 200), (44, 44, 54)));
        let enable = match (entry.is_enabled_fn, entry.set_enabled_fn) {
            (Some(g), Some(s)) => Some((g, s)),
            _ => None,
        };
        let enabled_now = enable.map(|(g, _)| g(world, entity)).unwrap_or(true);
        // Priority: a registered native bevy_ui drawer > declarative `fields` >
        // placeholder (component has only an egui `custom_ui_fn`).
        let native_drawer = native_reg.and_then(|r| r.get(entry.type_id));
        if native_drawer.is_some() {
            out.push(SectionSpec {
                title: entry.display_name,
                icon: entry.icon,
                type_id: entry.type_id,
                custom: false,
                native_drawer,
                remove_fn: entry.remove_fn,
                enable,
                enabled_now,
                header_bg,
                accent,
                fields: Vec::new(),
            });
            continue;
        }
        if entry.fields.is_empty() && entry.custom_ui_fn.is_some() {
            out.push(SectionSpec {
                title: entry.display_name,
                icon: entry.icon,
                type_id: entry.type_id,
                custom: true,
                native_drawer: None,
                remove_fn: entry.remove_fn,
                enable,
                enabled_now,
                header_bg,
                accent,
                fields: Vec::new(),
            });
            continue;
        }
        let mut fields = Vec::new();
        for f in &entry.fields {
            let val = (f.get_fn)(world, entity);
            let (kind, init) = match (&f.field_type, &val) {
                (FieldType::Float { speed, min, max }, Some(FieldValue::Float(v))) => (
                    FieldKind::Float {
                        speed: *speed,
                        min: *min,
                        max: *max,
                    },
                    FieldInit::Float(*v),
                ),
                (FieldType::Vec3 { speed }, Some(FieldValue::Vec3(a))) => {
                    (FieldKind::Vec3 { speed: *speed }, FieldInit::Vec3(*a))
                }
                (FieldType::Bool, Some(FieldValue::Bool(b))) => {
                    (FieldKind::Bool, FieldInit::Bool(*b))
                }
                (FieldType::Color, Some(FieldValue::Color(col))) => {
                    (FieldKind::Color, FieldInit::Color(*col))
                }
                (FieldType::Enum { options }, Some(FieldValue::Enum(s))) => {
                    (FieldKind::Enum { options }, FieldInit::Text(s.clone()))
                }
                _ => (FieldKind::ReadOnly, FieldInit::Text(format_value(val.as_ref()))),
            };
            fields.push(FieldSpec {
                name: f.name,
                kind,
                get_fn: f.get_fn,
                set_fn: f.set_fn,
                init,
            });
        }
        out.push(SectionSpec {
            title: entry.display_name,
            icon: entry.icon,
            type_id: entry.type_id,
            custom: false,
            native_drawer: None,
            remove_fn: entry.remove_fn,
            enable,
            enabled_now,
            header_bg,
            accent,
            fields,
        });
    }
    out
}

fn collect_adds(world: &World, entity: Option<Entity>) -> Vec<AddSpec> {
    let Some(entity) = entity else {
        return Vec::new();
    };
    let Some(reg) = world.get_resource::<InspectorRegistry>() else {
        return Vec::new();
    };
    reg.iter()
        .filter_map(|e| {
            let add_fn = e.add_fn?;
            (!(e.has_fn)(world, entity)).then_some(AddSpec {
                label: e.display_name,
                icon: e.icon,
                add_fn,
            })
        })
        .collect()
}

fn format_value(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::Float(f)) => format!("{f:.3}"),
        Some(FieldValue::Vec3(a)) => format!("{:.3}, {:.3}, {:.3}", a[0], a[1], a[2]),
        Some(FieldValue::Bool(b)) => b.to_string(),
        Some(FieldValue::Color(col)) => format!(
            "#{:02X}{:02X}{:02X}",
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8
        ),
        Some(FieldValue::String(s)) | Some(FieldValue::ReadOnly(s)) | Some(FieldValue::Enum(s)) => {
            s.clone()
        }
        Some(FieldValue::Asset(a)) => a.clone().unwrap_or_else(|| "—".into()),
        None => "—".into(),
    }
}

// ── Section ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SectionHeader {
    body: Entity,
    caret: Entity,
    open: bool,
}

#[derive(Component)]
struct RemoveBtn {
    remove_fn: Mutate,
    entity: Entity,
}

#[derive(Component)]
struct LockBtn {
    entity: Entity,
}

fn build_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    sec: &SectionSpec,
    entity: Entity,
    locked_here: bool,
) -> (Entity, Entity) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("inspector-section"),
        ))
        .id();

    // Body first (header references it).
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(4.0)),
                ..default()
            },
            Name::new("section-body"),
        ))
        .id();
    if sec.native_drawer.is_some() {
        // Body is filled by the registered native drawer once the build queue
        // has applied (it needs exclusive &mut World). See `rebuild_inspector`.
    } else if sec.custom {
        let note = empty_label(commands, fonts, "Custom inspector — pending native UI");
        commands.entity(body).add_child(note);
    } else {
        for field in &sec.fields {
            let r = build_field_row(commands, fonts, field, entity);
            commands.entity(body).add_child(r);
        }
    }

    // Header: caret · icon · title · spacer · [lock] · [enable] · [trash]
    let caret = phosphor_glyph(commands, fonts, "caret-down", TEXT_MUTED, 11.0);
    let icon = glyph_str(commands, fonts, sec.icon, sec.accent, 14.0);
    let title = commands
        .spawn((
            Text::new(sec.title),
            ui_font(&fonts.ui, 13.0),
            TextColor(c((238, 238, 246))),
            FocusPolicy::Pass,
        ))
        .id();
    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
        .id();

    let mut header_kids = vec![caret, icon, title, spacer];

    if sec.type_id == "name" {
        let lock = phosphor_glyph(
            commands,
            fonts,
            if locked_here { "lock-simple" } else { "lock-simple-open" },
            if locked_here { (120, 170, 255) } else { TEXT_MUTED },
            14.0,
        );
        commands
            .entity(lock)
            .insert((Interaction::default(), FocusPolicy::Block, LockBtn { entity }));
        header_kids.push(lock);
    }
    if let Some((_, set_enabled)) = sec.enable {
        let sw = toggle_switch(commands, sec.enabled_now);
        let g = sec.enable.unwrap().0;
        bind_2way(
            commands,
            sw,
            move |w| g(w, entity),
            move |w, v: &bool| set_enabled(w, entity, *v),
        );
        header_kids.push(sw);
    }
    if let Some(remove_fn) = sec.remove_fn {
        let trash = phosphor_glyph(commands, fonts, "trash", TEXT_MUTED, 13.0);
        commands
            .entity(trash)
            .insert((Interaction::default(), FocusPolicy::Block, RemoveBtn { remove_fn, entity }));
        header_kids.push(trash);
    }

    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c(sec.header_bg)),
            Interaction::default(),
            SectionHeader {
                body,
                caret,
                open: true,
            },
            Name::new("section-header"),
        ))
        .id();
    commands.entity(header).add_children(&header_kids);

    commands.entity(root).add_children(&[header, body]);
    (root, body)
}

fn build_field_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
) -> Entity {
    // The field's control(s) sit in a value container, then the shared
    // `inspector_row` adds a left-aligned label column — so declarative fields
    // and native drawers (which also use `inspector_row`) line up identically.
    let value = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("field-value"),
        ))
        .id();
    build_field_value(commands, fonts, field, entity, value);
    renzora_ember::inspector::inspector_row(commands, &fonts.ui, field.name, value)
}

fn build_field_value(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
    value_parent: Entity,
) {
    match field.kind {
        FieldKind::Float { speed, min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            let dv = drag_value(commands, &fonts.ui, "", TEXT_VALUE, init, speed.max(0.001));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            bind_2way(
                commands,
                dv,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| set_fn(w, entity, FieldValue::Float(*v)),
            );
            commands.entity(value_parent).add_child(dv);
        }
        FieldKind::Vec3 { speed } => {
            let init = if let FieldInit::Vec3(a) = field.init {
                a
            } else {
                [0.0; 3]
            };
            const AXES: [(&str, (u8, u8, u8)); 3] = [
                ("X", (230, 90, 90)),
                ("Y", (130, 200, 90)),
                ("Z", (90, 150, 230)),
            ];
            for (i, (axis, color)) in AXES.iter().enumerate() {
                let dv = drag_value(commands, &fonts.ui, axis, *color, init[i], speed.max(0.001));
                let (get_fn, set_fn) = (field.get_fn, field.set_fn);
                bind_2way(
                    commands,
                    dv,
                    move |w| match get_fn(w, entity) {
                        Some(FieldValue::Vec3(a)) => a[i],
                        _ => 0.0,
                    },
                    move |w, v: &f32| {
                        if let Some(FieldValue::Vec3(mut a)) = get_fn(w, entity) {
                            a[i] = *v;
                            set_fn(w, entity, FieldValue::Vec3(a));
                        }
                    },
                );
                commands.entity(value_parent).add_child(dv);
            }
        }
        FieldKind::Bool => {
            let init = matches!(field.init, FieldInit::Bool(true));
            let sw = toggle_switch(commands, init);
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            bind_2way(
                commands,
                sw,
                move |w| matches!(get_fn(w, entity), Some(FieldValue::Bool(true))),
                move |w, v: &bool| set_fn(w, entity, FieldValue::Bool(*v)),
            );
            commands.entity(value_parent).add_child(sw);
        }
        FieldKind::Color => {
            let init = if let FieldInit::Color(col) = field.init {
                col
            } else {
                [1.0; 3]
            };
            let swatch = commands
                .spawn((
                    Node {
                        width: Val::Px(44.0),
                        height: Val::Px(16.0),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(init[0], init[1], init[2])),
                    BorderColor::all(c((70, 70, 82))),
                    Name::new("color-swatch"),
                ))
                .id();
            let get_fn = field.get_fn;
            bind_with(
                commands,
                swatch,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Color(col)) => col,
                    _ => [0.0; 3],
                },
                |w, e, col: &[f32; 3]| {
                    if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                        bg.0 = Color::srgb(col[0], col[1], col[2]);
                    }
                },
            );
            commands.entity(value_parent).add_child(swatch);
        }
        FieldKind::Enum { options } => {
            let cur = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let dd = build_enum_dropdown(commands, fonts, entity, field.get_fn, field.set_fn, options, &cur);
            commands.entity(value_parent).add_child(dd);
        }
        FieldKind::ReadOnly => {
            let text = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let t = commands
                .spawn((
                    Text::new(text),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(c(TEXT_MUTED)),
                ))
                .id();
            commands.entity(value_parent).add_child(t);
        }
    }
}

// ── Enum dropdown ────────────────────────────────────────────────────────────

#[derive(Component)]
struct EnumTrigger {
    panel: Entity,
    open: bool,
}

#[derive(Component)]
struct EnumOption {
    set_fn: SetFn,
    entity: Entity,
    label: &'static str,
    panel: Entity,
}

fn build_enum_dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    get_fn: GetFn,
    set_fn: SetFn,
    options: &'static [&'static str],
    current: &str,
) -> Entity {
    // Popup of options.
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(c(PANEL_DARK)),
            BorderColor::all(c(BORDER)),
            GlobalZIndex(700),
            Name::new("enum-panel"),
        ))
        .id();
    let mut rows = Vec::with_capacity(options.len());
    for opt in options {
        let txt = commands
            .spawn((
                Text::new(*opt),
                ui_font(&fonts.ui, 11.0),
                TextColor(c(TEXT_VALUE)),
                FocusPolicy::Pass,
            ))
            .id();
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                EnumOption {
                    set_fn,
                    entity,
                    label: opt,
                    panel,
                },
                Name::new("enum-option"),
            ))
            .id();
        commands.entity(row).add_child(txt);
        rows.push(row);
    }
    commands.entity(panel).add_children(&rows);

    // Trigger: current value + caret.
    let value_text = commands
        .spawn((
            Text::new(current),
            ui_font(&fonts.ui, 11.0),
            TextColor(c(TEXT_VALUE)),
            FocusPolicy::Pass,
        ))
        .id();
    // Keep the trigger label in sync with the live value.
    bind_with(
        commands,
        value_text,
        move |w| match get_fn(w, entity) {
            Some(FieldValue::Enum(s)) => s,
            _ => String::new(),
        },
        |w, e, s: &String| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                if t.0 != *s {
                    t.0 = s.clone();
                }
            }
        },
    );
    let caret = phosphor_glyph(commands, fonts, "caret-down", TEXT_MUTED, 9.0);
    commands.entity(caret).insert(FocusPolicy::Pass);
    let trigger = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c((28, 28, 34))),
            BorderColor::all(c((70, 70, 82))),
            Interaction::default(),
            EnumTrigger { panel, open: false },
            Name::new("enum-trigger"),
        ))
        .id();
    commands.entity(trigger).add_children(&[value_text, caret]);

    let wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("enum-wrap"),
        ))
        .id();
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A Phosphor icon by *name* (resolved via ember's map).
fn phosphor_glyph(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    let e = renzora_ember::font::icon_text(commands, &fonts.phosphor, name, color, size);
    commands.entity(e).insert(FocusPolicy::Pass);
    e
}

/// A Phosphor icon given the *glyph string* directly (registry `entry.icon`).
fn glyph_str(
    commands: &mut Commands,
    fonts: &EmberFonts,
    glyph: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    commands
        .spawn((
            Text::new(glyph),
            TextFont {
                font: fonts.phosphor.clone(),
                font_size: size,
                ..default()
            },
            TextColor(c(color)),
            FocusPolicy::Pass,
        ))
        .id()
}

fn empty_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(c(TEXT_MUTED)),
            Node {
                margin: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

// ── Add Component bar + overlay ──────────────────────────────────────────────

#[derive(Component)]
struct AddButton;

#[derive(Component)]
struct AddOption {
    add_fn: Mutate,
}

fn add_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let icon = phosphor_glyph(commands, fonts, "puzzle-piece", TEXT_VALUE, 13.0);
    let label = commands
        .spawn((
            Text::new("Add Component"),
            ui_font(&fonts.ui, 12.0),
            TextColor(c(TEXT_VALUE)),
            FocusPolicy::Pass,
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c(HEADER_BG)),
            Interaction::default(),
            AddButton,
            Name::new("add-component"),
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);
    btn
}

fn add_overlay(commands: &mut Commands, fonts: &EmberFonts, adds: &[AddSpec]) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                max_height: Val::Px(260.0),
                overflow: Overflow::clip(),
                padding: UiRect::all(Val::Px(3.0)),
                row_gap: Val::Px(1.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c(PANEL_DARK)),
            BorderColor::all(c(BORDER)),
            Name::new("add-overlay"),
        ))
        .id();
    if adds.is_empty() {
        let l = empty_label(commands, fonts, "No components to add.");
        commands.entity(panel).add_child(l);
        return panel;
    }
    let mut rows = Vec::with_capacity(adds.len());
    for a in adds {
        let icon = glyph_str(commands, fonts, a.icon, TEXT_MUTED, 13.0);
        let label = commands
            .spawn((
                Text::new(a.label),
                ui_font(&fonts.ui, 12.0),
                TextColor(c(TEXT_VALUE)),
                FocusPolicy::Pass,
            ))
            .id();
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                AddOption { add_fn: a.add_fn },
                Name::new("add-option"),
            ))
            .id();
        commands.entity(row).add_children(&[icon, label]);
        rows.push(row);
    }
    commands.entity(panel).add_children(&rows);
    panel
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn section_collapse(
    mut headers: Query<(&Interaction, &mut SectionHeader), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut h) in &mut headers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        h.open = !h.open;
        if let Ok(mut n) = nodes.get_mut(h.body) {
            n.display = if h.open { Display::Flex } else { Display::None };
        }
        if let Ok(mut t) = texts.get_mut(h.caret) {
            let g = renzora_ember::font::icon_glyph(if h.open { "caret-down" } else { "caret-right" });
            if let Some(g) = g {
                t.0 = g.to_string();
            }
        }
    }
}

fn remove_click(
    q: Query<(&Interaction, &RemoveBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (remove_fn, entity) = (btn.remove_fn, btn.entity);
        cmds.push(move |w: &mut World| remove_fn(w, entity));
    }
}

fn lock_click(
    q: Query<(&Interaction, &LockBtn), Changed<Interaction>>,
    mut state: ResMut<NativeInspectorState>,
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

fn add_button_click(
    q: Query<&Interaction, (With<AddButton>, Changed<Interaction>)>,
    mut state: ResMut<NativeInspectorState>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            state.add_open = !state.add_open;
        }
    }
}

fn add_option_click(
    q: Query<(&Interaction, &AddOption), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
    selection: Option<Res<EditorSelection>>,
    mut state: ResMut<NativeInspectorState>,
) {
    let Some(cmds) = cmds else { return };
    let entity = state
        .locked
        .or_else(|| selection.and_then(|s| s.get()));
    let Some(entity) = entity else { return };
    for (interaction, opt) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let add_fn = opt.add_fn;
        cmds.push(move |w: &mut World| add_fn(w, entity));
        state.add_open = false;
    }
}

fn enum_toggle(
    mut triggers: Query<(&Interaction, &mut EnumTrigger), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut t) in &mut triggers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        t.open = !t.open;
        if let Ok(mut n) = nodes.get_mut(t.panel) {
            n.display = if t.open { Display::Flex } else { Display::None };
        }
    }
}

fn enum_option_click(
    q: Query<(&Interaction, &EnumOption), Changed<Interaction>>,
    mut triggers: Query<&mut EnumTrigger>,
    mut nodes: Query<&mut Node>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, opt) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (set_fn, entity, label) = (opt.set_fn, opt.entity, opt.label.to_string());
        cmds.push(move |w: &mut World| set_fn(w, entity, FieldValue::Enum(label.clone())));
        // Close the popup.
        if let Ok(mut n) = nodes.get_mut(opt.panel) {
            n.display = Display::None;
        }
        for mut t in &mut triggers {
            if t.panel == opt.panel {
                t.open = false;
            }
        }
    }
}
