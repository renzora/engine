//! Inspector entries for audio components (AudioPlayer, AudioListener).

use bevy::prelude::*;
use renzora_audio::{AudioListener, AudioPlayer, MixerState, RolloffType};
use renzora::{
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
        icon: "speaker-high",
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
        icon: "ear",
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
use renzora::AppEditorExt;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{inspector_row, inspector_stripe};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text};
use renzora_ember::theme::{hover_bg, rgb, text_muted, text_primary, value_text};
use renzora_ember::widgets::{
    drag_value, dropdown, knob_pivoted, slider, toggle_switch as ember_toggle, DragRange,
    HoverTooltip,
};
use renzora_inspector::asset_drop_field;

pub fn register_audio_native(app: &mut App) {
    app.register_native_inspector_ui("audio_player", audio_player_native);
    app.add_systems(
        Update,
        (rebuild_audio, audio_remove_clip_click, audio_reset_click).run_if(in_state(renzora::SplashState::Editor)),
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
        // Keys, not display names: the chosen string lands in
        // `AudioPlayer.bus`, which is a routing key.
        for bus in &mixer.custom_buses {
            buses.push(bus.key.clone());
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

/// Restores one `AudioPlayer` field to the value a fresh component has.
///
/// The setter and the default travel in the component because both are plain
/// data — a `fn` pointer and an `f32` — which lets one system reset any field
/// rather than needing one marker type per field.
#[derive(Component)]
struct AudioFieldReset {
    entity: Entity,
    set: fn(&mut ApComp, f32),
    default: f32,
}

/// Click a reset arrow, put the field back to its default.
fn audio_reset_click(
    q: Query<(&Interaction, &AudioFieldReset), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, reset) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, set, default) = (reset.entity, reset.set, reset.default);
        commands.queue(move |world: &mut World| {
            if let Some(mut data) = world.get_mut::<ApComp>(entity) {
                set(&mut data, default);
            }
        });
    }
}

/// A small reset arrow, shown only while the field differs from its default.
///
/// Hidden at the default rather than merely greyed out: a column of dead arrows
/// is noise on a drawer this long, and their appearing is itself the useful
/// signal — it marks which fields have been touched.
fn reset_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    getf: fn(&ApComp) -> f32,
    setf: fn(&mut ApComp, f32),
    // Not named `default`: that shadows bevy's `default()` inside the `Node`
    // literals below, and the error it produces points at the struct rather
    // than at the parameter.
    reset_to: f32,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            AudioFieldReset { entity, set: setf, default: reset_to },
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("Reset to default"),
            Name::new("audio-field-reset"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, "arrow-counter-clockwise", text_muted(), 11.0);
    commands.entity(btn).add_child(glyph);
    bind_bg(commands, btn, move |rx| match rx.get::<Interaction>(btn) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
    bind_display(commands, btn, move |rx| {
        rx.get::<ApComp>(entity)
            .map(getf)
            .is_some_and(|v| (v - reset_to).abs() > f32::EPSILON)
    });
    btn
}

/// The live value beside a control.
///
/// A slider with no number tells you where the handle is, not what the value is,
/// and "somewhere near the middle" is not a setting anyone can reproduce or talk
/// about. Mono-spaced so the row does not jitter as digits change width.
fn value_label(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    getf: fn(&ApComp) -> f32,
    fmt: fn(f32) -> String,
) -> Entity {
    let t = commands
        .spawn((
            Node { min_width: Val::Px(34.0), flex_shrink: 0.0, ..default() },
            Text::new(""),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    bind_text(commands, t, move |rx| {
        fmt(rx.get::<ApComp>(entity).map(getf).unwrap_or_default())
    });
    t
}

/// Wrap a control with its readout and reset arrow.
#[allow(clippy::too_many_arguments)]
fn with_readout(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    control: Entity,
    getf: fn(&ApComp) -> f32,
    setf: fn(&mut ApComp, f32),
    reset_to: f32,
    fmt: fn(f32) -> String,
) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    let readout = value_label(commands, fonts, entity, getf, fmt);
    let reset = reset_button(commands, fonts, entity, getf, setf, reset_to);
    commands.entity(row).add_children(&[control, readout, reset]);
    row
}

fn fmt_plain(v: f32) -> String {
    format!("{v:.2}")
}

/// Panning as a mixing desk writes it: `C`, `L42`, `R100`.
///
/// A bare `-0.42` has to be decoded every time it is read; which side it is on
/// is the thing you actually want to know.
fn fmt_pan(v: f32) -> String {
    let pct = (v.abs() * 100.0).round() as i32;
    if pct == 0 {
        String::from("C")
    } else if v < 0.0 {
        format!("L{pct}")
    } else {
        format!("R{pct}")
    }
}

#[allow(clippy::too_many_arguments)]
fn audio_slider_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    label: &str,
    getf: fn(&ApComp) -> f32,
    setf: fn(&mut ApComp, f32),
    min: f32,
    max: f32,
    reset_to: f32,
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
    let control = with_readout(commands, fonts, entity, s, getf, setf, reset_to, fmt_plain);
    inspector_row(commands, &fonts.ui, label, control)
}

/// The panning row: a knob rather than a slider, matching the mixer.
///
/// Pan is the one control here that is not a magnitude — it has a centre and two
/// directions away from it. A knob says that; a left-to-right slider says "more
/// of something".
fn audio_pan_row(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let k = knob_pivoted(commands, 0.5, 0.5);
    commands.queue(move |w: &mut World| {
        if let Some(mut n) = w.get_mut::<Node>(k) {
            n.width = Val::Px(28.0);
            n.height = Val::Px(28.0);
        }
    });
    bind_2way(
        commands,
        k,
        move |w| {
            let v = w.get::<ApComp>(entity).map(g_panning).unwrap_or(0.0);
            ((v + 1.0) / 2.0).clamp(0.0, 1.0)
        },
        move |w, t: &f32| {
            if let Some(mut d) = w.get_mut::<ApComp>(entity) {
                s_panning(&mut d, *t * 2.0 - 1.0);
            }
        },
    );
    let control = with_readout(commands, fonts, entity, k, g_panning, s_panning, 0.0, fmt_pan);
    inspector_row(commands, &fonts.ui, "Panning", control)
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
    let r = audio_slider_row(commands, fonts, entity, "Volume", g_volume, s_volume, 0.0, 2.0, 1.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Vol Jitter", g_vol_jitter, s_vol_jitter, 0.0, 1.0, 0.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Pitch", g_pitch, s_pitch, 0.1, 4.0, 1.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Pitch Jitter", g_pitch_jitter, s_pitch_jitter, 0.0, 0.5, 0.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_pan_row(commands, fonts, entity);
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
    let r = audio_slider_row(commands, fonts, entity, "Reverb", g_reverb, s_reverb, 0.0, 1.0, 0.0);
    children.push(field(commands, r, &mut stripe));
    let r = audio_slider_row(commands, fonts, entity, "Delay", g_delay, s_delay, 0.0, 1.0, 0.0);
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
