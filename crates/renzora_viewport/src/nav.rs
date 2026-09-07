//! Native (bevy_ui) viewport nav overlay — the pan, zoom, home and grid buttons
//! on the right side of each viewport.
//!
//! Pan and Zoom are press-and-drag: while held they accumulate `MouseMotion`
//! into [`NavOverlayState`]'s atomic deltas (the same ones the camera system
//! already consumes). Home and Grid are plain clicks. The cluster is an
//! [`OverlaySurface`] so hovering it suppresses viewport hover (the camera
//! won't orbit / box-select won't start under the buttons).
//!
//! Home and Grid spent a while at the foot of the tool shelf, on the argument
//! that a click-once control belongs with every other click-once control rather
//! than in a cluster built around a drag. That reads well written down and
//! poorly in use: all four are *view* controls, none of them changes what a
//! click in the viewport does, and splitting them put the two you reach for
//! most on the opposite edge of the screen from the camera you are aiming.
//! What they have in common is the camera, not the input gesture.
//!
//! So the four are one group, and the group is the thing with the chrome: a
//! single translucent rounded panel with transparent buttons inside it, rather
//! than four separate floating pills. It reads as one control surface, and it
//! puts far less opaque furniture over the scene.

use std::sync::atomic::Ordering;

use bevy::color::Hsla;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::viewport_types::{NavOverlayState, ViewportSettings};
use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::theme::{accent, hover_bg, panel_bg, rgb, text_primary};
use renzora_ember::widgets::OverlaySurface;

use crate::{AXIS_GIZMO_MARGIN, AXIS_GIZMO_SIZE};

const BTN: f32 = 36.0;

/// Breathing room between the buttons and the group's rounded edge.
const GROUP_PAD: f32 = 4.0;

/// How opaque the group's panel is.
///
/// Every glyph colour below is picked against `panel_bg()`, and that reasoning
/// only holds if the panel is actually the colour behind them. At the 0.55 this
/// started at, better than a third of what you saw through a button was the
/// *scene* — which is any colour at all — so a glyph chosen to contrast with a
/// dark panel sat on bright terrain and vanished. Opaque enough to own its own
/// background, translucent enough to still read as an overlay.
const GROUP_ALPHA: f32 = 0.72;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum NavButton {
    Pan,
    Zoom,
    Home,
    Grid,
}

impl NavButton {
    /// Whether this button is a press-and-drag rather than a click. Only the
    /// drag pair latches; a click acts once on press and is done.
    fn is_drag(self) -> bool {
        matches!(self, NavButton::Pan | NavButton::Zoom)
    }
}

/// Marks the glyph inside a nav button, so [`nav_visuals`] can tint Grid's icon
/// without walking into anything else a button might hold later.
#[derive(Component)]
struct NavGlyph;

/// Which nav drag-button is currently latched (continues off the button until
/// mouse release, mirroring egui's pointer-latched drag).
#[derive(Resource, Default)]
struct NavDragLatch(Option<NavButton>);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<NavDragLatch>();
    app.add_systems(
        Update,
        (nav_input, nav_visuals).run_if(in_state(SplashState::Editor)),
    );
}

/// The group's own fill: the panel colour, translucent enough to read the scene
/// through. This is the only opaque-ish chrome in the cluster now — the buttons
/// inside it are transparent until you touch them.
fn group_bg() -> Color {
    let (r, g, b) = panel_bg();
    Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        GROUP_ALPHA,
    )
}

/// Whether the cluster's panel is a light surface.
///
/// Rec. 709 luma on `panel_bg()`. Everything below has to push *away* from this
/// rather than in a fixed direction — see [`accent_glyph`].
fn on_light_surface() -> bool {
    let (r, g, b) = panel_bg();
    0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32 > 140.0
}

/// The resting glyph colour: whatever the theme says text on a panel is.
///
/// This was a hardcoded near-white, which is only the answer for half the
/// themes — on the Light theme it put `(235, 235, 240)` glyphs on a
/// `(244, 245, 248)` panel.
fn glyph() -> (u8, u8, u8) {
    text_primary()
}

/// The Grid glyph while the grid is on.
///
/// **Not the raw accent.** The accent is chosen to carry white text *on top of
/// it*, which makes it dark by construction — the shipped blue is `l = 0.66`
/// against white glyphs at `l ≈ 0.92` — so a glyph drawn *in* it reads dimmer
/// than the plain ones beside it, the exact opposite of what an on-toggle
/// should say. `tool_buttons` hits this on the shelf and lifts the accent by
/// 0.1, capped at `l = 0.75`.
///
/// In **HSL**, not `Luminance::lighter`: that works in Lab, where raising
/// lightness pulls the colour toward white and takes the chroma with it — a
/// pale grey-blue that reads washed out rather than lit, brighter and less
/// blue, when blue is the one thing this colour has to be. Nudging saturation
/// alongside the lightness is what keeps a less-saturated theme accent reading
/// as itself once it is this light.
///
/// **The direction is not fixed.** Lifting unconditionally is the mistake
/// `theme::mix`'s doc warns about, and it inverts exactly where you would
/// expect: on the Light theme the accent is `(38, 108, 200)` on a
/// `(244, 245, 248)` panel, and lightening it took the contrast from 4.74
/// *down* to 1.90 — measurably worse than leaving the accent alone. Darkening
/// it there gives 10.09. So the shift is away from the surface, whichever way
/// that is.
///
/// The cap is the real limit, and it is why this is only half the answer: a
/// saturated blue cannot get much brighter without going pale, so past
/// `l ≈ 0.8` every further step buys contrast by spending colour. Hence
/// [`ACTIVE_WASH`] — the rest of the visibility comes from behind the glyph
/// rather than from bleaching it.
fn accent_glyph() -> Color {
    let mut hsl = Hsla::from(rgb(accent()));
    hsl.lightness = if on_light_surface() {
        (hsl.lightness - 0.24).max(0.24)
    } else {
        (hsl.lightness + 0.28).min(0.82)
    };
    hsl.saturation = (hsl.saturation + 0.12).min(1.0);
    Color::from(hsl)
}

/// How much accent sits behind a lit toggle.
///
/// Deliberately a *wash*, not the fill a latched drag gets: at full strength it
/// would read as "the selected one of four", which is what a fill means
/// everywhere else in this editor and not what a toggle is saying. At a third
/// of that it reads as a lit key on a keyboard — unmistakably on, without
/// claiming the other three are off in the same sense.
///
/// It carries the visibility the glyph colour cannot (see [`accent_glyph`]),
/// and the two are balanced against each other: raising this darkens what the
/// glyph sits on, so it cannot go much further without eating the contrast it
/// is here to add. At `0.32` the tile reads clearly against the panel (1.6:1)
/// while the glyph still clears 5.5:1 on it, in both shipped themes.
const ACTIVE_WASH: f32 = 0.32;

fn accent_wash() -> Color {
    let (r, g, b) = accent();
    Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        ACTIVE_WASH,
    )
}

/// Build the nav cluster as an absolutely-positioned column on the right edge of
/// a viewport content node (below where the axis gizmo sits). Returns the cluster
/// root.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let cluster = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                // No toolbar offset: the bar is above the scene now, not over it.
                top: Val::Px(AXIS_GIZMO_SIZE + AXIS_GIZMO_MARGIN + 24.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(GROUP_PAD)),
                // A pill, not a rounded rectangle: the radius is the button's
                // own plus the padding, so the group's edge stays concentric
                // with the top and bottom buttons inside it.
                border_radius: BorderRadius::all(Val::Px(BTN / 2.0 + GROUP_PAD)),
                ..default()
            },
            BackgroundColor(group_bg()),
            RelativeCursorPosition::default(),
            OverlaySurface,
            Name::new("nav-overlay"),
        ))
        .id();

    let home = nav_btn(commands, fonts, NavButton::Home, "house");
    let pan = nav_btn(commands, fonts, NavButton::Pan, "hand");
    let zoom = nav_btn(commands, fonts, NavButton::Zoom, "magnifying-glass");
    let grid = nav_btn(commands, fonts, NavButton::Grid, "grid-four");
    // Home leads. It is the way *out* of wherever pan and zoom have taken you,
    // so it wants to be the button you can hit without looking — and the top of
    // a column, directly under the axis gizmo, is the one position in the
    // cluster you can find that way. Grid stays last: it is the only one of the
    // four that does not move the camera.
    commands
        .entity(cluster)
        .add_children(&[home, pan, zoom, grid]);
    // Hide the nav buttons during play mode for a clean game view, and in 2D
    // view (they're 3D-orbit pan/zoom controls).
    renzora_ember::reactive::tracked::bind_display(commands, cluster, |w| {
        let in_play = w
            .get_resource::<renzora::core::PlayModeState>()
            .map(|p| p.is_in_play_mode())
            .unwrap_or(false);
        let in_2d = w
            .get_resource::<renzora::core::viewport_types::ViewportSettings>()
            .map(|s| s.viewport_view == renzora::core::viewport_types::ViewportView::Two)
            .unwrap_or(false);
        !in_play && !in_2d
    });
    cluster
}

fn nav_btn(commands: &mut Commands, fonts: &EmberFonts, kind: NavButton, icon: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(BTN),
                height: Val::Px(BTN),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(BTN / 2.0)),
                ..default()
            },
            // Transparent at rest — the group behind carries the fill, and four
            // stacked pills inside a fifth read as a pile of chrome.
            BackgroundColor(Color::NONE),
            Interaction::default(),
            kind,
            Name::new("nav-btn"),
        ))
        .id();
    let g = icon_text(commands, &fonts.phosphor, icon, glyph(), 16.0);
    // Let clicks fall through the glyph to the button, or a press on the icon
    // itself (dead-center of the button) never reaches the button's `Interaction`
    // and the Grid toggle silently does nothing.
    commands
        .entity(g)
        .insert((bevy::picking::Pickable::IGNORE, NavGlyph));
    commands.entity(b).add_child(g);
    b
}

/// Recolor the buttons: accent fill while a drag is latched, hover wash on
/// hover, and a lit Grid — brightened glyph over an [`ACTIVE_WASH`] of accent —
/// while the grid is on.
///
/// Grid says "on" at a fifth of the strength a latched drag says "now", because
/// they are different claims: the fill is "you are holding this", and a
/// permanently filled button among four reads as the selected one of four. It
/// takes both halves to say it legibly, though. The glyph alone was the first
/// attempt and it was barely visible, for a reason that is structural rather
/// than a bad constant: the theme accent is dark by construction so white text
/// can sit on it, and lightening a saturated blue far enough to fix that turns
/// it pale. See [`accent_glyph`].
fn nav_visuals(
    nav: Res<NavOverlayState>,
    settings: Option<Res<ViewportSettings>>,
    mut buttons: Query<(&NavButton, &Interaction, &mut BackgroundColor, &Children)>,
    mut glyphs: Query<&mut TextColor, With<NavGlyph>>,
) {
    let grid_on = settings.is_some_and(|s| s.show_grid);
    for (kind, interaction, mut bg, children) in &mut buttons {
        let latched = match kind {
            NavButton::Pan => nav.pan_dragging.load(Ordering::Relaxed),
            NavButton::Zoom => nav.zoom_dragging.load(Ordering::Relaxed),
            NavButton::Home | NavButton::Grid => false,
        };
        let lit = *kind == NavButton::Grid && grid_on;
        let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        // Hover outranks the lit wash so the button still answers the cursor
        // while the grid is on — otherwise the one control you are most likely
        // to click twice in a row is the one that stops acknowledging you.
        bg.0 = match (latched, hovered, lit) {
            (true, _, _) => rgb(accent()),
            (false, true, _) => rgb(hover_bg()),
            (false, false, true) => accent_wash(),
            (false, false, false) => Color::NONE,
        };

        let tint = if lit { accent_glyph() } else { rgb(glyph()) };
        for child in children.iter() {
            if let Ok(mut color) = glyphs.get_mut(child) {
                if color.0 != tint {
                    color.0 = tint;
                }
            }
        }
    }
}

/// Pan/Zoom press-and-drag → accumulate into the camera-consumed atomics.
/// Home/Grid → act once, on press.
fn nav_input(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    nav: Res<NavOverlayState>,
    mut latch: ResMut<NavDragLatch>,
    // `ResMut` alone doesn't flag the resource changed — only a deref_mut does,
    // and both of those sit inside the just-pressed branch.
    settings: Option<ResMut<ViewportSettings>>,
    buttons: Query<(&NavButton, &Interaction)>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(kind) = buttons
            .iter()
            .find(|(_, i)| **i == Interaction::Pressed)
            .map(|(kind, _)| *kind)
        {
            if kind.is_drag() {
                latch.0 = Some(kind);
                nav.pan_dragging
                    .store(kind == NavButton::Pan, Ordering::Relaxed);
                nav.zoom_dragging
                    .store(kind == NavButton::Zoom, Ordering::Relaxed);
            } else if let Some(mut settings) = settings {
                match kind {
                    // Raise the flag rather than move the camera: the orbit
                    // state lives in `renzora_camera`, and the controller
                    // consumes this next frame. Keeps this crate free of a
                    // dependency on the camera one.
                    NavButton::Home => settings.pending_camera_home = true,
                    NavButton::Grid => settings.show_grid = !settings.show_grid,
                    NavButton::Pan | NavButton::Zoom => {}
                }
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        latch.0 = None;
        nav.pan_dragging.store(false, Ordering::Relaxed);
        nav.zoom_dragging.store(false, Ordering::Relaxed);
    }

    let Some(kind) = latch.0 else {
        // Drain so a future latch doesn't pick up pre-drag motion.
        for _ in motion.read() {}
        return;
    };
    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }
    match kind {
        NavButton::Pan => {
            nav.pan_delta_x
                .fetch_add((delta.x * 1000.0) as i32, Ordering::Relaxed);
            nav.pan_delta_y
                .fetch_add((delta.y * 1000.0) as i32, Ordering::Relaxed);
        }
        NavButton::Zoom => {
            // Negated: mouse Y grows downward, and the camera reads a positive
            // delta as "come closer". Dragging **up** zooms in, which is the way
            // round every other zoom in the editor works.
            nav.zoom_delta_y
                .fetch_add((-delta.y * 1000.0) as i32, Ordering::Relaxed);
        }
        // Unreachable: only `is_drag()` buttons ever reach the latch.
        NavButton::Home | NavButton::Grid => {}
    }
}

