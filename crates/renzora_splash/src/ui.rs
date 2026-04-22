use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Pos2, RichText, Sense, Stroke, StrokeKind, Vec2};
use egui_phosphor::regular as icons;

use crate::auth::SplashAuth;
use crate::config::AppConfig;
use crate::github::{self, GithubStats};
#[cfg(not(target_arch = "wasm32"))]
use crate::project::create_project;
use crate::project::{open_project, CurrentProject};
use crate::{SplashState, SplashWindowState};

/// Action for the splash to request of its Bevy window (applied by lib.rs).
#[derive(Default, Clone, Copy)]
pub enum WindowAction {
    #[default]
    None,
    Minimize,
    ToggleMaximize,
    Close,
    StartDrag,
    StartResize(CompassOctant),
}

const VERSION: &str = "r1-alpha5";
const WEBSITE_URL: &str = "https://renzora.com";
const YOUTUBE_URL: &str = "https://youtube.com/@renzoragame";
const DISCORD_URL: &str = "https://discord.gg/9UHUGUyDJv";
const GITHUB_URL: &str = "https://github.com/renzora/engine";
const ROADMAP_URL: &str = "https://github.com/renzora/engine/blob/main/roadmap.md";

const BG_COLOR: Color32 = Color32::from_rgb(5, 4, 10);
const PANEL_BG: Color32 = Color32::from_rgba_premultiplied(18, 20, 30, 230);
const PANEL_HOVER: Color32 = Color32::from_rgba_premultiplied(26, 30, 46, 240);
const BORDER: Color32 = Color32::from_rgb(48, 54, 74);
const BORDER_SOFT: Color32 = Color32::from_rgb(36, 40, 56);
const TEXT: Color32 = Color32::from_rgb(224, 228, 240);
const TEXT_MUTED: Color32 = Color32::from_rgb(130, 138, 160);
const ACCENT: Color32 = Color32::from_rgb(110, 150, 255);
const ACCENT_HOVER: Color32 = Color32::from_rgb(140, 175, 255);
const ERROR_COLOR: Color32 = Color32::from_rgb(239, 68, 68);

/// Upcoming roadmap items shown on the splash. Curated to stay short — the
/// full list lives in roadmap.md on GitHub.
const ROADMAP_ITEMS: &[&str] = &[
    "Prefab / template system for reusable entities",
    "Batch property editing across multiple entities",
    "Property-level undo/redo history in inspector",
    "Advanced asset search (regex, type, size filters)",
    "File tagging and categories in asset browser",
];

// ── Background: synthwave grid + 3D wireframes + constellation ─────

#[derive(Clone)]
struct Particle {
    pos: Vec2,
    vel: Vec2,
    size: f32,
}

#[derive(Clone, Copy)]
enum ShapeKind {
    Cube,
    Tetrahedron,
    Octahedron,
    Diamond,
}

/// A wireframe polytope that drifts in 2D and spins in 3D.
#[derive(Clone)]
struct Shape3D {
    pos: Vec2,
    vel: Vec2,
    rotation: [f32; 3],   // pitch, yaw, roll (radians)
    rot_speed: [f32; 3],  // angular velocity per axis
    size: f32,
    color: Color32,
    kind: ShapeKind,
}

#[derive(Clone, Default)]
struct BgState {
    particles: Vec<Particle>,
    shapes: Vec<Shape3D>,
    last_time: f64,
    grid_timer: f32,
    initialized: bool,
}

/// Returns (vertices, edges) for a given polytope, sized to unit-ish.
fn shape_geometry(kind: ShapeKind) -> (&'static [[f32; 3]], &'static [(usize, usize)]) {
    match kind {
        ShapeKind::Cube => {
            const V: [[f32; 3]; 8] = [
                [-1.0, -1.0, -1.0],
                [ 1.0, -1.0, -1.0],
                [-1.0,  1.0, -1.0],
                [ 1.0,  1.0, -1.0],
                [-1.0, -1.0,  1.0],
                [ 1.0, -1.0,  1.0],
                [-1.0,  1.0,  1.0],
                [ 1.0,  1.0,  1.0],
            ];
            const E: [(usize, usize); 12] = [
                (0, 1), (1, 3), (3, 2), (2, 0),
                (4, 5), (5, 7), (7, 6), (6, 4),
                (0, 4), (1, 5), (2, 6), (3, 7),
            ];
            (&V, &E)
        }
        ShapeKind::Tetrahedron => {
            // Regular tetrahedron inscribed in a cube.
            const V: [[f32; 3]; 4] = [
                [ 1.0,  1.0,  1.0],
                [-1.0, -1.0,  1.0],
                [-1.0,  1.0, -1.0],
                [ 1.0, -1.0, -1.0],
            ];
            const E: [(usize, usize); 6] = [
                (0, 1), (0, 2), (0, 3),
                (1, 2), (1, 3), (2, 3),
            ];
            (&V, &E)
        }
        ShapeKind::Octahedron => {
            const V: [[f32; 3]; 6] = [
                [ 1.2,  0.0,  0.0],
                [-1.2,  0.0,  0.0],
                [ 0.0,  1.2,  0.0],
                [ 0.0, -1.2,  0.0],
                [ 0.0,  0.0,  1.2],
                [ 0.0,  0.0, -1.2],
            ];
            const E: [(usize, usize); 12] = [
                (0, 2), (0, 3), (0, 4), (0, 5),
                (1, 2), (1, 3), (1, 4), (1, 5),
                (2, 4), (2, 5), (3, 4), (3, 5),
            ];
            (&V, &E)
        }
        ShapeKind::Diamond => {
            // Square bipyramid (tall).
            const V: [[f32; 3]; 6] = [
                [ 0.0,  1.4,  0.0],   // top
                [ 0.0, -1.4,  0.0],   // bottom
                [ 0.8,  0.0,  0.0],
                [-0.8,  0.0,  0.0],
                [ 0.0,  0.0,  0.8],
                [ 0.0,  0.0, -0.8],
            ];
            const E: [(usize, usize); 12] = [
                (0, 2), (0, 3), (0, 4), (0, 5),
                (1, 2), (1, 3), (1, 4), (1, 5),
                (2, 4), (4, 3), (3, 5), (5, 2),
            ];
            (&V, &E)
        }
    }
}

/// Apply X-then-Y-then-Z rotation to a 3D point.
fn rotate3(v: [f32; 3], r: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = (r[0].sin(), r[0].cos());
    let y1 = v[1] * cx - v[2] * sx;
    let z1 = v[1] * sx + v[2] * cx;
    let x1 = v[0];

    let (sy, cy) = (r[1].sin(), r[1].cos());
    let x2 = x1 * cy + z1 * sy;
    let z2 = -x1 * sy + z1 * cy;
    let y2 = y1;

    let (sz, cz) = (r[2].sin(), r[2].cos());
    let x3 = x2 * cz - y2 * sz;
    let y3 = x2 * sz + y2 * cz;
    let z3 = z2;

    [x3, y3, z3]
}

/// Perspective-project a rotated 3D point to screen pixels.
/// Returns (screen_pos, camera_space_z) — callers use z for depth shading.
fn project(v: [f32; 3], center: Pos2, size: f32) -> (Pos2, f32) {
    let z_cam = 4.0_f32;
    let z_total = z_cam + v[2];
    let scale = size * 2.5 / z_total;
    let px = center.x + v[0] * scale;
    let py = center.y + v[1] * scale;
    (Pos2::new(px, py), v[2])
}

fn hash01(seed: u64) -> f32 {
    let x = seed.wrapping_mul(0x9E3779B97F4A7C15);
    let x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    let x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    let x = x ^ (x >> 31);
    (x as f32 / u64::MAX as f32).abs()
}

fn draw_synthwave_grid(painter: &egui::Painter, screen: Vec2, grid_timer: f32, time: f64) {
    let horizon_y = screen.y * 0.45;
    let center_x = screen.x * 0.5;

    // Slow hue cycle on the grid so it's not static — visually anchors the scene.
    let hue = (time * 0.05) % 1.0;
    let r = (120.0 + 80.0 * (hue * 6.28).cos()) as u8;
    let g = (120.0 + 80.0 * (hue * 6.28 + 2.09).cos()) as u8;
    let b = (120.0 + 80.0 * (hue * 6.28 + 4.18).cos()) as u8;
    let base = Color32::from_rgb(r, g, b);
    let grid_color = base.gamma_multiply(0.15);
    let glow_color = base.gamma_multiply(0.08);

    let num_v_lines = 24;
    let num_v_segments = 12;
    for i in 0..=num_v_lines {
        let t = i as f32 / num_v_lines as f32;
        let x_bottom = center_x + (t - 0.5) * screen.x * 3.5;
        let x_top = center_x + (t - 0.5) * screen.x * 1.2;

        for s in 0..num_v_segments {
            let s_start = s as f32 / num_v_segments as f32;
            let s_end = (s + 1) as f32 / num_v_segments as f32;
            let y_start = horizon_y + s_start * (screen.y - horizon_y);
            let y_end = horizon_y + s_end * (screen.y - horizon_y);
            let x_start = x_top + s_start * (x_bottom - x_top);
            let x_end = x_top + s_end * (x_bottom - x_top);
            let alpha = (s_start * 2.5).min(1.0);

            painter.line_segment(
                [Pos2::new(x_start, y_start), Pos2::new(x_end, y_end)],
                Stroke::new(3.0, glow_color.gamma_multiply(alpha)),
            );
            painter.line_segment(
                [Pos2::new(x_start, y_start), Pos2::new(x_end, y_end)],
                Stroke::new(1.0, grid_color.gamma_multiply(alpha)),
            );
        }
    }

    let num_h_lines = 12;
    for i in 0..num_h_lines {
        let t = ((i as f32 + grid_timer) / num_h_lines as f32) % 1.0;
        let p = t * t;
        let y = horizon_y + p * (screen.y - horizon_y);
        let alpha = (p * 2.5).min(1.0);
        painter.line_segment(
            [Pos2::new(0.0, y), Pos2::new(screen.x, y)],
            Stroke::new(3.0, glow_color.gamma_multiply(alpha)),
        );
        painter.line_segment(
            [Pos2::new(0.0, y), Pos2::new(screen.x, y)],
            Stroke::new(1.0, grid_color.gamma_multiply(alpha)),
        );
    }
}

fn draw_background(painter: &egui::Painter, screen: Vec2, state: &mut BgState, time: f64, dt: f32) {
    if !state.initialized {
        state.particles.clear();
        for i in 0..70u64 {
            state.particles.push(Particle {
                pos: Vec2::new(hash01(i * 7 + 1) * screen.x, hash01(i * 7 + 2) * screen.y),
                vel: Vec2::new(
                    (hash01(i * 7 + 3) - 0.5) * 18.0,
                    (hash01(i * 7 + 4) - 0.5) * 18.0,
                ),
                size: 1.0 + hash01(i * 7 + 5) * 1.6,
            });
        }

        // Spawn 10 wireframes with random kinds, positions, drifts and rotation axes.
        let kinds = [
            ShapeKind::Cube,
            ShapeKind::Tetrahedron,
            ShapeKind::Octahedron,
            ShapeKind::Diamond,
        ];
        state.shapes.clear();
        for i in 0..10u64 {
            let seed = i * 31 + 1;
            let kind = kinds[(i as usize) % kinds.len()];
            let hue = i as f32 * 0.12;
            let color = match i % 4 {
                0 => Color32::from_rgb(90, 220, 255),
                1 => Color32::from_rgb(220, 120, 240),
                2 => Color32::from_rgb(255, 180, 120),
                _ => Color32::from_rgb(140, 220, 180),
            }.gamma_multiply(0.5 + 0.1 * hue.sin());
            state.shapes.push(Shape3D {
                pos: Vec2::new(hash01(seed) * screen.x, hash01(seed + 1) * screen.y),
                vel: Vec2::new(
                    (hash01(seed + 2) - 0.5) * 40.0,
                    (hash01(seed + 3) - 0.5) * 40.0,
                ),
                rotation: [
                    hash01(seed + 4) * std::f32::consts::TAU,
                    hash01(seed + 5) * std::f32::consts::TAU,
                    hash01(seed + 6) * std::f32::consts::TAU,
                ],
                rot_speed: [
                    (hash01(seed + 7) - 0.5) * 1.4,
                    (hash01(seed + 8) - 0.5) * 1.4,
                    (hash01(seed + 9) - 0.5) * 0.8,
                ],
                size: 24.0 + hash01(seed + 10) * 28.0,
                color,
                kind,
            });
        }
        state.initialized = true;
    }

    state.grid_timer = (state.grid_timer + dt * 0.35) % 1.0;
    draw_synthwave_grid(painter, screen, state.grid_timer, time);

    // 3D wireframes — spin in place while drifting across the canvas.
    for shape in state.shapes.iter_mut() {
        shape.pos += shape.vel * dt;
        for axis in 0..3 {
            shape.rotation[axis] += shape.rot_speed[axis] * dt;
        }
        let margin = shape.size;
        if shape.pos.x < margin { shape.pos.x = margin; shape.vel.x = shape.vel.x.abs(); }
        if shape.pos.x > screen.x - margin { shape.pos.x = screen.x - margin; shape.vel.x = -shape.vel.x.abs(); }
        if shape.pos.y < margin { shape.pos.y = margin; shape.vel.y = shape.vel.y.abs(); }
        if shape.pos.y > screen.y - margin { shape.pos.y = screen.y - margin; shape.vel.y = -shape.vel.y.abs(); }

        let (verts, edges) = shape_geometry(shape.kind);
        let center = Pos2::new(shape.pos.x, shape.pos.y);
        let rotated: Vec<[f32; 3]> = verts.iter().map(|v| rotate3(*v, shape.rotation)).collect();
        let projected: Vec<(Pos2, f32)> = rotated.iter().map(|v| project(*v, center, shape.size)).collect();

        for &(i, j) in edges.iter() {
            let (a, za) = projected[i];
            let (b, zb) = projected[j];
            // Depth fade: edges further from the camera are dimmer.
            let avg_z = (za + zb) * 0.5;
            // avg_z ranges roughly -1.4..1.4 for our shapes; remap to 0..1.
            let depth_t = ((avg_z + 1.4) / 2.8).clamp(0.0, 1.0);
            let fade = 0.35 + depth_t * 0.65;
            let c = Color32::from_rgba_unmultiplied(
                shape.color.r(),
                shape.color.g(),
                shape.color.b(),
                (shape.color.a() as f32 * fade) as u8,
            );
            painter.line_segment([a, b], Stroke::new(1.2, c));
        }
    }

    // Constellation particles + distance-faded links.
    for p in state.particles.iter_mut() {
        p.pos += p.vel * dt;
        if p.pos.x < 0.0 { p.pos.x += screen.x; }
        if p.pos.x > screen.x { p.pos.x -= screen.x; }
        if p.pos.y < 0.0 { p.pos.y += screen.y; }
        if p.pos.y > screen.y { p.pos.y -= screen.y; }
    }

    let link_dist: f32 = 140.0;
    let link_sq = link_dist * link_dist;
    let n = state.particles.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let a = state.particles[i].pos;
            let b = state.particles[j].pos;
            let d2 = (a.x - b.x).powi(2) + (a.y - b.y).powi(2);
            if d2 < link_sq {
                let fade = 1.0 - (d2.sqrt() / link_dist);
                let alpha = (fade * fade * 70.0) as u8;
                let c = Color32::from_rgba_unmultiplied(170, 200, 255, alpha);
                painter.line_segment(
                    [Pos2::new(a.x, a.y), Pos2::new(b.x, b.y)],
                    Stroke::new(1.0, c),
                );
            }
        }
    }
    for p in state.particles.iter() {
        painter.circle_filled(
            Pos2::new(p.pos.x, p.pos.y),
            p.size,
            Color32::from_rgba_unmultiplied(210, 220, 240, 180),
        );
    }

    // Soft vignette on bottom bar for legibility.
    painter.rect_filled(
        egui::Rect::from_min_size(Pos2::new(0.0, screen.y - 70.0), Vec2::new(screen.x, 70.0)),
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(5, 6, 12, 140),
    );
}

// ── URL helper ─────────────────────────────────────────────────────

fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
}

// ── Reusable button helpers ────────────────────────────────────────

/// Icon-prefixed link button. Returns the response so callers can open URLs.
fn link_button(ui: &mut egui::Ui, rect: egui::Rect, icon: &str, label: &str, url: &str, starred: bool) {
    let resp = ui.allocate_rect(rect, Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let bg = if hovered {
        Color32::from_rgba_unmultiplied(255, 255, 255, 26)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 12)
    };
    let border = if hovered { BORDER } else { BORDER_SOFT };
    let text_color = if starred && hovered {
        Color32::from_rgb(255, 215, 110)
    } else if starred {
        Color32::from_rgb(235, 195, 80)
    } else if hovered {
        Color32::WHITE
    } else {
        TEXT
    };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(7), bg, Stroke::new(1.0, border), StrokeKind::Inside);

    let content = format!("{icon}  {label}");
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        content,
        egui::FontId::proportional(12.5),
        text_color,
    );
    if resp.clicked() {
        open_url(url);
    }
}

fn compact_button(ui: &mut egui::Ui, label: &str, width: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let is_primary = label.starts_with('+');
    let bg = if is_primary {
        if resp.hovered() { ACCENT_HOVER } else { ACCENT }
    } else if resp.hovered() {
        Color32::from_rgba_unmultiplied(255, 255, 255, 22)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 10)
    };
    let border = if is_primary { Stroke::NONE } else { Stroke::new(1.0, BORDER) };
    let text_color = if is_primary { Color32::WHITE } else { TEXT };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(6), bg, border, StrokeKind::Inside);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.5),
        text_color,
    );
    resp.clicked()
}

fn input_box(ui: &mut egui::Ui, value: &mut String, hint: &str, password: bool) -> bool {
    let desired = Vec2::new(ui.available_width(), 38.0);
    let (frame_rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter().rect(
        frame_rect,
        CornerRadius::same(6),
        Color32::from_rgba_unmultiplied(10, 12, 20, 200),
        Stroke::new(1.0, BORDER_SOFT),
        StrokeKind::Inside,
    );
    let inner = frame_rect.shrink2(Vec2::new(12.0, 8.0));
    let resp = ui.put(
        inner,
        egui::TextEdit::singleline(value)
            .desired_width(inner.width())
            .hint_text(hint)
            .password(password)
            .frame(false)
            .text_color(TEXT),
    );
    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

// ── Panel chrome ───────────────────────────────────────────────────

fn render_panel_bg(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 6.0)),
        CornerRadius::same(10),
        Color32::from_rgba_unmultiplied(0, 0, 0, 60),
    );
    painter.rect(
        rect,
        CornerRadius::same(10),
        PANEL_BG,
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
}

// ── Sign-in card ───────────────────────────────────────────────────

fn render_sign_in_card(ui: &mut egui::Ui, auth: &mut SplashAuth) {
    let rect = ui.max_rect();
    render_panel_bg(ui.painter(), rect);

    let inner = rect.shrink(22.0);
    #[allow(deprecated)]
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner), |ui| {
        if let Some(user) = auth.user.clone() {
            render_welcome(ui, auth, &user);
        } else {
            render_sign_in_form(ui, auth);
        }
    });
}

fn render_sign_in_form(ui: &mut egui::Ui, auth: &mut SplashAuth) {
    ui.label(RichText::new("SIGN IN").size(10.0).color(TEXT_MUTED).strong().extra_letter_spacing(1.4));
    ui.add_space(2.0);
    ui.label(RichText::new("Welcome back to Renzora").size(16.0).color(Color32::WHITE).strong());

    ui.add_space(12.0);

    ui.label(RichText::new("Email").size(11.0).color(TEXT_MUTED));
    ui.add_space(3.0);
    let email_enter = input_box(ui, &mut auth.email, "you@example.com", false);
    ui.add_space(8.0);

    let pw_enter = input_box(ui, &mut auth.password, "Password", true);
    let enter_pressed = email_enter || pw_enter;
    ui.add_space(6.0);

    if let Some(err) = &auth.error {
        ui.label(RichText::new(err).size(11.0).color(ERROR_COLOR));
    } else {
        ui.add_space(14.0);
    }

    ui.add_space(4.0);

    let (btn_rect, btn_resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 38.0),
        Sense::click(),
    );
    if btn_resp.hovered() && !auth.loading {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let btn_bg = if auth.loading {
        Color32::from_rgb(60, 80, 130)
    } else if btn_resp.hovered() {
        ACCENT_HOVER
    } else {
        ACCENT
    };
    let painter = ui.painter();
    painter.rect(btn_rect, CornerRadius::same(6), btn_bg, Stroke::NONE, StrokeKind::Inside);
    let btn_label = if auth.loading {
        format!("{}  Signing in…", icons::CIRCLE_NOTCH)
    } else {
        format!("{}  Sign In", icons::SIGN_IN)
    };
    painter.text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        btn_label,
        egui::FontId::proportional(13.5),
        Color32::WHITE,
    );
    if (btn_resp.clicked() || enter_pressed) && !auth.loading {
        if auth.email.trim().is_empty() || auth.password.is_empty() {
            auth.error = Some("Email and password are required".into());
        } else {
            auth.start_login();
        }
    }

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new("No account?").size(11.0).color(TEXT_MUTED));
        let link = ui.add(
            egui::Label::new(RichText::new("Create one").size(11.0).color(ACCENT))
                .sense(Sense::click()),
        );
        if link.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if link.clicked() {
            open_url(&format!("{WEBSITE_URL}/register"));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let forgot = ui.add(
                egui::Label::new(RichText::new("Forgot password?").size(11.0).color(TEXT_MUTED).italics())
                    .sense(Sense::click()),
            );
            if forgot.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if forgot.clicked() {
                open_url(&format!("{WEBSITE_URL}/forgot"));
            }
        });
    });
}

fn render_welcome(ui: &mut egui::Ui, auth: &mut SplashAuth, user: &crate::auth::UserProfile) {
    ui.label(RichText::new("SIGNED IN").size(10.0).color(TEXT_MUTED).strong().extra_letter_spacing(1.4));
    ui.add_space(6.0);
    ui.label(
        RichText::new(format!("Welcome, {}", user.username))
            .size(18.0)
            .color(Color32::WHITE)
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(RichText::new(&user.email).size(11.0).color(TEXT_MUTED));
    ui.add_space(16.0);

    let (cb_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 48.0),
        Sense::hover(),
    );
    ui.painter().rect(
        cb_rect,
        CornerRadius::same(8),
        Color32::from_rgba_unmultiplied(110, 150, 255, 26),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(110, 150, 255, 90)),
        StrokeKind::Inside,
    );
    let painter = ui.painter();
    painter.text(
        cb_rect.left_center() + Vec2::new(14.0, -7.0),
        egui::Align2::LEFT_CENTER,
        "CREDITS",
        egui::FontId::proportional(10.0),
        TEXT_MUTED,
    );
    painter.text(
        cb_rect.left_center() + Vec2::new(14.0, 8.0),
        egui::Align2::LEFT_CENTER,
        format!("{}", user.credit_balance),
        egui::FontId::proportional(15.0),
        Color32::WHITE,
    );
    painter.text(
        cb_rect.right_center() + Vec2::new(-14.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        user.role.to_uppercase(),
        egui::FontId::proportional(10.5),
        ACCENT,
    );

    ui.add_space(ui.available_height().max(0.0) - 42.0);

    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 34.0),
        Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let bg = if resp.hovered() {
        Color32::from_rgba_unmultiplied(255, 255, 255, 22)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 10)
    };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(6), bg, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{}  Sign out", icons::SIGN_OUT),
        egui::FontId::proportional(12.0),
        TEXT,
    );
    if resp.clicked() {
        auth.sign_out();
    }
}

// ── Roadmap card ───────────────────────────────────────────────────

fn render_roadmap_card(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    render_panel_bg(ui.painter(), rect);

    let inner = rect.shrink(22.0);
    #[allow(deprecated)]
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(icons::ROCKET)
                    .size(14.0)
                    .color(ACCENT),
            );
            ui.label(
                RichText::new("ROADMAP")
                    .size(10.0)
                    .color(TEXT_MUTED)
                    .strong()
                    .extra_letter_spacing(1.4),
            );
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new("What's coming next")
                .size(14.0)
                .color(Color32::WHITE)
                .strong(),
        );
        ui.add_space(10.0);

        for item in ROADMAP_ITEMS {
            let (line_rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), 22.0),
                Sense::hover(),
            );
            let painter = ui.painter();
            painter.circle_filled(
                Pos2::new(line_rect.left() + 5.0, line_rect.center().y),
                2.5,
                ACCENT,
            );
            painter.text(
                line_rect.left_center() + Vec2::new(16.0, 0.0),
                egui::Align2::LEFT_CENTER,
                *item,
                egui::FontId::proportional(12.5),
                TEXT,
            );
        }

        ui.add_space(10.0);
        let (btn_rect, resp) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 30.0),
            Sense::click(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let bg = if resp.hovered() {
            Color32::from_rgba_unmultiplied(255, 255, 255, 22)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 10)
        };
        let painter = ui.painter();
        painter.rect(btn_rect, CornerRadius::same(6), bg, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        painter.text(
            btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}  View full roadmap on GitHub", icons::ARROW_SQUARE_OUT),
            egui::FontId::proportional(11.5),
            TEXT,
        );
        if resp.clicked() {
            open_url(ROADMAP_URL);
        }
    });
}

// ── Projects card ──────────────────────────────────────────────────

#[derive(Default, Clone)]
struct NewProjectName(String);

fn render_projects_card(
    ui: &mut egui::Ui,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<SplashState>,
) {
    let rect = ui.max_rect();
    render_panel_bg(ui.painter(), rect);

    let inner = rect.shrink(22.0);
    #[allow(deprecated)]
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("PROJECTS").size(10.0).color(TEXT_MUTED).strong().extra_letter_spacing(1.4));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if compact_button(ui, "Open Project", 104.0) {
                    open_existing_project(app_config, commands, next_state);
                }
                ui.add_space(6.0);
                if compact_button(ui, "+ New", 70.0) {
                    create_new_project(ui, app_config, commands, next_state);
                }
            });
        });

        ui.add_space(12.0);

        let mut new_name = ui.memory_mut(|mem| {
            mem.data.get_temp_mut_or_default::<NewProjectName>(egui::Id::new("splash_new_name")).0.clone()
        });
        let _ = input_box(ui, &mut new_name, "New project name…", false);
        ui.memory_mut(|mem| {
            mem.data.get_temp_mut_or_default::<NewProjectName>(egui::Id::new("splash_new_name")).0 = new_name;
        });

        ui.add_space(16.0);
        ui.label(RichText::new("Recents").size(15.0).color(Color32::WHITE).strong());
        ui.add_space(8.0);

        render_recent_list(ui, app_config, commands, next_state);
    });
}

fn render_recent_list(
    ui: &mut egui::Ui,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<SplashState>,
) {
    if app_config.recent_projects.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No recent projects yet").size(13.0).color(TEXT_MUTED).italics());
            ui.add_space(4.0);
            ui.label(RichText::new("Click + New or Open Project to get started.").size(11.0).color(TEXT_MUTED));
        });
        return;
    }

    let mut project_to_open: Option<CurrentProject> = None;
    let mut path_to_remove: Option<usize> = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let list_width = ui.available_width();
            for (idx, project_path) in app_config.recent_projects.iter().enumerate() {
                let display_name = project_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown Project");
                let path_display = project_path.to_string_lossy();

                #[cfg(not(target_arch = "wasm32"))]
                let exists = project_path.join("project.toml").exists();
                #[cfg(target_arch = "wasm32")]
                let exists = {
                    let slug = path_display.strip_prefix("web:/").unwrap_or(&path_display);
                    crate::web_storage::load_web_project(slug).is_some()
                };

                let (item_rect, response) = ui.allocate_exact_size(
                    Vec2::new(list_width, 60.0),
                    Sense::click(),
                );
                let hovered = response.hovered();
                if hovered && exists {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                let bg = if hovered && exists { PANEL_HOVER } else {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 8)
                };
                let border = if hovered && exists { ACCENT } else { BORDER_SOFT };
                ui.painter().rect(
                    item_rect,
                    CornerRadius::same(8),
                    bg,
                    Stroke::new(1.0, border),
                    StrokeKind::Inside,
                );

                let name_pos = item_rect.min + Vec2::new(14.0, 12.0);
                let path_pos = item_rect.min + Vec2::new(14.0, 34.0);
                let name_color = if !exists {
                    TEXT_MUTED
                } else if hovered {
                    Color32::WHITE
                } else {
                    TEXT
                };
                let name_label = if exists {
                    display_name.to_string()
                } else {
                    format!("{display_name}  (missing)")
                };
                ui.painter().text(
                    name_pos,
                    egui::Align2::LEFT_TOP,
                    name_label,
                    egui::FontId::proportional(14.0),
                    name_color,
                );
                let max_path_len = 60;
                let path_str = if path_display.len() > max_path_len {
                    format!("…{}", &path_display[path_display.len() - max_path_len..])
                } else {
                    path_display.to_string()
                };
                ui.painter().text(
                    path_pos,
                    egui::Align2::LEFT_TOP,
                    path_str,
                    egui::FontId::monospace(10.0),
                    TEXT_MUTED,
                );

                if !exists {
                    let remove_rect = egui::Rect::from_min_size(
                        item_rect.right_top() + Vec2::new(-72.0, 18.0),
                        Vec2::new(60.0, 24.0),
                    );
                    let rm_resp = ui.allocate_rect(remove_rect, Sense::click());
                    if rm_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let rm_color = if rm_resp.hovered() { ERROR_COLOR } else { TEXT_MUTED };
                    ui.painter().text(
                        remove_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Remove",
                        egui::FontId::proportional(10.5),
                        rm_color,
                    );
                    if rm_resp.clicked() {
                        path_to_remove = Some(idx);
                    }
                } else if response.clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let project_toml = project_path.join("project.toml");
                        match open_project(&project_toml) {
                            Ok(project) => project_to_open = Some(project),
                            Err(e) => eprintln!("Failed to open project: {e}"),
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let slug = path_display.strip_prefix("web:/").unwrap_or(&path_display);
                        if let Some(project) = crate::web_storage::load_web_project(slug) {
                            project_to_open = Some(project);
                        }
                    }
                }

                ui.add_space(8.0);
            }
        });

    if let Some(project) = project_to_open {
        app_config.add_recent_project(project.path.clone());
        let _ = app_config.save();
        commands.insert_resource(project);
        next_state.set(SplashState::Loading);
    }
    if let Some(idx) = path_to_remove {
        app_config.recent_projects.remove(idx);
        let _ = app_config.save();
    }
}

fn open_existing_project(
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<SplashState>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(file) = rfd::FileDialog::new()
            .set_title("Open Project")
            .add_filter("Project File", &["toml"])
            .pick_file()
        {
            match open_project(&file) {
                Ok(project) => {
                    app_config.add_recent_project(project.path.clone());
                    let _ = app_config.save();
                    commands.insert_resource(project);
                    next_state.set(SplashState::Loading);
                }
                Err(e) => eprintln!("Failed to open project: {e}"),
            }
        }
    }
    let _ = (app_config, commands, next_state);
}

fn create_new_project(
    ui: &mut egui::Ui,
    app_config: &mut AppConfig,
    commands: &mut Commands,
    next_state: &mut NextState<SplashState>,
) {
    let typed_name = ui.memory_mut(|mem| {
        mem.data.get_temp_mut_or_default::<NewProjectName>(egui::Id::new("splash_new_name")).0.clone()
    });
    let project_name = if typed_name.trim().is_empty() {
        "New Project".to_string()
    } else {
        typed_name.trim().to_string()
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Select Project Location")
            .pick_folder()
        {
            let slug = project_name.replace(' ', "_").to_lowercase();
            let project_path = folder.join(&slug);
            match create_project(&project_path, &project_name) {
                Ok(project) => {
                    app_config.add_recent_project(project_path);
                    let _ = app_config.save();
                    commands.insert_resource(project);
                    next_state.set(SplashState::Loading);
                }
                Err(e) => eprintln!("Failed to create project: {e}"),
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        match crate::web_storage::create_web_project(&project_name) {
            Ok(project) => {
                app_config.add_recent_project(project.path.clone());
                let _ = app_config.save();
                commands.insert_resource(project);
                next_state.set(SplashState::Loading);
            }
            Err(e) => web_sys::console::log_1(&format!("Failed to create project: {e}").into()),
        }
    }
}

// ── Bottom bar: social links + copyright ───────────────────────────

fn render_bottom_bar(ui: &mut egui::Ui, rect: egui::Rect, stars: Option<u64>) {
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(8, 10, 18, 200),
    );
    ui.painter().line_segment(
        [Pos2::new(rect.left(), rect.top()), Pos2::new(rect.right(), rect.top())],
        Stroke::new(1.0, BORDER_SOFT),
    );

    let center_y = rect.center().y;

    // Left: wordmark + version pill.
    let painter = ui.painter();
    let wordmark_pos = Pos2::new(rect.left() + 24.0, center_y);
    painter.text(
        wordmark_pos,
        egui::Align2::LEFT_CENTER,
        "Renzora Engine",
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
    // Measure so the version pill sits flush after the wordmark.
    let wordmark_size = painter.layout_no_wrap(
        "Renzora Engine".into(),
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    ).size();
    let version_font = egui::FontId::monospace(10.5);
    let version_size = painter.layout_no_wrap(VERSION.into(), version_font.clone(), ACCENT).size();
    let pill_w = version_size.x + 14.0;
    let pill_rect = egui::Rect::from_center_size(
        Pos2::new(wordmark_pos.x + wordmark_size.x + 10.0 + pill_w / 2.0, center_y),
        Vec2::new(pill_w, 18.0),
    );
    painter.rect(
        pill_rect,
        CornerRadius::same(9),
        Color32::from_rgba_unmultiplied(110, 150, 255, 40),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(110, 150, 255, 140)),
        StrokeKind::Inside,
    );
    painter.text(
        pill_rect.center(),
        egui::Align2::CENTER_CENTER,
        VERSION,
        version_font,
        ACCENT,
    );

    // Right-anchored social pills (laid out right-to-left).
    let star_label = match stars {
        Some(n) => format!("Star us on GitHub  ({})", github::format_count(n)),
        None => "Star us on GitHub".to_string(),
    };
    let links: [(&str, &str, &str, f32, bool); 4] = [
        (icons::STAR, star_label.as_str(), GITHUB_URL, 192.0, true),
        (icons::DISCORD_LOGO, "Discord", DISCORD_URL, 96.0, false),
        (icons::YOUTUBE_LOGO, "YouTube", YOUTUBE_URL, 96.0, false),
        (icons::GLOBE, "Website", WEBSITE_URL, 96.0, false),
    ];
    let gap = 8.0;
    let mut x_cursor = rect.right() - 24.0;
    for (icon, label, url, w, starred) in links.iter() {
        x_cursor -= w;
        let btn_rect = egui::Rect::from_min_size(
            Pos2::new(x_cursor, center_y - 15.0),
            Vec2::new(*w, 30.0),
        );
        link_button(ui, btn_rect, icon, label, url, *starred);
        x_cursor -= gap;
    }
}

// ── Custom window chrome (borderless window) ───────────────────────

/// Renders the top 36px strip: a draggable empty region on the left and
/// min/maximize/close buttons on the right. Returns a WindowAction if the
/// user interacted with any of them.
fn render_window_chrome(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    is_maximized: bool,
) -> WindowAction {
    let mut action = WindowAction::None;

    // Opaque background for the title strip — keeps the busy animated
    // background from bleeding through behind the window controls.
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        Color32::from_rgb(12, 14, 22),
    );
    ui.painter().line_segment(
        [Pos2::new(rect.left(), rect.bottom()), Pos2::new(rect.right(), rect.bottom())],
        Stroke::new(1.0, BORDER_SOFT),
    );

    // Right-anchored window buttons (close, maximize, minimize — right to left).
    let btn_size = Vec2::new(40.0, rect.height());
    let close_rect = egui::Rect::from_min_size(
        Pos2::new(rect.right() - btn_size.x, rect.top()),
        btn_size,
    );
    let max_rect = egui::Rect::from_min_size(
        Pos2::new(rect.right() - btn_size.x * 2.0, rect.top()),
        btn_size,
    );
    let min_rect = egui::Rect::from_min_size(
        Pos2::new(rect.right() - btn_size.x * 3.0, rect.top()),
        btn_size,
    );

    // Drag region — everything to the left of the three buttons.
    let drag_rect = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() - btn_size.x * 3.0, rect.height()),
    );
    let drag_resp = ui.allocate_rect(drag_rect, Sense::click_and_drag());
    if drag_resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if drag_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    if drag_resp.drag_started() {
        action = WindowAction::StartDrag;
    }

    // Render the three window buttons.
    let max_icon = if is_maximized {
        icons::ARROWS_IN_SIMPLE
    } else {
        icons::SQUARE
    };
    if window_button(ui, min_rect, icons::MINUS, false) {
        action = WindowAction::Minimize;
    }
    if window_button(ui, max_rect, max_icon, false) {
        action = WindowAction::ToggleMaximize;
    }
    if window_button(ui, close_rect, icons::X, true) {
        action = WindowAction::Close;
    }

    action
}

fn window_button(ui: &mut egui::Ui, rect: egui::Rect, icon: &str, is_close: bool) -> bool {
    let resp = ui.allocate_rect(rect, Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let bg = if hovered {
        if is_close {
            Color32::from_rgb(232, 17, 35)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 34)
        }
    } else {
        Color32::TRANSPARENT
    };
    let icon_color = if is_close && hovered {
        Color32::WHITE
    } else if hovered {
        Color32::WHITE
    } else {
        TEXT
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        icon_color,
    );
    resp.clicked()
}

// ── Resize zones for the borderless window ─────────────────────────

/// A hidden hit-region along an edge or corner of the window. Returns `true`
/// if the user started dragging it this frame.
fn resize_zone(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    cursor: CursorIcon,
) -> bool {
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(cursor);
    }
    resp.drag_started()
}

/// Adds the 8 resize hit-regions (corners + edges) at the outer border.
/// Returns a StartResize action if the user grabbed one, otherwise None.
fn render_resize_zones(ui: &mut egui::Ui, rect: egui::Rect) -> WindowAction {
    // Grippable — these extend INTO the window so corners and edges are
    // easy to grab without pixel-perfect aim.
    let t: f32 = 10.0;  // edge thickness
    let c: f32 = 18.0;  // corner size

    // Edges (between corners).
    let top_edge = egui::Rect::from_min_size(
        Pos2::new(rect.left() + c, rect.top()),
        Vec2::new(rect.width() - 2.0 * c, t),
    );
    let bottom_edge = egui::Rect::from_min_size(
        Pos2::new(rect.left() + c, rect.bottom() - t),
        Vec2::new(rect.width() - 2.0 * c, t),
    );
    let left_edge = egui::Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + c),
        Vec2::new(t, rect.height() - 2.0 * c),
    );
    let right_edge = egui::Rect::from_min_size(
        Pos2::new(rect.right() - t, rect.top() + c),
        Vec2::new(t, rect.height() - 2.0 * c),
    );

    // Corners.
    let nw = egui::Rect::from_min_size(rect.min, Vec2::splat(c));
    let ne = egui::Rect::from_min_size(Pos2::new(rect.right() - c, rect.top()), Vec2::splat(c));
    let sw = egui::Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() - c), Vec2::splat(c));
    let se = egui::Rect::from_min_size(Pos2::new(rect.right() - c, rect.bottom() - c), Vec2::splat(c));

    if resize_zone(ui, nw, CursorIcon::ResizeNorthWest) { return WindowAction::StartResize(CompassOctant::NorthWest); }
    if resize_zone(ui, ne, CursorIcon::ResizeNorthEast) { return WindowAction::StartResize(CompassOctant::NorthEast); }
    if resize_zone(ui, sw, CursorIcon::ResizeSouthWest) { return WindowAction::StartResize(CompassOctant::SouthWest); }
    if resize_zone(ui, se, CursorIcon::ResizeSouthEast) { return WindowAction::StartResize(CompassOctant::SouthEast); }
    if resize_zone(ui, top_edge, CursorIcon::ResizeNorth) { return WindowAction::StartResize(CompassOctant::North); }
    if resize_zone(ui, bottom_edge, CursorIcon::ResizeSouth) { return WindowAction::StartResize(CompassOctant::South); }
    if resize_zone(ui, left_edge, CursorIcon::ResizeWest) { return WindowAction::StartResize(CompassOctant::West); }
    if resize_zone(ui, right_edge, CursorIcon::ResizeEast) { return WindowAction::StartResize(CompassOctant::East); }

    WindowAction::None
}

// ── Entry point ────────────────────────────────────────────────────

pub fn render_splash(
    ctx: &egui::Context,
    app_config: &mut AppConfig,
    auth: &mut SplashAuth,
    stats: &mut GithubStats,
    win_state: &SplashWindowState,
    commands: &mut Commands,
    next_state: &mut NextState<SplashState>,
) -> WindowAction {
    ctx.request_repaint();
    auth.poll();
    stats.poll();

    let mut action = WindowAction::None;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_COLOR))
        .show(ctx, |ui| {
            let screen_rect = ui.max_rect();
            let screen = Vec2::new(screen_rect.width(), screen_rect.height());

            let mut bg = ui.memory_mut(|mem| {
                mem.data.get_temp_mut_or_default::<BgState>(egui::Id::new("splash_bg")).clone()
            });
            let t = ui.input(|i| i.time);
            let dt = if bg.last_time > 0.0 { (t - bg.last_time) as f32 } else { 0.016 };
            bg.last_time = t;
            draw_background(ui.painter(), screen, &mut bg, t, dt.min(0.1));
            ui.memory_mut(|mem| {
                *mem.data.get_temp_mut_or_default::<BgState>(egui::Id::new("splash_bg")) = bg;
            });

            // Top chrome strip (borderless window controls).
            let chrome_h = 36.0;
            let chrome_rect = egui::Rect::from_min_size(
                screen_rect.min,
                Vec2::new(screen_rect.width(), chrome_h),
            );
            action = render_window_chrome(ui, chrome_rect, win_state.maximized);

            // Bottom bar reserved area.
            let bottom_h = 56.0;
            let bottom_rect = egui::Rect::from_min_size(
                Pos2::new(screen_rect.min.x, screen_rect.max.y - bottom_h),
                Vec2::new(screen_rect.width(), bottom_h),
            );

            // Layout: left column (sign-in + roadmap stacked), right column (projects).
            let gap = 16.0;
            let mut left_w = 340.0;
            let mut right_w = 500.0;
            let available_w = screen_rect.width() - 48.0;
            let total_w = left_w + gap + right_w;
            if total_w > available_w {
                let scale = (available_w / total_w).max(0.65);
                left_w *= scale;
                right_w *= scale;
            }
            let total_w = left_w + gap + right_w;

            // Fixed card heights so content fits without clipping.
            let sign_in_h: f32 = 300.0;
            let roadmap_h: f32 = 260.0;
            let column_height = sign_in_h + gap + roadmap_h;

            // Center the column pair vertically between chrome and bottom bar.
            let content_top_min = chrome_rect.bottom() + 24.0;
            let content_bottom = bottom_rect.top() - 16.0;
            let available_h = (content_bottom - content_top_min).max(column_height);
            let content_top = content_top_min + (available_h - column_height) / 2.0;

            let start_x = screen_rect.min.x + (screen_rect.width() - total_w) / 2.0;

            let sign_in_rect = egui::Rect::from_min_size(
                Pos2::new(start_x, content_top),
                Vec2::new(left_w, sign_in_h),
            );
            let roadmap_rect = egui::Rect::from_min_size(
                Pos2::new(start_x, content_top + sign_in_h + gap),
                Vec2::new(left_w, roadmap_h),
            );
            let projects_rect = egui::Rect::from_min_size(
                Pos2::new(start_x + left_w + gap, content_top),
                Vec2::new(right_w, column_height),
            );

            #[allow(deprecated)]
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(sign_in_rect), |ui| {
                render_sign_in_card(ui, auth);
            });
            #[allow(deprecated)]
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(roadmap_rect), |ui| {
                render_roadmap_card(ui);
            });
            #[allow(deprecated)]
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(projects_rect), |ui| {
                render_projects_card(ui, app_config, commands, next_state);
            });

            render_bottom_bar(ui, bottom_rect, stats.stars);

            // Resize zones (drawn last so they win hit-testing at the edges).
            // Disabled when maximized — the OS wouldn't honour drag-resize anyway.
            if !win_state.maximized {
                let resize_action = render_resize_zones(ui, screen_rect);
                if matches!(resize_action, WindowAction::StartResize(_)) {
                    action = resize_action;
                }
            }

            // 1px outer border so the borderless window has a visible frame.
            ui.painter().rect_stroke(
                screen_rect.shrink(0.5),
                CornerRadius::ZERO,
                Stroke::new(1.0, BORDER),
                StrokeKind::Inside,
            );
        });

    action
}
