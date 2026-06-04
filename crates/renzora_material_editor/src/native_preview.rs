//! Bevy-native (ember) port of the egui `MaterialPreviewPanel`: the render-to-
//! texture material sphere/cube preview with a toolbar (shape selector, auto-
//! rotate, light/dark background) and mouse-orbit interaction — drag to rotate,
//! wheel to zoom — writing back into `MaterialPreviewOrbit`.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_editor::SplashState;
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, bind_with};
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu};

use crate::preview::{MaterialPreviewImage, MaterialPreviewOrbit, PreviewShape};
use crate::MaterialEditorState;

pub struct NativeMaterialPreview;

impl Plugin for NativeMaterialPreview {
    fn build(&self, app: &mut App) {
        app.register_panel_content("material_preview", false, build);
        app.add_systems(
            Update,
            (mat_btn_click, shape_combo_open, orbit_drag, orbit_zoom).run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Component, Clone, Copy)]
enum MatBtn {
    AutoRotate,
    Background,
}
#[derive(Component)]
struct ShapeCombo;
#[derive(Component)]
struct OrbitTarget;

fn orbit(w: &World) -> Option<&MaterialPreviewOrbit> {
    w.get_resource::<MaterialPreviewOrbit>()
}
fn has_material(w: &World) -> bool {
    w.get_resource::<MaterialEditorState>().is_some_and(|s| s.compiled_wgsl.is_some())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("native-material-preview"),
        ))
        .id();

    // Empty state.
    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() })
        .id();
    let note_lbl = commands.spawn((Text::new("No material compiled"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    commands.entity(note).add_child(note_lbl);
    bind_display(commands, note, |w| !has_material(w));

    // Body: toolbar + image.
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    bind_display(commands, body, has_material);

    let toolbar = build_toolbar(commands, fonts);

    let img_box = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, overflow: Overflow::clip(), ..default() })
        .id();
    let img = commands
        .spawn((
            ImageNode::default(),
            Node { height: Val::Percent(100.0), aspect_ratio: Some(1.0), ..default() },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            Interaction::default(),
            RelativeCursorPosition::default(),
            OrbitTarget,
            Name::new("material-preview-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<MaterialPreviewImage>().map(|p| p.handle.clone()),
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
    commands.entity(root).add_children(&[note, body]);
    renzora_editor::mark_drop_zone(commands, root);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), ..default() })
        .id();

    // Shape selector.
    let combo = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            ShapeCombo,
        ))
        .id();
    // Current shape glyph (raw phosphor char from PreviewShape::icon()).
    let glyph = commands.spawn((Text::new(""), ui_font(&fonts.phosphor, 11.0), TextColor(rgb(text_muted())))).id();
    bind_with(commands, glyph, |w| orbit(w).map(|o| o.shape.icon().to_string()).unwrap_or_default(), |w, e, s: &String| {
        if let Some(mut t) = w.get_mut::<Text>(e) {
            if t.0 != *s {
                t.0 = s.clone();
            }
        }
    });
    let label = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, label, |w| orbit(w).map(|o| o.shape.label().to_string()).unwrap_or_default());
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[glyph, label, caret]);

    let sep = commands.spawn((Node { width: Val::Px(1.0), height: Val::Px(14.0), margin: UiRect::horizontal(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(border())))).id();

    // Auto-rotate toggle (green when on).
    let (rotate, rotate_ic) = icon_btn(commands, fonts, "arrows-clockwise", text_muted(), MatBtn::AutoRotate);
    bind_text_color(commands, rotate_ic, |w| {
        let on = orbit(w).is_some_and(|o| o.auto_rotate);
        rgb(if on { play_green() } else { text_muted() })
    });

    // Background toggle (moon when dark, sun when light).
    let (bg, bg_ic) = icon_btn(commands, fonts, "moon", text_muted(), MatBtn::Background);
    bind_with(commands, bg_ic, |w| orbit(w).map(|o| o.dark_bg).unwrap_or(true), |w, e, dark: &bool| {
        if let Some(g) = icon_glyph(if *dark { "moon" } else { "sun" }) {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                let s = g.to_string();
                if t.0 != s {
                    t.0 = s;
                }
            }
        }
    });

    commands.entity(bar).add_children(&[combo, sep, rotate, bg]);
    bar
}

fn icon_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> (Entity, Entity) {
    let btn = commands
        .spawn((Node { width: Val::Px(22.0), height: Val::Px(18.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 11.0);
    commands.entity(btn).add_child(ic);
    (btn, ic)
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn mat_btn_click(q: Query<(&Interaction, &MatBtn), Changed<Interaction>>, orbit: Option<ResMut<MaterialPreviewOrbit>>) {
    let Some(mut orbit) = orbit else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            MatBtn::AutoRotate => orbit.auto_rotate = !orbit.auto_rotate,
            MatBtn::Background => orbit.dark_bg = !orbit.dark_bg,
        }
    }
}

fn shape_combo_open(
    q: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode), (With<ShapeCombo>, Changed<Interaction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = PreviewShape::ALL
        .iter()
        .map(|&shape| {
            menu_item(&mut commands, &fonts, "cube", shape.label(), move |w| {
                if let Some(mut o) = w.get_resource_mut::<MaterialPreviewOrbit>() {
                    o.shape = shape;
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn orbit_drag(
    windows: Query<&Window>,
    mut last: Local<Option<Vec2>>,
    q: Query<&Interaction, With<OrbitTarget>>,
    orbit: Option<ResMut<MaterialPreviewOrbit>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        *last = None;
        return;
    }
    let Some(c) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    if let Some(prev) = *last {
        if let Some(mut orbit) = orbit {
            let d = c - prev;
            if d != Vec2::ZERO {
                orbit.yaw += d.x * 0.01;
                orbit.pitch = (orbit.pitch - d.y * 0.01).clamp(-1.4, 1.4);
            }
        }
    }
    *last = Some(c);
}

fn orbit_zoom(mut wheel: MessageReader<MouseWheel>, q: Query<&RelativeCursorPosition, With<OrbitTarget>>, orbit: Option<ResMut<MaterialPreviewOrbit>>) {
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
        orbit.distance = (orbit.distance - dy * 0.3).clamp(1.5, 10.0);
    }
}
