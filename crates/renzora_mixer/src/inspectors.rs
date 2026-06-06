//! Inspector entries for audio components (AudioPlayer, AudioListener).

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_audio::{AudioListener, AudioPlayer, MixerState, RolloffType};
use renzora_editor::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};

pub fn register_audio_inspectors(registry: &mut InspectorRegistry) {
    registry.register(audio_player_entry());
    registry.register(audio_listener_entry());
}

/// Rolloff options, indexed to match the dropdown order below.
const ROLLOFF_LABELS: &[&str] = &["Logarithmic", "Linear"];

fn rolloff_to_index(r: &RolloffType) -> usize {
    match r {
        RolloffType::Logarithmic => 0,
        RolloffType::Linear => 1,
    }
}

fn rolloff_from_index(i: usize) -> RolloffType {
    match i {
        1 => RolloffType::Linear,
        _ => RolloffType::Logarithmic,
    }
}

fn audio_player_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "audio_player",
        display_name: "Audio Player",
        icon: regular::SPEAKER_HIGH,
        category: "Audio",
        has_fn: |world, entity| world.get::<AudioPlayer>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AudioPlayer::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AudioPlayer>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

fn audio_listener_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "audio_listener",
        display_name: "Audio Listener",
        icon: regular::EAR,
        category: "Audio",
        has_fn: |world, entity| world.get::<AudioListener>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(AudioListener { active: true });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<AudioListener>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<AudioListener>(entity)
                .map(|l| l.active)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, enabled| {
            if let Some(mut l) = world.get_mut::<AudioListener>(entity) {
                l.active = enabled;
            }
        }),
        fields: vec![FieldDef {
            name: "Active",
            field_type: FieldType::Bool,
            get_fn: |world, entity| {
                world
                    .get::<AudioListener>(entity)
                    .map(|l| FieldValue::Bool(l.active))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Bool(v) = val {
                    if let Some(mut l) = world.get_mut::<AudioListener>(entity) {
                        l.active = v;
                    }
                }
            },
        }],
    }
}

// ── Native (ember) Audio Player drawer ───────────────────────────────────────

use bevy::ecs::world::CommandQueue;
use renzora_audio::AudioPlayer as ApComp;
use renzora_editor::AppEditorExt;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{inspector_row, inspector_stripe};
use renzora_ember::reactive::{bind_2way, bind_display};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::widgets::{drag_value, dropdown, slider, toggle_switch as ember_toggle, DragRange};
use renzora_inspector::asset_drop_field;

pub fn register_audio_native(app: &mut App) {
    app.register_native_inspector_ui("audio_player", audio_player_native);
    app.add_systems(
        Update,
        (rebuild_audio, audio_remove_clip_click).run_if(in_state(renzora_editor::SplashState::Editor)),
    );
}

const AUDIO_EXTS: [&str; 4] = ["ogg", "wav", "mp3", "flac"];

#[derive(Component)]
struct AudioRoot {
    entity: Entity,
    sig: Option<u64>,
}
#[derive(Component)]
struct AudioRemoveClip {
    entity: Entity,
    index: usize,
}

// Field accessors (fn-pointers so the bind closures stay `Copy`).
fn g_volume(d: &ApComp) -> f32 { d.volume }
fn s_volume(d: &mut ApComp, v: f32) { d.volume = v; }
fn g_vol_jitter(d: &ApComp) -> f32 { d.volume_jitter }
fn s_vol_jitter(d: &mut ApComp, v: f32) { d.volume_jitter = v; }
fn g_pitch(d: &ApComp) -> f32 { d.pitch }
fn s_pitch(d: &mut ApComp, v: f32) { d.pitch = v; }
fn g_pitch_jitter(d: &ApComp) -> f32 { d.pitch_jitter }
fn s_pitch_jitter(d: &mut ApComp, v: f32) { d.pitch_jitter = v; }
fn g_panning(d: &ApComp) -> f32 { d.panning }
fn s_panning(d: &mut ApComp, v: f32) { d.panning = v; }
fn g_reverb(d: &ApComp) -> f32 { d.reverb_send }
fn s_reverb(d: &mut ApComp, v: f32) { d.reverb_send = v; }
fn g_delay(d: &ApComp) -> f32 { d.delay_send }
fn s_delay(d: &mut ApComp, v: f32) { d.delay_send = v; }
fn g_fade(d: &ApComp) -> f32 { d.fade_in }
fn s_fade(d: &mut ApComp, v: f32) { d.fade_in = v; }
fn g_min(d: &ApComp) -> f32 { d.spatial_min_distance }
fn s_min(d: &mut ApComp, v: f32) { d.spatial_min_distance = v; }
fn g_max(d: &ApComp) -> f32 { d.spatial_max_distance }
fn s_max(d: &mut ApComp, v: f32) { d.spatial_max_distance = v; }
fn g_autoplay(d: &ApComp) -> bool { d.autoplay }
fn s_autoplay(d: &mut ApComp, v: bool) { d.autoplay = v; }
fn g_looping(d: &ApComp) -> bool { d.looping }
fn s_looping(d: &mut ApComp, v: bool) { d.looping = v; }
fn g_spatial(d: &ApComp) -> bool { d.spatial }
fn s_spatial(d: &mut ApComp, v: bool) { d.spatial = v; }

fn audio_clip_get(w: &World, e: Entity) -> Option<FieldValue> {
    w.get::<ApComp>(e).map(|d| FieldValue::Asset(if d.clip.is_empty() { None } else { Some(d.clip.clone()) }))
}
fn audio_clip_set(w: &mut World, e: Entity, v: FieldValue) {
    if let FieldValue::Asset(p) = v {
        if let Some(mut d) = w.get_mut::<ApComp>(e) {
            d.clip = p.unwrap_or_default();
        }
    }
}
fn audio_pool_get(_w: &World, _e: Entity) -> Option<FieldValue> {
    Some(FieldValue::Asset(None))
}
fn audio_pool_add(w: &mut World, e: Entity, v: FieldValue) {
    if let FieldValue::Asset(Some(p)) = v {
        if let Some(mut d) = w.get_mut::<ApComp>(e) {
            d.clips.push(p);
        }
    }
}

fn audio_player_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(2.0)), ..default() },
            AudioRoot { entity, sig: None },
            Name::new("audio-player-inspector-root"),
        ))
        .id()
}

/// Built-in buses + any custom mixer buses (snapshot).
fn bus_names(world: &World) -> Vec<String> {
    let mut buses = vec!["Master".to_string(), "Sfx".to_string(), "Music".to_string(), "Ambient".to_string()];
    if let Some(mixer) = world.get_resource::<MixerState>() {
        for (name, _) in &mixer.custom_buses {
            buses.push(name.clone());
        }
    }
    buses
}

/// Rebuild rows when the clip-pool length or the available bus count changes.
fn rebuild_audio(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let buses = bus_names(world);
    let mut q = world.query::<(Entity, &AudioRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let Some(data) = world.get::<ApComp>(entity).cloned() else { continue };
        let sig = data.clips.len() as u64 | ((buses.len() as u64) << 32);
        if old_sig == Some(sig) {
            continue;
        }
        let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            build_audio_body(&mut commands, &fonts, root, entity, &data, &buses);
        }
        queue.apply(world);
        if let Some(mut ar) = world.get_mut::<AudioRoot>(root) {
            ar.sig = Some(sig);
        }
    }
}

fn audio_header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let h = commands
        .spawn(Node { margin: UiRect { top: Val::Px(6.0), bottom: Val::Px(1.0), ..default() }, ..default() })
        .id();
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(h).add_child(t);
    h
}

fn audio_slider_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    label: &str,
    getf: fn(&ApComp) -> f32,
    setf: fn(&mut ApComp, f32),
    min: f32,
    max: f32,
) -> Entity {
    let s = slider(commands, 0.0);
    bind_2way(
        commands,
        s,
        move |w| {
            let v = w.get::<ApComp>(entity).map(getf).unwrap_or(min);
            ((v - min) / (max - min)).clamp(0.0, 1.0)
        },
        move |w, t: &f32| {
            if let Some(mut d) = w.get_mut::<ApComp>(entity) {
                setf(&mut d, min + *t * (max - min));
            }
        },
    );
    inspector_row(commands, &fonts.ui, label, s)
}

fn audio_drag_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    label: &str,
    getf: fn(&ApComp) -> f32,
    setf: fn(&mut ApComp, f32),
    min: f32,
    max: f32,
    step: f32,
) -> Entity {
    let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(
        commands,
        dv,
        move |w| w.get::<ApComp>(entity).map(getf).unwrap_or(min),
        move |w, v: &f32| {
            if let Some(mut d) = w.get_mut::<ApComp>(entity) {
                setf(&mut d, *v);
            }
        },
    );
    inspector_row(commands, &fonts.ui, label, dv)
}

fn audio_toggle_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    label: &str,
    getf: fn(&ApComp) -> bool,
    setf: fn(&mut ApComp, bool),
) -> Entity {
    let t = ember_toggle(commands, false);
    bind_2way(
        commands,
        t,
        move |w| w.get::<ApComp>(entity).map(getf).unwrap_or(false),
        move |w, v: &bool| {
            if let Some(mut d) = w.get_mut::<ApComp>(entity) {
                setf(&mut d, *v);
            }
        },
    );
    inspector_row(commands, &fonts.ui, label, t)
}

fn audio_pool_row(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, index: usize, clip: &str) -> Entity {
    let name = clip.rsplit(['/', '\\']).next().unwrap_or(clip).to_string();
    let ctrl = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();
    let trash = commands
        .spawn((
            Node { padding: UiRect::all(Val::Px(2.0)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            Interaction::default(),
            AudioRemoveClip { entity, index },
            Name::new("audio-clip-remove"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 13.0);
    commands.entity(trash).add_child(ic);
    let label = commands
        .spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb((210, 210, 220)))))
        .id();
    commands.entity(ctrl).add_children(&[trash, label]);
    inspector_row(commands, &fonts.ui, "", ctrl)
}

#[allow(clippy::too_many_lines)]
fn build_audio_body(commands: &mut Commands, fonts: &EmberFonts, root: Entity, entity: Entity, data: &ApComp, buses: &[String]) {
    let mut children: Vec<Entity> = Vec::new();
    let mut stripe = 0usize;
    let exts: Vec<String> = AUDIO_EXTS.iter().map(|s| s.to_string()).collect();

    let field = |commands: &mut Commands, row: Entity, stripe: &mut usize| {
        commands.entity(row).insert(BackgroundColor(inspector_stripe(*stripe)));
        *stripe += 1;
        row
    };

    // ── Clip ──
    children.push(audio_header(commands, fonts, "Clip"));
    let file = asset_drop_field(commands, fonts, entity, audio_clip_get, audio_clip_set, exts.clone());
    let r = inspector_row(commands, &fonts.ui, "File", file);
    children.push(field(commands, r, &mut stripe));

    // ── Clip Pool ──
    children.push(audio_header(commands, fonts, "Clip Pool"));
    for (i, clip) in data.clips.iter().enumerate() {
        let r = audio_pool_row(commands, fonts, entity, i, clip);
        children.push(field(commands, r, &mut stripe));
    }
    let add = asset_drop_field(commands, fonts, entity, audio_pool_get, audio_pool_add, exts.clone());
    let r = inspector_row(commands, &fonts.ui, "Add", add);
    children.push(field(commands, r, &mut stripe));

    // ── Playback ──
    children.push(audio_header(commands, fonts, "Playback"));
    let r = audio_toggle_row(commands, fonts, entity, "Autoplay", g_autoplay, s_autoplay);
    children.push(field(commands, r, &mut stripe));
    let r = audio_toggle_row(commands, fonts, entity, "Looping", g_looping, s_looping);
    children.push(field(commands, r, &mut stripe));
    let r = audio_drag_row(commands, fonts, entity, "Fade In", g_fade, s_fade, 0.0, 10.0, 0.05);
    children.push(field(commands, r, &mut stripe));

    // ── Mix ──
    children.push(audio_header(commands, fonts, "Mix"));
    let r = audio_slider_row(commands, fonts, entity, "Volume", g_volume, s_volume, 0.0, 2.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Vol Jitter", g_vol_jitter, s_vol_jitter, 0.0, 1.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Pitch", g_pitch, s_pitch, 0.1, 4.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Pitch Jitter", g_pitch_jitter, s_pitch_jitter, 0.0, 0.5);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Panning", g_panning, s_panning, -1.0, 1.0);
    children.push(field(commands, r, &mut stripe));
    // Bus dropdown.
    let labels: Vec<&str> = buses.iter().map(|s| s.as_str()).collect();
    let sel = buses.iter().position(|b| *b == data.bus).unwrap_or(0);
    let dd = dropdown(commands, fonts, &labels, sel);
    let buses_a = buses.to_vec();
    let buses_b = buses.to_vec();
    bind_2way(
        commands,
        dd,
        move |w| {
            let cur = w.get::<ApComp>(entity).map(|d| d.bus.clone()).unwrap_or_default();
            buses_a.iter().position(|b| *b == cur).unwrap_or(0)
        },
        move |w, i: &usize| {
            if let Some(name) = buses_b.get(*i).cloned() {
                if let Some(mut d) = w.get_mut::<ApComp>(entity) {
                    d.bus = name;
                }
            }
        },
    );
    let r = inspector_row(commands, &fonts.ui, "Bus", dd);
    children.push(field(commands, r, &mut stripe));

    // ── Spatial ──
    children.push(audio_header(commands, fonts, "Spatial"));
    let r = audio_toggle_row(commands, fonts, entity, "Enabled", g_spatial, s_spatial);
    children.push(field(commands, r, &mut stripe));
    // Conditional rows — shown only while spatial is enabled.
    let r_min = audio_drag_row(commands, fonts, entity, "Min Distance", g_min, s_min, 0.01, 1000.0, 0.1);
    bind_display(commands, r_min, move |w| w.get::<ApComp>(entity).map(g_spatial).unwrap_or(false));
    children.push(field(commands, r_min, &mut stripe));
    let r_max = audio_drag_row(commands, fonts, entity, "Max Distance", g_max, s_max, 0.1, 10000.0, 0.5);
    bind_display(commands, r_max, move |w| w.get::<ApComp>(entity).map(g_spatial).unwrap_or(false));
    children.push(field(commands, r_max, &mut stripe));
    let roll = dropdown(commands, fonts, ROLLOFF_LABELS, rolloff_to_index(&data.spatial_rolloff));
    bind_2way(
        commands,
        roll,
        move |w| w.get::<ApComp>(entity).map(|d| rolloff_to_index(&d.spatial_rolloff)).unwrap_or(0),
        move |w, i: &usize| {
            if let Some(mut d) = w.get_mut::<ApComp>(entity) {
                d.spatial_rolloff = rolloff_from_index(*i);
            }
        },
    );
    let r_roll = inspector_row(commands, &fonts.ui, "Rolloff", roll);
    bind_display(commands, r_roll, move |w| w.get::<ApComp>(entity).map(g_spatial).unwrap_or(false));
    children.push(field(commands, r_roll, &mut stripe));

    // ── Sends ──
    children.push(audio_header(commands, fonts, "Sends"));
    let r = audio_slider_row(commands, fonts, entity, "Reverb", g_reverb, s_reverb, 0.0, 1.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Delay", g_delay, s_delay, 0.0, 1.0);
    children.push(field(commands, r, &mut stripe));

    commands.entity(root).add_children(&children);
}

fn audio_remove_clip_click(q: Query<(&Interaction, &AudioRemoveClip), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (e, i) = (b.entity, b.index);
        commands.queue(move |w: &mut World| {
            if let Some(mut d) = w.get_mut::<ApComp>(e) {
                if i < d.clips.len() {
                    d.clips.remove(i);
                }
            }
        });
    }
}
