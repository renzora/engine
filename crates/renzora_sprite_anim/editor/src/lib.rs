//! Editor-only half of `renzora_sprite_anim` — the **Sprite Anim** panel.
//!
//! The panel has **one job: create clips.** It does not switch the animation
//! image (the timeline keyframes that) and has no grid inputs — it reads the
//! selected sprite's existing [`SpriteSheet`] (hframes/vframes). You pick the
//! cells that make up one animation, name it, and **Create Clip** writes an
//! `.anim` with **two tracks** — `SpriteSheet.frame` (which cell, stepped
//! through your cells) and `SpriteImages.index` (which sheet) — onto the
//! entity's `AnimatorComponent`, so it's a first-class timeline clip. On create
//! it clears and re-focuses the name field for the next clip.
//!
//! Managing the image list and switching sheets is the **Sprite Image**
//! inspector component's job (it holds the list + the keyframeable index); the
//! panel just reads whichever sheet is currently indexed to pick cells from.

mod palette;
mod panel;

use bevy::prelude::*;

use renzora::core::animation::{AnimClip, Interp, PropertyKey, PropertyTrack, TrackValue};
use renzora::core::{CurrentProject, SpriteImagePath, SpriteImages, SpriteSheet};
use renzora::{EditorSelection, SplashState};
use renzora_animation::{AnimClipSlot, AnimatorComponent, AnimatorState};
use renzora_animation_editor::AnimationEditorState;
use renzora_ember::widgets::EmberTextInput;

/// The 2D sprite entity the panel operates on — the selection, when it's a
/// sprite. `None` shows the "select a sprite" hint.
#[derive(Resource, Default)]
pub struct SpriteTarget(pub Option<Entity>);

/// The ordered cells selected in the palette — the frames a **Create Clip** will
/// turn into `SpriteSheet.frame` keyframes, in this order.
#[derive(Resource, Default)]
pub struct SheetSelection {
    pub cells: Vec<UVec2>,
}

/// Read-only mirror of the target's [`SpriteSheet`] grid, for the palette. The
/// panel does **not** author the grid — the Sprite Sheet inspector component
/// does; this just reflects it so the palette overlays + cell math match.
#[derive(Resource)]
pub struct SheetGrid {
    pub cols: u32,
    pub rows: u32,
}

impl Default for SheetGrid {
    fn default() -> Self {
        Self { cols: 1, rows: 1 }
    }
}

/// Frame rate for a newly-created clip (initial keyframe spacing; retime in the
/// timeline afterward).
#[derive(Resource)]
pub struct SheetFps(pub f32);

impl Default for SheetFps {
    fn default() -> Self {
        Self(10.0)
    }
}

/// The sheet image shown in the palette (the target's currently-indexed image).
#[derive(Resource, Default)]
pub struct CurrentSheet(pub Option<String>);

/// Palette zoom (pixels-per-source-pixel).
#[derive(Resource)]
pub struct SheetZoom(pub f32);

impl Default for SheetZoom {
    fn default() -> Self {
        Self(2.0)
    }
}

#[derive(Default)]
pub struct SpriteAnimEditorPlugin;

impl Plugin for SpriteAnimEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SpriteAnimEditorPlugin");
        app.init_resource::<SpriteTarget>()
            .init_resource::<SheetSelection>()
            .init_resource::<SheetGrid>()
            .init_resource::<SheetFps>()
            .init_resource::<CurrentSheet>()
            .init_resource::<SheetZoom>();

        panel::register(app);

        app.add_systems(
            Update,
            (sync_target, sync_current_sheet, sync_grid_from_sheet, create_clip_button)
                .run_if(in_state(SplashState::Editor)),
        );
        palette::register(app);
    }
}

renzora::add!(SpriteAnimEditorPlugin, Editor);

/// Point [`SpriteTarget`] at the selected sprite (or clear it).
fn sync_target(
    selection: Res<EditorSelection>,
    sprites: Query<(), Or<(With<SpriteImagePath>, With<SpriteImages>, With<SpriteSheet>, With<Sprite>)>>,
    mut target: ResMut<SpriteTarget>,
) {
    let want = selection.get().filter(|&e| sprites.contains(e));
    if target.0 != want {
        target.0 = want;
    }
}

/// Mirror the target's currently-indexed image path into [`CurrentSheet`].
fn sync_current_sheet(
    target: Res<SpriteTarget>,
    images: Query<&SpriteImages>,
    paths: Query<&SpriteImagePath>,
    mut current: ResMut<CurrentSheet>,
) {
    let want = target.0.and_then(|e| {
        images
            .get(e)
            .ok()
            .and_then(|imgs| imgs.active_path().map(|s| s.to_string()))
            .or_else(|| paths.get(e).ok().map(|p| p.0.clone()))
    });
    if current.0 != want {
        current.0 = want;
    }
}

/// Reflect the target's [`SpriteSheet`] grid into [`SheetGrid`] (read-only — the
/// grid is authored in the Sprite Sheet inspector component, `1×1` if absent).
fn sync_grid_from_sheet(
    target: Res<SpriteTarget>,
    sheets: Query<&SpriteSheet>,
    mut grid: ResMut<SheetGrid>,
) {
    let (c, r) = target
        .0
        .and_then(|e| sheets.get(e).ok())
        .map(|s| (s.hframes.max(1), s.vframes.max(1)))
        .unwrap_or((1, 1));
    if grid.cols != c {
        grid.cols = c;
    }
    if grid.rows != r {
        grid.rows = r;
    }
}

/// **Create Clip**: two tracks onto the target's `AnimatorComponent` —
/// `SpriteSheet.frame` (stepped through the selected cells) and, when the sprite
/// has a `SpriteImages` library, `SpriteImages.index` (which sheet). Then clear
/// and re-focus the name field for the next clip.
#[allow(clippy::too_many_arguments)]
fn create_clip_button(
    buttons: Query<&Interaction, (Changed<Interaction>, With<panel::CreateClipButton>)>,
    target: Res<SpriteTarget>,
    grid: Res<SheetGrid>,
    fps: Res<SheetFps>,
    images: Query<&SpriteImages>,
    mut name_input: Query<&mut EmberTextInput, With<panel::ClipNameInput>>,
    mut sel: ResMut<SheetSelection>,
    mut commands: Commands,
) {
    if !buttons.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(entity) = target.0 else { return };
    if sel.cells.is_empty() {
        return;
    }
    let cols = grid.cols.max(1);
    let fps = fps.0.max(1.0);

    let mut name = name_input.iter().next().map(|i| i.value.trim().to_string()).unwrap_or_default();
    if name.is_empty() {
        name = "clip".to_string();
    }

    // Track 1 — `SpriteSheet.frame`, one stepped key per selected cell.
    let frame_keys: Vec<PropertyKey> = sel
        .cells
        .iter()
        .enumerate()
        .map(|(i, c)| PropertyKey {
            time: i as f32 / fps,
            value: TrackValue::Float((c.y * cols + c.x) as f32),
            interp: Interp::Stepped,
        })
        .collect();
    let duration = (sel.cells.len() as f32 / fps).max(1.0 / fps);

    let mut property_tracks = vec![PropertyTrack {
        target: "self".to_string(),
        // EXACT reflected short type-name (the write path is separator-sensitive).
        component: "SpriteSheet".to_string(),
        field: "frame".to_string(),
        keys: frame_keys,
    }];

    // Track 2 — `SpriteImages.index`, a one-key pin to this clip's sheet, so
    // switching to this clip switches the sheet (and away from it too).
    if let Some(imgs) = target.0.and_then(|e| images.get(e).ok()) {
        property_tracks.push(PropertyTrack {
            target: "self".to_string(),
            component: "SpriteImages".to_string(),
            field: "index".to_string(),
            keys: vec![PropertyKey {
                time: 0.0,
                value: TrackValue::Float(imgs.index as f32),
                interp: Interp::Stepped,
            }],
        });
    }

    let clip = AnimClip {
        name: name.clone(),
        duration,
        tracks: Vec::new(),
        property_tracks,
        markers: Vec::new(),
    };

    commands.queue(move |world: &mut World| {
        create_sprite_clip(world, entity, &name, &clip);
    });

    // Clear + re-focus the name field for the next clip.
    if let Some(mut input) = name_input.iter_mut().next() {
        input.value.clear();
        input.focused = true;
    }
    sel.cells.clear();
}

/// Write the clip's `.anim` and wire it onto the entity's `AnimatorComponent` —
/// the same sequence the timeline's `create_clip_on_entity` runs, seeded with
/// our property tracks.
fn create_sprite_clip(world: &mut World, entity: Entity, name: &str, clip: &AnimClip) {
    let Some(project_root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        warn!("[sprite_anim] Create Clip: no project open");
        return;
    };
    let rel = format!("animations/{name}.anim");
    let abs = project_root.join(&rel);
    if let Err(e) = renzora::core::animation::write_anim_file(clip, &abs) {
        warn!("[sprite_anim] Create Clip: failed to write {rel}: {e}");
        return;
    }

    if let Some(mut animator) = world.get_mut::<AnimatorComponent>(entity) {
        if animator.get_slot(name).is_none() {
            animator.add_clip(AnimClipSlot::new(name.to_string(), rel.clone()));
        }
        if animator.default_clip.is_none() {
            animator.default_clip = Some(name.to_string());
        }
    } else {
        let mut animator = AnimatorComponent::new();
        animator.add_clip(AnimClipSlot::new(name.to_string(), rel.clone()));
        animator.default_clip = Some(name.to_string());
        world.entity_mut(entity).insert(animator);
    }
    world.entity_mut(entity).remove::<AnimatorState>();

    if let Some(mut state) = world.get_resource_mut::<AnimationEditorState>() {
        state.selected_entity = Some(entity);
        state.selected_clip = Some(name.to_string());
        state.clip_duration = None;
        state.auto_fit_clip = None;
        state.scrub_time = 0.0;
    }
    info!("[sprite_anim] Created clip '{name}' → {rel}");
}
