//! Native (ember) inspector drawer for [`AnimatorComponent`] — the "animation
//! player" section in the Properties panel.
//!
//! The egui editor had a full custom inspector here (clip library with
//! drag-drop, default-clip selector, blend slider) that was reduced to a bare
//! blend-duration field during the egui purge. This restores it natively:
//!
//! - clip list: play button (plays the clip on the timeline preview), rename,
//!   speed, loop, remove,
//! - drop/browse `.anim` files to add clips,
//! - default clip dropdown,
//! - blend duration,
//! - `.animsm` state machine assignment.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora::{AppEditorExt, FieldValue};
use renzora_animation::{AnimClipSlot, AnimatorComponent};
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_row;
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_text_color};
use renzora_ember::theme::{accent, border, faint_bg, rgb, text_muted};
use renzora_ember::widgets::{bind_text_input, drag_value, dropdown, text_input, DragRange};
use renzora_ember::UiInteraction;
use renzora_inspector::asset_drop_field;

use crate::{AnimEditorAction, AnimEditorBridge};

pub fn register(app: &mut App) {
    app.register_native_inspector_ui("animator", animator_native);
    app.add_systems(
        Update,
        (
            rebuild_animator,
            play_clip_click,
            remove_clip_click,
            loop_clip_click,
            add_clip_drop,
            add_clip_drop_highlight,
        )
            .run_if(in_state(renzora::SplashState::Editor)),
    );
}

/// The "drop .anim files here" target under the clip list.
#[derive(Component)]
struct AddClipDropZone {
    entity: Entity,
}

#[derive(Component)]
struct AnimatorRoot {
    entity: Entity,
    sig: Option<u64>,
}
#[derive(Component)]
struct PlayClipBtn {
    entity: Entity,
    index: usize,
}
#[derive(Component)]
struct RemoveClipBtn {
    entity: Entity,
    index: usize,
}
#[derive(Component)]
struct LoopClipBtn {
    entity: Entity,
    index: usize,
}

fn animator_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            AnimatorRoot { entity, sig: None },
            Name::new("animator-inspector-root"),
        ))
        .id()
}

/// Structural signature — rebuild rows when the clip set, default clip, or
/// state-machine assignment changes. Speed/looping edit in place via
/// `bind_2way`, so they're excluded.
fn animator_sig(a: &AnimatorComponent) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.clips.len().hash(&mut h);
    for c in &a.clips {
        c.name.hash(&mut h);
        c.path.hash(&mut h);
    }
    a.default_clip.hash(&mut h);
    a.state_machine.hash(&mut h);
    h.finish()
}

fn rebuild_animator(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut q = world.query::<(Entity, &AnimatorRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> =
        q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let Some(data) = world.get::<AnimatorComponent>(entity).cloned() else { continue };
        let sig = animator_sig(&data);
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
            build_body(&mut commands, &fonts, root, entity, &data);
        }
        queue.apply(world);
        if let Some(mut ar) = world.get_mut::<AnimatorRoot>(root) {
            ar.sig = Some(sig);
        }
    }
}

/// A group label inside the card ("Clips", "Playback", "State Machine"):
/// muted, so it reads as a heading over the rows rather than as another row.
fn header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let h = commands
        .spawn(Node {
            margin: UiRect { top: Val::Px(8.0), bottom: Val::Px(3.0), ..default() },
            padding: UiRect::left(Val::Px(4.0)),
            ..default()
        })
        .id();
    // 12 lands on the 11px rung, a step under the rows' 13px.
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(h).add_child(t);
    h
}

fn build_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    root: Entity,
    entity: Entity,
    data: &AnimatorComponent,
) {
    let mut children: Vec<Entity> = Vec::new();

    // ── Clips ──
    children.push(header(commands, fonts, &renzora::lang::t("animation.animation_clips")));
    for (i, slot) in data.clips.iter().enumerate() {
        let block = clip_block(commands, fonts, entity, i, slot);
        children.push(block);
    }
    children.push(build_clip_drop_zone(commands, fonts, entity));

    // ── Playback ──
    children.push(header(commands, fonts, &renzora::lang::t("animation.playback")));

    // Default clip dropdown: "(none)" + clip names.
    let mut labels: Vec<String> = vec!["(none)".into()];
    labels.extend(data.clips.iter().map(|c| c.name.clone()));
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let sel = data
        .default_clip
        .as_deref()
        .and_then(|d| data.clips.iter().position(|c| c.name == d))
        .map(|i| i + 1)
        .unwrap_or(0);
    let dd = dropdown(commands, fonts, &label_refs, sel);
    let names_a: Vec<String> = data.clips.iter().map(|c| c.name.clone()).collect();
    let names_b = names_a.clone();
    bind_2way(
        commands,
        dd,
        move |w| {
            let cur = w.get::<AnimatorComponent>(entity).and_then(|a| a.default_clip.clone());
            cur.as_deref()
                .and_then(|d| names_a.iter().position(|n| n == d))
                .map(|i| i + 1)
                .unwrap_or(0)
        },
        move |w, i: &usize| {
            let name = if *i == 0 { None } else { names_b.get(*i - 1).cloned() };
            // Only the setting. It used to play the clip as well, like the egui
            // inspector, which set the model running in the viewport with the
            // timeline stopped; the default is what a game starts on, not a
            // preview control.
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(entity) {
                a.default_clip = name;
            }
        },
    );
    let r = inspector_row(commands, &fonts.ui, &renzora::lang::t("animation.default_clip"), dd);
    children.push(r);

    // Blend duration.
    let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.2, 0.01);
    commands.entity(dv).insert(DragRange { min: 0.0, max: 5.0 });
    bind_2way(
        commands,
        dv,
        move |w| w.get::<AnimatorComponent>(entity).map(|a| a.blend_duration).unwrap_or(0.2),
        move |w, v: &f32| {
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(entity) {
                a.blend_duration = *v;
            }
        },
    );
    let r = inspector_row(commands, &fonts.ui, &renzora::lang::t("animation.blend_time"), dv);
    children.push(r);

    // ── State Machine ──
    children.push(header(commands, fonts, &renzora::lang::t("animation.state_machine")));
    let sm = asset_drop_field(
        commands,
        fonts,
        entity,
        animator_sm_get,
        animator_sm_set,
        vec!["animsm".to_string()],
    );
    let r = inspector_row(commands, &fonts.ui, &renzora::lang::t("animation.file"), sm);
    children.push(r);

    commands.entity(root).add_children(&children);
}

/// One clip slot, as a small card: `[▶] name  [Speed 1] [loop] [trash]` over
/// the clip's file path.
///
/// It used to be two loose rows: the name, then the path with an unlabelled
/// number box and a checkbox squeezed after it. The number was the playback
/// speed, which nothing said, and a long path pushed both controls off the
/// edge. Now the controls sit on the name's line with a "Speed" prefix and a
/// loop icon that lights when looping, and the path has the second line to
/// itself, ending in "…" when it is too long.
fn clip_block(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    index: usize,
    slot: &AnimClipSlot,
) -> Entity {
    let block = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(4.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            // Recessed under the card, not lifted above it: the white wash it
            // had read as a grey slab on every clip.
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border()).with_alpha(0.5)),
        ))
        .id();

    // Row 1: play + name + speed + loop + trash.
    let r1 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let play = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            UiInteraction::default(),
            PlayClipBtn { entity, index },
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or("animation.play_clip", "Play")),
        ))
        .id();
    let play_ic = icon_text(commands, &fonts.phosphor, "play-circle", text_muted(), 16.0);
    {
        let name = slot.name.clone();
        bind_text_color(commands, play_ic, move |w| {
            let playing = selected_is_playing(w, entity, &name);
            rgb(if playing { accent() } else { text_muted() })
        });
    }
    commands.entity(play).add_child(play_ic);

    let name_in = text_input(commands, &fonts.ui, "name", &slot.name);
    commands.entity(name_in).insert(Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        min_width: Val::Px(40.0),
        height: Val::Px(24.0),
        padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(2.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(3.0)),
        ..default()
    });
    bind_text_input(
        commands,
        name_in,
        move |w| {
            w.get::<AnimatorComponent>(entity)
                .and_then(|a| a.clips.get(index))
                .map(|c| c.name.clone())
                .unwrap_or_default()
        },
        move |w, v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(entity) {
                let collides = a
                    .clips
                    .iter()
                    .enumerate()
                    .any(|(j, c)| j != index && c.name == trimmed);
                if collides {
                    return;
                }
                let old = a.clips.get(index).map(|c| c.name.clone());
                if let Some(slot) = a.clips.get_mut(index) {
                    slot.name = trimmed.clone();
                }
                if a.default_clip == old {
                    a.default_clip = Some(trimmed);
                }
            }
        },
    );

    let trash = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            UiInteraction::default(),
            RemoveClipBtn { entity, index },
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or("animation.remove_clip", "Remove clip")),
        ))
        .id();
    let trash_ic = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 13.0);
    commands.entity(trash).add_child(trash_ic);

    // Row 2: the clip's file, alone on its line.
    let r2 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(26.0), Val::Px(4.0), Val::Px(0.0), Val::Px(0.0)),
            ..default()
        })
        .id();
    let path_lbl = commands
        .spawn((
            Text::new(slot.path.clone()),
            // 12 lands on the 11px rung.
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    renzora_ember::widgets::ellipsize(commands, path_lbl, slot.path.clone(), None);
    // The speed, labelled: the bare number box it used to be read as a
    // mystery slider. 0.05x to 5x, 1 is the clip's own speed.
    let speed = drag_value(commands, &fonts.ui, "Speed", text_muted(), 1.0, 0.05);
    commands.entity(speed).insert((
        DragRange { min: 0.05, max: 5.0 },
        renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or(
            "animation.speed_tip",
            "Playback speed (1 = normal)",
        )),
    ));
    commands.entity(speed).entry::<Node>().and_modify(|mut n| {
        n.min_width = Val::Px(84.0);
        n.flex_shrink = 0.0;
    });
    bind_2way(
        commands,
        speed,
        move |w| {
            w.get::<AnimatorComponent>(entity)
                .and_then(|a| a.clips.get(index))
                .map(|c| c.speed)
                .unwrap_or(1.0)
        },
        move |w, v: &f32| {
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(entity) {
                if let Some(slot) = a.clips.get_mut(index) {
                    slot.speed = *v;
                }
            }
        },
    );
    // Loop as an icon toggle: lit in the accent while the clip loops.
    let loop_btn = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            LoopClipBtn { entity, index },
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or("animation.loop", "Loop")),
        ))
        .id();
    let looping = move |w: &renzora_ember::reactive::Rx| {
        w.get::<AnimatorComponent>(entity)
            .and_then(|a| a.clips.get(index))
            .map(|c| c.looping)
            .unwrap_or(true)
    };
    bind_bg(commands, loop_btn, move |w| {
        if looping(w) {
            rgb(accent()).with_alpha(0.22)
        } else {
            Color::NONE
        }
    });
    let loop_ic = icon_text(commands, &fonts.phosphor, "repeat", text_muted(), 14.0);
    bind_text_color(commands, loop_ic, move |w| rgb(if looping(w) { accent() } else { text_muted() }));
    commands.entity(loop_btn).add_child(loop_ic);

    commands.entity(r1).add_children(&[play, name_in, speed, loop_btn, trash]);
    commands.entity(r2).add_child(path_lbl);
    commands.entity(block).add_children(&[r1, r2]);
    block
}

/// Toggle a clip's looping.
fn loop_clip_click(
    q: Query<(&UiInteraction, &LoopClipBtn), Changed<UiInteraction>>,
    mut animators: Query<&mut AnimatorComponent>,
) {
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        if let Ok(mut a) = animators.get_mut(btn.entity) {
            if let Some(slot) = a.clips.get_mut(btn.index) {
                slot.looping = !slot.looping;
            }
        }
    }
}

/// Lit while the timeline preview is playing this clip on this entity, which
/// is what the play button now starts. It read the runtime's current clip
/// before, which the editor no longer sets.
fn selected_is_playing(w: &renzora_ember::reactive::Rx, entity: Entity, clip: &str) -> bool {
    w.get_resource::<crate::AnimationEditorState>().is_some_and(|s| {
        s.is_previewing
            && s.selected_entity == Some(entity)
            && s.selected_clip.as_deref() == Some(clip)
    })
}

/// The clip list's add target: a full-width well saying what to drop, in place
/// of the "Add Clip [Drag asset here] x" row it replaces. That row looked like
/// a field holding a value (it never did; it only ever accepted a drop), and
/// its clear button cleared nothing. It doubles as the empty state.
fn build_clip_drop_zone(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let zone = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(10.0)),
                margin: UiRect::top(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border())),
            bevy::ui::RelativeCursorPosition::default(),
            AddClipDropZone { entity },
            Name::new("animator-clip-drop-zone"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "file-plus", text_muted(), 14.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let txt = commands
        .spawn((
            Text::new(renzora::lang::t_or("animation.drop_clips", "Drop .anim files here to add clips")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(zone).add_children(&[ic, txt]);
    zone
}

/// Every `.anim` in the drag, so a multi-select from the asset browser adds
/// all of its clips in one drop.
fn dragged_clips(payload: &renzora_ui::AssetDragPayload) -> Vec<std::path::PathBuf> {
    payload
        .paths
        .iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("anim")))
        .cloned()
        .collect()
}

fn add_clip_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&bevy::ui::RelativeCursorPosition, &AddClipDropZone)>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(cmds)) = (payload, cmds) else { return };
    if !payload.is_detached {
        return;
    }
    let Some((_, zone)) = zones.iter().find(|(rcp, _)| rcp.cursor_over) else { return };
    let paths: Vec<String> = dragged_clips(&payload)
        .iter()
        .map(|p| {
            project
                .as_ref()
                .map(|pr| pr.make_asset_relative(p))
                .unwrap_or_else(|| p.to_string_lossy().to_string())
        })
        .collect();
    if paths.is_empty() {
        return;
    }
    let entity = zone.entity;
    cmds.push(move |w: &mut World| {
        for path in &paths {
            animator_add_clip(w, entity, path.clone());
        }
    });
}

/// Light the well in the accent while `.anim` files are dragged over it. Must
/// agree with [`add_clip_drop`]: a well that lights up but refuses the drop is
/// worse than one that never lights.
fn add_clip_drop_highlight(
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    mut zones: Query<(&bevy::ui::RelativeCursorPosition, &mut BorderColor, &mut BackgroundColor), With<AddClipDropZone>>,
) {
    for (rcp, mut bc, mut bg) in &mut zones {
        let active = payload
            .as_ref()
            .is_some_and(|p| p.is_detached && rcp.cursor_over && !dragged_clips(p).is_empty());
        let (want_bc, want_bg) = if active {
            (BorderColor::all(rgb(accent())), BackgroundColor(rgb(accent()).with_alpha(0.12)))
        } else {
            (BorderColor::all(rgb(border())), BackgroundColor(rgb(faint_bg())))
        };
        if *bc != want_bc {
            *bc = want_bc;
        }
        if *bg != want_bg {
            *bg = want_bg;
        }
    }
}

// ── Clip and state-machine accessors ─────────────────────────────────────────

fn animator_add_clip(w: &mut World, e: Entity, path: String) {
    let Some(mut a) = w.get_mut::<AnimatorComponent>(e) else { return };
    if a.clips.iter().any(|c| c.path == path) {
        return;
    }
    let base = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .to_string();
    let mut name = base.clone();
    let mut n = 1;
    while a.clips.iter().any(|c| c.name == name) {
        n += 1;
        name = format!("{base}{n}");
    }
    let first = a.clips.is_empty();
    a.clips.push(AnimClipSlot::new(name.clone(), path));
    if first && a.default_clip.is_none() {
        a.default_clip = Some(name);
    }
}

fn animator_sm_get(w: &World, e: Entity) -> Option<FieldValue> {
    w.get::<AnimatorComponent>(e)
        .map(|a| FieldValue::Asset(a.state_machine.clone()))
}

fn animator_sm_set(w: &mut World, e: Entity, v: FieldValue) {
    if let FieldValue::Asset(p) = v {
        if let Some(mut a) = w.get_mut::<AnimatorComponent>(e) {
            a.state_machine = p;
        }
    }
}

// ── Click systems ────────────────────────────────────────────────────────────

/// The clip's play button: plays (or pauses) it on the timeline preview.
///
/// It used to push a runtime `Play`, which ran the clip at full speed with the
/// timeline's transport still showing stopped and no control that stopped it
/// again. The runtime now ignores `Play` while editing, so the button drives
/// the same preview the timeline's Play does, and the timeline shows it.
fn play_clip_click(
    q: Query<(&UiInteraction, &PlayClipBtn), Changed<UiInteraction>>,
    animators: Query<&AnimatorComponent>,
    state: Option<ResMut<crate::AnimationEditorState>>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let Some(mut state) = state else { return };
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let Ok(animator) = animators.get(btn.entity) else { continue };
        let Some(slot) = animator.clips.get(btn.index) else { continue };
        let same = state.selected_entity == Some(btn.entity)
            && state.selected_clip.as_deref() == Some(slot.name.as_str());
        if same && state.is_previewing {
            state.is_previewing = false;
            continue;
        }
        if !same {
            // Through the bridge so the panels re-read the clip (duration,
            // fit) exactly as when it is picked in the timeline.
            state.selected_entity = Some(btn.entity);
            if let Some(bridge) = &bridge {
                if let Ok(mut p) = bridge.pending.lock() {
                    p.push(AnimEditorAction::SelectClip(Some(slot.name.clone())));
                }
            }
        }
        state.is_previewing = true;
    }
}

fn remove_clip_click(
    q: Query<(&UiInteraction, &RemoveClipBtn), Changed<UiInteraction>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let (e, i) = (btn.entity, btn.index);
        commands.queue(move |w: &mut World| {
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(e) {
                if i < a.clips.len() {
                    let removed = a.clips.remove(i);
                    if a.default_clip.as_deref() == Some(removed.name.as_str()) {
                        a.default_clip = a.clips.first().map(|c| c.name.clone());
                    }
                }
            }
        });
    }
}
