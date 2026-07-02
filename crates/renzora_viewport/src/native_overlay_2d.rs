//! Native (bevy_ui) 2D viewport chrome — rulers, selection outline + resize
//! handles, and a live cursor-coordinate readout.
//!
//! This is the screen-space half of the 2D editor: the world-space grid lives
//! in `renzora_gizmo::grid_2d` (drawn into the offscreen image *under* the
//! sprites), while everything here is painted *over* the rendered viewport as
//! flat, root-level absolute UI nodes in window coordinates — the same approach
//! `native_modal_hud` uses. We do it in `renzora_viewport` (not the gizmo crate)
//! for two reasons: Bevy's 2D gizmos can't draw text (rulers need tick labels),
//! and gizmos render at z=0 *beneath* sprites, whereas selection handles must
//! sit on top of whatever they're framing.
//!
//! Geometry comes from the editor 2D camera (`EditorCamera2d`) projected through
//! `ViewportState`'s panel rect, so the overlay tracks pan/zoom exactly. The
//! whole thing is rebuilt every frame (cheap — a few dozen nodes) and gated on
//! 2D view + edit mode, so it never bleeds into 3D or the runtime view.

use bevy::prelude::*;
use bevy::sprite::Sprite;
use bevy::text::FontSource;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::{EditorCamera2d, Node2d, PlayModeState};
use renzora_editor_framework::EditorSelection;
use renzora_ember::font::{ui_font, EmberFonts};

/// Marker on every node the 2D overlay spawns, so the whole thing can be
/// cleared in one query each frame before it's rebuilt.
#[derive(Component)]
struct Overlay2dPart;

/// Thickness of the ruler bars, in window pixels.
const RULER: f32 = 18.0;
/// Target spacing between labelled ruler ticks, in window pixels. The actual
/// step is rounded to a 1-2-5 sequence so labels land on readable values.
const TICK_TARGET_PX: f32 = 80.0;
/// Side length of the square resize handles, in window pixels. Matches the
/// `HANDLE_HIT_RADIUS` the picker uses so what you see is what you can grab.
const HANDLE_PX: f32 = 9.0;

/// Z-band for the 2D overlay chrome. Must sit above the dock content (the
/// rendered viewport image) but BELOW every popup/menu in the editor — ember
/// popovers/menus and the viewport header's dropdowns live at 500-700, so an
/// open toolbar dropdown must paint over the rulers, not under them.
/// Selection chrome sits BELOW the ruler bars: the outline of a selection
/// near the panel edge slides *under* the rulers instead of striping across
/// them (the rulers are frame chrome; nothing in-world may paint over them).
const Z_SELECTION: i32 = 90;
const Z_HANDLE: i32 = 91;
const Z_RULER_BAR: i32 = 100;
const Z_RULER_TICK: i32 = 101;
const Z_RULER_LABEL: i32 = 102;
const Z_CURSOR_READOUT: i32 = 110;

/// Distance of the rotate handle above the selection's top-center handle, in
/// window pixels. Matches `picker_2d::ROTATE_HANDLE_OFFSET_PX` so the drawn
/// handle is exactly where the picker hit-tests it.
const ROTATE_HANDLE_OFFSET_PX: f32 = 22.0;

/// Accent colour for selection chrome (matches the 3D outline orange).
const ACCENT: Color = Color::srgb(1.0, 0.6, 0.1);

pub(crate) fn register(app: &mut App) {
    // Not gated on `in_two_view`/`not_in_play_mode`: the system must run every
    // frame so it can *clear* its overlay when the view leaves 2D (otherwise the
    // last frame's rulers/handles linger in the 3D/UI/play views). It draws only
    // while actually in 2D edit mode — see the guard inside.
    app.add_systems(
        Update,
        render_overlay_2d.run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

/// Project a world-space point to window pixels through the 2D camera + panel
/// rect. Returns `None` if the point is off-screen or the panel has no size.
fn world_to_window(
    camera: &Camera,
    cam_gt: &GlobalTransform,
    vs: &ViewportState,
    world: Vec2,
) -> Option<Vec2> {
    let img = camera.world_to_viewport(cam_gt, world.extend(0.0)).ok()?;
    let image_size = vs.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return None;
    }
    let panel = Vec2::new(
        img.x * vs.screen_size.x / image_size.x,
        img.y * vs.screen_size.y / image_size.y,
    );
    Some(panel + vs.screen_position)
}

/// Inverse of [`world_to_window`]: window pixels → 2D world space.
fn window_to_world(
    camera: &Camera,
    cam_gt: &GlobalTransform,
    vs: &ViewportState,
    win: Vec2,
) -> Option<Vec2> {
    let image_size = vs.current_size.as_vec2();
    if vs.screen_size.x <= 0.0 || vs.screen_size.y <= 0.0 {
        return None;
    }
    let in_rect = win - vs.screen_position;
    let scaled = Vec2::new(
        in_rect.x * image_size.x / vs.screen_size.x,
        in_rect.y * image_size.y / vs.screen_size.y,
    );
    camera.viewport_to_world_2d(cam_gt, scaled).ok()
}

/// Round a raw spacing to the nearest 1-2-5×10ⁿ value so ruler ticks land on
/// human-readable world coordinates regardless of zoom.
fn nice_step(raw: f32) -> f32 {
    if !(raw > 0.0) {
        return 1.0;
    }
    let pow = 10f32.powf(raw.log10().floor());
    let frac = raw / pow;
    let mult = if frac < 1.5 {
        1.0
    } else if frac < 3.0 {
        2.0
    } else if frac < 7.0 {
        5.0
    } else {
        10.0
    };
    mult * pow
}

/// A sprite's effective render size in world units (custom_size → atlas rect →
/// native image size). `None` while the image is still loading.
fn sprite_size(sprite: &Sprite, images: &Assets<Image>) -> Option<Vec2> {
    if let Some(c) = sprite.custom_size {
        return Some(c);
    }
    if let Some(r) = sprite.rect {
        return Some(r.size());
    }
    let img = images.get(&sprite.image)?;
    Some(img.size_f32())
}

/// Format a world coordinate for a ruler label: whole numbers when the step is
/// ≥ 1, otherwise enough decimals to distinguish adjacent ticks.
fn fmt_coord(v: f32, step: f32) -> String {
    if step >= 1.0 {
        format!("{}", v.round() as i64)
    } else if step >= 0.1 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

#[allow(clippy::too_many_arguments)]
fn render_overlay_2d(
    mut commands: Commands,
    existing: Query<Entity, With<Overlay2dPart>>,
    viewport: Option<Res<ViewportState>>,
    settings: Option<Res<ViewportSettings>>,
    play: Option<Res<PlayModeState>>,
    fonts: Option<Res<EmberFonts>>,
    selection: Option<Res<EditorSelection>>,
    images: Res<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<EditorCamera2d>>,
    sprites: Query<(&GlobalTransform, &Sprite)>,
    node2ds: Query<&GlobalTransform, With<Node2d>>,
) {
    // Clear last frame's overlay first — unconditionally, so switching away from
    // 2D (or entering play) removes the rulers/handles instead of leaving them
    // stuck on screen.
    for e in &existing {
        commands.entity(e).despawn();
    }

    // Draw only in 2D view + edit mode.
    let (in_2d, show_rulers) = settings
        .map(|s| (s.viewport_view == ViewportView::Two, s.show_rulers_2d))
        .unwrap_or((false, true));
    let editing = !play.is_some_and(|p| p.is_in_play_mode());
    if !in_2d || !editing {
        return;
    }

    let Some(vs) = viewport else { return };
    // A workspace without a viewport panel leaves `screen_position`/`screen_size`
    // frozen at their last-docked values (the panel's resize requests stop), so
    // the rect alone can't tell us the panel is gone — `docked` can.
    if !vs.docked || vs.screen_size.x <= 1.0 || vs.screen_size.y <= 1.0 {
        return;
    }
    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        return;
    };

    let origin = vs.screen_position;
    let size = vs.screen_size;

    // ── Ruler bars (background) ──────────────────────────────────────────────
    // Toolbar-toggleable (`show_rulers_2d`, on by default). The cursor
    // coordinate readout below is NOT part of the toggle — it's useful chrome
    // even in a ruler-free view.
    let bar = Color::srgba(0.10, 0.11, 0.13, 0.92);
    let tick_col = Color::srgba(1.0, 1.0, 1.0, 0.35);
    if show_rulers {
        // Top ruler.
        commands.spawn((
            overlay_node(origin.x, origin.y, size.x, RULER),
            BackgroundColor(bar),
            GlobalZIndex(Z_RULER_BAR),
            bevy::ui::FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
            Overlay2dPart,
            Name::new("ruler-top"),
        ));
        // Left ruler.
        commands.spawn((
            overlay_node(origin.x, origin.y, RULER, size.y),
            BackgroundColor(bar),
            GlobalZIndex(Z_RULER_BAR),
            bevy::ui::FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
            Overlay2dPart,
            Name::new("ruler-left"),
        ));
    }

    // Visible world bounds from the two panel corners.
    let tl = window_to_world(camera, cam_gt, &vs, origin);
    let br = window_to_world(camera, cam_gt, &vs, origin + size);
    if let (true, Some(tl), Some(br)) = (show_rulers, tl, br) {
        let world_left = tl.x.min(br.x);
        let world_right = tl.x.max(br.x);
        let world_top = tl.y.max(br.y);
        let world_bottom = tl.y.min(br.y);

        let span_x = (world_right - world_left).max(1e-6);
        let span_y = (world_top - world_bottom).max(1e-6);
        let ppw_x = size.x / span_x;
        let ppw_y = size.y / span_y;
        let step_x = nice_step(TICK_TARGET_PX / ppw_x);
        let step_y = nice_step(TICK_TARGET_PX / ppw_y);

        let font = fonts.as_ref().map(|f| f.ui.clone());

        // Vertical ticks along the top ruler.
        let mut x = (world_left / step_x).ceil() * step_x;
        let mut guard = 0;
        while x <= world_right && guard < 512 {
            guard += 1;
            if let Some(win) = world_to_window(camera, cam_gt, &vs, Vec2::new(x, world_top)) {
                if win.x >= origin.x + RULER {
                    spawn_tick(&mut commands, win.x - 0.5, origin.y, 1.0, RULER, tick_col);
                    if let Some(ref f) = font {
                        spawn_label(
                            &mut commands,
                            f,
                            &fmt_coord(x, step_x),
                            win.x + 2.0,
                            origin.y + 2.0,
                        );
                    }
                }
            }
            x += step_x;
        }

        // Horizontal ticks along the left ruler.
        let mut y = (world_bottom / step_y).ceil() * step_y;
        guard = 0;
        while y <= world_top && guard < 512 {
            guard += 1;
            if let Some(win) = world_to_window(camera, cam_gt, &vs, Vec2::new(world_left, y)) {
                if win.y >= origin.y + RULER {
                    spawn_tick(&mut commands, origin.x, win.y - 0.5, RULER, 1.0, tick_col);
                    if let Some(ref f) = font {
                        spawn_label(&mut commands, f, &fmt_coord(y, step_y), origin.x + 2.0, win.y);
                    }
                }
            }
            y += step_y;
        }
    }

    // ── Cursor marker + coordinate readout ───────────────────────────────────
    if vs.hovered {
        if let Ok(window) = windows.single() {
            if let Some(cursor) = window.cursor_position() {
                let inside = cursor.x >= origin.x
                    && cursor.y >= origin.y
                    && cursor.x <= origin.x + size.x
                    && cursor.y <= origin.y + size.y;
                if inside {
                    // Marker lines projected onto each ruler — meaningless
                    // without the ruler bars behind them, so they toggle too.
                    if show_rulers {
                        spawn_tick(&mut commands, cursor.x - 0.5, origin.y, 1.0, RULER, ACCENT);
                        spawn_tick(&mut commands, origin.x, cursor.y - 0.5, RULER, 1.0, ACCENT);
                    }
                    if let (Some(world), Some(f)) = (
                        window_to_world(camera, cam_gt, &vs, cursor),
                        fonts.as_ref().map(|f| f.ui.clone()),
                    ) {
                        let label = format!("{:.0}, {:.0}", world.x, world.y);
                        commands.spawn((
                            Text::new(label),
                            ui_font(&f, 11.0),
                            TextColor(Color::WHITE),
                            overlay_node(cursor.x + 12.0, cursor.y + 12.0, 0.0, 0.0),
                            GlobalZIndex(Z_CURSOR_READOUT),
                            bevy::ui::FocusPolicy::Pass,
                            bevy::picking::Pickable::IGNORE,
                            Overlay2dPart,
                            Name::new("cursor-coords"),
                        ));
                    }
                }
            }
        }
    }

    // ── Selection outline + resize handles ───────────────────────────────────
    let Some(selection) = selection else { return };
    let Some(entity) = selection.get() else {
        return;
    };

    // Resolve the selection's centre + half-size in world units, plus its z
    // rotation. Sprites use their effective render size; bare Node2d groups
    // get a small fixed box so empty groups still show feedback.
    let (center, half, angle) = if let Ok((gt, sprite)) = sprites.get(entity) {
        let Some(s) = sprite_size(sprite, &images) else {
            return;
        };
        if s.x <= 0.0 || s.y <= 0.0 {
            return;
        }
        let angle = gt.rotation().to_euler(EulerRot::XYZ).2;
        (gt.translation().truncate(), s * 0.5, angle)
    } else if let Ok(gt) = node2ds.get(entity) {
        let angle = gt.rotation().to_euler(EulerRot::XYZ).2;
        (gt.translation().truncate(), Vec2::splat(20.0), angle)
    } else {
        return;
    };

    // The selection frame follows the entity's rotation: project the centre
    // and the two (rotated) local half-axes into window space, then draw one
    // border node of the projected size rotated via `UiTransform` — bevy_ui
    // rotates a node about its own centre, which is exactly the frame pivot.
    let rot = Rot2::radians(angle);
    let Some(center_win) = world_to_window(camera, cam_gt, &vs, center) else {
        return;
    };
    let Some(x_edge) = world_to_window(camera, cam_gt, &vs, center + rot * Vec2::new(half.x, 0.0))
    else {
        return;
    };
    let Some(y_edge) = world_to_window(camera, cam_gt, &vs, center + rot * Vec2::new(0.0, half.y))
    else {
        return;
    };
    let ax = x_edge - center_win;
    let ay = y_edge - center_win;
    let rect = Vec2::new(ax.length() * 2.0, ay.length() * 2.0);
    if rect.x <= 0.0 || rect.y <= 0.0 {
        return;
    }
    // Screen-space rotation of the box (window y points down, so this is the
    // world rotation mirrored — reading it off the projected axis keeps the
    // two spaces from ever disagreeing).
    let screen_rot = Rot2::radians(ax.to_angle());

    // Outline.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(center_win.x - rect.x * 0.5),
            top: Val::Px(center_win.y - rect.y * 0.5),
            width: Val::Px(rect.x),
            height: Val::Px(rect.y),
            border: UiRect::all(Val::Px(1.5)),
            ..default()
        },
        bevy::ui::UiTransform::from_rotation(screen_rot),
        BackgroundColor(Color::NONE),
        BorderColor::all(ACCENT),
        GlobalZIndex(Z_SELECTION),
        bevy::ui::FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        Overlay2dPart,
        Name::new("selection-outline-2d"),
    ));

    // Eight resize handles at the (rotated) corners + edge midpoints, each
    // rotated with the frame so corner squares stay flush with the outline.
    let local_offsets = [
        Vec2::new(-half.x, half.y),
        Vec2::new(0.0, half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, 0.0),
        Vec2::new(half.x, 0.0),
        Vec2::new(-half.x, -half.y),
        Vec2::new(0.0, -half.y),
        Vec2::new(half.x, -half.y),
    ];
    for offset in local_offsets {
        let Some(pos) = world_to_window(camera, cam_gt, &vs, center + rot * offset) else {
            continue;
        };
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(pos.x - HANDLE_PX * 0.5),
                top: Val::Px(pos.y - HANDLE_PX * 0.5),
                width: Val::Px(HANDLE_PX),
                height: Val::Px(HANDLE_PX),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            bevy::ui::UiTransform::from_rotation(screen_rot),
            BackgroundColor(Color::WHITE),
            BorderColor::all(ACCENT),
            GlobalZIndex(Z_HANDLE),
            bevy::ui::FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
            Overlay2dPart,
            Name::new("selection-handle-2d"),
        ));
    }

    // Rotate handle: a circle floating above the top-center handle along the
    // box's local up, at a constant on-screen offset. The picker hit-tests
    // the same spot (`picker_2d::ROTATE_HANDLE_OFFSET_PX`).
    if let Some(top_center) = world_to_window(camera, cam_gt, &vs, center + rot * Vec2::new(0.0, half.y)) {
        let dir = (top_center - center_win).normalize_or_zero();
        if dir != Vec2::ZERO {
            let pos = top_center + dir * ROTATE_HANDLE_OFFSET_PX;
            // Connector stem from the outline to the handle, so the circle
            // reads as attached to the selection.
            let stem_len = ROTATE_HANDLE_OFFSET_PX - HANDLE_PX * 0.5;
            let stem_center = top_center + dir * (stem_len * 0.5);
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(stem_center.x - 0.5),
                    top: Val::Px(stem_center.y - stem_len * 0.5),
                    width: Val::Px(1.0),
                    height: Val::Px(stem_len),
                    ..default()
                },
                // The stem is a vertical bar; rotate it to lie along `dir`.
                // `dir` is 90° off the bar's axis reference, hence the offset.
                bevy::ui::UiTransform::from_rotation(Rot2::radians(
                    dir.to_angle() - std::f32::consts::FRAC_PI_2,
                )),
                BackgroundColor(ACCENT),
                GlobalZIndex(Z_HANDLE),
                bevy::ui::FocusPolicy::Pass,
                bevy::picking::Pickable::IGNORE,
                Overlay2dPart,
                Name::new("rotate-stem-2d"),
            ));
            let d = HANDLE_PX + 2.0;
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(pos.x - d * 0.5),
                    top: Val::Px(pos.y - d * 0.5),
                    width: Val::Px(d),
                    height: Val::Px(d),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(d * 0.5)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                BorderColor::all(ACCENT),
                GlobalZIndex(Z_HANDLE),
                bevy::ui::FocusPolicy::Pass,
                bevy::picking::Pickable::IGNORE,
                Overlay2dPart,
                Name::new("rotate-handle-2d"),
            ));
        }
    }
}

/// Absolute-positioned node helper in window coords.
fn overlay_node(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: if width > 0.0 {
            Val::Px(width)
        } else {
            Val::Auto
        },
        height: if height > 0.0 {
            Val::Px(height)
        } else {
            Val::Auto
        },
        ..default()
    }
}

/// Spawn a thin tick line (used for ruler ticks and the cursor marker).
fn spawn_tick(commands: &mut Commands, left: f32, top: f32, width: f32, height: f32, color: Color) {
    commands.spawn((
        overlay_node(left, top, width, height),
        BackgroundColor(color),
        GlobalZIndex(Z_RULER_TICK),
        bevy::ui::FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        Overlay2dPart,
        Name::new("ruler-tick"),
    ));
}

/// Spawn a small ruler tick label.
fn spawn_label(commands: &mut Commands, font: &FontSource, text: &str, left: f32, top: f32) {
    commands.spawn((
        Text::new(text.to_string()),
        ui_font(font, 9.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        overlay_node(left, top, 0.0, 0.0),
        GlobalZIndex(Z_RULER_LABEL),
        bevy::ui::FocusPolicy::Pass,
        bevy::picking::Pickable::IGNORE,
        Overlay2dPart,
        Name::new("ruler-label"),
    ));
}
