//! The image gallery: which images exist, which one the main viewer shows, and
//! the thumbnail strip that selects between them.

use bevy::prelude::*;

use crate::auth::marketplace::MediaItem;
use crate::thumbs::HubThumbs;
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_with, keyed_list};
use renzora_ember::theme::*;

use super::{apply_thumb, empty_snapshot, thumb_image, ItemOverlay, StripThumbBtn, HUE_STORE};

/// The image URLs for the gallery: the `/media` images in order, or — if none
/// were returned — the asset's own single thumbnail as a one-item fallback.
fn image_urls(w: &Rx) -> Vec<String> {
    let Some(s) = w.get_resource::<ItemOverlay>() else {
        return Vec::new();
    };
    let imgs: Vec<String> = s
        .media
        .iter()
        .filter(|m| m.media_type == "image")
        .map(|m| m.url.clone())
        .collect();
    if !imgs.is_empty() {
        return imgs;
    }
    s.asset
        .as_ref()
        .and_then(|a| a.thumbnail_url.clone())
        .into_iter()
        .collect()
}

/// The loaded texture for the currently-selected gallery image, if ready.
pub(super) fn selected_image_handle(w: &Rx) -> Option<Handle<Image>> {
    let urls = image_urls(w);
    if urls.is_empty() {
        return None;
    }
    let sel = w
        .get_resource::<ItemOverlay>()
        .map(|s| s.selected_media)
        .unwrap_or(0);
    let url = urls.get(sel).or_else(|| urls.first())?;
    w.get_resource::<HubThumbs>().and_then(|t| t.get(url))
}

/// All media of a given type, in sorted order.
pub(super) fn media_by_type(w: &Rx, ty: &str) -> Vec<MediaItem> {
    w.get_resource::<ItemOverlay>()
        .map(|s| {
            s.media
                .iter()
                .filter(|m| m.media_type == ty)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The horizontal thumbnail strip beneath the main viewer. Shown only when the
/// gallery holds more than one image; each thumb selects its image on click.
pub(super) fn build_strip(commands: &mut Commands) -> Entity {
    let strip = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, strip, strip_snapshot);
    strip
}

/// Keyed snapshot of the strip: one thumb per image, keyed by index, with the
/// selected flag folded into the content hash so selecting rebuilds just the
/// two affected thumbs (old + new) rather than the whole strip.
fn strip_snapshot(world: &Rx) -> KeyedSnapshot {
    let urls = image_urls(world);
    if urls.len() <= 1 {
        return empty_snapshot();
    }
    let sel = world
        .get_resource::<ItemOverlay>()
        .map(|s| s.selected_media)
        .unwrap_or(0);
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = urls
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (u, i == sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    let urls2 = urls;
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| strip_thumb(c, &urls2[i], i, i == sel)),
    }
}

/// One strip thumbnail: a small clickable image with a selection border.
fn strip_thumb(commands: &mut Commands, url: &str, index: usize, selected: bool) -> Entity {
    let cell = commands
        .spawn((
            Node {
                width: Val::Px(64.0),
                height: Val::Px(42.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(if selected { rgb(HUE_STORE) } else { rgb(border()) }),
            Interaction::default(),
            StripThumbBtn(index),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("gallery-thumb"),
        ))
        .id();
    let img = thumb_image(commands);
    let u = url.to_string();
    bind_with(
        commands,
        img,
        move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&u)),
        apply_thumb,
    );
    commands.entity(cell).add_child(img);
    cell
}
