//! Bevy-native (ember) inspector panel.
//!
//! The egui inspector is registry-driven: each `InspectorRegistry` entry either
//! declares `fields` (a `FieldType` + get/set fn-pointers) or supplies a bespoke
//! `custom_ui_fn` egui closure. The declarative-field entries are renderable
//! generically, so this native panel rebuilds its sections + field rows whenever
//! the selection or the selected entity's component set changes, two-way-binding
//! each field through its get/set. `custom_ui_fn` entries show a placeholder
//! until the bevy_ui drawer contract lands (the rework shared with the viewport
//! mode/tool drawers).
//!
//! Increment 1: Float / Vec3 / Bool editing + read-only display for the rest.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor::{EditorSelection, FieldType, FieldValue, InspectorRegistry};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::{checkbox, collapsible, drag_value, DragRange};

type GetFn = fn(&World, Entity) -> Option<FieldValue>;
type SetFn = fn(&mut World, Entity, FieldValue);

#[derive(Component)]
struct InspectorRoot;

/// Tracks the structural signature the panel was last built for, so the
/// exclusive rebuild only runs on selection / component-set / panel changes.
#[derive(Resource, Default)]
struct NativeInspectorState {
    sig: Option<u64>,
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
                    padding: UiRect::all(Val::Px(6.0)),
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                InspectorRoot,
                Name::new("inspector-root"),
            ))
            .id()
    });
    app.add_systems(
        Update,
        rebuild_inspector.run_if(in_state(SplashState::Editor)),
    );
}

#[derive(Clone, Copy)]
enum FieldKind {
    Float { speed: f32, min: f32, max: f32 },
    Vec3 { speed: f32 },
    Bool,
    ReadOnly,
}

enum FieldInit {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
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
    /// `custom_ui_fn` present — render a placeholder (pending the drawer contract).
    custom: bool,
    fields: Vec<FieldSpec>,
}

/// Exclusive (the registry's `has_fn`/`get_fn` predicates take `&World`).
fn rebuild_inspector(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let entity = world.get_resource::<EditorSelection>().and_then(|s| s.get());
    let mut cq = world.query_filtered::<Entity, With<InspectorRoot>>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let sig = inspector_signature(world, container, entity);
    if world.resource::<NativeInspectorState>().sig == Some(sig) {
        return;
    }

    let sections = collect_sections(world, entity);
    let existing: Vec<Entity> = world
        .get::<Children>(container)
        .map(|c| c.iter().collect())
        .unwrap_or_default();

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for child in existing {
            commands.entity(child).despawn();
        }
        match entity {
            None => {
                let label = empty_label(&mut commands, &fonts, "No entity selected");
                commands.entity(container).add_child(label);
            }
            Some(entity) => {
                if sections.is_empty() {
                    let label = empty_label(&mut commands, &fonts, "No inspectable components.");
                    commands.entity(container).add_child(label);
                }
                for sec in &sections {
                    let (root, body) = collapsible(&mut commands, &fonts, None, sec.title, true);
                    if sec.custom {
                        let note = empty_label(
                            &mut commands,
                            &fonts,
                            "Custom inspector — pending native UI",
                        );
                        commands.entity(body).add_child(note);
                    } else {
                        for field in &sec.fields {
                            let r = build_field_row(&mut commands, &fonts, field, entity);
                            commands.entity(body).add_child(r);
                        }
                    }
                    commands.entity(container).add_child(root);
                }
            }
        }
    }
    queue.apply(world);
    world.resource_mut::<NativeInspectorState>().sig = Some(sig);
}

/// Hash of (panel, selected entity, present component type_ids) — changes only
/// on a structural change, not on field-value edits (those are reactive).
fn inspector_signature(world: &World, container: Entity, entity: Option<Entity>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    container.to_bits().hash(&mut h);
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
    let mut out = Vec::new();
    for entry in reg.iter() {
        if !(entry.has_fn)(world, entity) {
            continue;
        }
        if entry.custom_ui_fn.is_some() {
            out.push(SectionSpec {
                title: entry.display_name,
                custom: true,
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
            custom: false,
            fields,
        });
    }
    out
}

fn format_value(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::Float(f)) => format!("{f:.3}"),
        Some(FieldValue::Vec3(a)) => format!("{:.3}, {:.3}, {:.3}", a[0], a[1], a[2]),
        Some(FieldValue::Bool(b)) => b.to_string(),
        Some(FieldValue::Color(c)) => format!(
            "#{:02X}{:02X}{:02X}",
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8
        ),
        Some(FieldValue::String(s)) | Some(FieldValue::ReadOnly(s)) | Some(FieldValue::Enum(s)) => {
            s.clone()
        }
        Some(FieldValue::Asset(a)) => a.clone().unwrap_or_else(|| "—".into()),
        None => "—".into(),
    }
}

fn build_field_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
) -> Entity {
    let label = commands
        .spawn((
            Text::new(field.name),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgb_u8(200, 200, 212)),
            Node {
                width: Val::Px(96.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();

    let widget = match field.kind {
        FieldKind::Float { speed, min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), init, speed.max(0.001));
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
            dv
        }
        FieldKind::Vec3 { speed } => {
            let init = if let FieldInit::Vec3(a) = field.init {
                a
            } else {
                [0.0; 3]
            };
            build_vec3(commands, fonts, entity, field.get_fn, field.set_fn, init, speed)
        }
        FieldKind::Bool => {
            let init = matches!(field.init, FieldInit::Bool(true));
            let cb = checkbox(commands, init);
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            bind_2way(
                commands,
                cb,
                move |w| matches!(get_fn(w, entity), Some(FieldValue::Bool(true))),
                move |w, v: &bool| set_fn(w, entity, FieldValue::Bool(*v)),
            );
            cb
        }
        FieldKind::ReadOnly => {
            let text = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            commands
                .spawn((
                    Text::new(text),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(Color::srgb_u8(160, 160, 172)),
                ))
                .id()
        }
    };

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            Name::new("inspector-field"),
        ))
        .id();
    commands.entity(row).add_children(&[label, widget]);
    row
}

#[allow(clippy::too_many_arguments)]
fn build_vec3(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    get_fn: GetFn,
    set_fn: SetFn,
    init: [f32; 3],
    speed: f32,
) -> Entity {
    const AXES: [(&str, (u8, u8, u8)); 3] = [
        ("X", (230, 90, 90)),
        ("Y", (130, 200, 90)),
        ("Z", (90, 150, 230)),
    ];
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                flex_grow: 1.0,
                ..default()
            },
            Name::new("inspector-vec3"),
        ))
        .id();
    let mut kids = Vec::with_capacity(3);
    for (i, (axis, color)) in AXES.iter().enumerate() {
        let dv = drag_value(commands, &fonts.ui, axis, *color, init[i], speed.max(0.001));
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
        kids.push(dv);
    }
    commands.entity(row).add_children(&kids);
    row
}

fn empty_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgb_u8(150, 150, 162)),
            Node {
                margin: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}
