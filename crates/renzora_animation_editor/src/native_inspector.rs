//! Native (ember) inspector drawer for [`AnimatorComponent`] — the "animation
//! player" section in the Properties panel.
//!
//! The egui editor had a full custom inspector here (clip library with
//! drag-drop, default-clip selector, blend slider) that was reduced to a bare
//! blend-duration field during the egui purge. This restores it natively:
//!
//! - clip list: play button, rename, speed, loop, remove,
//! - drop/browse `.anim` files to add clips,
//! - default clip dropdown (selecting also plays it),
//! - blend duration,
//! - `.animsm` state machine assignment.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora::{AppEditorExt, FieldValue};
use renzora_animation::{AnimClipSlot, AnimatorComponent};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{inspector_row, inspector_stripe};
use renzora_ember::reactive::{bind_2way, bind_text_color};
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};
use renzora_ember::widgets::{
    bind_text_input, checkbox, drag_value, dropdown, text_input, DragRange,
};
use renzora_inspector::asset_drop_field;

use crate::{AnimEditorAction, AnimEditorBridge};

pub fn register_animator_native(app: &mut App) {
    app.register_native_inspector_ui("animator", animator_native);
    app.add_systems(
        Update,
        (rebuild_animator, play_clip_click, remove_clip_click)
            .run_if(in_state(renzora::SplashState::Editor)),
    );
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

fn header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let h = commands
        .spawn(Node {
            margin: UiRect { top: Val::Px(6.0), bottom: Val::Px(1.0), ..default() },
            ..default()
        })
        .id();
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
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
    let mut stripe = 0usize;
    let striped = |commands: &mut Commands, row: Entity, stripe: &mut usize| {
        commands
            .entity(row)
            .insert(BackgroundColor(inspector_stripe(*stripe)));
        *stripe += 1;
        row
    };

    // ── Clips ──
    children.push(header(commands, fonts, &renzora::lang::t("animation.animation_clips")));
    if data.clips.is_empty() {
        let note = commands
            .spawn((
                Text::new(renzora::lang::t("animation.no_clips_drop")),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        children.push(note);
    }
    for (i, slot) in data.clips.iter().enumerate() {
        let block = clip_block(commands, fonts, entity, i, slot);
        children.push(striped(commands, block, &mut stripe));
    }
    let add = asset_drop_field(
        commands,
        fonts,
        entity,
        animator_add_get,
        animator_add_clip,
        vec!["anim".to_string()],
    );
    let r = inspector_row(commands, &fonts.ui, &renzora::lang::t("animation.add_clip"), add);
    children.push(striped(commands, r, &mut stripe));

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
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(entity) {
                a.default_clip = name.clone();
            }
            // Selecting a default clip also plays it, like the egui inspector.
            if let Some(name) = name {
                if let Some(mut queue) =
                    w.get_resource_mut::<renzora_animation::AnimationCommandQueue>()
                {
                    queue.commands.push(renzora_animation::AnimationCommand::Play {
                        entity,
                        name,
                        looping: true,
                        speed: 1.0,
                    });
                }
            }
        },
    );
    let r = inspector_row(commands, &fonts.ui, &renzora::lang::t("animation.default_clip"), dd);
    children.push(striped(commands, r, &mut stripe));

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
    children.push(striped(commands, r, &mut stripe));

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
    children.push(striped(commands, r, &mut stripe));

    commands.entity(root).add_children(&children);
}

/// One clip slot: `[▶] name … [trash]` over `path · speed · loop`.
fn clip_block(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    index: usize,
    slot: &AnimClipSlot,
) -> Entity {
    let block = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        })
        .id();

    // Row 1: play + name + trash.
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
            Interaction::default(),
            PlayClipBtn { entity, index },
        ))
        .id();
    let play_ic = icon_text(commands, &fonts.phosphor, "play-circle", text_muted(), 14.0);
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
        min_width: Val::Px(50.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
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
            Interaction::default(),
            RemoveClipBtn { entity, index },
        ))
        .id();
    let trash_ic = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 12.0);
    commands.entity(trash).add_child(trash_ic);
    commands.entity(r1).add_children(&[play, name_in, trash]);

    // Row 2: path + speed + loop.
    let r2 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::left(Val::Px(24.0)),
            ..default()
        })
        .id();
    let path_lbl = commands
        .spawn((
            Text::new(slot.path.clone()),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let speed = drag_value(commands, &fonts.ui, "", (210, 210, 220), 1.0, 0.05);
    commands.entity(speed).insert(DragRange { min: 0.05, max: 5.0 });
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
    let loop_cb = checkbox(commands, true);
    bind_2way(
        commands,
        loop_cb,
        move |w| {
            w.get::<AnimatorComponent>(entity)
                .and_then(|a| a.clips.get(index))
                .map(|c| c.looping)
                .unwrap_or(true)
        },
        move |w, v: &bool| {
            if let Some(mut a) = w.get_mut::<AnimatorComponent>(entity) {
                if let Some(slot) = a.clips.get_mut(index) {
                    slot.looping = *v;
                }
            }
        },
    );
    let loop_lbl = commands
        .spawn((Text::new("loop"), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted()))))
        .id();
    commands
        .entity(r2)
        .add_children(&[path_lbl, speed, loop_cb, loop_lbl]);

    commands.entity(block).add_children(&[r1, r2]);
    block
}

fn selected_is_playing(w: &World, entity: Entity, clip: &str) -> bool {
    w.get::<renzora_animation::AnimatorState>(entity)
        .and_then(|s| s.current_clip.as_deref())
        == Some(clip)
}

// ── asset_drop_field accessors ───────────────────────────────────────────────

fn animator_add_get(_w: &World, _e: Entity) -> Option<FieldValue> {
    Some(FieldValue::Asset(None))
}

fn animator_add_clip(w: &mut World, e: Entity, v: FieldValue) {
    let FieldValue::Asset(Some(path)) = v else { return };
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

fn play_clip_click(
    q: Query<(&Interaction, &PlayClipBtn), Changed<Interaction>>,
    animators: Query<&AnimatorComponent>,
    queue: Option<ResMut<renzora_animation::AnimationCommandQueue>>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(animator) = animators.get(btn.entity) else { continue };
        let Some(slot) = animator.clips.get(btn.index) else { continue };
        queue.commands.push(renzora_animation::AnimationCommand::Play {
            entity: btn.entity,
            name: slot.name.clone(),
            looping: slot.looping,
            speed: slot.speed,
        });
        // Keep the animation panels in sync with what was just played.
        if let Some(bridge) = &bridge {
            if let Ok(mut p) = bridge.pending.lock() {
                p.push(AnimEditorAction::SelectClip(Some(slot.name.clone())));
            }
        }
    }
}

fn remove_clip_click(
    q: Query<(&Interaction, &RemoveClipBtn), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
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
