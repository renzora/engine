//! Frees font atlases no text is drawing from any more.
//!
//! Bevy keeps one set of glyph atlases per font, size, variation and smoothing,
//! and never frees one: `FontAtlasSet` has a `clear`, but nothing calls it. So
//! every size text has ever been drawn at stays resident, a 512x512 image on the
//! CPU and again on the GPU per atlas. Changing the UI scale or font leaves the
//! old sizes behind for the rest of the session, and a bug that churned through
//! font ids once filled the GPU with them until the editor crashed.
//!
//! An atlas is in use while some laid-out text still points into it: each glyph
//! in a `TextLayoutInfo` names its atlas image. That covers UI text, 2D text and
//! hidden text alike (a hidden menu keeps its layout, so it keeps its atlases).
//! The 3D text meshes and world-space UI read glyphs out of an atlas only while
//! they build, copying them into their own texture, so they need nothing kept.
//!
//! A set is freed once none of its atlases has been in use for [`UNUSED_FOR`].
//! The delay is there so a tooltip or menu that is shown, closed and shown again
//! does not rasterize its glyphs afresh each time. A freed set is rebuilt the
//! next time text needs that font and size, exactly as it was built the first
//! time. The scan walks every glyph on screen, so it runs every [`SCAN_EVERY`]
//! rather than every frame.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::text::{FontAtlasKey, FontAtlasSet, TextLayoutInfo};

/// How long a set of atlases has to go unused before it is freed.
const UNUSED_FOR: f64 = 10.0;

/// How often the glyphs are scanned for the atlases they use.
const SCAN_EVERY: f64 = 2.0;

pub(crate) fn plugin(app: &mut App) {
    // `Last`, after every text system has written this frame's layouts, so a
    // glyph added this frame is already counted as using its atlas.
    app.add_systems(Last, free_unused_font_atlases);
}

/// When each atlas set was last seen unused, and when the glyphs were last
/// scanned.
#[derive(Default)]
struct AtlasUse {
    unused_since: HashMap<FontAtlasKey, f64>,
    last_scan: Option<f64>,
}

fn free_unused_font_atlases(
    time: Res<Time<Real>>,
    atlases: Option<ResMut<FontAtlasSet>>,
    layouts: Query<&TextLayoutInfo>,
    mut state: Local<AtlasUse>,
) {
    let Some(mut atlases) = atlases else {
        return;
    };
    let now = time.elapsed_secs_f64();
    if state.last_scan.is_some_and(|at| now - at < SCAN_EVERY) {
        return;
    }
    state.last_scan = Some(now);

    let in_use: HashSet<AssetId<Image>> = layouts
        .iter()
        .flat_map(|layout| layout.glyphs.iter().map(|glyph| glyph.atlas_info.texture))
        .collect();

    let AtlasUse { unused_since, .. } = &mut *state;
    let mut expired = Vec::new();
    for (key, set) in atlases.iter() {
        if set.iter().any(|atlas| in_use.contains(&atlas.texture.id())) {
            unused_since.remove(key);
            continue;
        }
        let since = *unused_since.entry(*key).or_insert(now);
        if now - since >= UNUSED_FOR {
            expired.push(*key);
        }
    }
    // Only a set that is really going away is touched, so a session where
    // nothing changes never marks the resource as changed.
    for key in expired {
        atlases.remove(&key);
        unused_since.remove(&key);
    }
    // Forget sets that no longer exist, so the map cannot outgrow the atlases.
    let live: HashSet<FontAtlasKey> = atlases.keys().copied().collect();
    unused_since.retain(|key, _| live.contains(key));
}
