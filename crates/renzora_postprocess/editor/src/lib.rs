//! Editor-only half of `renzora_postprocess`: the **Post Process** inspector.
//!
//! Drop a shader effect from the Assets panel onto it and the effect is added
//! to the end of the list. Each effect is a section with its sliders, read from
//! the shader's own annotations, plus move up, move down, an on switch and
//! remove.
//!
//! The list changes shape at runtime (effects added, removed, reordered, and a
//! shader's fields changing when the file is edited), which the inspector's own
//! rebuild does not see: it only rebuilds when the set of components changes.
//! So the drawer owns a container and refills it whenever a signature of the
//! list's structure changes, the way the Scripts drawer does. Slider values are
//! not in the signature; they are bound both ways and update in place.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use bevy::ecs::world::CommandQueue;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora::{AppEditorExt, EditorCommands, InspectorEntry, PostEffect, PostEffects, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{fill_control, inspector_row, inspector_stripe};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::theme::{accent, border, rgb, section_bg, text_muted};
use renzora_ember::widgets::{
    drag_value, section_with_header_icon_open, toggle_switch, DragRange, DragSnap, HoverTooltip,
};
use renzora_ember::UiInteraction;
use renzora_postprocess::library::EffectStatus;
use renzora_postprocess::{effect_file, EffectLibrary, FieldKind, FieldRole};

const TYPE_ID: &str = "post_effects";

/// Shader files the drop zone considers. Whether one is an effect is then
/// decided by its `@effect` header.
const EXTENSIONS: [&str; 2] = ["wesl", "wgsl"];

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: TYPE_ID,
        display_name: "Post Process",
        icon: "magic-wand",
        category: "post_process",
        has_fn: |world, entity| world.get::<PostEffects>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PostEffects::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PostEffects>();
        }),
        // Each effect has its own switch; there is nothing for a whole-list
        // switch to mean that removing the component does not.
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

#[derive(Component)]
struct EffectsRoot {
    entity: Entity,
    sig: Option<u64>,
}

#[derive(Component)]
struct DropZone {
    entity: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Up,
    Down,
    Remove,
}

#[derive(Component)]
struct EffectButton {
    entity: Entity,
    index: usize,
    action: Action,
}

fn drawer(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            EffectsRoot { entity, sig: None },
            Name::new("post-effects-root"),
        ))
        .id()
}

/// Everything about the list that changes the drawer's layout.
fn signature(world: &World, entity: Entity, root: Entity) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut h);
    if let Some(list) = world.get::<PostEffects>(entity) {
        for effect in &list.effects {
            effect.shader.hash(&mut h);
            effect.enabled.hash(&mut h);
        }
        list.effects.len().hash(&mut h);
    }
    if let Some(library) = world.get_resource::<EffectLibrary>() {
        library.signature().hash(&mut h);
    }
    h.finish()
}

fn rebuild(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut q = world.query::<(Entity, &EffectsRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> =
        q.iter(world).map(|(r, er)| (r, er.entity, er.sig)).collect();

    for (root, entity, old) in roots {
        let sig = signature(world, entity, root);
        if old == Some(sig) {
            continue;
        }
        let effects: Vec<PostEffect> = world
            .get::<PostEffects>(entity)
            .map(|l| l.effects.clone())
            .unwrap_or_default();
        let statuses: Vec<Option<EffectStatus>> = {
            let library = world.get_resource::<EffectLibrary>();
            effects
                .iter()
                .map(|e| library.and_then(|l| l.status(&e.shader)).cloned())
                .collect()
        };
        let existing: Vec<Entity> = world
            .get::<Children>(root)
            .map(|c| c.iter().collect())
            .unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for child in existing {
                commands.entity(child).despawn();
            }
            let zone = drop_zone(&mut commands, &fonts, entity);
            commands.entity(root).add_child(zone);
            let count = effects.len();
            for (index, (effect, status)) in effects.iter().zip(&statuses).enumerate() {
                let section = effect_section(
                    &mut commands,
                    &fonts,
                    entity,
                    index,
                    count,
                    effect,
                    status.as_ref(),
                );
                commands.entity(root).add_child(section);
            }
        }
        queue.apply(world);
        if let Some(mut er) = world.get_mut::<EffectsRoot>(root) {
            er.sig = Some(sig);
        }
    }
}

fn drop_zone(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let text = commands
        .spawn((
            Text::new(renzora::lang::t("comp.post_effects.drop")),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            PASS_THROUGH,
        ))
        .id();
    let zone = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(10.0)),
                margin: UiRect::bottom(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.02)),
            BorderColor::all(rgb(border())),
            bevy::ui::RelativeCursorPosition::default(),
            DropZone { entity },
            Name::new("post-effects-drop"),
        ))
        .id();
    commands.entity(zone).add_child(text);
    zone
}

fn effect_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    index: usize,
    count: usize,
    effect: &PostEffect,
    status: Option<&EffectStatus>,
) -> Entity {
    let (title, icon) = match status {
        Some(EffectStatus::Ready(info)) => (info.name.clone(), info.icon.clone()),
        _ => (file_title(&effect.shader), "sparkle".to_string()),
    };
    let (root, header, body, _) = section_with_header_icon_open(
        commands,
        fonts,
        &icon,
        &title,
        accent(),
        section_bg(),
        true,
    );
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
        ..default()
    });

    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH))
        .id();
    // A button that cannot act (moving the first effect up) is drawn dimmed and
    // carries no action, rather than vanishing and shifting the others.
    let button = |commands: &mut Commands, glyph: &str, tip: &str, action: Action, live: bool| {
        let colour = if live { (150, 150, 162) } else { (80, 80, 90) };
        let b = icon_text(commands, &fonts.phosphor, glyph, colour, 13.0);
        commands.entity(b).insert((
            UiInteraction::default(),
            BLOCK,
            HoverTooltip::new(renzora::lang::t(tip)),
        ));
        if live {
            commands.entity(b).insert(EffectButton { entity, index, action });
        }
        b
    };
    let up = button(commands, "caret-up", "comp.post_effects.move_up", Action::Up, index > 0);
    let down = button(
        commands,
        "caret-down",
        "comp.post_effects.move_down",
        Action::Down,
        index + 1 < count,
    );
    let mut buttons = vec![spacer, up, down];
    let toggle = toggle_switch(commands, effect.enabled);
    commands.entity(toggle).insert(BLOCK);
    bind_2way(
        commands,
        toggle,
        move |w| {
            w.get::<PostEffects>(entity)
                .and_then(|l| l.effects.get(index))
                .is_some_and(|e| e.enabled)
        },
        move |w, on: &bool| {
            if let Some(mut list) = w.get_mut::<PostEffects>(entity) {
                if let Some(e) = list.effects.get_mut(index) {
                    e.enabled = *on;
                }
            }
        },
    );
    buttons.push(toggle);
    buttons.push(button(commands, "trash", "comp.post_effects.remove", Action::Remove, true));
    commands.entity(header).add_children(&buttons);

    let rows: Vec<Entity> = match status {
        Some(EffectStatus::Ready(info)) => {
            let rows: Vec<Entity> = info
                .sliders()
                .map(|field| {
                    let FieldRole::Slider { min, max, step } = field.role else {
                        unreachable!("sliders() yields only sliders");
                    };
                    let init = effect.value(&field.name).unwrap_or(field.default);
                    let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), init, step);
                    // Ranges are open when the shader gives none; a bounded
                    // drag fills the field to show where the value sits.
                    if min > f32::MIN && max < f32::MAX {
                        commands.entity(dv).insert(DragRange { min, max });
                    }
                    if field.kind != FieldKind::F32 {
                        commands.entity(dv).insert(DragSnap(1.0));
                    }
                    let (get_name, set_name, default) =
                        (field.name.clone(), field.name.clone(), field.default);
                    bind_2way(
                        commands,
                        dv,
                        move |w| {
                            w.get::<PostEffects>(entity)
                                .and_then(|l| l.effects.get(index))
                                .and_then(|e| e.value(&get_name))
                                .unwrap_or(default)
                        },
                        move |w, v: &f32| {
                            if let Some(mut list) = w.get_mut::<PostEffects>(entity) {
                                if let Some(e) = list.effects.get_mut(index) {
                                    e.set_value(&set_name, *v);
                                }
                            }
                        },
                    );
                    fill_control(commands, dv);
                    inspector_row(commands, &fonts.ui, &field.label, dv)
                })
                .collect();
            if rows.is_empty() {
                vec![message_row(commands, fonts, &renzora::lang::t("comp.post_effects.no_settings"), false)]
            } else {
                rows
            }
        }
        Some(EffectStatus::Failed(error)) => vec![message_row(commands, fonts, error, true)],
        _ => vec![message_row(
            commands,
            fonts,
            &renzora::lang::t("comp.post_effects.loading"),
            false,
        )],
    };
    for (i, row) in rows.iter().enumerate() {
        commands.entity(*row).insert(BackgroundColor(inspector_stripe(i)));
    }
    commands.entity(body).add_children(&rows);
    root
}

fn message_row(commands: &mut Commands, fonts: &EmberFonts, text: &str, error: bool) -> Entity {
    let colour = if error {
        Color::srgb_u8(230, 110, 100)
    } else {
        rgb(text_muted())
    };
    let label = commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(colour)))
        .id();
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    commands.entity(row).add_child(label);
    row
}

/// `shaders/CRT/crt.wesl` -> `Crt`, for an effect whose file has not parsed.
fn file_title(path: &str) -> String {
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    effect_file::title_case(stem)
}

fn button_clicks(
    q: Query<(&UiInteraction, &EffectButton), Changed<UiInteraction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, button) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let (entity, index, action) = (button.entity, button.index, button.action);
        cmds.push(move |w: &mut World| {
            let Some(mut list) = w.get_mut::<PostEffects>(entity) else {
                return;
            };
            let len = list.effects.len();
            match action {
                Action::Up if index > 0 && index < len => list.effects.swap(index - 1, index),
                Action::Down if index + 1 < len => list.effects.swap(index, index + 1),
                Action::Remove if index < len => {
                    list.effects.remove(index);
                }
                _ => {}
            }
        });
    }
}

/// Whether a dragged file is an effect, read once per file per drag. Reading
/// it on every hovered frame would be a disk read per frame.
#[derive(Default)]
struct EffectFileCache(HashMap<PathBuf, bool>);

impl EffectFileCache {
    fn is_effect(&mut self, path: &PathBuf) -> bool {
        *self.0.entry(path.clone()).or_insert_with(|| {
            std::fs::read_to_string(path).is_ok_and(|s| effect_file::is_effect(&s))
        })
    }
}

fn accepts(payload: &renzora_ui::AssetDragPayload, cache: &mut EffectFileCache) -> bool {
    payload.is_detached && payload.matches_extensions(&EXTENSIONS) && cache.is_effect(&payload.path)
}

fn drop_effect(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&bevy::ui::RelativeCursorPosition, &DropZone)>,
    cmds: Option<Res<EditorCommands>>,
    mut cache: Local<EffectFileCache>,
) {
    // The drag ended without landing here: forget what was read, so an effect
    // file edited before the next drag is read again.
    if payload.is_none() {
        cache.0.clear();
        return;
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(cmds)) = (payload, cmds) else {
        return;
    };
    if !accepts(&payload, &mut cache) {
        return;
    }
    let Some((_, zone)) = zones.iter().find(|(rcp, _)| rcp.cursor_over) else {
        return;
    };
    let path = match &project {
        Some(p) => p.make_asset_relative(&payload.path),
        None => payload.path.to_string_lossy().replace('\\', "/"),
    };
    let entity = zone.entity;
    cmds.push(move |w: &mut World| {
        if let Some(mut list) = w.get_mut::<PostEffects>(entity) {
            list.push(path.clone());
        }
    });
}

/// Light the drop zone's border while an effect file is dragged over it. Uses
/// the same test as [`drop_effect`], so it never lights for a drop it refuses.
fn drop_highlight(
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    mut zones: Query<(&bevy::ui::RelativeCursorPosition, &mut BorderColor), With<DropZone>>,
    mut cache: Local<EffectFileCache>,
) {
    if payload.is_none() {
        cache.0.clear();
    }
    for (rcp, mut colour) in &mut zones {
        let active = payload
            .as_ref()
            .is_some_and(|p| rcp.cursor_over && accepts(p, &mut cache));
        let want = BorderColor::all(if active {
            Color::srgb_u8(120, 140, 200)
        } else {
            rgb(border())
        });
        if *colour != want {
            *colour = want;
        }
    }
}

#[derive(Default)]
pub struct PostProcessEditorPlugin;

impl Plugin for PostProcessEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] PostProcessEditorPlugin");
        app.register_inspector(inspector_entry());
        app.register_native_inspector_ui(TYPE_ID, drawer);
        app.add_systems(
            Update,
            (rebuild, button_clicks, drop_effect, drop_highlight)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("inspector")),
        );
    }
}

renzora::add!(PostProcessEditorPlugin, Editor);
