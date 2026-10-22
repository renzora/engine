//! The timeline's menus and cards: the "entity / clip ▾" picker, the "⋯"
//! overflow card, and the empty state shown before an entity has a clip.
//!
//! The overflow card is where the old toolbar's long tail went: clip length
//! (also draggable as the end handle in the ruler), the event marker field,
//! zoom, row height, adding a property track or a key, and Save. Autosave runs every 1.5 s
//! while there are edits, so Save is rarely needed, and none of these is
//! reached for often enough to earn a permanent place on the bar.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_animation::{AnimClipSlot, AnimatorComponent, AnimatorState};
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    drag_value, menu_item, menu_sep, screen_menu, screen_menu_parts, text_input, trigger_rect, DragRange,
};
use renzora_ember::UiInteraction;

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

use super::build::{hover_wash, PLAY_BLUE};
use super::clip::{cur_clip, set_clip_duration, state, TimelineClip};
use super::edit::unused_clip_name;
use super::props::TimelineOps;
use super::{
    AddMarkerBtn, AnimBtn, ClipCombo, MarkerNameField, NewClipBtn, NewClipNameField, OverflowBtn, RowHeightBtn,
    SaveClipBtn, ROW_PRESETS,
};

/// The "New clip" card (a name field and Create).
#[derive(Component)]
pub(super) struct NewClipCard;

/// One of the empty state's three actions.
#[derive(Component, Clone, Copy)]
pub(super) enum EmptyCard {
    Record,
    KeyProperty,
    UseClip,
}

// ── Clip picker ──────────────────────────────────────────────────────────────

pub(super) fn clip_picker_open(
    q: Query<(&UiInteraction, &RelativeCursorPosition, &ComputedNode), (With<ClipCombo>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<AnimationEditorState>>,
    animators: Query<&AnimatorComponent>,
    mut commands: Commands,
) {
    let (Some(fonts), Some(state)) = (fonts, state) else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == UiInteraction::Pressed) else { return };
    let Some(entity) = state.selected_entity else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let rect = trigger_rect(cursor, rcp, cn);
    let menu = screen_menu(&mut commands, rect.min.x, rect.max.y + 2.0);
    let mut kids: Vec<Entity> = Vec::new();
    if let Ok(animator) = animators.get(entity) {
        let default_clip = animator.default_clip.clone();
        let current = state.selected_clip.clone();
        for slot in &animator.clips {
            let name = slot.name.clone();
            let (looping, speed) = (slot.looping, slot.speed);
            let label = if default_clip.as_deref() == Some(&slot.name) {
                format!("{} {}", slot.name, renzora::lang::t("animation.default_suffix"))
            } else {
                slot.name.clone()
            };
            let icon = if current.as_deref() == Some(&slot.name) { "check" } else { "film-strip" };
            kids.push(menu_item(&mut commands, &fonts, icon, &label, move |w| {
                if let Some(bridge) = w.get_resource::<AnimEditorBridge>() {
                    if let Ok(mut p) = bridge.pending.lock() {
                        p.push(AnimEditorAction::SelectClip(Some(name.clone())));
                    }
                }
                if let Some(mut queue) = w.get_resource_mut::<renzora_animation::AnimationCommandQueue>() {
                    queue.commands.push(renzora_animation::AnimationCommand::Play { entity, name: name.clone(), looping, speed });
                }
            }));
        }
        if !animator.clips.is_empty() {
            kids.push(menu_sep(&mut commands));
        }
    }
    let at = Vec2::new(rect.min.x, rect.max.y + 2.0);
    if let Some(current) = state.selected_clip.clone() {
        kids.push(menu_item(&mut commands, &fonts, "pencil-simple", &renzora::lang::t_or("timeline.rename_clip_menu", "Rename clip..."), move |w| {
            spawn_rename_clip_card(w, at, &current);
        }));
    }
    kids.push(menu_item(&mut commands, &fonts, "plus", &renzora::lang::t_or("timeline.new_clip_menu", "New clip..."), move |w| {
        spawn_new_clip_card(w, at);
    }));
    kids.push(menu_item(&mut commands, &fonts, "folder-open", &renzora::lang::t_or("timeline.use_existing_clip", "Use an existing clip..."), move |w| {
        open_project_clip_menu(w, at);
    }));
    commands.entity(menu).add_children(&kids);
}

/// A small card with a name field and Create, opened from the clip picker.
fn spawn_new_clip_card(world: &mut World, at: Vec2) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let (content, card) = screen_menu_parts(&mut commands, at.x, at.y);
        commands.entity(card).insert(NewClipCard);
        commands.entity(card).entry::<Node>().and_modify(|mut n| {
            n.min_width = Val::Px(260.0);
            n.padding = UiRect::all(Val::Px(10.0));
        });
        let title = commands.spawn((Text::new(renzora::lang::t_or("timeline.new_clip_title", "New clip")), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
        let row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(8.0)), ..default() }).id();
        let field = text_input(&mut commands, &fonts.ui, &renzora::lang::t_or("timeline.clip_name", "Clip name"), "");
        commands.entity(field).insert((
            NewClipNameField,
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), height: Val::Px(26.0), padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(1.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
        ));
        let create = solid_button(&mut commands, &fonts, &renzora::lang::t_or("timeline.create", "Create"), NewClipBtn);
        commands.entity(row).add_children(&[field, create]);
        commands.entity(content).add_children(&[title, row]);
    }
    queue.apply(world);
}

/// The "Rename clip" card: the clip's name, editable, and Rename.
#[derive(Component)]
pub(super) struct RenameClipCard;
/// The card's name field; carries the name being replaced.
#[derive(Component)]
pub(super) struct RenameClipField(pub(super) String);
#[derive(Component)]
pub(super) struct RenameClipBtn;

fn spawn_rename_clip_card(world: &mut World, at: Vec2, current: &str) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let (content, card) = screen_menu_parts(&mut commands, at.x, at.y);
        commands.entity(card).insert(RenameClipCard);
        commands.entity(card).entry::<Node>().and_modify(|mut n| {
            n.min_width = Val::Px(260.0);
            n.padding = UiRect::all(Val::Px(10.0));
        });
        let title = commands.spawn((Text::new(renzora::lang::t_or("timeline.rename_clip_title", "Rename clip")), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
        let row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(8.0)), ..default() }).id();
        let field = text_input(&mut commands, &fonts.ui, &renzora::lang::t_or("timeline.clip_name", "Clip name"), current);
        commands.entity(field).insert((
            RenameClipField(current.to_string()),
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), height: Val::Px(26.0), padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(1.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
        ));
        let rename = solid_button(&mut commands, &fonts, &renzora::lang::t_or("timeline.rename", "Rename"), RenameClipBtn);
        commands.entity(row).add_children(&[field, rename]);
        commands.entity(content).add_children(&[title, row]);
    }
    queue.apply(world);
}

pub(super) fn rename_clip_click(
    q: Query<&UiInteraction, (With<RenameClipBtn>, Changed<UiInteraction>)>,
    field: Query<(&renzora_ember::widgets::EmberTextInput, &RenameClipField)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let Some((input, old)) = field.iter().next() else { return };
    let (new, old) = (input.value.trim().to_string(), old.0.clone());
    cmds.push(move |world: &mut World| {
        let Some(entity) = world.get_resource::<AnimationEditorState>().and_then(|s| s.selected_entity) else { return };
        rename_clip(world, entity, &old, &new);
        let cards: Vec<Entity> = world.query_filtered::<Entity, With<RenameClipCard>>().iter(world).collect();
        for e in cards {
            if let Ok(em) = world.get_entity_mut(e) {
                em.despawn();
            }
        }
    });
}

/// Rename a clip slot on `entity` (its name, not its `.anim` file), carrying
/// the default clip and the timeline's selection along. Refuses an empty name
/// or one another slot already has, the same rules as the inspector's field.
fn rename_clip(world: &mut World, entity: Entity, old: &str, new: &str) {
    if new.is_empty() || new == old {
        return;
    }
    let Some(mut animator) = world.get_mut::<AnimatorComponent>(entity) else { return };
    if animator.clips.iter().any(|c| c.name == new) {
        return;
    }
    let Some(slot) = animator.clips.iter_mut().find(|c| c.name == old) else { return };
    slot.name = new.to_string();
    if animator.default_clip.as_deref() == Some(old) {
        animator.default_clip = Some(new.to_string());
    }
    // The runtime graph is keyed by clip name: drop it so it rebuilds.
    world.entity_mut(entity).remove::<AnimatorState>();
    if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
        if s.selected_clip.as_deref() == Some(old) {
            s.selected_clip = Some(new.to_string());
        }
    }
}

/// Every `.anim` under the project, for "Use an existing clip". Skips build
/// output and hidden folders; capped so a huge project can't stall the menu.
fn project_anim_files(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "dist" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("anim") {
                if let Ok(rel) = p.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
                if out.len() >= 400 {
                    return out;
                }
            }
        }
    }
    out.sort();
    out
}

fn open_project_clip_menu(world: &mut World, at: Vec2) {
    let Some(root) = world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone()) else { return };
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let files = project_anim_files(&root);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let menu = screen_menu(&mut commands, at.x, at.y);
        let kids: Vec<Entity> = if files.is_empty() {
            vec![menu_item(&mut commands, &fonts, "info", &renzora::lang::t_or("timeline.no_project_clips", "No .anim clips in this project"), |_| {})]
        } else {
            files
                .into_iter()
                .map(|rel| {
                    let label = rel.clone();
                    menu_item(&mut commands, &fonts, "film-strip", &label, move |w| {
                        let Some(entity) = w.get_resource::<AnimationEditorState>().and_then(|s| s.selected_entity) else { return };
                        adopt_clip(w, entity, &rel);
                    })
                })
                .collect()
        };
        commands.entity(menu).add_children(&kids);
    }
    queue.apply(world);
}

/// Give `entity` a slot for an existing `.anim` (project-relative path) and
/// select it, creating the animator if it has none.
fn adopt_clip(world: &mut World, entity: Entity, rel: &str) {
    let stem = std::path::Path::new(rel).file_stem().and_then(|s| s.to_str()).unwrap_or("clip").to_string();
    let existing = world
        .get::<AnimatorComponent>(entity)
        .and_then(|a| a.clips.iter().find(|c| c.path == rel).map(|c| c.name.clone()));
    let name = match existing {
        Some(n) => n,
        None => {
            // Only the slot name has to be free: the file already exists.
            let slots: Vec<String> = world
                .get::<AnimatorComponent>(entity)
                .map(|a| a.clips.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default();
            let mut n = stem.clone();
            let mut i = 2;
            while slots.contains(&n) {
                n = format!("{stem}_{i}");
                i += 1;
            }
            if let Some(mut animator) = world.get_mut::<AnimatorComponent>(entity) {
                animator.add_clip(AnimClipSlot::new(n.clone(), rel.to_string()));
                if animator.default_clip.is_none() {
                    animator.default_clip = Some(n.clone());
                }
            } else {
                let mut animator = AnimatorComponent::new();
                animator.add_clip(AnimClipSlot::new(n.clone(), rel.to_string()));
                animator.default_clip = Some(n.clone());
                world.entity_mut(entity).insert(animator);
            }
            world.entity_mut(entity).remove::<AnimatorState>();
            n
        }
    };
    if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
        s.selected_clip = Some(name);
        s.clip_duration = None;
        s.auto_fit_clip = None;
        s.scrub_time = 0.0;
    }
}

// ── Overflow ─────────────────────────────────────────────────────────────────

pub(super) fn overflow_open(
    q: Query<(&UiInteraction, &RelativeCursorPosition, &ComputedNode), (With<OverflowBtn>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == UiInteraction::Pressed) else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let rect = trigger_rect(cursor, rcp, cn);
    let (content, card) = screen_menu_parts(&mut commands, (rect.max.x - 280.0).max(0.0), rect.max.y + 2.0);
    commands.entity(card).entry::<Node>().and_modify(|mut n| {
        n.min_width = Val::Px(280.0);
        n.padding = UiRect::all(Val::Px(10.0));
    });
    commands.entity(content).entry::<Node>().and_modify(|mut n| n.row_gap = Val::Px(8.0));

    // Length.
    let len_row = labeled_row(&mut commands, &fonts, &renzora::lang::t_or("timeline.clip_length", "Clip length (s)"));
    let len_dv = drag_value(&mut commands, &fonts.mono, "", text_primary(), 2.0, 0.05);
    commands.entity(len_dv).insert(DragRange { min: 0.2, max: 600.0 });
    commands.entity(len_dv).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(0.0);
    });
    bind_2way(&mut commands, len_dv, |w: &Rx| cur_clip(w.untracked()).map(|c| c.duration).unwrap_or(2.0), |w: &mut World, v: &f32| set_clip_duration(w, *v));
    commands.entity(len_row).add_child(len_dv);

    // Event marker at the playhead.
    let ev_row = labeled_row(&mut commands, &fonts, &renzora::lang::t_or("timeline.event", "Event"));
    let ev_field = text_input(&mut commands, &fonts.ui, "event", "");
    commands.entity(ev_field).insert((
        MarkerNameField,
        Node { flex_grow: 1.0, min_width: Val::Px(0.0), height: Val::Px(24.0), padding: UiRect::axes(Val::Px(renzora_ember::widgets::PAD_X), Val::Px(1.0)), align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
    ));
    let ev_btn = ghost_button(&mut commands, &fonts, "flag", &renzora::lang::t_or("timeline.add_event", "Add at playhead"), AddMarkerBtn);
    commands.entity(ev_row).add_children(&[ev_field, ev_btn]);

    // Zoom.
    let zoom_row = labeled_row(&mut commands, &fonts, &renzora::lang::t_or("timeline.zoom_label", "Zoom"));
    let z_out = ghost_icon(&mut commands, &fonts, "magnifying-glass-minus", AnimBtn::ZoomOut);
    // No wrap and no shrink: framing a short clip lands in the thousands
    // ("2262 px/s"), which wrapped onto two lines in the old 64px box.
    let z_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 12.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { min_width: Val::Px(78.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, ..default() },
        ))
        .id();
    bind_text(&mut commands, z_lbl, |w| format!("{:.0} px/s", state(w.untracked()).map(|s| s.timeline_zoom).unwrap_or(0.0)));
    let z_in = ghost_icon(&mut commands, &fonts, "magnifying-glass-plus", AnimBtn::ZoomIn);
    let z_fit = ghost_button(&mut commands, &fonts, "arrows-out-line-horizontal", &renzora::lang::t_or("timeline.frame_all", "Frame all"), AnimBtn::FrameAll);
    commands.entity(zoom_row).add_children(&[z_out, z_lbl, z_in, z_fit]);

    // Row height: three presets (Alt+wheel steps between them and beyond).
    let rows_row = labeled_row(&mut commands, &fonts, &renzora::lang::t_or("timeline.row_height", "Row height"));
    commands.entity(rows_row).insert(renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or(
        "timeline.row_height_tip",
        "Alt+wheel over the timeline also changes it",
    )));
    let names = [("timeline.rows_compact", "Compact"), ("timeline.rows_normal", "Normal"), ("timeline.rows_roomy", "Roomy")];
    for (h, (key, fallback)) in ROW_PRESETS.into_iter().zip(names) {
        let b = commands
            .spawn((
                Node { height: Val::Px(24.0), padding: UiRect::horizontal(Val::Px(8.0)), align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() },
                BackgroundColor(Color::NONE),
                UiInteraction::default(),
                RowHeightBtn(h),
            ))
            .id();
        bind_bg(&mut commands, b, move |w| {
            if state(w.untracked()).is_some_and(|s| (s.track_height - h).abs() < 0.5) {
                return rgb(accent()).with_alpha(0.22);
            }
            match w.get::<UiInteraction>(b) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
                _ => Color::NONE,
            }
        });
        let t = commands.spawn((Text::new(renzora::lang::t_or(key, fallback)), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
        commands.entity(b).add_child(t);
        commands.entity(rows_row).add_child(b);
    }

    let sep = menu_sep(&mut commands);
    let add_track = ghost_button(&mut commands, &fonts, "list-plus", &renzora::lang::t_or("timeline.add_property_track", "Add property track  (N)"), AnimBtn::AddProperty);
    let add_key = ghost_button(&mut commands, &fonts, "diamond", &renzora::lang::t_or("timeline.key_all", "Key every property track  (K)"), AnimBtn::AddKey);
    let save = ghost_button(&mut commands, &fonts, "floppy-disk", &renzora::lang::t_or("timeline.save_now", "Save now"), SaveClipBtn);
    let hint = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(&mut commands, hint, |w| {
        if w.get_resource::<TimelineClip>().is_some_and(|c| c.dirty) {
            renzora::lang::t_or("timeline.unsaved_autosave", "Unsaved edits, autosaving")
        } else {
            renzora::lang::t_or("timeline.all_saved", "All edits saved")
        }
    });
    bind_text_color(&mut commands, hint, |w| {
        let dirty = w.get_resource::<TimelineClip>().is_some_and(|c| c.dirty);
        rgb(if dirty { (255, 210, 120) } else { text_muted() })
    });
    commands.entity(content).add_children(&[len_row, ev_row, zoom_row, rows_row, sep, add_track, add_key, save, hint]);
}

/// A row height preset in the "⋯" menu.
pub(super) fn row_height_click(q: Query<(&UiInteraction, &RowHeightBtn), Changed<UiInteraction>>, bridge: Option<Res<AnimEditorBridge>>) {
    let Some(bridge) = bridge else { return };
    for (i, b) in &q {
        if *i == UiInteraction::Pressed {
            super::edit::push(&bridge, AnimEditorAction::SetTrackHeight(b.0));
        }
    }
}

fn labeled_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), width: Val::Percent(100.0), ..default() }).id();
    let l = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap(), Node { width: Val::Px(92.0), flex_shrink: 0.0, ..default() })).id();
    commands.entity(row).add_child(l);
    row
}

fn ghost_icon<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, marker: M) -> Entity {
    super::build::icon_btn(commands, fonts, icon, text_primary(), marker).0
}

fn ghost_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, marker: M) -> Entity {
    let b = commands
        .spawn((Node { height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), UiInteraction::default(), marker))
        .id();
    hover_wash(commands, b);
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    commands.entity(b).add_children(&[ic, t]);
    b
}

pub(super) fn solid_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, label: &str, marker: M) -> Entity {
    let b = commands
        .spawn((Node { height: Val::Px(26.0), padding: UiRect::horizontal(Val::Px(14.0)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(7.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(PLAY_BLUE)), UiInteraction::default(), marker))
        .id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb((255, 255, 255))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    commands.entity(b).add_child(t);
    b
}

// ── Empty state ──────────────────────────────────────────────────────────────

/// Whether the selected entity has no clip at all (the three cards apply).
fn wants_cards(w: &Rx) -> bool {
    let world = w.untracked();
    let Some(e) = state(world).and_then(|s| s.selected_entity) else { return false };
    world.get::<AnimatorComponent>(e).is_none_or(|a| a.clips.is_empty())
}

/// The empty state: three ways to start when the entity has no clip, or a
/// plain message (nothing selected, a clip still loading).
pub(super) fn build_empty_state(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: Val::Px(18.0), ..default() })
        .id();

    let msg = commands.spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::justify(bevy::text::Justify::Center))).id();
    bind_text(commands, msg, super::clip::empty_msg);
    bind_display(commands, msg, |w| !wants_cards(w));

    let cards_block = commands.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(18.0), ..default() }).id();
    bind_display(commands, cards_block, wants_cards);
    let head = commands.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), ..default() }).id();
    let title = commands.spawn((Text::new(""), ui_font(&fonts.ui, 16.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, title, |w| {
        let name = state(w.untracked())
            .and_then(|s| s.selected_entity)
            .and_then(|e| w.get::<Name>(e).map(|n| n.as_str().to_string()))
            .unwrap_or_default();
        format!("{name} {}", renzora::lang::t_or("timeline.has_no_animation", "has no animation yet"))
    });
    let sub = commands.spawn((Text::new(renzora::lang::t_or("timeline.pick_how_to_start", "Pick how you want to start. You can always add more tracks later.")), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    commands.entity(head).add_children(&[title, sub]);

    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), ..default() }).id();
    let record = card(commands, fonts, "record", (248, 113, 113), "timeline.card_record", "Record", "timeline.card_record_desc", "Move the entity in the viewport and every change is keyed at the playhead.", "timeline.card_record_btn", "Start recording", true, EmptyCard::Record);
    let key = card(commands, fonts, "diamond", (96, 165, 250), "timeline.card_key", "Key a property", "timeline.card_key_desc", "Pick Position, Rotation or any field, and add the first key now.", "timeline.card_key_btn", "Choose a property", false, EmptyCard::KeyProperty);
    let clip = card(commands, fonts, "list", (110, 231, 160), "timeline.card_clip", "Use an existing clip", "timeline.card_clip_desc", "Apply a clip from the project, or one that came with an imported model.", "timeline.card_clip_btn", "Browse clips", false, EmptyCard::UseClip);
    commands.entity(row).add_children(&[record, key, clip]);

    let hints = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(14.0), ..default() }).id();
    for (k, key_fallback, d, d_fallback) in [
        ("timeline.hint_k", "K", "timeline.hint_k_desc", "key the selected field"),
        ("timeline.hint_space", "Space", "timeline.hint_space_desc", "play"),
        ("timeline.hint_step", ",  .", "timeline.hint_step_desc", "step a frame"),
    ] {
        let h = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
        let kbd = commands
            .spawn((Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb((36, 36, 44))), BorderColor::all(rgb((48, 48, 58)))))
            .id();
        let kt = commands.spawn((Text::new(renzora::lang::t_or(k, key_fallback)), ui_font(&fonts.ui, 11.0), TextColor(rgb((196, 196, 206))))).id();
        commands.entity(kbd).add_child(kt);
        let dt = commands.spawn((Text::new(renzora::lang::t_or(d, d_fallback)), ui_font(&fonts.ui, 11.0), TextColor(rgb((138, 138, 150))))).id();
        commands.entity(h).add_children(&[kbd, dt]);
        commands.entity(hints).add_child(h);
    }
    commands.entity(cards_block).add_children(&[head, row, hints]);
    commands.entity(root).add_children(&[msg, cards_block]);
    root
}

#[allow(clippy::too_many_arguments)]
fn card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    hue: (u8, u8, u8),
    title_key: &str,
    title: &str,
    desc_key: &str,
    desc: &str,
    btn_key: &str,
    btn: &str,
    primary: bool,
    action: EmptyCard,
) -> Entity {
    let c = commands
        .spawn((
            Node { width: Val::Px(230.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::all(Val::Px(16.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(10.0)), ..default() },
            // The theme's recessed surface and border, never a white wash: a
            // translucent white over the panel reads as flat grey.
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let badge = commands
        .spawn((Node { width: Val::Px(36.0), height: Val::Px(36.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(9.0)), ..default() }, BackgroundColor(rgb(hue).with_alpha(0.15))))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, hue, 18.0);
    commands.entity(badge).add_child(ic);
    let t = commands.spawn((Text::new(renzora::lang::t_or(title_key, title)), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    let d = commands.spawn((Text::new(renzora::lang::t_or(desc_key, desc)), ui_font(&fonts.ui, 12.0), TextColor(rgb((154, 154, 166))))).id();
    let b = commands
        .spawn((
            Node { height: Val::Px(28.0), margin: UiRect::top(Val::Px(4.0)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(if primary { rgb(PLAY_BLUE) } else { rgb(hover_bg()) }),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            action,
        ))
        .id();
    let bt = commands.spawn((Text::new(renzora::lang::t_or(btn_key, btn)), ui_font(&fonts.ui, 12.0), TextColor(rgb(if primary { (255, 255, 255) } else { text_primary() })), PASS_THROUGH)).id();
    commands.entity(b).add_child(bt);
    commands.entity(c).add_children(&[badge, t, d, b]);
    c
}

pub(super) fn empty_card_click(
    q: Query<(&UiInteraction, &EmptyCard), Changed<UiInteraction>>,
    windows: Query<&Window>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    let cursor = windows.iter().find_map(|w| w.cursor_position()).unwrap_or(Vec2::splat(200.0));
    for (i, card) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let card = *card;
        cmds.push(move |world: &mut World| {
            let Some(entity) = world.get_resource::<AnimationEditorState>().and_then(|s| s.selected_entity) else { return };
            if matches!(card, EmptyCard::UseClip) {
                open_project_clip_menu(world, cursor);
                return;
            }
            let has_clip = world.get::<AnimatorComponent>(entity).is_some_and(|a| !a.clips.is_empty());
            if !has_clip {
                let base = world.get::<Name>(entity).map(|n| n.as_str().to_string()).unwrap_or_else(|| "clip".into());
                let name = unused_clip_name(world, entity, &base);
                crate::setup::create_clip_on_entity(world, entity, &name);
            }
            if let Some(mut ops) = world.get_resource_mut::<TimelineOps>() {
                match card {
                    EmptyCard::Record => ops.pending_record_setup = true,
                    EmptyCard::KeyProperty => ops.pending_property_pick = Some(cursor),
                    EmptyCard::UseClip => {}
                }
            }
        });
    }
}
