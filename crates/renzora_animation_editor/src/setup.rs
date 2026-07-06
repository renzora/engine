//! Guided setup actions shared by the animation panels' empty states.
//!
//! The animation workflow needs three things wired up before any panel has
//! something to show: a selected entity, an [`AnimatorComponent`] with clip
//! slots, and (optionally) a `.animsm` state machine. Instead of five panels
//! that just say what's missing, the empty states offer the next step
//! directly:
//!
//! - a clickable list of animation candidates in the scene (entities with an
//!   animator, or with a model that clips could be scanned for),
//! - "Scan for clips" — re-runs `.anim` discovery on the selected entity's
//!   model folder (the same discovery that runs automatically on model drop,
//!   re-runnable here for models placed *before* their animations were
//!   imported),
//! - "Create State Machine" — writes a starter `.animsm` (one state per clip)
//!   next to the clips and assigns it on the animator.

use std::hash::{Hash, Hasher};
use std::path::Path;

use bevy::prelude::*;

use renzora::core::{CurrentProject, HideInHierarchy, MeshInstanceData};
use renzora_animation::{
    state_machine::{AnimState, AnimationStateMachine, StateMotion},
    AnimClip, AnimClipSlot, AnimatorComponent, AnimatorState,
};
use renzora_editor_framework::{EditorCommands, EditorSelection, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::AnimationEditorState;

pub struct AnimSetupPlugin;

impl Plugin for AnimSetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SetupFeedback>();
        app.add_systems(
            Update,
            (select_entity_click, scan_clips_click, create_sm_click, create_anim_click, clear_feedback_on_select)
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

/// Result message of the last setup action (e.g. "Found 3 clips" or why a
/// scan came up empty), shown under the action buttons.
#[derive(Resource, Default)]
pub struct SetupFeedback {
    pub for_entity: Option<Entity>,
    pub message: Option<String>,
    pub is_error: bool,
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SelectEntityBtn(Entity);

/// "Scan for clips" — shown when the selected entity has a model but no clips.
#[derive(Component)]
pub struct ScanClipsBtn;

/// "Create State Machine" — shown when the animator has clips but no `.animsm`.
#[derive(Component)]
pub struct CreateSmBtn;

/// "Create Animation" — shown when the selected entity has no clips. Makes an
/// empty `.anim` for authoring property tracks (works on non-model entities
/// like a sun light that have no skeleton to import clips from).
#[derive(Component)]
pub struct CreateAnimBtn;

// ── World accessors ──────────────────────────────────────────────────────────

fn selected_entity(w: &World) -> Option<Entity> {
    w.get_resource::<AnimationEditorState>()?.selected_entity
}

fn selected_model_path(w: &World) -> Option<String> {
    let e = selected_entity(w)?;
    w.get::<MeshInstanceData>(e)?.model_path.clone()
}

/// Whether the "Scan for clips" action applies: a selected entity with a model
/// but no clip slots yet.
pub fn can_scan_clips(w: &World) -> bool {
    let Some(e) = selected_entity(w) else {
        return false;
    };
    selected_model_path(w).is_some()
        && w.get::<AnimatorComponent>(e).is_none_or(|a| a.clips.is_empty())
}

/// Whether the "Create State Machine" action applies: an animator with clips
/// but no `.animsm` assigned.
pub fn can_create_sm(w: &World) -> bool {
    selected_entity(w)
        .and_then(|e| w.get::<AnimatorComponent>(e))
        .is_some_and(|a| !a.clips.is_empty() && a.state_machine.is_none())
}

/// Whether the "Create Animation" action applies: a selected entity with no
/// clip slots (no animator, or an animator that has none yet).
pub fn can_create_anim(w: &World) -> bool {
    let Some(e) = selected_entity(w) else {
        return false;
    };
    w.get::<AnimatorComponent>(e).is_none_or(|a| a.clips.is_empty())
}

/// Feedback line for the selected entity, if any.
pub fn feedback_message(w: &World) -> Option<(String, bool)> {
    let fb = w.get_resource::<SetupFeedback>()?;
    if fb.for_entity != selected_entity(w) {
        return None;
    }
    fb.message.clone().map(|m| (m, fb.is_error))
}

// ── Shared widgets ───────────────────────────────────────────────────────────

/// An accent-tinted action button for empty states: `[icon] label`.
pub fn action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    marker: impl Component,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, accent(), 13.0);
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(accent())),
        ))
        .id();
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

/// The feedback line under setup action buttons (shows the last scan result).
pub fn feedback_label(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
            Node {
                max_width: Val::Px(420.0),
                ..default()
            },
        ))
        .id();
    bind_text(commands, lbl, |w| {
        feedback_message(w).map(|(m, _)| m).unwrap_or_default()
    });
    bind_text_color(commands, lbl, |w| {
        let err = feedback_message(w).is_some_and(|(_, e)| e);
        rgb(if err { (220, 150, 100) } else { (120, 200, 120) })
    });
    lbl
}

/// A clickable list of animation candidates in the scene: entities with an
/// [`AnimatorComponent`], plus model entities clips could be scanned for.
/// Clicking a row selects the entity (all animation panels follow selection).
pub fn candidates_list(commands: &mut Commands) -> Entity {
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(360.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, candidates_snapshot);
    list
}

/// Whether the scene has any animation candidates at all (drives the
/// "import a model first" hint).
pub fn scene_has_candidates(w: &World) -> bool {
    !collect_candidates(w).is_empty()
}

/// (entity, name, has_animator, clip_count)
fn collect_candidates(world: &World) -> Vec<(Entity, String, bool, usize)> {
    let mut rows = Vec::new();
    for archetype in world.archetypes().iter() {
        for arch_entity in archetype.entities() {
            let entity = arch_entity.id();
            let Some(name) = world.get::<Name>(entity) else {
                continue;
            };
            if world.get::<HideInHierarchy>(entity).is_some() {
                continue;
            }
            let animator = world.get::<AnimatorComponent>(entity);
            let has_model = world
                .get::<MeshInstanceData>(entity)
                .is_some_and(|m| m.model_path.is_some());
            if animator.is_none() && !has_model {
                continue;
            }
            let clip_count = animator.map(|a| a.clips.len()).unwrap_or(0);
            rows.push((entity, name.as_str().to_string(), animator.is_some(), clip_count));
        }
    }
    // Animator-bearing entities first, then by name for a stable order.
    rows.sort_by(|a, b| (!a.2, &a.1).cmp(&(!b.2, &b.1)));
    rows
}

fn candidates_snapshot(world: &World) -> KeyedSnapshot {
    let rows = collect_candidates(world);
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(entity, name, has_animator, clips)| {
            let mut k = hasher();
            entity.to_bits().hash(&mut k);
            let mut h = hasher();
            (name, has_animator, clips).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (entity, name, has_animator, clips) = &rows[i];
            candidate_row(c, f, *entity, name, *has_animator, *clips)
        }),
    }
}

fn candidate_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    name: &str,
    has_animator: bool,
    clips: usize,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            SelectEntityBtn(entity),
        ))
        .id();
    let ic = icon_text(
        commands,
        &fonts.phosphor,
        if has_animator { "person-simple-run" } else { "cube" },
        if has_animator { accent() } else { text_muted() },
        13.0,
    );
    let lbl = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let count = commands
        .spawn((
            Text::new(if has_animator {
                format!("{} clip{}", clips, if clips == 1 { "" } else { "s" })
            } else {
                "no clips".to_string()
            }),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_children(&[ic, lbl, count]);
    row
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn select_entity_click(
    q: Query<(&Interaction, &SelectEntityBtn), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
) {
    let Some(selection) = selection else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            selection.set(Some(btn.0));
        }
    }
}

/// Drop stale feedback when the user moves to another entity.
fn clear_feedback_on_select(
    state: Res<AnimationEditorState>,
    mut feedback: ResMut<SetupFeedback>,
) {
    if feedback.message.is_some() && feedback.for_entity != state.selected_entity {
        feedback.for_entity = None;
        feedback.message = None;
        feedback.is_error = false;
    }
}

/// "Scan for clips": re-run `.anim` discovery on the selected entity's model
/// folder and attach/refresh its [`AnimatorComponent`]. This is the manual
/// counterpart of the automatic discovery that runs when a model is dropped —
/// needed when the model entered the scene *before* its animations existed on
/// disk (e.g. re-imported with extraction enabled afterwards).
fn scan_clips_click(
    q: Query<&Interaction, (With<ScanClipsBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    cmds.push(|world: &mut World| {
        let Some(entity) = world
            .get_resource::<AnimationEditorState>()
            .and_then(|s| s.selected_entity)
        else {
            return;
        };
        let Some(project_root) = world
            .get_resource::<CurrentProject>()
            .map(|p| p.path.clone())
        else {
            set_feedback(world, entity, renzora::lang::t("animation.no_project_open"), true);
            return;
        };
        let Some(model_path) = world
            .get::<MeshInstanceData>(entity)
            .and_then(|m| m.model_path.clone())
        else {
            set_feedback(world, entity, renzora::lang::t("animation.selected_no_model"), true);
            return;
        };

        match renzora_animation::discover_animation_clips(&model_path, &project_root) {
            Some(mut animator) => {
                let n = animator.clips.len();
                let default = animator.default_clip.clone();
                // Keep an already-assigned state machine across re-scans.
                animator.state_machine = world
                    .get::<AnimatorComponent>(entity)
                    .and_then(|a| a.state_machine.clone());
                world.entity_mut(entity).insert(animator);
                // Drop the runtime state so the animation graph rebuilds with
                // the discovered clips.
                world.entity_mut(entity).remove::<AnimatorState>();
                if let Some(mut state) = world.get_resource_mut::<AnimationEditorState>() {
                    state.selected_clip = default;
                    state.clip_duration = None;
                    state.auto_fit_clip = None;
                }
                set_feedback(
                    world,
                    entity,
                    format!("Found {} clip{}", n, if n == 1 { "" } else { "s" }),
                    false,
                );
            }
            None => {
                let dir = Path::new(&model_path)
                    .parent()
                    .unwrap_or(Path::new(""))
                    .join("animations")
                    .to_string_lossy()
                    .replace('\\', "/");
                set_feedback(
                    world,
                    entity,
                    format!(
                        "No .anim files in {}/ — re-import the model with \"Extract animations\" enabled",
                        dir
                    ),
                    true,
                );
            }
        }
    });
}

/// "Create State Machine": write a starter `.animsm` (one state per clip,
/// entry = the default clip, no transitions) next to the clips and assign it
/// on the animator. If the file already exists it is assigned, not overwritten.
fn create_sm_click(
    q: Query<&Interaction, (With<CreateSmBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    cmds.push(|world: &mut World| {
        let Some(entity) = world
            .get_resource::<AnimationEditorState>()
            .and_then(|s| s.selected_entity)
        else {
            return;
        };
        let Some(project_root) = world
            .get_resource::<CurrentProject>()
            .map(|p| p.path.clone())
        else {
            set_feedback(world, entity, renzora::lang::t("animation.no_project_open"), true);
            return;
        };
        let Some(animator) = world.get::<AnimatorComponent>(entity) else {
            return;
        };
        if animator.clips.is_empty() {
            set_feedback(world, entity, renzora::lang::t("animation.add_clips_first"), true);
            return;
        }

        let sm = AnimationStateMachine {
            states: animator
                .clips
                .iter()
                .map(|c| AnimState {
                    name: c.name.clone(),
                    motion: StateMotion::Clip(c.name.clone()),
                    speed: 1.0,
                    looping: c.looping,
                })
                .collect(),
            transitions: Vec::new(),
            default_state: animator
                .default_clip
                .clone()
                .or_else(|| animator.clips.first().map(|c| c.name.clone()))
                .unwrap_or_default(),
        };

        // Place it next to the clips: "models/animations/walk.anim" →
        // "models/animations/<stem>.animsm", named after the model file.
        let first_clip_dir = Path::new(&animator.clips[0].path)
            .parent()
            .unwrap_or(Path::new("animations"))
            .to_string_lossy()
            .replace('\\', "/");
        let stem = world
            .get::<MeshInstanceData>(entity)
            .and_then(|m| m.model_path.as_deref().map(String::from))
            .and_then(|p| {
                Path::new(&p)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "state_machine".into());
        let rel_path = format!("{}/{}.animsm", first_clip_dir, stem);
        let abs_path = project_root.join(&rel_path);

        if !abs_path.exists() {
            let ron = match ron::ser::to_string_pretty(
                &sm,
                ron::ser::PrettyConfig::new().indentor("  ".into()),
            ) {
                Ok(ron) => ron,
                Err(e) => {
                    set_feedback(world, entity, format!("Serialize failed: {e}"), true);
                    return;
                }
            };
            if let Some(parent) = abs_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&abs_path, ron) {
                set_feedback(world, entity, format!("Write failed: {e}"), true);
                return;
            }
        }

        if let Some(mut animator) = world.get_mut::<AnimatorComponent>(entity) {
            animator.state_machine = Some(rel_path.clone());
        }
        // Rebuild the runtime state so the state machine asset gets loaded.
        world.entity_mut(entity).remove::<AnimatorState>();
        set_feedback(world, entity, format!("Created {}", rel_path), false);
    });
}

/// "Create Animation": write an empty `.anim` under `animations/<Entity>.anim`,
/// attach an [`AnimatorComponent`] (if missing) with that clip as default, and
/// select it — the starting point for authoring property tracks on any entity.
fn create_anim_click(
    q: Query<&Interaction, (With<CreateAnimBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    cmds.push(|world: &mut World| {
        let Some(entity) = world
            .get_resource::<AnimationEditorState>()
            .and_then(|s| s.selected_entity)
        else {
            return;
        };
        // Empty-state action: name the entity's first clip after the entity.
        let raw_name = world
            .get::<Name>(entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| "entity".into());
        let clip_name = sanitize_clip_name(&raw_name);
        create_clip_on_entity(world, entity, &clip_name);
    });
}

/// Sanitize an arbitrary string into a filesystem/slot-safe clip name
/// (alphanumerics, `_`, `-`; everything else → `_`). Empty input falls back to
/// a generic name so we never build an `animations/.anim` path with no stem.
pub fn sanitize_clip_name(raw: &str) -> String {
    let safe: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if safe.is_empty() { "animation".into() } else { safe }
}

/// Create (or adopt) a clip named `clip_name` on `entity`'s animator and select
/// it. Writes an empty `animations/<clip_name>.anim` if none exists yet, adds a
/// slot for it (making it the default when the animator had none), and resets
/// the timeline onto it.
///
/// Shared by the empty-state "Create Animation" button and the timeline's inline
/// "new clip" field, so one entity can hold several clips — e.g. one per facing
/// direction for a sprite — rather than the single clip the empty-state button
/// alone allowed.
pub fn create_clip_on_entity(world: &mut World, entity: Entity, clip_name: &str) {
    let Some(project_root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone())
    else {
        set_feedback(world, entity, renzora::lang::t("animation.no_project_open"), true);
        return;
    };
    let rel_path = format!("animations/{}.anim", clip_name);
    let abs_path = project_root.join(&rel_path);

    if !abs_path.exists() {
        let clip = AnimClip {
            name: clip_name.to_string(),
            duration: 2.0,
            tracks: Vec::new(),
            property_tracks: Vec::new(),
            markers: Vec::new(),
        };
        if let Err(e) = renzora::core::write_anim_file(&clip, &abs_path) {
            set_feedback(world, entity, format!("Write failed: {e}"), true);
            return;
        }
    }

    // Attach/extend the animator with the clip and make it default when first.
    if let Some(mut animator) = world.get_mut::<AnimatorComponent>(entity) {
        if animator.get_slot(clip_name).is_none() {
            animator.add_clip(AnimClipSlot::new(clip_name.to_string(), rel_path.clone()));
        }
        if animator.default_clip.is_none() {
            animator.default_clip = Some(clip_name.to_string());
        }
    } else {
        let mut animator = AnimatorComponent::new();
        animator.add_clip(AnimClipSlot::new(clip_name.to_string(), rel_path.clone()));
        animator.default_clip = Some(clip_name.to_string());
        world.entity_mut(entity).insert(animator);
    }
    // Rebuild the runtime state so the clip loads.
    world.entity_mut(entity).remove::<AnimatorState>();
    if let Some(mut state) = world.get_resource_mut::<AnimationEditorState>() {
        state.selected_clip = Some(clip_name.to_string());
        state.clip_duration = None;
        state.auto_fit_clip = None;
        state.scrub_time = 0.0;
    }
    set_feedback(world, entity, format!("Created {}", rel_path), false);
}

fn set_feedback(world: &mut World, entity: Entity, message: String, is_error: bool) {
    if let Some(mut fb) = world.get_resource_mut::<SetupFeedback>() {
        fb.for_entity = Some(entity);
        fb.message = Some(message);
        fb.is_error = is_error;
    }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
