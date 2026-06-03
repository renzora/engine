//! Bevy-native (ember) inspector drawer for `ScriptComponent`.
//!
//! Lives here (not in `renzora_scripting`) because scripting can't depend on
//! ember (an `ember → hui → scripting` cycle), and this crate already depends on
//! both. The script inspector is dynamic — the attached-scripts list and each
//! script's exposed variables change at runtime — so it owns a `ScriptsRoot`
//! container and a `rebuild_scripts` system that re-fills it whenever a
//! structural signature (script ids/paths/enabled + variable names/types) changes
//! (the inspector's own rebuild only fires on the component *set* changing).
//!
//! v1: per-script header (name + enabled toggle + remove) and an editor per
//! variable for all 8 `ScriptValue` types. Tabs, the Add-Script search, and the
//! script-path asset-drop are follow-ups.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor::EditorCommands;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field, inspector_row, inspector_stripe};
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::{bind_text_input, drag_value, text_input, toggle_switch};
use renzora_scripting::{ScriptComponent, ScriptEngine, ScriptValue};

pub fn register(app: &mut App) {
    use renzora_editor::{AppEditorExt, SplashState};
    app.register_native_inspector_ui("script_component", script_component_native);
    app.add_systems(
        Update,
        (rebuild_scripts, script_remove_click).run_if(in_state(SplashState::Editor)),
    );
}

#[derive(Component)]
struct ScriptsRoot {
    entity: Entity,
    sig: Option<u64>,
}

#[derive(Component)]
struct ScriptRemoveBtn {
    entity: Entity,
    script_id: u32,
}

fn script_component_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            ScriptsRoot { entity, sig: None },
            Name::new("scripts-root"),
        ))
        .id()
}

// ── Specs (collected under the exclusive borrow) ─────────────────────────────

struct VarSpec {
    name: String,
    display: String,
    value: ScriptValue,
}

struct ScriptSpec {
    id: u32,
    name: String,
    enabled: bool,
    vars: Vec<VarSpec>,
}

fn collect_script_specs(world: &World, entity: Entity) -> Vec<ScriptSpec> {
    let Some(sc) = world.get::<ScriptComponent>(entity) else {
        return Vec::new();
    };
    let engine = world.get_resource::<ScriptEngine>();
    let mut out = Vec::new();
    for entry in &sc.scripts {
        let name = if let Some(p) = &entry.script_path {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else if !entry.script_id.is_empty() {
            entry.script_id.clone()
        } else {
            format!("Script #{}", entry.id)
        };

        let defs = entry
            .script_path
            .as_ref()
            .and_then(|p| engine.map(|e| e.get_script_props(p)))
            .unwrap_or_default();
        let mut vars = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for def in &defs {
            let value = entry
                .variables
                .get(&def.name)
                .cloned()
                .unwrap_or_else(|| def.default_value.clone());
            vars.push(VarSpec {
                name: def.name.clone(),
                display: def.display_name.clone(),
                value,
            });
            seen.insert(def.name.clone());
        }
        for (k, v) in entry.variables.iter_all() {
            if !seen.contains(k) {
                vars.push(VarSpec {
                    name: k.clone(),
                    display: k.clone(),
                    value: v.clone(),
                });
            }
        }
        out.push(ScriptSpec {
            id: entry.id,
            name,
            enabled: entry.enabled,
            vars,
        });
    }
    out
}

fn scripts_sig(specs: &[ScriptSpec], root: Entity) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.to_bits().hash(&mut h);
    for s in specs {
        s.id.hash(&mut h);
        s.name.hash(&mut h);
        s.enabled.hash(&mut h);
        for v in &s.vars {
            v.name.hash(&mut h);
            v.value.type_name().hash(&mut h);
        }
    }
    h.finish()
}

// ── Rebuild ──────────────────────────────────────────────────────────────────

fn rebuild_scripts(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut q = world.query::<(Entity, &ScriptsRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> =
        q.iter(world).map(|(re, sr)| (re, sr.entity, sr.sig)).collect();

    for (root, entity, old_sig) in roots {
        let specs = collect_script_specs(world, entity);
        let sig = scripts_sig(&specs, root);
        if old_sig == Some(sig) {
            continue;
        }
        let existing: Vec<Entity> = world
            .get::<Children>(root)
            .map(|c| c.iter().collect())
            .unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            if specs.is_empty() {
                let l = muted_label(&mut commands, &fonts, "No scripts attached.");
                commands.entity(root).add_child(l);
            }
            for spec in &specs {
                let section = build_script_section(&mut commands, &fonts, entity, spec);
                commands.entity(root).add_child(section);
            }
        }
        queue.apply(world);
        if let Some(mut sr) = world.get_mut::<ScriptsRoot>(root) {
            sr.sig = Some(sig);
        }
    }
}

fn build_script_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    spec: &ScriptSpec,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            Name::new("script-section"),
        ))
        .id();

    let title = commands
        .spawn((
            Text::new(spec.name.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgb_u8(220, 220, 230)),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let toggle = toggle_switch(commands, spec.enabled);
    let id = spec.id;
    bind_2way(
        commands,
        toggle,
        move |w| script_enabled(w, entity, id),
        move |w, v: &bool| set_script_enabled(w, entity, id, *v),
    );
    let trash = icon_text(commands, &fonts.phosphor, "trash", (150, 150, 162), 13.0);
    commands.entity(trash).insert((
        Interaction::default(),
        ScriptRemoveBtn {
            entity,
            script_id: spec.id,
        },
    ));
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(44, 44, 54)),
            Name::new("script-header"),
        ))
        .id();
    commands
        .entity(header)
        .add_children(&[title, spacer, toggle, trash]);

    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("script-vars"),
        ))
        .id();
    for (i, var) in spec.vars.iter().enumerate() {
        let row = build_var_row(commands, fonts, entity, spec.id, var);
        commands
            .entity(row)
            .insert(BackgroundColor(inspector_stripe(i)));
        commands.entity(body).add_child(row);
    }

    commands.entity(root).add_children(&[header, body]);
    root
}

fn build_var_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    id: u32,
    var: &VarSpec,
) -> Entity {
    let n = var.name.clone();
    let control = match &var.value {
        ScriptValue::Float(init) => {
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), *init, 0.1);
            let (gn, sn) = (n.clone(), n.clone());
            bind_2way(
                commands,
                dv,
                move |w| match get_var(w, entity, id, &gn) {
                    Some(ScriptValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| set_var(w, entity, id, &sn, ScriptValue::Float(*v)),
            );
            dv
        }
        ScriptValue::Int(init) => {
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), *init as f32, 1.0);
            let (gn, sn) = (n.clone(), n.clone());
            bind_2way(
                commands,
                dv,
                move |w| match get_var(w, entity, id, &gn) {
                    Some(ScriptValue::Int(v)) => v as f32,
                    _ => 0.0,
                },
                move |w, v: &f32| set_var(w, entity, id, &sn, ScriptValue::Int(*v as i32)),
            );
            dv
        }
        ScriptValue::Bool(init) => {
            let sw = toggle_switch(commands, *init);
            let (gn, sn) = (n.clone(), n.clone());
            bind_2way(
                commands,
                sw,
                move |w| matches!(get_var(w, entity, id, &gn), Some(ScriptValue::Bool(true))),
                move |w, v: &bool| set_var(w, entity, id, &sn, ScriptValue::Bool(*v)),
            );
            sw
        }
        ScriptValue::String(init) | ScriptValue::Entity(init) => {
            let is_entity = matches!(var.value, ScriptValue::Entity(_));
            let ti = text_input(commands, &fonts.ui, "—", init);
            let (gn, sn) = (n.clone(), n.clone());
            bind_text_input(
                commands,
                ti,
                move |w| match get_var(w, entity, id, &gn) {
                    Some(ScriptValue::String(s)) | Some(ScriptValue::Entity(s)) => s,
                    _ => String::new(),
                },
                move |w, s: String| {
                    let val = if is_entity {
                        ScriptValue::Entity(s)
                    } else {
                        ScriptValue::String(s)
                    };
                    set_var(w, entity, id, &sn, val);
                },
            );
            ti
        }
        ScriptValue::Vec2(init) => {
            vec_axes(commands, fonts, entity, id, &n, 2, [init.x, init.y, 0.0])
        }
        ScriptValue::Vec3(init) => {
            vec_axes(commands, fonts, entity, id, &n, 3, [init.x, init.y, init.z])
        }
        ScriptValue::Color(init) => {
            let gn = n.clone();
            let sn = n.clone();
            let init = *init;
            color_field(
                commands,
                move |w| match get_var(w, entity, id, &gn) {
                    Some(ScriptValue::Color(c)) => [c.x, c.y, c.z],
                    _ => [init.x, init.y, init.z],
                },
                move |w, rgb: [f32; 3]| {
                    let alpha = match get_var(w, entity, id, &sn) {
                        Some(ScriptValue::Color(c)) => c.w,
                        _ => 1.0,
                    };
                    set_var(
                        w,
                        entity,
                        id,
                        &sn,
                        ScriptValue::Color(Vec4::new(rgb[0], rgb[1], rgb[2], alpha)),
                    );
                },
            )
        }
    };
    inspector_row(commands, &fonts.ui, &var.display, control)
}

fn vec_axes(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    id: u32,
    name: &str,
    count: usize,
    init: [f32; 3],
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
                min_width: Val::Px(0.0),
                ..default()
            },
            Name::new("var-vec"),
        ))
        .id();
    let mut kids = Vec::with_capacity(count);
    for i in 0..count {
        let dv = drag_value(commands, &fonts.ui, AXES[i].0, AXES[i].1, init[i], 0.05);
        let (gn, sn) = (name.to_string(), name.to_string());
        bind_2way(
            commands,
            dv,
            move |w| read_vec_axis(w, entity, id, &gn, i),
            move |w, v: &f32| write_vec_axis(w, entity, id, &sn, i, *v),
        );
        kids.push(dv);
    }
    commands.entity(row).add_children(&kids);
    row
}

fn script_remove_click(
    q: Query<(&Interaction, &ScriptRemoveBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, id) = (btn.entity, btn.script_id);
        cmds.push(move |w: &mut World| {
            if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                if let Some(idx) = sc.scripts.iter().position(|s| s.id == id) {
                    sc.remove_script(idx);
                }
            }
        });
    }
}

// ── Variable get/set helpers ─────────────────────────────────────────────────

fn get_var(w: &World, entity: Entity, id: u32, name: &str) -> Option<ScriptValue> {
    w.get::<ScriptComponent>(entity)?
        .scripts
        .iter()
        .find(|s| s.id == id)?
        .variables
        .get(name)
        .cloned()
}

fn set_var(w: &mut World, entity: Entity, id: u32, name: &str, val: ScriptValue) {
    if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
        if let Some(s) = sc.scripts.iter_mut().find(|s| s.id == id) {
            s.variables.set(name.to_string(), val);
        }
    }
}

fn read_vec_axis(w: &World, entity: Entity, id: u32, name: &str, axis: usize) -> f32 {
    match get_var(w, entity, id, name) {
        Some(ScriptValue::Vec2(v)) => [v.x, v.y, 0.0][axis],
        Some(ScriptValue::Vec3(v)) => [v.x, v.y, v.z][axis],
        _ => 0.0,
    }
}

fn write_vec_axis(w: &mut World, entity: Entity, id: u32, name: &str, axis: usize, val: f32) {
    let new = match get_var(w, entity, id, name) {
        Some(ScriptValue::Vec2(mut v)) => {
            match axis {
                0 => v.x = val,
                _ => v.y = val,
            }
            ScriptValue::Vec2(v)
        }
        Some(ScriptValue::Vec3(mut v)) => {
            match axis {
                0 => v.x = val,
                1 => v.y = val,
                _ => v.z = val,
            }
            ScriptValue::Vec3(v)
        }
        _ => return,
    };
    set_var(w, entity, id, name, new);
}

fn script_enabled(w: &World, entity: Entity, id: u32) -> bool {
    w.get::<ScriptComponent>(entity)
        .and_then(|sc| sc.scripts.iter().find(|s| s.id == id))
        .map(|s| s.enabled)
        .unwrap_or(false)
}

fn set_script_enabled(w: &mut World, entity: Entity, id: u32, val: bool) {
    if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
        if let Some(s) = sc.scripts.iter_mut().find(|s| s.id == id) {
            s.enabled = val;
        }
    }
}

fn muted_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgb_u8(150, 150, 162)),
            Node {
                margin: UiRect::all(Val::Px(6.0)),
                ..default()
            },
        ))
        .id()
}
