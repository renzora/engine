//! Bevy-native (ember) Record panel: a record/stop button + live status,
//! ffmpeg-readiness notices, and Source/FPS/Quality/Preset combo rows. The
//! capture/encode machinery is unchanged (backend-agnostic systems); this only
//! renders the panel UI, reading the recording resources and pushing the
//! `start_recording` / `request_stop` / config-change commands.

use bevy::prelude::*;

use renzora_editor::{EditorCommands, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu};

use crate::{
    request_stop, start_recording, FfmpegReadiness, FfmpegState, InnerSnapshot, RecordTarget,
    RecordingConfig, RecordingState,
};

const RED: (u8, u8, u8) = (220, 60, 60);

const TARGETS: [(RecordTarget, &str); 2] = [
    (RecordTarget::Viewport, "Viewport (3D scene only)"),
    (RecordTarget::Window, "Entire window (full editor)"),
];
const FPS: [u32; 4] = [24, 30, 60, 120];
const CRF: [(u8, &str); 5] = [
    (0, "Lossless (huge files)"),
    (12, "Archival"),
    (17, "Visually lossless"),
    (20, "High"),
    (23, "Default"),
];
const PRESETS: [&str; 5] = ["ultrafast", "fast", "medium", "slow", "veryslow"];

pub struct NativeRecordPanel;

impl Plugin for NativeRecordPanel {
    fn build(&self, app: &mut App) {
        app.register_panel_content("record", true, build);
        app.add_systems(
            Update,
            (record_btn_click, stop_btn_click, record_combo_open).run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State predicates (read the shared recording resources) ───────────────────

fn snap(w: &World) -> InnerSnapshot {
    w.get_resource::<RecordingState>()
        .and_then(|s| s.inner.lock().ok().map(|g| InnerSnapshot::from(&*g)))
        .unwrap_or(InnerSnapshot::Idle)
}

fn ffmpeg_ready(w: &World) -> bool {
    matches!(w.get_resource::<FfmpegReadiness>().map(|r| r.snapshot()), Some(FfmpegState::Ready))
}
fn ffmpeg_preparing(w: &World) -> bool {
    matches!(w.get_resource::<FfmpegReadiness>().map(|r| r.snapshot()), Some(FfmpegState::Preparing))
}
fn ffmpeg_failed(w: &World) -> Option<String> {
    match w.get_resource::<FfmpegReadiness>().map(|r| r.snapshot()) {
        Some(FfmpegState::Failed(e)) => Some(e),
        _ => None,
    }
}

fn is_idle(w: &World) -> bool {
    matches!(snap(w), InnerSnapshot::Idle | InnerSnapshot::Done(_))
}

#[derive(Component, Clone, Copy)]
enum RecCombo {
    Source,
    Fps,
    Quality,
    Preset,
}
#[derive(Component)]
struct RecordBtn;
#[derive(Component)]
struct StopBtn;

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::all(Val::Px(8.0)), ..default() })
        .id();

    // Top row: record/stop button + status.
    let top = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();

    let record = pill_button(commands, fonts, "record", "Record", RED, RecordBtn);
    bind_display(commands, record, |w| is_idle(w) && ffmpeg_ready(w));
    let stop = pill_button(commands, fonts, "stop", "Stop", RED, StopBtn);
    bind_display(commands, stop, |w| matches!(snap(w), InnerSnapshot::Recording { .. }));
    let preparing = dim_button(commands, fonts, "Preparing ffmpeg…");
    bind_display(commands, preparing, |w| is_idle(w) && ffmpeg_preparing(w));
    let unavailable = dim_button(commands, fonts, "ffmpeg unavailable");
    bind_display(commands, unavailable, |w| ffmpeg_failed(w).is_some() && is_idle(w));
    let encoding = dim_button(commands, fonts, "Encoding…");
    bind_display(commands, encoding, |w| matches!(snap(w), InnerSnapshot::Stopping));

    let status = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, status, |w| match snap(w) {
        InnerSnapshot::Recording { elapsed_secs, frame_index, size } => {
            format!("\u{25cf} REC  {}\n{} frames  \u{b7}  {}\u{d7}{}", fmt_elapsed(elapsed_secs), frame_index, size.x, size.y)
        }
        InnerSnapshot::Stopping => "Finalising file\u{2026}".to_string(),
        _ if ffmpeg_ready(w) => "Idle".to_string(),
        _ => String::new(),
    });
    bind_text_color(commands, status, |w| {
        if matches!(snap(w), InnerSnapshot::Recording { .. }) { rgb(RED) } else { rgb(text_muted()) }
    });
    commands.entity(top).add_children(&[record, stop, preparing, unavailable, encoding, status]);

    // ffmpeg failure alert.
    let alert = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(RED)), Node { flex_shrink: 0.0, ..default() }))
        .id();
    bind_text(commands, alert, |w| ffmpeg_failed(w).map(|e| format!("\u{26a0} ffmpeg unavailable: {e}")).unwrap_or_default());
    bind_display(commands, alert, |w| ffmpeg_failed(w).is_some());

    // Settings header + combos.
    let header = commands.spawn((Text::new("Settings"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    let combos = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    let source = combo_row(commands, fonts, "Source", RecCombo::Source, |w| {
        let t = w.resource::<RecordingConfig>().target;
        TARGETS.iter().find(|(v, _)| *v == t).map(|(_, l)| l.to_string()).unwrap_or_default()
    });
    let fps = combo_row(commands, fonts, "FPS", RecCombo::Fps, |w| w.resource::<RecordingConfig>().fps.to_string());
    let quality = combo_row(commands, fonts, "Quality", RecCombo::Quality, |w| {
        let c = w.resource::<RecordingConfig>().crf;
        CRF.iter().find(|(v, _)| *v == c).map(|(_, l)| l.to_string()).unwrap_or_default()
    });
    let preset = combo_row(commands, fonts, "Preset", RecCombo::Preset, |w| w.resource::<RecordingConfig>().preset.clone());
    commands.entity(combos).add_children(&[source, fps, quality, preset]);

    let note = commands
        .spawn((Text::new("Saved as MP4 (H.264) under <project>/recordings/"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder())), bevy::text::TextLayout::new_with_no_wrap()))
        .id();

    commands.entity(root).add_children(&[top, alert, header, combos, note]);
    root
}

fn pill_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, bg: (u8, u8, u8), marker: M) -> Entity {
    let btn = commands
        .spawn((
            Node { height: Val::Px(28.0), min_width: Val::Px(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), padding: UiRect::horizontal(Val::Px(10.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(bg)),
            Interaction::default(),
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, (255, 255, 255), 13.0);
    let lbl = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb((255, 255, 255))))).id();
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

fn dim_button(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Node { height: Val::Px(28.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::horizontal(Val::Px(10.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(card_bg())),
        ))
        .with_children(|p| {
            p.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))));
        })
        .id()
}

fn combo_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, kind: RecCombo, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();
    let l = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { width: Val::Px(64.0), flex_shrink: 0.0, ..default() })).id();
    let btn = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(input_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            kind,
        ))
        .id();
    let v = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() }, bevy::text::TextLayout::new_with_no_wrap())).id();
    bind_text(commands, v, value);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(btn).add_children(&[v, caret]);
    commands.entity(row).add_children(&[l, btn]);
    row
}

fn input_bg() -> (u8, u8, u8) {
    popup_bg()
}

fn fmt_elapsed(secs: f32) -> String {
    let t = secs as u64;
    format!("{:02}:{:02}", t / 60, t % 60)
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn record_btn_click(q: Query<&Interaction, (With<RecordBtn>, Changed<Interaction>)>, cmds: Option<Res<EditorCommands>>) {
    let Some(cmds) = cmds else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        cmds.push(|w: &mut World| start_recording(w));
    }
}

fn stop_btn_click(q: Query<&Interaction, (With<StopBtn>, Changed<Interaction>)>, cmds: Option<Res<EditorCommands>>) {
    let Some(cmds) = cmds else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        cmds.push(|w: &mut World| request_stop(w));
    }
}

fn record_combo_open(
    q: Query<(&Interaction, &RecCombo, &bevy::ui::RelativeCursorPosition, &bevy::ui::ComputedNode), Changed<Interaction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, kind, rcp, cn)) = q.iter().find(|(i, _, _, _)| **i == Interaction::Pressed) else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = match kind {
        RecCombo::Source => TARGETS
            .iter()
            .map(|&(v, label)| menu_item(&mut commands, &fonts, "monitor", label, move |w| set_cfg(w, |c| c.target = v)))
            .collect(),
        RecCombo::Fps => FPS
            .iter()
            .map(|&v| menu_item(&mut commands, &fonts, "gauge", &v.to_string(), move |w| set_cfg(w, |c| c.fps = v)))
            .collect(),
        RecCombo::Quality => CRF
            .iter()
            .map(|&(v, label)| menu_item(&mut commands, &fonts, "sliders", label, move |w| set_cfg(w, |c| c.crf = v)))
            .collect(),
        RecCombo::Preset => PRESETS
            .iter()
            .map(|&p| menu_item(&mut commands, &fonts, "timer", p, move |w| set_cfg(w, |c| c.preset = p.to_string())))
            .collect(),
    };
    commands.entity(menu).add_children(&kids);
}

fn set_cfg(world: &mut World, f: impl FnOnce(&mut RecordingConfig)) {
    if let Some(mut cfg) = world.get_resource_mut::<RecordingConfig>() {
        f(&mut cfg);
    }
}
