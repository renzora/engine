//! Renzora Tutorial — interactive guided tour of the editor.
//!
//! Renders animated overlays (arrows, highlights, cards) that walk the user
//! through the editor basics: adding entities, moving them, adding components,
//! setting up the world, post-processing, blueprints, and more.

pub mod overlay;
pub mod steps;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_egui::egui::{self, Pos2, Rect};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use renzora_editor::{DockTree, DockingState, SplashState};
use renzora_theme::ThemeManager;

use overlay::TutorialAction;
use steps::TutorialStep;

// ── Plugin ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TutorialPlugin");
        app.init_resource::<TutorialState>()
            .add_systems(
                Update,
                check_tutorial_requested.run_if(in_state(SplashState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                tutorial_overlay_system
                    .after(renzora_editor::editor_ui_system)
                    .run_if(in_state(SplashState::Editor))
                    .run_if(|state: Res<TutorialState>| state.active),
            );
    }
}

/// Watches for the `TutorialRequested` marker resource and starts the tutorial.
fn check_tutorial_requested(
    mut commands: Commands,
    requested: Option<Res<renzora::core::TutorialRequested>>,
    mut state: ResMut<TutorialState>,
) {
    if requested.is_some() {
        commands.remove_resource::<renzora::core::TutorialRequested>();
        state.start();
    }
}

// ── State ──────────────────────────────────────────────────────────────────────

/// Persistent state for the tutorial system.
#[derive(Resource)]
pub struct TutorialState {
    /// Whether the tutorial overlay is currently showing.
    pub active: bool,
    /// Index into the steps list.
    pub current_step: usize,
    /// When the current step started (elapsed seconds).
    pub step_start: f64,
    /// Total elapsed time for animation purposes.
    pub elapsed: f64,
    /// Cached step list.
    pub steps: Vec<TutorialStep>,
    /// Total number of steps (cached for overlay).
    pub total_steps: usize,
}

impl Default for TutorialState {
    fn default() -> Self {
        let steps = steps::build_steps();
        let total = steps.len();
        Self {
            active: false,
            current_step: 0,
            step_start: 0.0,
            elapsed: 0.0,
            steps,
            total_steps: total,
        }
    }
}

impl TutorialState {
    /// Start the tutorial from the beginning.
    pub fn start(&mut self) {
        self.active = true;
        self.current_step = 0;
        self.step_start = 0.0;
        self.elapsed = 0.0;
    }

    /// Dismiss the tutorial.
    pub fn dismiss(&mut self) {
        self.active = false;
    }

    fn advance(&mut self) {
        if self.current_step + 1 < self.total_steps {
            self.current_step += 1;
            self.step_start = self.elapsed;
        } else {
            self.dismiss();
        }
    }

    fn go_back(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            self.step_start = self.elapsed;
        }
    }
}

// ── System ─────────────────────────────────────────────────────────────────────

/// Exclusive system that draws the tutorial overlay on top of the editor.
fn tutorial_overlay_system(world: &mut World) {
    // Read time
    let time = world.resource::<Time>().elapsed_secs_f64();

    // Update elapsed on state (temporarily remove to avoid borrow issues)
    let mut state = world.remove_resource::<TutorialState>().unwrap();
    state.elapsed = time;

    if !state.active || state.current_step >= state.steps.len() {
        state.active = false;
        world.insert_resource(state);
        return;
    }

    // Get egui context
    let ctx = {
        let mut sys_state =
            bevy::ecs::system::SystemState::<EguiContexts>::new(world);
        let mut contexts = sys_state.get_mut(world);
        let ctx = match contexts.ctx_mut() {
            Ok(c) => c.clone(),
            Err(_) => {
                world.insert_resource(state);
                return;
            }
        };
        sys_state.apply(world);
        ctx
    };

    // Get theme
    let theme = world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.active_theme.clone())
        .unwrap_or_default();

    // Compute panel rects from the dock tree
    let panel_rects = compute_panel_rects(world, &ctx);

    // Get current step (clone to avoid borrow)
    let step = state.steps[state.current_step].clone();

    // Render the overlay
    let action = overlay::render_overlay(&ctx, &state, &step, &theme, &panel_rects);

    // Handle action
    match action {
        TutorialAction::Next => state.advance(),
        TutorialAction::Back => state.go_back(),
        TutorialAction::Skip => state.dismiss(),
        TutorialAction::None => {}
    }

    // Keep repainting while tutorial is active
    if state.active {
        ctx.request_repaint();
    }

    world.insert_resource(state);
}


// ── Panel rect computation ─────────────────────────────────────────────────────

/// Walk the dock tree to compute the screen rect for each panel.
/// This mirrors the splitting logic in `dock_renderer` to figure out where each
/// panel ends up on screen.
fn compute_panel_rects(world: &World, ctx: &egui::Context) -> HashMap<String, Rect> {
    let mut rects = HashMap::new();

    let Some(docking) = world.get_resource::<DockingState>() else {
        return rects;
    };

    // The central panel occupies the area below title bar + doc tabs and above status bar.
    // Title bar ≈ 28px, doc tabs ≈ 28px, status bar ≈ 20px (approximate).
    let screen = ctx.input(|i| i.viewport_rect());
    let top_offset = 56.0; // title bar + doc tabs
    let bottom_offset = 20.0; // status bar
    let available = Rect::from_min_max(
        Pos2::new(screen.min.x, screen.min.y + top_offset),
        Pos2::new(screen.max.x, screen.max.y - bottom_offset),
    );

    walk_tree(&docking.tree, available, &mut rects);
    rects
}

fn walk_tree(tree: &DockTree, rect: Rect, out: &mut HashMap<String, Rect>) {
    // Tab bar height offset
    const TAB_BAR_H: f32 = 28.0;
    const RESIZE_HANDLE: f32 = 4.0;

    match tree {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (r1, r2) = match direction {
                renzora_editor::SplitDirection::Horizontal => {
                    let mid = rect.min.x + rect.width() * ratio;
                    (
                        Rect::from_min_max(rect.min, Pos2::new(mid - RESIZE_HANDLE / 2.0, rect.max.y)),
                        Rect::from_min_max(Pos2::new(mid + RESIZE_HANDLE / 2.0, rect.min.y), rect.max),
                    )
                }
                renzora_editor::SplitDirection::Vertical => {
                    let mid = rect.min.y + rect.height() * ratio;
                    (
                        Rect::from_min_max(rect.min, Pos2::new(rect.max.x, mid - RESIZE_HANDLE / 2.0)),
                        Rect::from_min_max(Pos2::new(rect.min.x, mid + RESIZE_HANDLE / 2.0), rect.max),
                    )
                }
            };
            walk_tree(first, r1, out);
            walk_tree(second, r2, out);
        }
        DockTree::Leaf { tabs, .. } => {
            // The content area is below the tab bar
            let content = Rect::from_min_max(
                Pos2::new(rect.min.x, rect.min.y + TAB_BAR_H),
                rect.max,
            );
            for tab in tabs {
                out.insert(tab.clone(), content);
            }
        }
        DockTree::Empty => {}
    }
}
