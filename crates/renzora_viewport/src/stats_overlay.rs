//! The statistics readout — a small block of numbers in the scene's bottom-left
//! corner: how many objects are in the scene, how much geometry they add up to,
//! and how tall the terrain you have in hand actually is.
//!
//! **Why the bottom-left.** Every other piece of viewport chrome is already
//! spoken for: the axis gizmo sits top-right with the nav cluster hanging under
//! it, the height ruler slides in on the right, and the tool shelf runs down
//! from the top-left. The bottom-left is the one corner nothing else wants, and
//! a readout you glance at rather than aim at belongs where it can't be in the
//! way of the thing you're aiming at.
//!
//! **It doesn't take the pointer.** [`Pickable::IGNORE`] and no
//! `OverlaySurface`, so a click in that corner reaches the scene underneath and
//! selects whatever is there. A passive readout that swallowed clicks would
//! quietly cost you a corner of the viewport, and there is nothing here to
//! click — the numbers aren't controls.
//!
//! **Off by default**, on the Display dropdown's Overlays section. The numbers
//! are something you go looking for rather than something you want in the
//! corner of every session, and the whole readout costs nothing while hidden —
//! [`update_stats`] skips its expensive half when the setting is off.
//!
//! **Where the numbers come from.** [`ViewportStats`], refreshed by
//! [`update_stats`]. Two halves with very different costs, which is why they're
//! updated differently:
//!
//! - **Geometry totals** are cheap per *instance*, not per triangle: a mesh's
//!   vertex count is the length of an attribute it already stores, so counting
//!   is one asset lookup per `Mesh3d`. Still throttled, because a big scene has
//!   a lot of instances and nobody reads a counter at 120 Hz.
//! - **Terrain elevation** is genuinely expensive — folding a min/max over every
//!   chunk's heightfield is millions of floats on a large terrain. So it is
//!   cached per chunk and refolded only for chunks whose data actually changed,
//!   which during a sculpt stroke is the handful under the brush.
//!
//! That second half is also why this system has no `run_if` and does its own
//! throttling internally. Change detection is measured against the system's
//! last run, and the scheduler advances that whether or not the body returned
//! early — so a system that skips itself on a timer sees `is_changed()` go
//! false for edits that happened while it was skipping, and the cache silently
//! goes stale. The timer therefore guards only the part that has no change
//! detection to lose.
//!
//! The same trap is why the cache is emptied when the overlay is *reopened*
//! rather than kept across a hidden spell. Skipping the fold while hidden has
//! exactly the failure above — every edit made in the meantime is invisible —
//! so the cache is thrown away instead of trusted, and one full refold pays
//! for the whole time it was off.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use renzora::core::viewport_types::ViewportSettings;
use renzora::core::PlayModeState;
use renzora_editor_framework::EditorSelection;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_text};
use renzora_ember::theme::{panel_bg, rgb, text_muted, text_primary};
use renzora_terrain::data::{TerrainChunkData, TerrainData};

/// How often the geometry totals are recounted. Fast enough that adding a model
/// updates the readout while you're still looking at it, slow enough that the
/// per-instance walk is nowhere near the frame budget.
const REFRESH_SECS: f32 = 0.25;

/// The numbers on display. Recomputed by [`update_stats`]; the rows read it
/// through ordinary reactive bindings, so a value that doesn't change doesn't
/// touch the UI.
#[derive(Resource, Default)]
pub struct ViewportStats {
    /// `Mesh3d` instances in the scene — objects as the renderer counts them,
    /// which is not the same as entities in the hierarchy.
    pub objects: u64,
    pub vertices: u64,
    pub triangles: u64,
    /// The terrain the readout is describing, if any. `None` when the scene has
    /// no terrain, or has several and none of them is selected.
    pub terrain: Option<TerrainStats>,
}

/// The elevation half of the readout, for one terrain.
#[derive(PartialEq, Clone, Copy)]
pub struct TerrainStats {
    /// Lowest and highest point of the *actual* heightfield, in world metres.
    pub low: f32,
    pub high: f32,
    /// The envelope those heights are stored inside — `TerrainData`'s
    /// `min_height`/`max_height`, offset into world space. Shown next to the
    /// relief because the pair is the useful thing: the relief alone doesn't
    /// tell you whether you have headroom left to sculpt into.
    pub floor: f32,
    pub ceiling: f32,
}

/// Per-chunk normalized (min, max), kept between runs so a sculpt stroke only
/// refolds the chunks it touched. Keyed by chunk entity; entries for despawned
/// chunks are dropped on the next full pass.
#[derive(Resource, Default)]
struct ChunkReliefCache(HashMap<Entity, (f32, f32)>);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ViewportStats>()
        .init_resource::<ChunkReliefCache>()
        .add_systems(Update, update_stats);
}

/// Build the readout for a viewport's content node. Slot 0 only — the numbers
/// describe the scene, not the view, so four copies would say the same thing
/// four times.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                // Wide enough that the value column doesn't jitter left and
                // right as digits are added — a number that moves while you
                // watch it is harder to read than one that doesn't.
                min_width: Val::Px(136.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            // Translucent rather than solid: it sits over the render, and at
            // full opacity a block of chrome in the corner reads as a hole in
            // the scene rather than as something laid on top of it.
            BackgroundColor(rgb(panel_bg()).with_alpha(0.72)),
            bevy::picking::Pickable::IGNORE,
            bevy::ui::FocusPolicy::Pass,
            Name::new("vp-stats"),
        ))
        .id();

    // Hidden in play mode — the game's own HUD owns the screen then. It was
    // also hidden in UI view, when the UI editor covered this panel and there
    // was no 3D scene behind the stats to count; the editor is its own panel
    // now, so the viewport always has a scene.
    bind_display(commands, root, |w| {
        w.get_resource::<ViewportSettings>()
            .is_some_and(|s| s.show_stats)
            && !w
                .get_resource::<PlayModeState>()
                .is_some_and(|p| p.is_in_play_mode())
    });

    let objects = stat_row(commands, fonts, &renzora::lang::t("viewport.stats.objects"), |w| {
        thousands(stats(w, |s| s.objects))
    });
    let verts = stat_row(commands, fonts, &renzora::lang::t("viewport.stats.verts"), |w| {
        thousands(stats(w, |s| s.vertices))
    });
    let tris = stat_row(commands, fonts, &renzora::lang::t("viewport.stats.tris"), |w| {
        thousands(stats(w, |s| s.triangles))
    });
    commands.entity(root).add_children(&[objects, verts, tris]);

    // The terrain pair appears only when there is a terrain to describe, so the
    // block stays three rows tall in an ordinary scene.
    let height = stat_row(commands, fonts, &renzora::lang::t("viewport.stats.height"), |w| {
        terrain_stats(w)
            .map(|t| format!("{} – {}", metres(t.low), metres(t.high)))
            .unwrap_or_default()
    });
    let range = stat_row(commands, fonts, &renzora::lang::t("viewport.stats.range"), |w| {
        terrain_stats(w)
            .map(|t| format!("{} – {}", metres(t.floor), metres(t.ceiling)))
            .unwrap_or_default()
    });
    for row in [height, range] {
        bind_display(commands, row, |w| terrain_stats(w).is_some());
    }
    commands.entity(root).add_children(&[height, range]);

    root
}

/// One `Label   value` line. The label is muted and the value is not: the labels
/// are the same five words every time and you stop reading them, while the
/// numbers are the entire point.
fn stat_row<F>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: F) -> Entity
where
    F: for<'w> Fn(&renzora_ember::reactive::Rx<'w>) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("vp-stats-row"),
        ))
        .id();

    let name = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let val = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, val, value);

    commands.entity(row).add_children(&[name, val]);
    row
}

fn stats<T>(w: &renzora_ember::reactive::Rx, f: impl Fn(&ViewportStats) -> T) -> T
where
    T: Default,
{
    w.get_resource::<ViewportStats>().map(f).unwrap_or_default()
}

fn terrain_stats(w: &renzora_ember::reactive::Rx) -> Option<TerrainStats> {
    w.get_resource::<ViewportStats>().and_then(|s| s.terrain)
}

/// Group digits so a seven-figure triangle count is readable at a glance —
/// `1,284,096` rather than `1284096`. Deliberately not abbreviated to `1.3M`:
/// the counter's job is to show you a number *changing*, and an abbreviation
/// sits still through the first hundred thousand triangles you add.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// One decimal place and a unit. Terrain elevations are metres and the useful
/// precision is decimetres — more digits is noise on a number that moves as you
/// sculpt.
fn metres(v: f32) -> String {
    format!("{v:.1}m")
}

/// Refresh [`ViewportStats`]. See the module docs for why the two halves are
/// timed differently and why this runs unconditionally.
#[allow(clippy::too_many_arguments)]
fn update_stats(
    time: Res<Time>,
    mut elapsed: Local<f32>,
    mut was_shown: Local<bool>,
    settings: Option<Res<ViewportSettings>>,
    selection: Option<Res<EditorSelection>>,
    mut stats: ResMut<ViewportStats>,
    mut cache: ResMut<ChunkReliefCache>,
    meshes: Res<Assets<Mesh>>,
    mesh_instances: Query<&Mesh3d>,
    chunks: Query<(Entity, Ref<TerrainChunkData>, &ChildOf)>,
    terrains: Query<(Entity, &TerrainData, &GlobalTransform)>,
) {
    let shown = settings.as_ref().is_some_and(|s| s.show_stats);
    if shown && !*was_shown {
        // Every edit made while the overlay was hidden went past unfolded, so
        // the cache is arbitrarily stale on reopen. Emptying it is what makes
        // skipping the loop below safe: the next pass finds nothing cached and
        // refolds the terrain whole, once.
        cache.0.clear();
    }
    *was_shown = shown;

    // The relief cache, every frame while the overlay is up, so no edit slips
    // past change detection. A frame with no sculpting in it costs one flag
    // check per chunk.
    if shown {
        for (entity, chunk, _) in &chunks {
            if chunk.is_changed() || !cache.0.contains_key(&entity) {
                cache.0.insert(entity, fold_relief(&chunk));
            }
        }
    }

    *elapsed += time.delta_secs();
    if *elapsed < REFRESH_SECS {
        return;
    }
    *elapsed = 0.0;
    if !shown {
        return;
    }

    // Drop cache entries for chunks that no longer exist. Only on the throttled
    // tick — it is the one part that walks the whole map.
    if cache.0.len() > chunks.iter().len() {
        cache.0.retain(|e, _| chunks.get(*e).is_ok());
    }

    let mut objects = 0u64;
    let mut vertices = 0u64;
    let mut triangles = 0u64;
    for handle in &mesh_instances {
        objects += 1;
        let Some(mesh) = meshes.get(&handle.0) else {
            continue;
        };
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).map(|p| p.len());
        vertices += positions.unwrap_or(0) as u64;
        triangles += match mesh.indices() {
            Some(indices) => (indices.len() / 3) as u64,
            // Unindexed: every three positions are a triangle.
            None => (positions.unwrap_or(0) / 3) as u64,
        };
    }
    stats.objects = objects;
    stats.vertices = vertices;
    stats.triangles = triangles;

    stats.terrain = describe_terrain(&selection, &cache, &chunks, &terrains);
}

/// Fold a chunk's composed heights into a normalized (min, max).
///
/// Reads `heights` rather than `base_heights`: the composed buffer is what the
/// mesh is built from, so it is the ground you can actually see and stand on.
fn fold_relief(chunk: &TerrainChunkData) -> (f32, f32) {
    chunk
        .heights
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), h| {
            (lo.min(*h), hi.max(*h))
        })
}

/// Pick the terrain to describe and turn its cached normalized relief into world
/// metres.
///
/// The selected terrain wins — including when a *chunk* is what's selected,
/// since clicking terrain in the viewport picks the chunk you clicked, not the
/// root. Failing that, a scene with exactly one terrain describes that one,
/// which is the common case and saves selecting something to see the numbers.
/// A scene with several and nothing selected shows nothing rather than picking
/// one arbitrarily.
fn describe_terrain(
    selection: &Option<Res<EditorSelection>>,
    cache: &ChunkReliefCache,
    chunks: &Query<(Entity, Ref<TerrainChunkData>, &ChildOf)>,
    terrains: &Query<(Entity, &TerrainData, &GlobalTransform)>,
) -> Option<TerrainStats> {
    let selected = selection.as_ref().and_then(|s| s.get());
    let root = selected
        .filter(|e| terrains.get(*e).is_ok())
        .or_else(|| {
            // A chunk: walk up to the terrain it belongs to.
            selected
                .and_then(|e| chunks.get(e).ok())
                .map(|(_, _, parent)| parent.parent())
                .filter(|e| terrains.get(*e).is_ok())
        })
        .or_else(|| {
            let mut it = terrains.iter();
            let only = it.next()?;
            it.next().is_none().then_some(only.0)
        })?;

    let (_, data, xform) = terrains.get(root).ok()?;

    let (lo, hi) = chunks
        .iter()
        .filter(|(_, _, parent)| parent.parent() == root)
        .filter_map(|(e, _, _)| cache.0.get(&e).copied())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), (l, h)| {
            (lo.min(l), hi.max(h))
        });
    if !lo.is_finite() || !hi.is_finite() {
        return None;
    }

    // Normalized 0–1 maps onto `min_height..max_height`, and the whole terrain
    // then rides its root's transform — so the figure quoted is the world
    // elevation you'd read off a gizmo, not a terrain-local one.
    let base = xform.translation().y;
    let range = data.height_range();
    Some(TerrainStats {
        low: base + data.min_height + lo * range,
        high: base + data.min_height + hi * range,
        floor: base + data.min_height,
        ceiling: base + data.max_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_from_the_right() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_284_096), "1,284,096");
    }

    #[test]
    fn relief_folds_to_the_extremes() {
        let mut chunk = TerrainChunkData::new(0, 0, 3, 0.2);
        chunk.heights[0] = 0.05;
        chunk.heights[4] = 0.9;
        assert_eq!(fold_relief(&chunk), (0.05, 0.9));
    }

    /// A flat chunk has no relief, and the fold must still produce a usable
    /// pair rather than an inverted one — the readout shows `0.0m – 0.0m`.
    #[test]
    fn flat_relief_is_not_inverted() {
        let chunk = TerrainChunkData::new(0, 0, 3, 0.2);
        let (lo, hi) = fold_relief(&chunk);
        assert_eq!(lo, 0.2);
        assert_eq!(hi, 0.2);
    }
}
