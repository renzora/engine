//! Inspector entries for physics components.
//!
//! Registered automatically when the `editor` feature is enabled.

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::{EditorSelection, InspectorEntry, ToolEntry, ToolSection};

use crate::{
    ColliderEditMode, CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType,
};

/// Register all physics inspector entries, spawn presets, icons, and observers via `AppEditorExt`.
/// Queue `root` and every descendant with a `Mesh3d` (lacking a collision
/// shape) into `ColliderStampQueue`. A background system drains it a batch
/// per frame so the hierarchy panel can show a live progress bar.
fn stamp_mesh_colliders_on_descendants(world: &mut World, root: Entity) {
    let mut stack = vec![root];
    let mut targets: Vec<Entity> = Vec::new();
    while let Some(e) = stack.pop() {
        let has_mesh = world.entity(e).contains::<bevy::prelude::Mesh3d>();
        let has_shape = world.entity(e).contains::<CollisionShapeData>();
        if has_mesh && !has_shape {
            targets.push(e);
        }
        if let Some(children) = world.get::<bevy::prelude::Children>(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
    }
    let total = targets.len();
    if total == 0 {
        renzora::console_log::console_warn("Physics", "No descendant mesh entities found to stamp");
        return;
    }
    if let Some(mut queue) = world.get_resource_mut::<crate::ColliderStampQueue>() {
        queue.root = Some(root);
        queue.total = total;
        queue.remaining = targets;
    }
    renzora::console_log::console_info(
        "Physics",
        format!("Queued {} entities for mesh collider stamping", total),
    );
}

/// Remove `PhysicsBodyData` + `CollisionShapeData` from `root` and every
/// descendant that currently has them. Also despawns the avian backend
/// components via `despawn_physics_components` to keep things clean.
fn strip_colliders_on_descendants(world: &mut World, root: Entity) {
    let mut stack = vec![root];
    let mut targets: Vec<Entity> = Vec::new();
    while let Some(e) = stack.pop() {
        let has_body = world.entity(e).contains::<PhysicsBodyData>();
        let has_shape = world.entity(e).contains::<CollisionShapeData>();
        if has_body || has_shape {
            targets.push(e);
        }
        if let Some(children) = world.get::<bevy::prelude::Children>(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
    }
    let count = targets.len();
    for e in &targets {
        world
            .entity_mut(*e)
            .remove::<PhysicsBodyData>()
            .remove::<CollisionShapeData>()
            .remove::<crate::data::RuntimePhysics>();
        let mut cmds = world.commands();
        crate::despawn_physics_components(&mut cmds, *e);
    }
    // Clear any pending stamp queue entries that reference the just-stripped root
    // so a strip immediately after a stamp doesn't keep adding components.
    if let Some(mut queue) = world.get_resource_mut::<crate::ColliderStampQueue>() {
        queue.remaining.retain(|e| !targets.contains(e));
        if queue.remaining.is_empty() {
            queue.root = None;
            queue.total = 0;
        }
    }
    renzora::console_log::console_success(
        "Physics",
        format!("Stripped colliders from {} descendant entities", count),
    );
}

pub fn register_physics_inspectors(app: &mut App) {
    // Auto-insert default collider on new MeshPrimitive entities
    app.add_observer(auto_insert_collider_for_shape);
    use renzora_editor::{AppEditorExt, EntityPreset};

    app.register_inspector(physics_body_entry())
        .register_inspector(collision_shape_entry());
    // Native (bevy_ui) inspector drawers.
    app.register_native_inspector_ui("physics_body", physics_body_native);
    app.register_native_inspector_ui("collision_shape", collision_shape_native);
    app.add_systems(
        bevy::prelude::Update,
        (edit_collider_click, stamp_strip_click).run_if(bevy::prelude::in_state(renzora_editor::SplashState::Editor)),
    );

    app.register_tool(
        ToolEntry::new(
            "physics.edit_collider",
            regular::PENCIL_SIMPLE,
            "Edit Collider — drag handles to resize/move",
            ToolSection::Custom("physics"),
        )
        .visible_if(|world| {
            let Some(sel) = world.resource::<EditorSelection>().get() else {
                return false;
            };
            world.get::<CollisionShapeData>(sel).is_some()
        })
        .active_if(|world| {
            world
                .get_resource::<ColliderEditMode>()
                .map(|c| c.active)
                .unwrap_or(false)
        })
        .on_activate(|world| {
            if let Some(mut m) = world.get_resource_mut::<ColliderEditMode>() {
                m.active = !m.active;
            }
        }),
    );

    // Spawn presets
    app.register_entity_preset(EntityPreset {
        id: "rigid_body",
        display_name: "Rigid Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("RigidBody3D"),
                    Transform::default(),
                    PhysicsBodyData::default(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "static_body",
        display_name: "Static Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("StaticBody3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "kinematic_body",
        display_name: "Kinematic Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("KinematicBody3D"),
                    Transform::default(),
                    PhysicsBodyData::kinematic_body(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "box_collider",
        display_name: "Box Collider",
        icon: regular::BOUNDING_BOX,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("BoxShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "sphere_collider",
        display_name: "Sphere Collider",
        icon: regular::GLOBE_HEMISPHERE_EAST,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("SphereShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::sphere(0.5),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "capsule_collider",
        display_name: "Capsule Collider",
        icon: regular::PILL,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("CapsuleShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::capsule(0.5, 0.5),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "cylinder_collider",
        display_name: "Cylinder Collider",
        icon: regular::CYLINDER,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("CylinderShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::cylinder(0.5, 0.5),
                ))
                .id()
        },
    });
}

/// Map a shape ID to a default collision shape for auto-physics on spawned meshes.
pub fn default_collider_for_shape(id: &str) -> Option<CollisionShapeData> {
    Some(match id {
        "cube" => CollisionShapeData::cuboid(Vec3::splat(0.5)),
        "sphere" => CollisionShapeData::sphere(0.5),
        "cylinder" => CollisionShapeData::cylinder(0.5, 0.5),
        "capsule" => CollisionShapeData::capsule(0.5, 0.25),
        "cone" => CollisionShapeData::cylinder(0.5, 0.5),
        "hemisphere" => CollisionShapeData::sphere(0.5),
        "plane" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.001, 0.5)),
        "wedge" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "wall" => CollisionShapeData::cuboid(Vec3::new(0.5, 1.0, 0.05)),
        "ramp" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.25, 1.0)),
        "doorway" => CollisionShapeData::cuboid(Vec3::new(0.5, 1.0, 0.05)),
        "window_wall" => CollisionShapeData::cuboid(Vec3::new(0.5, 1.0, 0.05)),
        "pillar" => CollisionShapeData::cylinder(0.15, 1.0),
        "l_shape" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "t_shape" => CollisionShapeData::cuboid(Vec3::new(0.75, 0.5, 0.5)),
        "cross_shape" => CollisionShapeData::cuboid(Vec3::new(0.75, 0.5, 0.75)),
        "corner" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "stairs" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "half_cylinder" => CollisionShapeData::cylinder(0.5, 0.5),
        "quarter_pipe" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "curved_wall" => CollisionShapeData::cylinder(0.5, 1.0),
        "spiral_stairs" => CollisionShapeData::cylinder(0.5, 1.0),
        "pipe" => CollisionShapeData::cylinder(0.5, 0.5),
        "ring" => CollisionShapeData::cylinder(0.5, 0.1),
        "funnel" => CollisionShapeData::cylinder(0.5, 0.5),
        "gutter" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.25, 0.5)),
        "torus" => CollisionShapeData::cylinder(0.5, 0.15),
        "prism" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "pyramid" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        _ => return None,
    })
}

// ── Physics Body ────────────────────────────────────────────────────────────

fn physics_body_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "physics_body",
        display_name: "Physics Body",
        icon: regular::CUBE,
        category: "physics",
        has_fn: |world, entity| world.get::<PhysicsBodyData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PhysicsBodyData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PhysicsBodyData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

/// Native (bevy_ui / ember) drawer for `PhysicsBodyData` — mirrors the inspector layout.
fn physics_body_native(world: &mut World, entity: Entity) -> Entity {
    use renzora_ember::font::ui_font;
    use renzora_ember::inspector::{inspector_body, inspector_row, inspector_stripe};
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::theme::{rgb, text_muted};
    use renzora_ember::widgets::{drag_value, dropdown, DragRange};

    let Some(b) = world.get::<PhysicsBodyData>(entity) else {
        return world.spawn(Node::default()).id();
    };
    let (bt_idx, mass, grav, lin, ang) = (
        match b.body_type {
            PhysicsBodyType::RigidBody => 0usize,
            PhysicsBodyType::StaticBody => 1,
            PhysicsBodyType::KinematicBody => 2,
        },
        b.mass,
        b.gravity_scale,
        b.linear_damping,
        b.angular_damping,
    );

    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();
        let mut rows: Vec<Entity> = Vec::new();

        // Body Type dropdown.
        let dd = dropdown(commands, fonts, &["Rigid Body", "Static Body", "Kinematic Body"], bt_idx);
        bind_2way(
            commands,
            dd,
            move |w| match w.get::<PhysicsBodyData>(entity).map(|b| b.body_type) {
                Some(PhysicsBodyType::StaticBody) => 1usize,
                Some(PhysicsBodyType::KinematicBody) => 2,
                _ => 0,
            },
            move |w, v: &usize| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                    b.body_type = match v {
                        1 => PhysicsBodyType::StaticBody,
                        2 => PhysicsBodyType::KinematicBody,
                        _ => PhysicsBodyType::RigidBody,
                    };
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, "Body Type", dd));

        // Numeric fields (label, init, step, min, max, get, set).
        let float_row =
            |commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts, label: &'static str, init: f32, step: f32, min: f32, max: f32, get: fn(&PhysicsBodyData) -> f32, set: fn(&mut PhysicsBodyData, f32)| {
                let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), init, step);
                commands.entity(dv).insert(DragRange { min, max });
                bind_2way(
                    commands,
                    dv,
                    move |w| w.get::<PhysicsBodyData>(entity).map(get).unwrap_or(0.0),
                    move |w, v: &f32| {
                        if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                            set(&mut b, *v);
                        }
                    },
                );
                inspector_row(commands, &fonts.ui, label, dv)
            };
        rows.push(float_row(commands, fonts, "Mass", mass, 0.1, 0.001, f32::MAX, |b| b.mass, |b, v| b.mass = v));
        rows.push(float_row(commands, fonts, "Gravity Scale", grav, 0.05, -10.0, 10.0, |b| b.gravity_scale, |b, v| b.gravity_scale = v));
        rows.push(float_row(commands, fonts, "Linear Damping", lin, 0.01, 0.0, 100.0, |b| b.linear_damping, |b, v| b.linear_damping = v));
        rows.push(float_row(commands, fonts, "Angular Damping", ang, 0.01, 0.0, 100.0, |b| b.angular_damping, |b, v| b.angular_damping = v));

        // Alternating row stripe (matches the inspector look).
        for (i, r) in rows.iter().enumerate() {
            commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(col).add_children(&rows);

        // "Lock Axes" sub-header + the two grouped X/Y/Z checkbox rows.
        let header = commands
            .spawn((
                Text::new("Lock Axes"),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
                Node { margin: UiRect::new(Val::Px(2.0), Val::ZERO, Val::Px(4.0), Val::Px(2.0)), ..default() },
            ))
            .id();
        let trans = lock_row(commands, fonts, entity, "Translation", false);
        let rot = lock_row(commands, fonts, entity, "Rotation", true);
        commands.entity(trans).insert(BackgroundColor(inspector_stripe(0)));
        commands.entity(rot).insert(BackgroundColor(inspector_stripe(1)));
        commands.entity(col).add_children(&[header, trans, rot]);

        col
    })
}

/// One "Lock Axes" row: a label + three X/Y/Z checkboxes bound to the body's
/// translation- or rotation-lock flags.
fn lock_row(commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts, entity: Entity, label: &str, rotation: bool) -> Entity {
    use renzora_ember::font::ui_font;
    use renzora_ember::inspector::inspector_row;
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::theme::{rgb, text_muted};
    use renzora_ember::widgets::checkbox;

    let group = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let mut cells: Vec<Entity> = Vec::new();
    for axis in 0..3usize {
        // Initial value is corrected by the two-way binding on the first frame.
        let cb = checkbox(commands, false);
        bind_2way(
            commands,
            cb,
            move |w| read_lock(w, entity, rotation, axis),
            move |w, v: &bool| write_lock(w, entity, rotation, axis, *v),
        );
        let txt = commands
            .spawn((
                Text::new(["X", "Y", "Z"][axis]),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        let cell = commands
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), ..default() })
            .id();
        commands.entity(cell).add_children(&[cb, txt]);
        cells.push(cell);
    }
    commands.entity(group).add_children(&cells);
    inspector_row(commands, &fonts.ui, label, group)
}

fn read_lock(w: &World, e: Entity, rotation: bool, axis: usize) -> bool {
    w.get::<PhysicsBodyData>(e)
        .map(|b| {
            if rotation {
                [b.lock_rotation_x, b.lock_rotation_y, b.lock_rotation_z][axis]
            } else {
                [b.lock_translation_x, b.lock_translation_y, b.lock_translation_z][axis]
            }
        })
        .unwrap_or(false)
}

fn write_lock(w: &mut World, e: Entity, rotation: bool, axis: usize, v: bool) {
    if let Some(mut b) = w.get_mut::<PhysicsBodyData>(e) {
        if rotation {
            match axis {
                0 => b.lock_rotation_x = v,
                1 => b.lock_rotation_y = v,
                _ => b.lock_rotation_z = v,
            }
        } else {
            match axis {
                0 => b.lock_translation_x = v,
                1 => b.lock_translation_y = v,
                _ => b.lock_translation_z = v,
            }
        }
    }
}

// ── Collision Shape ─────────────────────────────────────────────────────────

fn collision_shape_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "collision_shape",
        display_name: "Collision Shape",
        icon: regular::BOUNDING_BOX,
        category: "physics",
        has_fn: |world, entity| world.get::<CollisionShapeData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(CollisionShapeData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CollisionShapeData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

// ── Collision Shape native drawer ───────────────────────────────────────────

#[derive(Component)]
struct EditColliderBtn;
#[derive(Component)]
struct StampBtn {
    entity: Entity,
}
#[derive(Component)]
struct StripBtn {
    entity: Entity,
}

fn edit_active(w: &World) -> bool {
    w.get_resource::<ColliderEditMode>().map(|c| c.active).unwrap_or(false)
}

fn shape_of(w: &World, e: Entity) -> Option<CollisionShapeType> {
    w.get::<CollisionShapeData>(e).map(|s| s.shape_type)
}

/// Native (ember) drawer for `CollisionShapeData` — mirrors `collision_shape_ui`:
/// the Edit/Stamp/Strip buttons, the shape dropdown, the per-shape parameters
/// (shown/hidden by shape type), and offset/friction/restitution/sensor.
fn collision_shape_native(world: &mut World, entity: Entity) -> Entity {
    use renzora_ember::font::{icon_text, ui_font};
    use renzora_ember::inspector::{inspector_body, inspector_row, inspector_stripe};
    use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text};
    use renzora_ember::theme::{accent, card_bg, rgb, text_muted, text_primary};
    use renzora_ember::widgets::{drag_value, dropdown, toggle_switch, DragRange};

    let Some(s) = world.get::<CollisionShapeData>(entity) else {
        return world.spawn(Node::default()).id();
    };
    let shape_idx = match s.shape_type {
        CollisionShapeType::Box => 0usize,
        CollisionShapeType::Sphere => 1,
        CollisionShapeType::Capsule => 2,
        CollisionShapeType::Cylinder => 3,
        CollisionShapeType::Mesh => 4,
    };

    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();

        let float_row = |commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts, label: &'static str, speed: f32, min: f32, max: f32, get: fn(&CollisionShapeData) -> f32, set: fn(&mut CollisionShapeData, f32)| {
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.0, speed);
            commands.entity(dv).insert(DragRange { min, max });
            bind_2way(
                commands,
                dv,
                move |w| w.get::<CollisionShapeData>(entity).map(get).unwrap_or(0.0),
                move |w, v: &f32| {
                    if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                        set(&mut s, *v);
                    }
                },
            );
            inspector_row(commands, &fonts.ui, label, dv)
        };

        let vec3_row = |commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts, label: &'static str, get: fn(&CollisionShapeData) -> Vec3, set: fn(&mut CollisionShapeData, Vec3)| {
            const AXES: [(&str, (u8, u8, u8)); 3] = [("X", (230, 90, 90)), ("Y", (130, 200, 90)), ("Z", (90, 150, 230))];
            let group = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).id();
            let mut kids = Vec::new();
            for (i, (ax, axc)) in AXES.iter().enumerate() {
                let dv = drag_value(commands, &fonts.ui, ax, *axc, 0.0, 0.01);
                bind_2way(
                    commands,
                    dv,
                    move |w| w.get::<CollisionShapeData>(entity).map(|s| get(s)[i]).unwrap_or(0.0),
                    move |w, v: &f32| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                            let mut cur = get(&s);
                            cur[i] = *v;
                            set(&mut s, cur);
                        }
                    },
                );
                kids.push(dv);
            }
            commands.entity(group).add_children(&kids);
            inspector_row(commands, &fonts.ui, label, group)
        };

        // Edit Collider toggle button (active highlight + Edit/Editing… label).
        let edit_btn = commands
            .spawn((
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(rgb(card_bg())),
                Interaction::default(),
                EditColliderBtn,
                Name::new("edit-collider-btn"),
            ))
            .id();
        let edit_ic = icon_text(commands, &fonts.phosphor, "pencil-simple", text_primary(), 12.0);
        let edit_lbl = commands.spawn((Text::new("Edit"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
        bind_text(commands, edit_lbl, |w| if edit_active(w) { "Editing\u{2026}" } else { "Edit" }.to_string());
        commands.entity(edit_btn).add_children(&[edit_ic, edit_lbl]);
        bind_bg(commands, edit_btn, |w| if edit_active(w) { rgb(accent()) } else { rgb(card_bg()) });
        let r_edit = inspector_row(commands, &fonts.ui, "Edit Collider", edit_btn);

        // Apply to children: Stamp / Strip.
        let action_button = |commands: &mut Commands, fonts: &renzora_ember::font::EmberFonts, icon: &str, label: &str| {
            let btn = commands
                .spawn((
                    Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                    BackgroundColor(rgb(card_bg())),
                    Interaction::default(),
                ))
                .id();
            let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
            let lbl = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
            commands.entity(btn).add_children(&[ic, lbl]);
            btn
        };
        let apply_group = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() }).id();
        let stamp = action_button(commands, fonts, "tree-structure", "Stamp");
        commands.entity(stamp).insert(StampBtn { entity });
        let strip = action_button(commands, fonts, "trash", "Strip");
        commands.entity(strip).insert(StripBtn { entity });
        commands.entity(apply_group).add_children(&[stamp, strip]);
        let r_apply = inspector_row(commands, &fonts.ui, "Apply to children", apply_group);

        // Shape dropdown.
        let dd = dropdown(commands, fonts, &["Box", "Sphere", "Capsule", "Cylinder", "Mesh"], shape_idx);
        bind_2way(
            commands,
            dd,
            move |w| match shape_of(w, entity) {
                Some(CollisionShapeType::Sphere) => 1usize,
                Some(CollisionShapeType::Capsule) => 2,
                Some(CollisionShapeType::Cylinder) => 3,
                Some(CollisionShapeType::Mesh) => 4,
                _ => 0,
            },
            move |w, v: &usize| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                    s.shape_type = match v {
                        1 => CollisionShapeType::Sphere,
                        2 => CollisionShapeType::Capsule,
                        3 => CollisionShapeType::Cylinder,
                        4 => CollisionShapeType::Mesh,
                        _ => CollisionShapeType::Box,
                    };
                }
            },
        );
        let r_shape = inspector_row(commands, &fonts.ui, "Shape", dd);

        // Per-shape parameters (shown/hidden by shape type).
        let r_half_extents = vec3_row(commands, fonts, "Half Extents", |s| s.half_extents, |s, v| s.half_extents = v);
        bind_display(commands, r_half_extents, move |w| matches!(shape_of(w, entity), Some(CollisionShapeType::Box)));
        let r_radius = float_row(commands, fonts, "Radius", 0.01, 0.001, f32::MAX, |s| s.radius, |s, v| s.radius = v);
        bind_display(commands, r_radius, move |w| matches!(shape_of(w, entity), Some(CollisionShapeType::Sphere | CollisionShapeType::Capsule | CollisionShapeType::Cylinder)));
        let r_half_height = float_row(commands, fonts, "Half Height", 0.01, 0.001, f32::MAX, |s| s.half_height, |s, v| s.half_height = v);
        bind_display(commands, r_half_height, move |w| matches!(shape_of(w, entity), Some(CollisionShapeType::Capsule | CollisionShapeType::Cylinder)));
        let mesh_note = commands
            .spawn((Text::new("Uses the entity's mesh as a trimesh collider."), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
            .id();
        let r_mesh = inspector_row(commands, &fonts.ui, "", mesh_note);
        bind_display(commands, r_mesh, move |w| matches!(shape_of(w, entity), Some(CollisionShapeType::Mesh)));

        // Offset / friction / restitution / sensor.
        let r_offset = vec3_row(commands, fonts, "Offset", |s| s.offset, |s, v| s.offset = v);
        let r_friction = float_row(commands, fonts, "Friction", 0.01, 0.0, 2.0, |s| s.friction, |s, v| s.friction = v);
        let r_restitution = float_row(commands, fonts, "Restitution", 0.01, 0.0, 2.0, |s| s.restitution, |s, v| s.restitution = v);

        let sensor = toggle_switch(commands, false);
        bind_2way(
            commands,
            sensor,
            move |w| w.get::<CollisionShapeData>(entity).map(|s| s.is_sensor).unwrap_or(false),
            move |w, v: &bool| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                    s.is_sensor = *v;
                }
            },
        );
        let r_sensor = inspector_row(commands, &fonts.ui, "Is Sensor", sensor);

        let rows = [r_edit, r_apply, r_shape, r_half_extents, r_radius, r_half_height, r_mesh, r_offset, r_friction, r_restitution, r_sensor];
        for (i, r) in rows.iter().enumerate() {
            commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(col).add_children(&rows);
        col
    })
}

fn edit_collider_click(q: Query<&Interaction, (Changed<Interaction>, With<EditColliderBtn>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if let Some(mut m) = w.get_resource_mut::<ColliderEditMode>() {
                m.active = !m.active;
            }
        });
    }
}

fn stamp_strip_click(stamp: Query<(&Interaction, &StampBtn), Changed<Interaction>>, strip: Query<(&Interaction, &StripBtn), Changed<Interaction>>, mut commands: Commands) {
    for (i, b) in &stamp {
        if *i == Interaction::Pressed {
            let e = b.entity;
            commands.queue(move |w: &mut World| stamp_mesh_colliders_on_descendants(w, e));
        }
    }
    for (i, b) in &strip {
        if *i == Interaction::Pressed {
            let e = b.entity;
            commands.queue(move |w: &mut World| strip_colliders_on_descendants(w, e));
        }
    }
}

/// Auto-insert a default static body + collider on newly spawned MeshPrimitive entities.
fn auto_insert_collider_for_shape(
    trigger: On<Insert, renzora::MeshPrimitive>,
    mut commands: Commands,
    query: Query<&renzora::MeshPrimitive, Without<CollisionShapeData>>,
) {
    let entity = trigger.entity;
    if let Ok(prim) = query.get(entity) {
        if let Some(collider) = default_collider_for_shape(&prim.0) {
            commands
                .entity(entity)
                .insert((PhysicsBodyData::static_body(), collider));
        }
    }
}
