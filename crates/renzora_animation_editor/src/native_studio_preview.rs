//! Bevy-native (ember) port of the egui `StudioPreviewPanel`: the isolated
//! animation render-to-texture preview (`StudioPreviewImage.handle`) with a
//! vertical toolbar (skeleton / floor / wireframe toggles + reset-camera) and
//! mouse-orbit interaction — drag to rotate, wheel to zoom — writing back into
//! `StudioPreviewOrbit` / `StudioPreviewSettings`. An empty "select an animated
//! entity" state shows when there is no preview texture yet.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text_color, bind_with};
use renzora_ember::theme::*;

use crate::studio_preview::{
    StudioPreviewImage, StudioPreviewOrbit, StudioPreviewSettings, StudioPreviewTracker,
};

pub struct NativeStudioPreview;

impl Plugin for NativeStudioPreview {
    fn build(&self, app: &mut App) {
        app.register_panel_content("studio_preview", false, build);
        app.add_systems(
            Update,
            (tool_btn_click, orbit_drag, orbit_zoom).run_if(in_state(SplashState::Editor)),
        );
    }
}

/// Toolbar actions — mirror the egui panel's `action` codes.
#[derive(Component, Clone, Copy)]
enum ToolBtn {
    Skeleton,
    Floor,
    Wireframe,
    ResetCamera,
}

#[derive(Component)]
struct OrbitTarget;

fn settings(w: &World) -> Option<&StudioPreviewSettings> {
    w.get_resource::<StudioPreviewSettings>()
}

/// True once the offscreen preview image has a real (non-default) handle.
fn has_preview(w: &World) -> bool {
    w.get_resource::<StudioPreviewImage>()
        .is_some_and(|p| p.handle != Handle::default())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("native-studio-preview"),
        ))
        .id();

    // ── Empty state: shown until the preview texture exists ──
    let empty = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let empty_icon = icon_text(commands, &fonts.phosphor, "video-camera", text_muted(), 32.0);
    let empty_a = commands
        .spawn((
            Text::new("Select an animated entity"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let empty_b = commands
        .spawn((
            Text::new("to preview animations here"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands
        .entity(empty)
        .add_children(&[empty_icon, empty_a, empty_b]);
    bind_display(commands, empty, |w| !has_preview(w));

    // ── Body: vertical toolbar + image ──
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    bind_display(commands, body, has_preview);

    let toolbar = build_toolbar(commands, fonts);

    let img_box = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.14)),
            Interaction::default(),
            RelativeCursorPosition::default(),
            OrbitTarget,
            Name::new("studio-preview-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<StudioPreviewImage>().map(|p| p.handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );
    commands.entity(img_box).add_child(img);

    commands.entity(body).add_children(&[toolbar, img_box]);
    commands.entity(root).add_children(&[empty, body]);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    const TOOLBAR_WIDTH: f32 = 36.0;
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(TOOLBAR_WIDTH),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(Val::Px(4.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(30, 30, 35)),
            BorderColor::all(Color::srgb_u8(50, 50, 55)),
        ))
        .id();

    let skeleton = tool_button(commands, fonts, "bone", ToolBtn::Skeleton, |s| s.show_skeleton);
    let floor = tool_button(commands, fonts, "grid-four", ToolBtn::Floor, |s| s.show_floor);
    let wireframe = tool_button(commands, fonts, "cube", ToolBtn::Wireframe, |s| s.show_wireframe);
    let reset = tool_button(commands, fonts, "crosshair", ToolBtn::ResetCamera, |_| false);

    commands
        .entity(bar)
        .add_children(&[skeleton, floor, wireframe, reset]);
    bar
}

/// A 28×28 toolbar toggle button whose icon turns accent-colored while `active`.
fn tool_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    marker: ToolBtn,
    active: fn(&StudioPreviewSettings) -> bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 16.0);
    bind_text_color(commands, ic, move |w| {
        let on = settings(w).is_some_and(active);
        rgb(if on { accent() } else { text_muted() })
    });
    commands.entity(btn).add_child(ic);
    btn
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn tool_btn_click(
    q: Query<(&Interaction, &ToolBtn), Changed<Interaction>>,
    settings: Option<ResMut<StudioPreviewSettings>>,
    orbit: Option<ResMut<StudioPreviewOrbit>>,
    tracker: Option<ResMut<StudioPreviewTracker>>,
) {
    let Some(mut settings) = settings else { return };
    let mut orbit = orbit;
    let mut tracker = tracker;
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            ToolBtn::Skeleton => settings.show_skeleton = !settings.show_skeleton,
            ToolBtn::Floor => settings.show_floor = !settings.show_floor,
            ToolBtn::Wireframe => settings.show_wireframe = !settings.show_wireframe,
            ToolBtn::ResetCamera => {
                if let Some(orbit) = orbit.as_mut() {
                    orbit.yaw = 0.5;
                    orbit.pitch = 0.2;
                }
                if let Some(tracker) = tracker.as_mut() {
                    tracker.auto_fitted = false;
                }
            }
        }
    }
}

fn orbit_drag(
    windows: Query<&Window>,
    mut last: Local<Option<Vec2>>,
    q: Query<&Interaction, With<OrbitTarget>>,
    orbit: Option<ResMut<StudioPreviewOrbit>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        *last = None;
        return;
    }
    let Some(c) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    if let Some(prev) = *last {
        if let Some(mut orbit) = orbit {
            let d = c - prev;
            if d != Vec2::ZERO {
                // Match the egui panel: yaw -= dx * 0.005, pitch clamp -1.5..1.5.
                orbit.yaw -= d.x * 0.005;
                orbit.pitch = (orbit.pitch + d.y * 0.005).clamp(-1.5, 1.5);
            }
        }
    }
    *last = Some(c);
}

fn orbit_zoom(
    mut wheel: MessageReader<MouseWheel>,
    q: Query<&RelativeCursorPosition, With<OrbitTarget>>,
    orbit: Option<ResMut<StudioPreviewOrbit>>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    if !q.iter().any(|r| r.cursor_over) {
        return;
    }
    if let Some(mut orbit) = orbit {
        // Multiplicative zoom, clamped 0.2..50 — scroll up = closer.
        let zoom_factor = 1.0 - dy * 0.1;
        orbit.distance = (orbit.distance * zoom_factor).clamp(0.2, 50.0);
    }
}
