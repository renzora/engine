//! The video section: one poster card per video, opened in the browser on click
//! (native video decode is out of scope).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::auth::marketplace::MediaItem;
use crate::thumbs::HubThumbs;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, bind_with, keyed_list};
use renzora_ember::theme::*;

use super::gallery::media_by_type;
use super::{apply_thumb, empty_snapshot, section_label, thumb_image, VideoBtn};

/// The video section — hidden when the asset has no video.
pub(super) fn build_video(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    bind_display(commands, wrap, |w| !media_by_type(w, "video").is_empty());

    let label = section_label(commands, fonts, "Video");
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, video_snapshot);
    commands.entity(wrap).add_children(&[label, list]);
    wrap
}

/// Keyed snapshot of the video posters — one card per video, keyed by id.
fn video_snapshot(world: &Rx) -> KeyedSnapshot {
    let videos = media_by_type(world, "video");
    if videos.is_empty() {
        return empty_snapshot();
    }
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = videos
        .iter()
        .map(|m| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            m.id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&m.url, &m.thumbnail_url).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| video_card(c, f, &videos[i])),
    }
}

/// One video poster: the thumbnail with a centered play triangle, a caption, and
/// a click that opens the video URL in the browser.
fn video_card(commands: &mut Commands, fonts: &EmberFonts, item: &MediaItem) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(180.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            VideoBtn(item.url.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("video-card"),
        ))
        .id();

    if let Some(poster) = item.thumbnail_url.clone() {
        let img = thumb_image(commands);
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&poster)),
            apply_thumb,
        );
        commands.entity(card).add_child(img);
    }

    // Play-triangle badge (over the poster).
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(52.0),
                height: Val::Px(52.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(26.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            FocusPolicy::Pass,
        ))
        .id();
    let tri = icon_text(commands, &fonts.phosphor, "play", (240, 240, 245), 22.0);
    commands.entity(tri).insert(FocusPolicy::Pass);
    commands.entity(badge).add_child(tri);
    commands.entity(card).add_child(badge);

    // "Opens in your browser" caption, bottom-left.
    let caption = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            FocusPolicy::Pass,
        ))
        .id();
    let caption_text = commands
        .spawn((
            Text::new("Opens in your browser"),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb((224, 224, 230))),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(caption).add_child(caption_text);
    commands.entity(card).add_child(caption);
    card
}
