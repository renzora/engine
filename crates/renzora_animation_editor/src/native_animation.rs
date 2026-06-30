//! Bevy-native (ember) port of the egui `AnimationPanel` (panel id `animation`,
//! titled "Properties"): the read-mostly inspector for the selected entity's
//! [`AnimatorComponent`] / [`AnimatorState`].
//!
//! Mirrors the egui layout exactly — a vertical stack of collapsible sections:
//! Clip Properties, Clip Library (click a row to select + play that clip), Bone
//! Tracks, State Machine, Parameters (read-only float/bool display + trigger
//! Fire button), Layers and Animator Settings. The dynamic lists (clip library,
//! bone tracks, parameters, layers) are `keyed_list`s keyed on structure; the
//! fixed info rows use reactive `bind_text` / `bind_display`. Mutations go
//! through the same paths the egui panel used: clip-select pushes an
//! [`AnimEditorAction::SelectClip`] onto the [`AnimEditorBridge`] and a `Play`
//! command onto the [`renzora_animation::AnimationCommandQueue`] via
//! [`EditorCommands`]; the trigger Fire button pushes
//! [`AnimEditorAction::FireTrigger`].

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_animation::{AnimClip, AnimatorComponent, AnimatorState};
use renzora_editor_framework::{EditorCommands, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::collapsible;

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

const TRANSLATION: (u8, u8, u8) = (100, 149, 237);
const ROTATION: (u8, u8, u8) = (120, 200, 120);
const SCALE: (u8, u8, u8) = (200, 120, 120);
const LABEL_W: f32 = 96.0;

pub struct NativeAnimationPanel;

impl Plugin for NativeAnimationPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativeAnimPanelClip>();
        app.register_panel_content("animation", true, build);
        app.add_systems(
            Update,
            (cache_panel_clip, clip_row_click, fire_click).run_if(in_state(SplashState::Editor)),
        );
    }
}

/// Disk-loaded copy of the currently selected clip (for Duration / Tracks /
/// Keyframes / Bone-Tracks display), reloaded when the `(entity, clip)`
/// selection changes — mirrors the egui panel's per-frame `clip_data` read.
#[derive(Resource, Default)]
struct NativeAnimPanelClip {
    key: Option<(Entity, String)>,
    clip: Option<AnimClip>,
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ClipRowBtn {
    name: String,
    looping: bool,
    speed: f32,
}
#[derive(Component)]
struct FireBtn {
    name: String,
}

// ── Accessors ────────────────────────────────────────────────────────────────

fn state(w: &World) -> Option<&AnimationEditorState> {
    w.get_resource::<AnimationEditorState>()
}

fn selected_entity(w: &World) -> Option<Entity> {
    state(w)?.selected_entity
}

fn animator(w: &World) -> Option<&AnimatorComponent> {
    let e = selected_entity(w)?;
    w.get::<AnimatorComponent>(e)
}

fn anim_state(w: &World) -> Option<&AnimatorState> {
    let e = selected_entity(w)?;
    w.get::<AnimatorState>(e)
}

fn cur_clip(w: &World) -> Option<&AnimClip> {
    w.get_resource::<NativeAnimPanelClip>()
        .and_then(|c| c.clip.as_ref())
}

fn selected_clip(w: &World) -> Option<String> {
    state(w)?.selected_clip.clone()
}

/// Whether the body (sections) should show vs the empty-state note: requires a
/// selected entity with an `AnimatorComponent` that has at least one clip.
fn ready(w: &World) -> bool {
    animator(w).is_some_and(|a| !a.clips.is_empty())
}

fn empty_headline(w: &World) -> String {
    let Some(e) = selected_entity(w) else {
        return renzora::lang::t("animation.no_entity_selected");
    };
    let has_model = w
        .get::<renzora::core::MeshInstanceData>(e)
        .is_some_and(|m| m.model_path.is_some());
    let clip_count = w
        .get::<AnimatorComponent>(e)
        .map(|a| a.clips.len())
        .unwrap_or(0);
    if clip_count == 0 && !has_model {
        renzora::lang::t("animation.entity_no_model")
    } else if clip_count == 0 {
        renzora::lang::t("animation.no_clips_yet")
    } else {
        String::new()
    }
}

fn empty_hint(w: &World) -> String {
    match selected_entity(w) {
        None if crate::setup::scene_has_candidates(w) => {
            renzora::lang::t("animation.hint_pick_entity")
        }
        None => {
            renzora::lang::t("animation.hint_import_model")
        }
        Some(_) if crate::setup::can_scan_clips(w) => {
            renzora::lang::t("animation.hint_scan_folder")
        }
        Some(_) => renzora::lang::t("animation.hint_play_on_models"),
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("native-animation-panel"),
        ))
        .id();

    // Empty state (no entity / no component / no clips) — a guided setup
    // block: contextual headline + hint, a "Scan for clips" action when the
    // selected entity has a model, and a clickable list of animation
    // candidates in the scene when nothing (useful) is selected.
    let note = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(24.0)),
            ..default()
        })
        .id();
    let note_ic = icon_text(commands, &fonts.phosphor, "film-strip", text_muted(), 24.0);
    let note_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
        ))
        .id();
    bind_text(commands, note_lbl, empty_headline);
    let hint_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
        ))
        .id();
    bind_text(commands, hint_lbl, empty_hint);

    let scan_btn = crate::setup::action_button(
        commands,
        fonts,
        "magnifying-glass",
        &renzora::lang::t("animation.scan_for_clips"),
        crate::setup::ScanClipsBtn,
    );
    bind_display(commands, scan_btn, crate::setup::can_scan_clips);

    let create_anim_btn = crate::setup::action_button(
        commands,
        fonts,
        "plus-circle",
        &renzora::lang::t("animation.create_animation"),
        crate::setup::CreateAnimBtn,
    );
    bind_display(commands, create_anim_btn, crate::setup::can_create_anim);

    let feedback = crate::setup::feedback_label(commands, fonts);

    let candidates = crate::setup::candidates_list(commands);
    bind_display(commands, candidates, |w| {
        !ready(w) && !crate::setup::can_scan_clips(w)
    });

    commands
        .entity(note)
        .add_children(&[note_ic, note_lbl, hint_lbl, scan_btn, create_anim_btn, feedback, candidates]);
    bind_display(commands, note, |w| !ready(w));

    // Body: collapsible sections.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    bind_display(commands, body, ready);

    let clip_props = build_clip_props(commands, fonts);
    let clip_library = build_clip_library(commands, fonts);
    let bone_tracks = build_bone_tracks(commands, fonts);
    let state_machine = build_state_machine(commands, fonts);
    let parameters = build_parameters(commands, fonts);
    let layers = build_layers(commands, fonts);
    let settings = build_settings(commands, fonts);

    commands.entity(body).add_children(&[
        clip_props,
        clip_library,
        bone_tracks,
        state_machine,
        parameters,
        layers,
        settings,
    ]);
    commands.entity(root).add_children(&[note, body]);
    root
}

// ── Info-row helper ──────────────────────────────────────────────────────────

/// A labelled inline property row (label on the left, reactive value on the
/// right), striped by `idx` — the native analogue of egui `inline_property`.
fn info_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    label: &str,
    color: (u8, u8, u8),
    get: impl Fn(&World) -> String + Send + Sync + 'static,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let val = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    bind_text(commands, val, get);
    commands.entity(row).add_children(&[lbl, val]);
    row
}

// ── Clip Properties ──────────────────────────────────────────────────────────

fn build_clip_props(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("film-strip"), &renzora::lang::t("animation.clip_properties"), true);
    // Hidden when no clip slot is selected (matches egui's `if let Some(slot)`).
    bind_display(commands, root, |w| {
        let Some(name) = selected_clip(w) else {
            return false;
        };
        animator(w).is_some_and(|a| a.clips.iter().any(|s| s.name == name))
    });

    let rows = [
        info_row(commands, fonts, 0, &renzora::lang::t("common.name"), text_primary(), |w| {
            slot_field(w, |s| s.name.clone())
        }),
        info_row(commands, fonts, 1, &renzora::lang::t("animation.path"), text_muted(), |w| {
            slot_field(w, |s| s.path.clone())
        }),
        info_row(commands, fonts, 2, &renzora::lang::t("animation.speed"), text_primary(), |w| {
            slot_field(w, |s| format!("{:.2}x", s.speed))
        }),
        info_row(commands, fonts, 3, &renzora::lang::t("animation.looping"), text_primary(), |w| {
            slot_field(w, |s| if s.looping { renzora::lang::t("common.yes") } else { renzora::lang::t("common.no") })
        }),
        info_row(commands, fonts, 4, &renzora::lang::t("animation.duration"), text_primary(), |w| {
            cur_clip(w).map(|c| format!("{:.2}s", c.duration)).unwrap_or_default()
        }),
        info_row(commands, fonts, 5, &renzora::lang::t("animation.tracks"), text_primary(), |w| {
            cur_clip(w).map(|c| format!("{} bones", c.tracks.len())).unwrap_or_default()
        }),
        info_row(commands, fonts, 6, &renzora::lang::t("animation.keyframes"), text_primary(), |w| {
            cur_clip(w)
                .map(|c| {
                    let kf: usize = c
                        .tracks
                        .iter()
                        .map(|t| t.translations.len() + t.rotations.len() + t.scales.len())
                        .sum();
                    format!("{}", kf)
                })
                .unwrap_or_default()
        }),
    ];
    // Duration / Tracks / Keyframes only show once the clip is loaded.
    for &r in &rows[4..] {
        bind_display(commands, r, |w| cur_clip(w).is_some());
    }
    commands.entity(body).add_children(&rows);
    root
}

/// Read a field of the selected clip slot, or empty string.
fn slot_field(w: &World, f: impl Fn(&renzora_animation::AnimClipSlot) -> String) -> String {
    let Some(name) = selected_clip(w) else {
        return String::new();
    };
    animator(w)
        .and_then(|a| a.clips.iter().find(|s| s.name == name))
        .map(f)
        .unwrap_or_default()
}

// ── Clip Library ─────────────────────────────────────────────────────────────

fn build_clip_library(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("list-bullets"), &renzora::lang::t("animation.clip_library"), true);
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, clip_library_snapshot);
    commands.entity(body).add_child(list);
    root
}

fn clip_library_snapshot(world: &World) -> KeyedSnapshot {
    let Some(a) = animator(world) else {
        return empty();
    };
    // (name, looping, speed, is_default)
    let rows: Vec<(String, bool, f32, bool)> = a
        .clips
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                s.looping,
                s.speed,
                a.default_clip.as_deref() == Some(&s.name),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (name, looping, speed, is_def))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, looping, speed.to_bits(), is_def).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, looping, speed, is_def) = &rows[i];
            clip_row(c, f, name, *looping, *speed, *is_def)
        }),
    }
}

fn clip_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    looping: bool,
    speed: f32,
    is_default: bool,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ClipRowBtn {
                name: name.to_string(),
                looping,
                speed,
            },
        ))
        .id();
    // Selected-row tint (accent @ ~10% alpha), matching the egui highlight.
    {
        let n = name.to_string();
        renzora_ember::reactive::bind_bg(commands, row, move |w| {
            if selected_clip(w).as_deref() == Some(&n) {
                let a = accent();
                Color::srgba_u8(a.0, a.1, a.2, 25)
            } else {
                Color::NONE
            }
        });
    }

    // Status icon: PLAY_CIRCLE if currently playing, else CIRCLE.
    let status = icon_text(commands, &fonts.phosphor, "circle", text_muted(), 12.0);
    {
        let n = name.to_string();
        bind_text_color(commands, status, move |w| {
            let playing = anim_state(w)
                .and_then(|s| s.current_clip.as_deref())
                == Some(n.as_str());
            rgb(if playing { accent() } else { text_muted() })
        });
    }
    {
        let n = name.to_string();
        renzora_ember::reactive::bind_text(commands, status, move |w| {
            let playing = anim_state(w)
                .and_then(|s| s.current_clip.as_deref())
                == Some(n.as_str());
            let glyph = renzora_ember::font::icon_glyph(if playing { "play-circle" } else { "circle" })
                .unwrap_or(' ');
            glyph.to_string()
        });
    }

    // Name.
    let name_lbl = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    {
        let n = name.to_string();
        bind_text_color(commands, name_lbl, move |w| {
            let selected = selected_clip(w).as_deref() == Some(&n);
            let playing = anim_state(w).and_then(|s| s.current_clip.as_deref()) == Some(n.as_str());
            rgb(if selected || playing { accent() } else { text_primary() })
        });
    }

    let mut kids = vec![status, name_lbl];

    if is_default {
        let def = commands
            .spawn((
                Text::new(renzora::lang::t("animation.default_suffix")),
                ui_font(&fonts.ui, 9.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        kids.push(def);
    }

    // Spacer pushes loop/speed to the right.
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    kids.push(gap);

    let speed_lbl = commands
        .spawn((
            Text::new(format!("{:.1}x", speed)),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let loop_ic = icon_text(
        commands,
        &fonts.phosphor,
        if looping { "repeat" } else { "arrow-right" },
        text_muted(),
        10.0,
    );
    kids.push(speed_lbl);
    kids.push(loop_ic);

    commands.entity(row).add_children(&kids);
    row
}

// ── Bone Tracks ──────────────────────────────────────────────────────────────

fn build_bone_tracks(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Collapsed by default, mirroring the egui panel.
    let (root, body) = collapsible(commands, fonts, Some("bone"), &renzora::lang::t("animation.bone_tracks"), false);
    bind_display(commands, root, |w| cur_clip(w).is_some());

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    keyed_list(commands, list, bone_tracks_snapshot);
    commands.entity(body).add_child(list);
    root
}

fn bone_tracks_snapshot(world: &World) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world) else {
        return empty();
    };
    // (bone, t_count, r_count, s_count)
    let rows: Vec<(String, usize, usize, usize)> = clip
        .tracks
        .iter()
        .map(|t| {
            (
                t.bone_name.clone(),
                t.translations.len(),
                t.rotations.len(),
                t.scales.len(),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (name, tc, rc, sc))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, tc, rc, sc).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, tc, rc, sc) = &rows[i];
            bone_track_row(c, f, i, name, *tc, *rc, *sc)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn bone_track_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    name: &str,
    tc: usize,
    rc: usize,
    sc: usize,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let t = channel(commands, fonts, "T", tc, TRANSLATION);
    let r = channel(commands, fonts, "R", rc, ROTATION);
    let s = channel(commands, fonts, "S", sc, SCALE);
    commands.entity(row).add_children(&[lbl, gap, t, r, s]);
    row
}

/// A "T 12" channel indicator — coloured when it has keyframes.
fn channel(
    commands: &mut Commands,
    fonts: &EmberFonts,
    ch: &str,
    count: usize,
    color: (u8, u8, u8),
) -> Entity {
    let col = if count > 0 { color } else { text_muted() };
    commands
        .spawn((
            Text::new(format!("{}{}", ch, count)),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(col)),
            Node {
                margin: UiRect::left(Val::Px(4.0)),
                ..default()
            },
        ))
        .id()
}

// ── State Machine ────────────────────────────────────────────────────────────

fn build_state_machine(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("graph"), &renzora::lang::t("animation.state_machine"), true);
    bind_display(commands, root, |w| {
        animator(w).is_some_and(|a| a.state_machine.is_some())
    });

    let file = info_row(commands, fonts, 0, &renzora::lang::t("animation.file"), text_muted(), |w| {
        animator(w)
            .and_then(|a| a.state_machine.clone())
            .unwrap_or_default()
    });
    let st = info_row(commands, fonts, 1, &renzora::lang::t("animation.state"), accent(), |w| {
        anim_state(w)
            .and_then(|s| s.current_state.clone())
            .unwrap_or_default()
    });
    bind_display(commands, st, |w| {
        anim_state(w).is_some_and(|s| s.current_state.is_some())
    });
    let time = info_row(commands, fonts, 2, &renzora::lang::t("animation.time"), text_primary(), |w| {
        anim_state(w).map(|s| format!("{:.2}s", s.state_time)).unwrap_or_default()
    });
    bind_display(commands, time, |w| anim_state(w).is_some());

    commands.entity(body).add_children(&[file, st, time]);
    root
}

// ── Parameters (read-only display) ───────────────────────────────────────────

fn build_parameters(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("faders"), &renzora::lang::t("animation.parameters"), true);
    bind_display(commands, root, |w| {
        anim_state(w).is_some_and(|s| {
            !s.params.floats.is_empty()
                || !s.params.bools.is_empty()
                || !s.params.triggers.is_empty()
        })
    });
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    keyed_list(commands, list, params_snapshot);
    commands.entity(body).add_child(list);
    root
}

enum ParamItem {
    Float(String),
    Bool(String),
    Trigger(String),
}

fn params_snapshot(world: &World) -> KeyedSnapshot {
    let Some(st) = anim_state(world) else {
        return empty();
    };
    let mut floats: Vec<String> = st.params.floats.keys().cloned().collect();
    floats.sort();
    let mut bools: Vec<String> = st.params.bools.keys().cloned().collect();
    bools.sort();
    let mut triggers: Vec<String> = st.params.triggers.keys().cloned().collect();
    triggers.sort();

    let mut data: Vec<ParamItem> = Vec::new();
    data.extend(floats.into_iter().map(ParamItem::Float));
    data.extend(bools.into_iter().map(ParamItem::Bool));
    data.extend(triggers.into_iter().map(ParamItem::Trigger));

    let items: Vec<(u64, u64)> = data
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match it {
                ParamItem::Float(n) => (0u8, n).hash(&mut h),
                ParamItem::Bool(n) => (1u8, n).hash(&mut h),
                ParamItem::Trigger(n) => (2u8, n).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &data[i] {
            ParamItem::Float(n) => param_float_row(c, f, i, n),
            ParamItem::Bool(n) => param_bool_row(c, f, i, n),
            ParamItem::Trigger(n) => param_trigger_row(c, f, i, n),
        }),
    }
}

fn param_row_base(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(lbl);
    row
}

fn param_float_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = param_row_base(commands, fonts, idx, name);
    let val = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let n = name.to_string();
    bind_text(commands, val, move |w| {
        anim_state(w)
            .and_then(|s| s.params.floats.get(&n).copied())
            .map(|v| format!("{:.3}", v))
            .unwrap_or_default()
    });
    commands.entity(row).add_child(val);
    row
}

fn param_bool_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = param_row_base(commands, fonts, idx, name);
    let ic = icon_text(commands, &fonts.phosphor, "circle", text_muted(), 12.0);
    {
        let n = name.to_string();
        renzora_ember::reactive::bind_text(commands, ic, move |w| {
            let on = anim_state(w).and_then(|s| s.params.bools.get(&n).copied()).unwrap_or(false);
            renzora_ember::font::icon_glyph(if on { "check-circle" } else { "circle" })
                .unwrap_or(' ')
                .to_string()
        });
    }
    {
        let n = name.to_string();
        bind_text_color(commands, ic, move |w| {
            let on = anim_state(w).and_then(|s| s.params.bools.get(&n).copied()).unwrap_or(false);
            rgb(if on { accent() } else { text_muted() })
        });
    }
    let val = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let n = name.to_string();
    bind_text(commands, val, move |w| {
        let on = anim_state(w).and_then(|s| s.params.bools.get(&n).copied()).unwrap_or(false);
        if on { "true" } else { "false" }.into()
    });
    commands.entity(row).add_children(&[ic, val]);
    row
}

fn param_trigger_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = param_row_base(commands, fonts, idx, name);
    let btn = commands
        .spawn((
            Node {
                min_width: Val::Px(40.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            FireBtn {
                name: name.to_string(),
            },
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(renzora::lang::t("animation.fire")),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(accent())),
        ))
        .id();
    commands.entity(btn).add_child(lbl);
    commands.entity(row).add_child(btn);
    row
}

// ── Layers ───────────────────────────────────────────────────────────────────

fn build_layers(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("stack"), &renzora::lang::t("animation.layers"), true);
    bind_display(commands, root, |w| {
        animator(w).is_some_and(|a| !a.layers.is_empty())
    });

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, layers_snapshot);
    commands.entity(body).add_child(list);
    root
}

fn layers_snapshot(world: &World) -> KeyedSnapshot {
    let Some(a) = animator(world) else {
        return empty();
    };
    // (name, weight, blend, clip, mask_len)
    let rows: Vec<(String, f32, String, Option<String>, Option<usize>)> = a
        .layers
        .iter()
        .map(|l| {
            (
                l.name.clone(),
                l.weight,
                format!("{:?}", l.blend_mode),
                l.current_clip.clone(),
                l.mask.as_ref().map(|m| m.len()),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (name, weight, blend, clip, mask))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, weight.to_bits(), blend, clip, mask).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, weight, blend, clip, mask) = &rows[i];
            layer_block(c, f, name, *weight, blend, clip.as_deref(), *mask)
        }),
    }
}

fn layer_block(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    weight: f32,
    blend: &str,
    clip: Option<&str>,
    mask: Option<usize>,
) -> Entity {
    let block = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    let mut idx = 0;
    let mut rows = vec![
        static_row(commands, fonts, idx, &renzora::lang::t("animation.layer"), name, text_primary()),
        {
            idx += 1;
            static_row(commands, fonts, idx, &renzora::lang::t("animation.weight"), &format!("{:.0}%", weight * 100.0), text_primary())
        },
        {
            idx += 1;
            static_row(commands, fonts, idx, &renzora::lang::t("animation.blend"), blend, text_muted())
        },
    ];
    if let Some(clip) = clip {
        idx += 1;
        rows.push(static_row(commands, fonts, idx, &renzora::lang::t("animation.clip"), clip, accent()));
    }
    if let Some(mask) = mask {
        idx += 1;
        rows.push(static_row(commands, fonts, idx, &renzora::lang::t("animation.mask"), &format!("{} bones", mask), text_muted()));
    }
    commands.entity(block).add_children(&rows);
    block
}

/// A non-reactive labelled row (value baked at build time).
fn static_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    label: &str,
    value: &str,
    color: (u8, u8, u8),
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let val = commands
        .spawn((
            Text::new(value.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[lbl, val]);
    row
}

// ── Animator Settings ────────────────────────────────────────────────────────

fn build_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Collapsed by default, mirroring the egui panel.
    let (root, body) = collapsible(commands, fonts, Some("gear"), &renzora::lang::t("animation.animator_settings"), false);

    let default_clip = info_row(commands, fonts, 0, &renzora::lang::t("animation.default_clip"), text_primary(), |w| {
        animator(w)
            .and_then(|a| a.default_clip.clone())
            .unwrap_or_else(|| renzora::lang::t("common.none"))
    });
    let blend = info_row(commands, fonts, 1, &renzora::lang::t("animation.blend_time"), text_primary(), |w| {
        animator(w).map(|a| format!("{:.2}s", a.blend_duration)).unwrap_or_default()
    });
    let clips = info_row(commands, fonts, 2, &renzora::lang::t("animation.clips"), text_primary(), |w| {
        animator(w).map(|a| format!("{}", a.clips.len())).unwrap_or_default()
    });
    let sm = info_row(commands, fonts, 3, &renzora::lang::t("animation.state_machine"), text_primary(), |w| {
        animator(w)
            .map(|a| if a.state_machine.is_some() { renzora::lang::t("common.yes") } else { renzora::lang::t("common.no") })
            .unwrap_or_default()
    });
    let init = info_row(commands, fonts, 4, &renzora::lang::t("animation.initialized"), text_primary(), |w| {
        anim_state(w)
            .map(|s| if s.initialized { renzora::lang::t("common.yes") } else { renzora::lang::t("common.no") })
            .unwrap_or_default()
    });
    bind_display(commands, init, |w| anim_state(w).is_some());

    commands
        .entity(body)
        .add_children(&[default_clip, blend, clips, sm, init]);
    root
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn empty() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Load the selected clip's `.anim` from disk when the selection changes — the
/// native analogue of the egui panel's inline `clip_data` read.
fn cache_panel_clip(
    mut cache: ResMut<NativeAnimPanelClip>,
    state: Res<AnimationEditorState>,
    animators: Query<&AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let key = match (state.selected_entity, state.selected_clip.as_deref()) {
        (Some(e), Some(c)) => Some((e, c.to_string())),
        _ => None,
    };
    if key == cache.key {
        return;
    }
    cache.key = key.clone();
    cache.clip = None;
    let (Some((entity, clip_name)), Some(project)) = (key, project) else {
        return;
    };
    let Ok(animator) = animators.get(entity) else {
        return;
    };
    let Some(slot) = animator.clips.iter().find(|s| s.name == clip_name) else {
        return;
    };
    let path = project.path.join(&slot.path);
    if let Ok(content) = std::fs::read_to_string(&path) {
        cache.clip = ron::from_str::<AnimClip>(&content).ok();
    }
}

/// Click a clip-library row → select it (bridge) + play it (command queue),
/// exactly like the egui panel.
fn clip_row_click(
    q: Query<(&Interaction, &ClipRowBtn), Changed<Interaction>>,
    state: Option<Res<AnimationEditorState>>,
    bridge: Option<Res<AnimEditorBridge>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let (Some(state), Some(bridge), Some(cmds)) = (state, bridge, cmds) else {
        return;
    };
    let Some(entity) = state.selected_entity else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut p) = bridge.pending.lock() {
            p.push(AnimEditorAction::SelectClip(Some(btn.name.clone())));
        }
        let name = btn.name.clone();
        let looping = btn.looping;
        let speed = btn.speed;
        cmds.push(move |world: &mut World| {
            if let Some(mut queue) =
                world.get_resource_mut::<renzora_animation::AnimationCommandQueue>()
            {
                queue
                    .commands
                    .push(renzora_animation::AnimationCommand::Play {
                        entity,
                        name,
                        looping,
                        speed,
                    });
            }
        });
    }
}

/// Trigger "Fire" button → push a `FireTrigger` action onto the bridge.
fn fire_click(
    q: Query<(&Interaction, &FireBtn), Changed<Interaction>>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let Some(bridge) = bridge else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut p) = bridge.pending.lock() {
            p.push(AnimEditorAction::FireTrigger {
                name: btn.name.clone(),
            });
        }
    }
}
