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

use std::path::PathBuf;

use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field, inspector_row, inspector_stripe};
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::{
    bind_text_input, drag_value, icon_menu_button, section_with_header_icon_open, text_input,
    toggle_switch, HoverTooltip, Section,
};
use renzora_ember::theme::{accent, border, rgb, section_bg, text_muted};
use renzora_scripting::{ScriptComponent, ScriptEngine, ScriptValue};

pub fn register(app: &mut App) {
    use renzora_editor_framework::{AppEditorExt, SplashState};
    app.register_native_inspector_ui("script_component", script_component_native);
    app.init_resource::<ScriptSectionsOpen>();
    app.add_systems(
        Update,
        (
            rebuild_scripts,
            remember_script_sections,
            script_remove_click,
            script_open_click,
            add_script_drop,
            add_script_drop_highlight,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
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

/// The script section header's leading file icon, made clickable — pressing it
/// opens the script in the code editor. Carries the entry's (project-relative)
/// path so the click handler doesn't have to re-find the script by id.
#[derive(Component)]
struct ScriptOpenBtn {
    path: PathBuf,
}

#[derive(Component)]
struct AddScriptDropZone {
    entity: Entity,
}

/// Identifies a per-script section header so its collapse state can be keyed.
#[derive(Component)]
struct ScriptSectionKey {
    entity: Entity,
    script_id: u32,
}

/// Collapse state per `(entity, script id)`, remembered across drawer rebuilds
/// — the drawer despawns and rebuilds on every structural change (add/remove
/// script, enable toggle), which would otherwise pop every section back open.
#[derive(Resource, Default)]
struct ScriptSectionsOpen(bevy::platform::collections::HashMap<(Entity, u32), bool>);

/// Mirror header clicks (ember's `section_toggle` flips [`Section`]) into
/// [`ScriptSectionsOpen`] so the next rebuild restores each section as-is.
fn remember_script_sections(
    sections: Query<(&Section, &ScriptSectionKey), Changed<Section>>,
    mut open: ResMut<ScriptSectionsOpen>,
) {
    for (sec, key) in &sections {
        open.0.insert((key.entity, key.script_id), sec.is_open());
    }
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
    /// Project-relative script file path, when the entry is file-backed —
    /// drives the header icon's click-to-open-in-code-editor.
    path: Option<PathBuf>,
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
            path: entry.script_path.clone(),
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
        // The open-in-editor button bakes the path in, so a re-pointed entry
        // must rebuild even when the filename (the displayed name) is the same.
        s.path.hash(&mut h);
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
        let available = scan_scripts(world);
        let existing: Vec<Entity> = world
            .get::<Children>(root)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        // Restore each script section's collapse state (default open).
        let open_states: Vec<bool> = specs
            .iter()
            .map(|s| {
                world
                    .get_resource::<ScriptSectionsOpen>()
                    .and_then(|m| m.0.get(&(entity, s.id)).copied())
                    .unwrap_or(true)
            })
            .collect();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            // Always show the add/drop bar, above the scripts — it's also the
            // empty state.
            let bar = build_add_bar(&mut commands, &fonts, entity, &available);
            commands.entity(root).add_child(bar);
            for (spec, open) in specs.iter().zip(&open_states) {
                let section = build_script_section(&mut commands, &fonts, entity, spec, *open);
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
    open: bool,
) -> Entity {
    // Shared ember section: caret + icon + name header that collapses on click,
    // so an entity carrying several scripts stays scannable.
    let (root, header, body, icon) = section_with_header_icon_open(
        commands,
        fonts,
        "file-code",
        &spec.name,
        accent(),
        section_bg(),
        open,
    );
    commands.entity(header).insert(ScriptSectionKey {
        entity,
        script_id: spec.id,
    });
    // The header's script icon doubles as "open in code editor" for file-backed
    // entries. FocusPolicy::Block keeps the press from also collapsing the
    // section (same as the toggle/trash affordances below).
    if let Some(path) = &spec.path {
        commands.entity(icon).insert((
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(renzora::lang::t("comp.script.open")),
            ScriptOpenBtn { path: path.clone() },
        ));
    }
    // Compact the shared section like the inspector's component cards: kill the
    // widget's 8px bottom margin + header↔body gap and tighten the paddings so
    // scripts stack flush. (Full `Node` overrides — mirror the widget's other
    // layout fields when changing them.)
    commands.entity(root).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        ..default()
    });
    commands.entity(header).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    commands.entity(body).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(4.0)),
        // Preserve the collapsed state `section_with_header_open` encoded in the
        // body's `display`; a bare `Node` would default to `Flex` and desync a
        // start-collapsed section from its `Section.open` flag.
        display: if open { Display::Flex } else { Display::None },
        ..default()
    });

    // Trailing header affordances: spacer pushes the enable toggle + per-script
    // remove to the right. Both block the press from reaching the header behind
    // them, so using them doesn't also collapse/expand the section.
    let spacer = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let toggle = toggle_switch(commands, spec.enabled);
    commands.entity(toggle).insert(bevy::ui::FocusPolicy::Block);
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
        bevy::ui::FocusPolicy::Block,
        ScriptRemoveBtn {
            entity,
            script_id: spec.id,
        },
    ));
    commands.entity(header).add_children(&[spacer, toggle, trash]);

    for (i, var) in spec.vars.iter().enumerate() {
        let row = build_var_row(commands, fonts, entity, spec.id, var);
        commands
            .entity(row)
            .insert(BackgroundColor(inspector_stripe(i)));
        commands.entity(body).add_child(row);
    }

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
            renzora_ember::inspector::fill_control(commands, dv);
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
            renzora_ember::inspector::fill_control(commands, dv);
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
            renzora_ember::inspector::fill_control(commands, ti);
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
        renzora_ember::inspector::fill_control(commands, dv);
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

/// Header-icon click → open the script in the code editor. Mirrors the asset
/// browser's routing: load the file straight into `CodeEditorState` via the
/// contract `OpenCodeEditorFile` resource, then reveal the Code Editor panel in
/// whichever dock backend is live.
fn script_open_click(
    q: Query<(&Interaction, &ScriptOpenBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let path = btn.path.clone();
        cmds.push(move |w: &mut World| {
            // Stored paths are project-relative (see `make_rel`); the code
            // editor reads from disk, so resolve against the open project.
            let abs = if path.is_absolute() {
                path.clone()
            } else {
                match w.get_resource::<renzora::core::CurrentProject>() {
                    Some(p) => p.path.join(&path),
                    None => path.clone(),
                }
            };
            w.insert_resource(renzora::core::OpenCodeEditorFile { path: abs });
            if let Some(mut docking) =
                w.get_resource_mut::<renzora_editor_framework::DockingState>()
            {
                docking.tree.focus_or_add_panel("code_editor");
            }
            if let Some(mut dock) = w.get_resource_mut::<renzora_ember::dock::Dock>() {
                dock.tree.focus_or_add_panel("code_editor");
            }
            if let Some(mut dirty) = w.get_resource_mut::<renzora_ember::dock::DockDirty>() {
                dirty.0 = true;
            }
        });
    }
}

// ── Add-script bar (drop target + floating "+" menu) ─────────────────────────

/// Always-shown bar at the top of the drawer: the "Drop to add script" target
/// with a compact "+" icon button floating over its right edge that opens a
/// scrolling list of the project's scripts. It doubles as the empty state when
/// no scripts are attached.
fn build_add_bar(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    available: &[(String, PathBuf)],
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                // Anchor for the absolutely-positioned "+" button.
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("script-add-bar"),
        ))
        .id();

    // Drop-to-add target (full width; the floated button overlays its right edge).
    let drop_txt = commands
        .spawn((
            Text::new(renzora::lang::t("comp.script.drop")),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let drop = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.02)),
            BorderColor::all(rgb(border())),
            bevy::ui::RelativeCursorPosition::default(),
            AddScriptDropZone { entity },
            Name::new("script-drop-zone"),
        ))
        .id();
    commands.entity(drop).add_child(drop_txt);

    // "+" icon button opening a floating menu of the project's scripts. An
    // empty project shows one inert row — its index misses `paths` below, so
    // picking it no-ops.
    let none_msg = renzora::lang::t("comp.script.none_in_project");
    let names: Vec<&str> = if available.is_empty() {
        vec![none_msg.as_str()]
    } else {
        available.iter().map(|(d, _)| d.as_str()).collect()
    };
    let paths: Vec<PathBuf> = available.iter().map(|(_, p)| p.clone()).collect();
    let dd = icon_menu_button(commands, fonts, "plus", "file-code", &names, move |w, i| {
        let Some(abs) = paths.get(i) else { return };
        let rel = w
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| make_rel(abs, p))
            .unwrap_or_else(|| abs.clone());
        if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
            sc.add_file_script(rel);
        }
    });
    commands
        .entity(dd)
        .insert(HoverTooltip::new(renzora::lang::t("comp.script.add")));
    // Float the button over the drop zone's right edge, vertically centered.
    let float = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(4.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("script-add-float"),
        ))
        .id();
    commands.entity(float).add_child(dd);

    commands.entity(root).add_children(&[drop, float]);
    root
}

/// Recursively scan the project for `.lua`/`.rhai` files, returning
/// `(display-relative-path, absolute-path)` pairs sorted by display path.
fn scan_scripts(world: &World) -> Vec<(String, PathBuf)> {
    let Some(project) = world.get_resource::<renzora::core::CurrentProject>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    scan_scripts_inner(&project.path, &project.path, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn scan_scripts_inner(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "target" | "node_modules" | ".git") {
                continue;
            }
            scan_scripts_inner(&path, root, out);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "lua" | "rhai") {
                let display = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                out.push((display, path));
            }
        }
    }
}

/// Strip the project prefix and force forward slashes so the stored path is
/// OS-independent (matches the egui `make_relative`).
fn make_rel(abs: &std::path::Path, project: &renzora::core::CurrentProject) -> PathBuf {
    let rel = abs.strip_prefix(&project.path).unwrap_or(abs);
    PathBuf::from(rel.to_string_lossy().replace('\\', "/"))
}

fn add_script_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&bevy::ui::RelativeCursorPosition, &AddScriptDropZone)>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(cmds)) = (payload, cmds) else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(&["lua", "rhai", "blueprint", "bp"]) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let rel = project
            .as_ref()
            .map(|p| make_rel(&payload.path, p))
            .unwrap_or_else(|| payload.path.clone());
        let entity = zone.entity;
        cmds.push(move |w: &mut World| {
            if let Some(mut sc) = w.get_mut::<ScriptComponent>(entity) {
                sc.add_file_script(rel.clone());
            }
        });
        break;
    }
}

/// Highlight the drop target's border while a compatible script is dragged over.
fn add_script_drop_highlight(
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    mut zones: Query<(&bevy::ui::RelativeCursorPosition, &mut BorderColor), With<AddScriptDropZone>>,
) {
    for (rcp, mut bc) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| {
            p.is_detached
                && rcp.cursor_over
                && p.matches_extensions(&["lua", "rhai", "blueprint", "bp"])
        });
        let want = BorderColor::all(if active {
            Color::srgb_u8(120, 140, 200)
        } else {
            rgb(border())
        });
        if *bc != want {
            *bc = want;
        }
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
