//! Sprite rehydration and the derived-rect systems.
//!
//! Bevy's `Sprite` carries no `#[reflect(Serialize, Deserialize)]`, so a scene
//! save drops it wholesale. Everything a sprite needs is persisted as separate
//! serializable components — `SpriteImagePath`, `SpriteCustomSize`,
//! `SpriteSheet`, `SpriteAtlasRegion` — and rebuilt into a live `Sprite` here.
//!
//! The observers come in pairs on purpose. A preset spawn and a drag-drop both
//! insert `Sprite` and `SpriteImagePath` in the same bundle, so whichever
//! observer fires last has to find the other component already present and
//! reconcile — hence [`on_sprite_image_path_inserted`] and
//! [`on_sprite_inserted_apply_image_path`] doing the same job from both sides.

use bevy::prelude::*;
use renzora::{EditorCamera, PlayModeState};

/// Loads `Sprite.image` from `SpriteImagePath` whenever the path
/// component is added or its string changes.
///
/// Two responsibilities:
/// 1. **Update existing sprite**: bind / clear the image handle and
///    swap placeholder-blue ↔ white as appropriate.
/// 2. **Re-create missing sprite**: Bevy 0.18's `Sprite` doesn't have
///    `#[reflect(Serialize, Deserialize)]`, so scene save drops it
///    entirely. On load, an entity carries `SpriteImagePath` and the
///    required components (Anchor, Transform, Visibility), but no
///    `Sprite` — nothing renders. Inserting a fresh `Sprite` with the
///    bound image (or placeholder colour for an empty path) restores
///    rendering. This is the rehydration path mirroring
///    `rehydrate_meshes` for `MeshPrimitive`.
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "render_2d")]
pub fn on_sprite_image_path_inserted(
    trigger: On<Insert, renzora::core::SpriteImagePath>,
    paths: Query<&renzora::core::SpriteImagePath>,
    sizes: Query<&renzora::core::SpriteCustomSize>,
    has_sprite: Query<(), With<bevy::sprite::Sprite>>,
    mut sprites_mut: Query<&mut bevy::sprite::Sprite>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::CurrentProject>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else {
        return;
    };
    let filter = sprite_filter(project.as_deref());
    let persisted = sizes.get(entity).ok().map(|s| s.0);
    if has_sprite.get(entity).is_ok() {
        apply_sprite_image_path(entity, &path.0, persisted, &mut sprites_mut, &asset_server, filter);
    } else {
        spawn_sprite_for_path(entity, &path.0, persisted, &asset_server, &mut commands, filter);
    }
}

/// Companion observer: when `Sprite` is inserted on an entity that
/// already has `SpriteImagePath`, bind the image. Catches the
/// reverse insert order — preset spawns and drag-drop both insert
/// Sprite and SpriteImagePath in the same bundle, so whichever
/// observer fires last finds the other component already present.
#[cfg(feature = "render_2d")]
pub fn on_sprite_inserted_apply_image_path(
    trigger: On<Insert, bevy::sprite::Sprite>,
    paths: Query<&renzora::core::SpriteImagePath>,
    sizes: Query<&renzora::core::SpriteCustomSize>,
    mut sprites_mut: Query<&mut bevy::sprite::Sprite>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::CurrentProject>>,
) {
    let entity = trigger.entity;
    let Ok(path) = paths.get(entity) else {
        return;
    };
    let filter = sprite_filter(project.as_deref());
    let persisted = sizes.get(entity).ok().map(|s| s.0);
    apply_sprite_image_path(entity, &path.0, persisted, &mut sprites_mut, &asset_server, filter);
}

/// Editor-side persistence bridge for sprite size: mirror `Sprite.custom_size`
/// (runtime truth, set by the 2D resize handles / inspector) into the
/// serializable [`renzora::core::SpriteCustomSize`] so scene save keeps it —
/// bevy's `Sprite` itself doesn't round-trip through reflection serialization
/// and is dropped wholesale by the save filter. Only meaningful while editing:
/// gated on an editor camera existing and play mode being off so a shipped
/// game (or a script animating sizes in play) doesn't churn component inserts.
#[cfg(feature = "render_2d")]
pub fn mirror_sprite_custom_size(
    editor_camera: Query<(), With<EditorCamera>>,
    play_mode: Option<Res<PlayModeState>>,
    changed: Query<
        (Entity, &bevy::sprite::Sprite, Option<&renzora::core::SpriteCustomSize>),
        Changed<bevy::sprite::Sprite>,
    >,
    mut commands: Commands,
) {
    if editor_camera.is_empty() || play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    for (entity, sprite, mirrored) in &changed {
        match (sprite.custom_size, mirrored) {
            (Some(size), Some(m)) if m.0 == size => {}
            (Some(size), _) => {
                commands
                    .entity(entity)
                    .try_insert(renzora::core::SpriteCustomSize(size));
            }
            (None, Some(_)) => {
                commands
                    .entity(entity)
                    .try_remove::<renzora::core::SpriteCustomSize>();
            }
            (None, None) => {}
        }
    }
}

/// Load-order safety net for the persisted sprite size: when
/// `SpriteCustomSize` is inserted on an entity that already has a `Sprite`,
/// push it onto the live `Sprite.custom_size`.
///
/// Scene files serialize `SpriteImagePath` before `SpriteCustomSize`, and the
/// image-path observer spawns the `Sprite` as soon as the path lands — so the
/// two insertion orders / observer-command flush timings split into two cases:
/// if the `Sprite` already exists when the size component arrives, this
/// observer applies it; if the size component is already present when the
/// `Sprite` is inserted, `apply_sprite_image_path` reconciles it instead.
/// Together they guarantee a user-resized sprite reopens at its saved size
/// regardless of order. The compare-first write avoids tripping
/// `Changed<Sprite>` when the value is already in sync (the common editor case,
/// where `mirror_sprite_custom_size` re-inserts the same size it just read).
#[cfg(feature = "render_2d")]
pub fn on_sprite_custom_size_inserted(
    trigger: On<Insert, renzora::core::SpriteCustomSize>,
    sizes: Query<&renzora::core::SpriteCustomSize>,
    mut sprites: Query<&mut bevy::sprite::Sprite>,
) {
    let entity = trigger.entity;
    let Ok(size) = sizes.get(entity) else {
        return;
    };
    let Ok(mut sprite) = sprites.get_mut(entity) else {
        return;
    };
    if sprite.custom_size != Some(size.0) {
        sprite.custom_size = Some(size.0);
    }
}

/// Derive `Sprite.rect` from a [`renzora::core::SpriteSheet`] grid and the
/// loaded image's pixel dimensions.
///
/// Runs every frame (editor and runtime) rather than on `Changed<SpriteSheet>`
/// because the rect depends on the *image* as much as the grid: the texture
/// loads asynchronously and can be swapped via `SpriteImagePath`, neither of
/// which touches `SpriteSheet`. The write is compare-first so an idle sprite
/// doesn't trip `Changed<Sprite>` (render re-extraction, the custom-size
/// mirror) every frame.
#[cfg(feature = "render_2d")]
pub fn apply_sprite_sheet_crop(
    images: Res<Assets<Image>>,
    mut sprites: Query<(&renzora::core::SpriteSheet, &mut bevy::sprite::Sprite)>,
) {
    for (sheet, mut sprite) in &mut sprites {
        let hframes = sheet.hframes.max(1);
        let vframes = sheet.vframes.max(1);
        let desired = if hframes == 1 && vframes == 1 {
            // 1×1 grid = the whole image; leave the rect unset so the sprite
            // behaves exactly like one without a SpriteSheet (native sizing
            // keeps tracking the image if it's swapped).
            None
        } else {
            // Image not loaded yet → keep the current rect and retry next
            // frame once the asset lands.
            let Some(image) = images.get(&sprite.image) else {
                continue;
            };
            let size = image.size_f32();
            let frame_w = size.x / hframes as f32;
            let frame_h = size.y / vframes as f32;
            // Wrap rather than clamp so a linear 0→N animation track loops
            // cleanly through the sheet.
            let idx = sheet.frame % (hframes * vframes);
            let col = (idx % hframes) as f32;
            let row = (idx / hframes) as f32;
            // Inset the rect by a whisker on every side. At a fractional
            // camera zoom, the interpolated UV at a quad edge can overshoot
            // the cell boundary by float error; with nearest sampling that
            // one-boundary miss fetches the NEIGHBOURING cell's edge texel,
            // painting a 1px line down the whole tile edge (colored bleed —
            // or a "gap" when the neighbour texel is transparent). 0.05px is
            // far above the interpolation error and far below a visible
            // sampling shift.
            const EDGE_INSET: f32 = 0.05;
            Some(Rect::new(
                col * frame_w + EDGE_INSET,
                row * frame_h + EDGE_INSET,
                (col + 1.0) * frame_w - EDGE_INSET,
                (row + 1.0) * frame_h - EDGE_INSET,
            ))
        };
        if sprite.rect != desired {
            sprite.rect = desired;
        }
    }
}

/// Derive `Sprite.rect` from a [`renzora::core::SpriteAtlasRegion`] block —
/// the multi-cell counterpart of [`apply_sprite_sheet_crop`]. A painted
/// tilemap "object" (a tree stamped from a multi-tile palette block) is a
/// single sprite showing a `w × h` slice of the atlas; this system keeps its
/// `Sprite.rect` in sync with the persisted block on load and after texture
/// swaps.
///
/// Unlike the sprite-sheet crop this needs no loaded image — the rect is
/// `cell * tile_px`, pure arithmetic on the stored block — so it also lands
/// correctly on the first frame after a scene load, before the atlas asset
/// finishes loading. The same `EDGE_INSET` trick keeps a fractional camera
/// zoom from bleeding the neighbouring atlas cell across the block's outer
/// edge. Compare-first so an idle object doesn't trip `Changed<Sprite>` every
/// frame.
#[cfg(feature = "render_2d")]
pub fn apply_sprite_atlas_region(
    mut sprites: Query<(&renzora::core::SpriteAtlasRegion, &mut bevy::sprite::Sprite)>,
) {
    for (region, mut sprite) in &mut sprites {
        let w = region.w.max(1);
        let h = region.h.max(1);
        let px = region.tile_px.max(1) as f32;
        const EDGE_INSET: f32 = 0.05;
        let desired = Some(Rect::new(
            region.col as f32 * px + EDGE_INSET,
            region.row as f32 * px + EDGE_INSET,
            (region.col + w) as f32 * px - EDGE_INSET,
            (region.row + h) as f32 * px - EDGE_INSET,
        ));
        if sprite.rect != desired {
            sprite.rect = desired;
        }
    }
}

/// Y-sort: overwrite `Transform.translation.z` from world Y for every
/// [`renzora::core::YSort`] entity, so 2D entities lower on screen draw in
/// front (Bevy's 2D transparent pass sorts by Z — no render work needed).
///
/// The world Y comes from `GlobalTransform`, i.e. last frame's propagated
/// value — one frame of sort lag on a moving parent, which is invisible at
/// game speeds and avoids re-propagating transforms twice a frame. The scale
/// maps ±50k world units onto the ±0.5 band around `z_base`, so neighbouring
/// integer bands never interleave; the clamp pins runaway positions to the
/// band edge instead of letting them cross into another layer. Compare-first
/// write so parked entities don't trip `Changed<Transform>` (and a re-save of
/// an unchanged scene) every frame.
#[cfg(feature = "render_2d")]
pub fn apply_y_sort(
    mut q: Query<(&renzora::core::YSort, &mut Transform, &GlobalTransform)>,
) {
    /// One world unit of Y = this much Z. f32 around a small `z_base` resolves
    /// steps of ~1e-7, so 1e-5 still separates sub-pixel Y differences.
    const Y_SORT_SCALE: f32 = 1e-5;
    for (ysort, mut transform, global) in &mut q {
        let sort = ((global.translation().y + ysort.offset) * Y_SORT_SCALE).clamp(-0.5, 0.5);
        let z = ysort.z_base - sort;
        if transform.translation.z != z {
            transform.translation.z = z;
        }
    }
}

/// Clear the derived crop when the sheet component is removed, restoring the
/// full-image sprite. Without this the last frame's rect would stick around
/// forever — nothing else writes `Sprite.rect`.
#[cfg(feature = "render_2d")]
pub fn on_sprite_sheet_removed(
    trigger: On<Remove, renzora::core::SpriteSheet>,
    mut sprites: Query<&mut bevy::sprite::Sprite>,
) {
    if let Ok(mut sprite) = sprites.get_mut(trigger.entity) {
        sprite.rect = None;
    }
}

/// Resolve the project's configured 2D image filter. Defaults to
/// `Nearest` when no project is loaded — keeps the behaviour
/// pixel-perfect by default.
#[cfg(feature = "render_2d")]
fn sprite_filter(project: Option<&renzora::CurrentProject>) -> renzora::core::TextureFilter {
    project
        .map(|p| p.config.rendering_2d.image_filter)
        .unwrap_or_default()
}

#[cfg(feature = "render_2d")]
fn apply_sprite_image_path(
    entity: Entity,
    path: &str,
    persisted_size: Option<Vec2>,
    sprites_mut: &mut Query<&mut bevy::sprite::Sprite>,
    asset_server: &AssetServer,
    filter: renzora::core::TextureFilter,
) {
    let Ok(mut sprite) = sprites_mut.get_mut(entity) else {
        return;
    };
    let placeholder_blue = Color::srgba(0.5, 0.7, 1.0, 1.0);

    if path.is_empty() {
        if sprite.image != Handle::<Image>::default() {
            info!("[sprite] {:?} cleared image (empty path)", entity);
            sprite.image = Default::default();
            sprite.color = placeholder_blue;
            if sprite.custom_size.is_none() {
                sprite.custom_size = Some(Vec2::splat(100.0));
            }
        }
        return;
    }

    let expected = load_sprite_image(asset_server, path, filter);
    if sprite.image.id() != expected.id() {
        info!(
            "[sprite] {:?} bound image \"{}\" (replaced handle, filter={:?})",
            entity, path, filter
        );
        sprite.image = expected;
        sprite.color = Color::WHITE;
        // A saved `SpriteCustomSize` (user resized the sprite) wins;
        // otherwise `None` → Bevy uses the image's native dimensions.
        // Forcing a fixed size here would silently squash a 1024×1024
        // source into 100 world units, blowing away most of the
        // pixel-art detail.
        sprite.custom_size = persisted_size;
    } else if sprite.color == placeholder_blue {
        sprite.color = Color::WHITE;
        sprite.custom_size = persisted_size;
    } else if let Some(size) = persisted_size {
        // Image already bound and not a placeholder, yet the entity carries
        // a saved resize size the live `Sprite` hasn't picked up. This is the
        // scene-load case: `.bsn` serializes `SpriteImagePath` before
        // `SpriteCustomSize`, so the image-path observer spawns/binds the
        // Sprite (image set, white) while the size component isn't present
        // yet — neither branch above restores it. When this observer re-runs
        // as the Sprite is inserted, `SpriteCustomSize` is now present, so
        // reconcile it here. Without this a resized sprite reopens (and ships)
        // at its image's native dimensions.
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
}

/// Load a sprite texture with the project's configured filter. Bevy's
/// default ImagePlugin uses linear filtering (right for 3D PBR, wrong
/// for pixel art — every scaled pixel becomes a smear). Per-asset
/// override via `load_with_settings` keeps 3D textures linear while
/// sprite textures land with whatever the project asks for.
#[cfg(feature = "render_2d")]
fn load_sprite_image(
    asset_server: &AssetServer,
    path: &str,
    filter: renzora::core::TextureFilter,
) -> Handle<Image> {
    use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
    let descriptor = match filter {
        renzora::core::TextureFilter::Nearest => ImageSamplerDescriptor::nearest(),
        renzora::core::TextureFilter::Linear => ImageSamplerDescriptor::linear(),
    };
    asset_server
        .load_builder()
        .with_settings(move |settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(descriptor.clone());
        })
        .load::<Image>(path.to_owned())
}

/// Insert a `Sprite` from scratch when one's missing. Used by the
/// rehydration path — reflection-loaded entities carry
/// `SpriteImagePath` but not `Sprite`. Defaults match the editor's
/// preset: 100×100 placeholder for empty path, white-tinted with
/// the loaded texture for a bound path.
#[cfg(feature = "render_2d")]
fn spawn_sprite_for_path(
    entity: Entity,
    path: &str,
    persisted_size: Option<Vec2>,
    asset_server: &AssetServer,
    commands: &mut Commands,
    filter: renzora::core::TextureFilter,
) {
    let placeholder_blue = Color::srgba(0.5, 0.7, 1.0, 1.0);
    let sprite = if path.is_empty() {
        bevy::sprite::Sprite {
            color: placeholder_blue,
            custom_size: Some(Vec2::splat(100.0)),
            ..Default::default()
        }
    } else {
        // `custom_size: None` → Bevy uses the loaded image's native
        // pixel dimensions, so a 32×32 source renders as 32 world units,
        // a 1024×1024 source as 1024. Critical for pixel art: forcing
        // a fixed size silently downsamples the source before our
        // viewport upscale, killing the crisp-pixel look. A saved
        // `SpriteCustomSize` (the user resized it) overrides that.
        bevy::sprite::Sprite {
            color: Color::WHITE,
            custom_size: persisted_size,
            image: load_sprite_image(asset_server, path, filter),
            ..Default::default()
        }
    };
    info!(
        "[sprite] {:?} rehydrated Sprite component (path \"{}\", filter={:?})",
        entity, path, filter
    );
    commands.entity(entity).insert(sprite);
}
