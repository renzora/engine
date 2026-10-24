//! Onion skinning on the stage: faint copies of the character posed a few
//! frames before and after the playhead, so the arc and spacing of a motion
//! can be judged without scrubbing back and forth.
//!
//! Each ghost is one more instance of the model the stage shows (the same
//! scene asset as the stage copy, see [`crate::studio_preview`]), on the
//! stage's render layer, with every mesh given one flat, see-through material:
//! warm for the frames behind the playhead, cool for the frames ahead, fading
//! with distance. The ghosts are posed straight from the open clip's edit
//! buffer with the same [`ChannelSpline`] playback uses, so a key edited in
//! the timeline or reshaped in the Curves view moves its ghosts the same
//! frame; a bone the clip does not key holds whatever the character holds now.
//!
//! The ghosts are not [`StudioPreviewModel`]s on purpose: the stage pairs its
//! bones with the real model's through that marker, frames the camera on it
//! and draws its skeleton, and a ghost must do none of those. They are hidden
//! during playback, where they would only trail the motion they are meant to
//! let you study.

use std::collections::HashMap;

use bevy::animation::AnimationTargetId;
use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use renzora::core::{EditorLocked, HideInHierarchy};
use renzora::ChannelSpline;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{menu_item, screen_menu};
use renzora_ember::UiInteraction;

use super::{StageMode, StageUi};
use crate::rig::on_click;
use crate::studio_preview::{StudioPreviewModel, StudioPreviewTracker, STUDIO_PREVIEW_LAYER};
use crate::timeline::clip::TimelineClip;

/// The editor steps the playhead a frame at 30 fps; ghosts are spaced in
/// those frames.
const FRAME: f32 = 1.0 / 30.0;
/// Frames behind the playhead: a warm orange. Frames ahead: a cool blue. The
/// convention other packages use, so it reads without a legend.
const BEFORE: (u8, u8, u8) = (255, 132, 84);
const AFTER: (u8, u8, u8) = (88, 182, 255);

/// What the onion skin shows. Editor state, not saved with the scene.
#[derive(Resource, Clone, PartialEq)]
pub(crate) struct OnionSkin {
    pub(crate) enabled: bool,
    /// Ghosts behind and ahead of the playhead.
    pub(crate) before: u32,
    pub(crate) after: u32,
    /// Frames between ghosts.
    pub(crate) step: u32,
    /// The nearest ghost's opacity; further ones fade from it.
    pub(crate) opacity: f32,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self { enabled: false, before: 2, after: 2, step: 3, opacity: 0.3 }
    }
}

impl OnionSkin {
    /// The frame offsets wanted, nearest first on each side.
    fn offsets(&self) -> Vec<i32> {
        let before = (1..=self.before as i32).map(|k| -k);
        let after = 1..=self.after as i32;
        before.chain(after).collect()
    }

    fn alpha(&self, offset: i32) -> f32 {
        let n = if offset < 0 { self.before } else { self.after }.max(1) as f32;
        let far = (offset.unsigned_abs() as f32 - 1.0) / n;
        self.opacity * (1.0 - far * 0.6)
    }
}

/// A ghost's root. How far from the playhead it sits is in [`OnionState`].
#[derive(Component)]
pub(crate) struct OnionGhost;

/// An entity inside a ghost that has been given the ghost's layer and, for a
/// mesh, its material, so it is not walked again.
#[derive(Component)]
struct OnionStamped;

/// The ghosts that exist, the scene they were made from, their materials and
/// their bones.
#[derive(Resource, Default)]
struct OnionState {
    scene: Option<Handle<bevy::world_serialization::WorldAsset>>,
    ghosts: Vec<(Entity, i32)>,
    materials: HashMap<i32, Handle<StandardMaterial>>,
    /// The settings the materials were made for.
    tinted: Option<OnionSkin>,
    /// Each ghost's named nodes, with the target the clip binds them by.
    bones: HashMap<Entity, Vec<(Entity, String, AnimationTargetId)>>,
}

/// The open clip's bone channels, built once per edit rather than per ghost
/// per frame.
#[derive(Resource, Default)]
struct OnionCurves {
    channels: HashMap<AnimationTargetId, [Option<ChannelSpline>; 3]>,
    duration: f32,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<OnionSkin>().init_resource::<OnionState>().init_resource::<OnionCurves>();
    app.add_observer(stamp_new_ghost_child);
    app.add_systems(
        Update,
        (rebuild_curves, sync_ghosts, stamp_ghosts)
            .chain()
            .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
    app.add_systems(
        PostUpdate,
        pose_ghosts
            .after(crate::timeline::pose::apply_pose)
            .before(bevy::transform::TransformSystems::Propagate)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

fn rebuild_curves(cache: Res<TimelineClip>, mut curves: ResMut<OnionCurves>, skin: Res<OnionSkin>) {
    if !skin.enabled || !(cache.is_changed() || skin.is_changed()) {
        return;
    }
    curves.channels.clear();
    let Some(clip) = cache.clip.as_ref() else { return };
    curves.duration = clip.duration;
    for t in clip.tracks.iter().filter(|t| !t.muted) {
        let chan = |c: u8| (t.key_count(c) > 0).then(|| t.channel_spline(c));
        curves.channels.insert(renzora_animation::bone_target(&t.bone_name), [chan(0), chan(1), chan(2)]);
    }
}

/// The scene the stage copy was spawned from: the [`StudioPreviewModel`]
/// root's scene child.
fn stage_scene(world: &mut World) -> Option<Handle<bevy::world_serialization::WorldAsset>> {
    let root = world.query_filtered::<Entity, With<StudioPreviewModel>>().iter(world).next()?;
    let kids: Vec<Entity> = world.get::<Children>(root)?.iter().collect();
    kids.into_iter().find_map(|k| world.get::<bevy::world_serialization::WorldAssetRoot>(k).map(|r| r.0.clone()))
}

fn wanted(world: &World) -> bool {
    world.get_resource::<OnionSkin>().is_some_and(|s| s.enabled)
        && world.get_resource::<StageUi>().is_some_and(|u| u.mode == StageMode::Animate)
        && world.get_resource::<renzora_ember::dock::Dock>().is_some_and(|d| d.tree.contains_panel(super::STAGE_PANEL))
        && world.get_resource::<StudioPreviewTracker>().is_some_and(|t| t.has_model)
}

/// Spawn and despawn ghosts to match the settings and the stage's model, and
/// keep their materials tinted to the settings.
fn sync_ghosts(world: &mut World) {
    let scene = if wanted(world) { stage_scene(world) } else { None };
    let skin = world.resource::<OnionSkin>().clone();
    let offsets = if scene.is_some() { skin.offsets() } else { Vec::new() };
    let (stale, have) = {
        let st = world.resource::<OnionState>();
        (st.scene != scene, st.ghosts.clone())
    };
    // A different model (or none): every ghost goes, and is remade below.
    let keep: Vec<(Entity, i32)> = have.iter().copied().filter(|(_, o)| !stale && offsets.contains(o)).collect();
    for (e, _) in have.iter().filter(|g| !keep.contains(g)) {
        if let Ok(em) = world.get_entity_mut(*e) {
            em.despawn();
        }
    }
    let mut ghosts = keep;
    if let Some(scene) = scene.clone() {
        for &offset in &offsets {
            if ghosts.iter().any(|(_, o)| *o == offset) {
                continue;
            }
            let root = world
                .spawn((
                    Transform::default(),
                    Visibility::Hidden,
                    RenderLayers::layer(STUDIO_PREVIEW_LAYER),
                    OnionGhost,
                    HideInHierarchy,
                    EditorLocked,
                    Name::new("Onion Ghost"),
                ))
                .id();
            world.spawn((
                bevy::world_serialization::WorldAssetRoot(scene.clone()),
                Transform::default(),
                Visibility::Inherited,
                RenderLayers::layer(STUDIO_PREVIEW_LAYER),
                HideInHierarchy,
                ChildOf(root),
            ));
            ghosts.push((root, offset));
        }
    }
    // One material per ghost: its side's colour at its own opacity.
    let retint = world.resource::<OnionState>().tinted.as_ref() != Some(&skin);
    let mut materials = std::mem::take(&mut world.resource_mut::<OnionState>().materials);
    materials.retain(|o, _| offsets.contains(o));
    {
        let mut assets = world.resource_mut::<Assets<StandardMaterial>>();
        for &offset in &offsets {
            let (r, g, b) = if offset < 0 { BEFORE } else { AFTER };
            let color = Color::srgba_u8(r, g, b, 255).with_alpha(skin.alpha(offset));
            match materials.get(&offset) {
                Some(h) if retint => {
                    if let Some(mut m) = assets.get_mut(h) {
                        m.base_color = color;
                    }
                }
                Some(_) => {}
                None => {
                    let h = assets.add(StandardMaterial { base_color: color, alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, ..default() });
                    materials.insert(offset, h);
                }
            }
        }
    }
    let mut st = world.resource_mut::<OnionState>();
    st.materials = materials;
    st.tinted = Some(skin);
    st.scene = scene;
    st.bones.retain(|g, _| ghosts.iter().any(|(e, _)| e == g));
    st.ghosts = ghosts;
}

/// A node joining a ghost gets the stage's layer at once, before it can be
/// drawn by the scene camera or listed in the hierarchy for a frame. The same
/// job the studio preview's own observer does for the stage copy.
fn stamp_new_ghost_child(trigger: On<Insert<ChildOf>>, parents: Query<&ChildOf>, ghosts: Query<(), With<OnionGhost>>, mut commands: Commands) {
    let mut cursor = trigger.entity;
    while let Ok(c) = parents.get(cursor) {
        if ghosts.contains(c.parent()) {
            commands.entity(trigger.entity).try_insert((RenderLayers::layer(STUDIO_PREVIEW_LAYER), HideInHierarchy));
            return;
        }
        cursor = c.parent();
    }
}

/// Give each ghost's meshes its material (as the scene spawns them, over a few
/// frames), and collect each ghost's bones once it has them.
fn stamp_ghosts(world: &mut World) {
    let ghosts = world.resource::<OnionState>().ghosts.clone();
    if ghosts.is_empty() {
        return;
    }
    for (root, offset) in ghosts {
        let Some(mat) = world.resource::<OnionState>().materials.get(&offset).cloned() else { continue };
        let nodes = crate::rig::descendants(world, root);
        let fresh: Vec<Entity> = nodes.iter().copied().filter(|&e| e != root && world.get::<OnionStamped>(e).is_none()).collect();
        for e in &fresh {
            let is_mesh = world.get::<Mesh3d>(*e).is_some();
            let mut em = world.entity_mut(*e);
            em.insert((OnionStamped, RenderLayers::layer(STUDIO_PREVIEW_LAYER), HideInHierarchy));
            if is_mesh {
                em.insert((MeshMaterial3d(mat.clone()), NotShadowCaster));
            }
        }
        let known = world.resource::<OnionState>().bones.get(&root).map(Vec::len).unwrap_or(0);
        if !fresh.is_empty() || known == 0 {
            let bones: Vec<(Entity, String, AnimationTargetId)> = nodes
                .into_iter()
                .filter(|&e| e != root)
                .filter_map(|e| {
                    let name = crate::rig::bone_name(world, e)?;
                    let target = renzora_animation::bone_target(&name);
                    Some((e, name, target))
                })
                .collect();
            world.resource_mut::<OnionState>().bones.insert(root, bones);
        }
    }
}

/// Pose every ghost at its time: the clip's value for each keyed channel,
/// the character's own pose for everything else. After the hand-made pose is
/// applied (so an unkeyed pose shows on every ghost too) and before
/// transforms propagate.
fn pose_ghosts(world: &mut World) {
    let ghosts = world.resource::<OnionState>().ghosts.clone();
    if ghosts.is_empty() {
        return;
    }
    let (scrub, playing) = world.get_resource::<crate::AnimationEditorState>().map(|s| (s.scrub_time, s.is_previewing)).unwrap_or((0.0, true));
    let step = world.resource::<OnionSkin>().step.max(1) as f32 * FRAME;
    let stage_root = world.query_filtered::<&Transform, With<StudioPreviewModel>>().iter(world).next().copied().unwrap_or_default();
    let mut writes: Vec<(Entity, Transform)> = Vec::new();
    let mut shown: Vec<(Entity, Visibility)> = Vec::new();
    {
        let curves = world.resource::<OnionCurves>();
        let state = world.resource::<OnionState>();
        let rig = world.resource::<super::clone::StageRig>();
        for (root, offset) in &ghosts {
            let t = scrub + *offset as f32 * step;
            let visible = !playing && t >= -1e-4 && t <= curves.duration + 1e-4;
            shown.push((*root, if visible { Visibility::Inherited } else { Visibility::Hidden }));
            if !visible {
                continue;
            }
            writes.push((*root, stage_root));
            for (e, name, target) in state.bones.get(root).map(Vec::as_slice).unwrap_or(&[]) {
                let Some(base) = rig.real_by_name.get(name).and_then(|r| world.get::<Transform>(*r)).copied().or_else(|| world.get::<Transform>(*e).copied()) else {
                    continue;
                };
                let mut pose = base;
                if let Some([tr, ro, sc]) = curves.channels.get(target) {
                    if let Some(s) = tr {
                        pose.translation = s.sample_vec3(t);
                    }
                    if let Some(s) = ro {
                        pose.rotation = s.sample_quat(t);
                    }
                    if let Some(s) = sc {
                        pose.scale = s.sample_vec3(t);
                    }
                }
                writes.push((*e, pose));
            }
        }
    }
    for (e, v) in shown {
        if let Some(mut vis) = world.get_mut::<Visibility>(e) {
            if *vis != v {
                *vis = v;
            }
        }
    }
    for (e, t) in writes {
        if let Some(mut tr) = world.get_mut::<Transform>(e) {
            if *tr != t {
                *tr = t;
            }
        }
    }
}

// ── Toolbar ──────────────────────────────────────────────────────────────────

fn is_on(w: &Rx) -> bool {
    w.get_resource::<OnionSkin>().is_some_and(|s| s.enabled)
}

/// The onion skin's place on the stage toolbar: a toggle, and a caret opening
/// its settings (how many ghosts, which sides, how far apart). Shown in
/// Animate, the only mode that poses.
pub(crate) fn toolbar_toggle(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(0.0), flex_shrink: 0.0, ..default() }).id();
    let toggle = pill(commands, fonts, "ghost", "Onion", is_on, |w| {
        let mut s = w.resource_mut::<OnionSkin>();
        s.enabled = !s.enabled;
    });
    commands.entity(toggle).insert(renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or(
        "stage.onion_tip",
        "Onion skin: ghosts of the pose a few frames before (orange) and after (blue) the playhead",
    )));
    let caret = pill(commands, fonts, "caret-down", "", |_| false, open_settings);
    commands.entity(caret).entry::<Node>().and_modify(|mut n| n.min_width = Val::Px(18.0));
    commands.entity(wrap).add_children(&[toggle, caret]);
    bind_display(commands, wrap, |w| w.get_resource::<StageUi>().is_some_and(|u| u.mode == StageMode::Animate));
    wrap
}

/// A toolbar button in the stage's own style (see `view::pill`), lit while `on`.
fn pill(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    on: impl for<'w> Fn(&Rx<'w>) -> bool + Send + Sync + Clone + 'static,
    click: impl Fn(&mut World) + Send + Sync + 'static,
) -> Entity {
    let size = 26.0;
    let b = commands
        .spawn((
            Node {
                height: Val::Px(size),
                padding: UiRect::horizontal(Val::Px(if label.is_empty() { 0.0 } else { 10.0 })),
                min_width: Val::Px(size),
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let lit = on.clone();
    bind_bg(commands, b, move |w| {
        let hot = matches!(w.get::<UiInteraction>(b), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        match (lit(w), hot) {
            (true, _) => rgb(accent()),
            (false, true) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), if label.is_empty() { 11.0 } else { 14.0 });
    let lit = on.clone();
    bind_text_color(commands, ic, move |w| if lit(w) { rgb(on_accent()) } else { rgb(text_muted()) });
    commands.entity(b).add_child(ic);
    if !label.is_empty() {
        let t = commands.spawn((Text::new(renzora::lang::t_or("stage.onion", label)), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
        bind_text_color(commands, t, move |w| if on(w) { rgb(on_accent()) } else { rgb(text_primary()) });
        commands.entity(b).add_child(t);
    }
    on_click(commands, b, click);
    b
}

/// The settings menu, under the cursor.
fn open_settings(world: &mut World) {
    let Some(at) = world.query::<&Window>().iter(world).find_map(|w| w.cursor_position()) else { return };
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let skin = world.resource::<OnionSkin>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let menu = screen_menu(&mut commands, at.x, at.y + 10.0);
        let mut kids = Vec::new();
        let tick = |on: bool, icon: &'static str| if on { "check" } else { icon };
        let each = skin.before.max(skin.after);
        for n in [1u32, 2, 3, 5] {
            let label = match n {
                1 => renzora::lang::t_or("stage.onion_one", "1 ghost each side"),
                _ => format!("{n} {}", renzora::lang::t_or("stage.onion_ghosts", "ghosts each side")),
            };
            kids.push(menu_item(&mut commands, &fonts, tick(each == n, "ghost"), &label, move |w| {
                let mut s = w.resource_mut::<OnionSkin>();
                s.enabled = true;
                if s.before > 0 {
                    s.before = n;
                }
                if s.after > 0 {
                    s.after = n;
                }
            }));
        }
        let sides = [
            ("stage.onion_both", "Before and after", true, true),
            ("stage.onion_before", "Before only", true, false),
            ("stage.onion_after", "After only", false, true),
        ];
        for (key, fallback, before, after) in sides {
            let on = (skin.before > 0) == before && (skin.after > 0) == after;
            kids.push(menu_item(&mut commands, &fonts, tick(on, "arrows-horizontal"), &renzora::lang::t_or(key, fallback), move |w| {
                let mut s = w.resource_mut::<OnionSkin>();
                let n = s.before.max(s.after).max(1);
                s.enabled = true;
                s.before = if before { n } else { 0 };
                s.after = if after { n } else { 0 };
            }));
        }
        for step in [1u32, 2, 3, 5, 10] {
            let label = match step {
                1 => renzora::lang::t_or("stage.onion_every_frame", "Every frame"),
                _ => format!("{} {step} {}", renzora::lang::t_or("stage.onion_every", "Every"), renzora::lang::t_or("stage.onion_frames", "frames")),
            };
            kids.push(menu_item(&mut commands, &fonts, tick(skin.step == step, "timer"), &label, move |w| {
                let mut s = w.resource_mut::<OnionSkin>();
                s.enabled = true;
                s.step = step;
            }));
        }
        commands.entity(menu).add_children(&kids);
    }
    queue.apply(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_are_nearest_first_on_each_side() {
        let s = OnionSkin { before: 2, after: 1, ..default() };
        assert_eq!(s.offsets(), vec![-1, -2, 1]);
        let s = OnionSkin { before: 0, after: 2, ..default() };
        assert_eq!(s.offsets(), vec![1, 2]);
    }

    #[test]
    fn further_ghosts_are_fainter() {
        let s = OnionSkin { before: 3, after: 3, opacity: 0.3, ..default() };
        assert!((s.alpha(-1) - 0.3).abs() < 1e-6);
        assert!(s.alpha(-3) < s.alpha(-2) && s.alpha(-2) < s.alpha(-1));
        assert!(s.alpha(3) > 0.0);
    }
}
