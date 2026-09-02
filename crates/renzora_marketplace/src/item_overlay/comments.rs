//! The star rating row, the comment composer and the comment thread.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::auth::marketplace::AssetComment;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::{accent_button, text_input, EmberForm};

use super::{
    note_snapshot, signed_in, ItemCommentInput, ItemOverlay, ItemPostBtn, StarBtn, GOLD, HUE_STORE,
};

/// Five stars showing the average (hover previews your vote), plus the aggregate
/// text and the viewer's current rating.
pub(super) fn build_rating_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let stars_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(1.0),
            ..default()
        })
        .id();

    // Build the star containers first, then wire each glyph's color to the
    // hovered star (preview) or the rounded average (resting).
    let mut stars: Vec<(Entity, Entity)> = Vec::with_capacity(5);
    for i in 1..=5 {
        let star = commands
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Interaction::default(),
                StarBtn(i),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new("item-star"),
            ))
            .id();
        let glyph = icon_text(commands, &fonts.phosphor, "star", GOLD, 18.0);
        commands.entity(glyph).insert(FocusPolicy::Pass);
        commands.entity(star).add_child(glyph);
        commands.entity(stars_row).add_child(star);
        stars.push((star, glyph));
    }
    let containers: Vec<Entity> = stars.iter().map(|(s, _)| *s).collect();
    for (idx, (_, glyph)) in stars.iter().enumerate() {
        let containers = containers.clone();
        let value = idx as i32 + 1;
        bind_text_color(commands, *glyph, move |w| {
            let displayed = hovered_star(w, &containers).unwrap_or_else(|| {
                w.get_resource::<ItemOverlay>()
                    .and_then(|s| s.rating.as_ref())
                    .map(|r| r.average.round() as i32)
                    .unwrap_or(0)
            });
            if value <= displayed {
                rgb(GOLD)
            } else {
                rgb(text_muted())
            }
        });
    }

    let aggregate = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, aggregate, |w| {
        match w.get_resource::<ItemOverlay>().and_then(|s| s.rating.clone()) {
            Some(r) if r.count > 0 => format!("{:.1} ({})", r.average, r.count),
            _ => "No ratings yet".to_string(),
        }
    });

    let yours = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(HUE_STORE))))
        .id();
    bind_text(commands, yours, |w| {
        w.get_resource::<ItemOverlay>()
            .and_then(|s| s.rating.as_ref().and_then(|r| r.user_rating))
            .map(|n| format!("\u{00b7} your rating: {n}/5"))
            .unwrap_or_default()
    });
    bind_display(commands, yours, |w| {
        w.get_resource::<ItemOverlay>()
            .and_then(|s| s.rating.as_ref().and_then(|r| r.user_rating))
            .is_some()
    });

    commands.entity(row).add_children(&[stars_row, aggregate, yours]);
    row
}

/// The highest 1-based star index currently hovered/pressed, if any.
fn hovered_star(w: &Rx, stars: &[Entity]) -> Option<i32> {
    stars.iter().enumerate().rev().find_map(|(i, &e)| {
        matches!(
            w.get::<Interaction>(e),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        )
        .then_some(i as i32 + 1)
    })
}

/// The comment composer: a text input + Post button wired as an [`EmberForm`]
/// (Enter submits), shown only when signed in; otherwise a sign-in prompt.
pub(super) fn build_composer(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();

    // Post button first — it's the form's submit target.
    let post = accent_button(commands, fonts, HUE_STORE, "Post");
    commands.entity(post).insert(ItemPostBtn);

    let form = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            EmberForm { submit: post },
        ))
        .id();
    let input = text_input(commands, &fonts.ui, "Add a comment...", "");
    commands.entity(input).insert((
        ItemCommentInput,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    commands.entity(form).add_children(&[input, post]);
    bind_display(commands, form, signed_in);

    // Signed-out prompt (mutually exclusive with the form).
    let prompt = commands
        .spawn((
            Text::new("Sign in to rate and comment."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, prompt, |w| !signed_in(w));

    commands.entity(wrap).add_children(&[form, prompt]);
    wrap
}

/// One comment: author + timestamp header, then the body text.
fn comment_row(commands: &mut Commands, fonts: &EmberFonts, c: &AssetComment) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let who = commands
        .spawn((
            Text::new(c.user_name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let when = commands
        .spawn((
            Text::new(c.created_at.clone()),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(placeholder())),
        ))
        .id();
    commands.entity(head).add_children(&[who, when]);
    let body = commands
        .spawn((
            Text::new(c.content.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_children(&[head, body]);
    row
}

/// Keyed snapshot of the comments list — a loading/empty note, or one row per
/// comment keyed by id (rebuilt only when a comment's content changes).
pub(super) fn comments_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ItemOverlay>() else {
        return note_snapshot("");
    };
    if state.comments_loading && state.comments.is_empty() {
        return note_snapshot("Loading comments...");
    }
    if state.comments.is_empty() {
        return note_snapshot("No comments yet. Be the first to comment.");
    }
    let comments = state.comments.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = comments
        .iter()
        .map(|c| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            c.id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&c.user_name, &c.content, &c.created_at).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| comment_row(c, f, &comments[i])),
    }
}
