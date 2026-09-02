//! Opening the overlay, and the shell it builds: the backdrop, the card, the
//! pinned header (main viewer + close X) and the scrollable body.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use crate::auth::marketplace::AssetSummary;
use crate::auth::session::AuthSession;
use crate::thumbs::HubThumbs;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{accent_button, accent_chip, scroll_area, HoverTint, OverlaySurface};

#[cfg(not(target_arch = "wasm32"))]
use renzora_audio::AudioLink;

use super::audio::build_audio;
use super::comments::{build_composer, build_rating_row, comments_snapshot};
use super::gallery::{build_strip, selected_image_handle};
use super::net::{fetch_comments, fetch_media, fetch_rating};
use super::video::build_video;
use super::{
    apply_thumb, clone_session, divider, section_label, ItemBackdrop, ItemCloseBtn, ItemInstallBtn,
    ItemOverlay, MainImageBtn, StoreCardBtn, GREEN, HUE_STORE,
};

/// Card body click → open the detail overlay for that asset. One open per frame;
/// the pill children capture their own clicks (they're `Block`), so this only
/// fires for a genuine body/thumbnail press.
pub(super) fn store_card_click(
    q: Query<(&Interaction, &StoreCardBtn), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let asset = btn.0.clone();
        commands.queue(move |world: &mut World| open(world, asset));
        break;
    }
}

/// Build and show the detail overlay for `asset`. Exclusive-world (queued from
/// the card click) so it can read `EmberFonts` / `AuthSession`, request the
/// preview thumbnail, and kick the comments/rating fetches in one shot.
fn open(world: &mut World, asset: AssetSummary) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };

    // Replace any overlay already up (defensive — the close system normally runs
    // first, but a queued open shouldn't leak a second backdrop either).
    if let Some(old) = world.get_resource::<ItemOverlay>().and_then(|s| s.root) {
        if let Ok(e) = world.get_entity_mut(old) {
            e.despawn();
        }
    }
    // Stop any clip from the previous overlay first: the state resource is about
    // to be overwritten, and dropping a voice id doesn't stop playback.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut link = world.remove_resource::<AudioLink>();
        if let (Some(mut old), Some(link)) =
            (world.get_resource_mut::<ItemOverlay>(), link.as_mut())
        {
            super::audio::stop_audio_inner(&mut old.audio, link);
        }
        if let Some(link) = link {
            world.insert_resource(link);
        }
    }

    // Reuse the thumbnail the card already requested; request again in case this
    // asset scrolled out of the visible set that `request_store_thumbs` covers.
    if let (Some(mut thumbs), Some(url)) =
        (world.get_resource_mut::<HubThumbs>(), asset.thumbnail_url.clone())
    {
        thumbs.request(&url);
    }

    let session = world
        .get_resource::<AuthSession>()
        .filter(|s| s.is_signed_in())
        .map(clone_session);

    let mut queue = CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        build_overlay(&mut commands, &fonts, &asset)
    };
    queue.apply(world);

    // Seed state and fire the initial fetches.
    let mut state = ItemOverlay {
        root: Some(root),
        asset_id: asset.id.clone(),
        session,
        comments_loading: true,
        asset: Some(asset.clone()),
        ..default()
    };
    // Comments and ratings (reviews) are both keyed by asset id.
    fetch_comments(&mut state, &asset.id);
    fetch_rating(&mut state, &asset.id);
    fetch_media(&mut state, &asset.id);
    world.insert_resource(state);

    // Kick the 3D turntable for model/animation assets (a no-op that resets the
    // rig for anything else, so a prior model never lingers behind a new card).
    crate::model_viewer::open_model_preview(world, &asset);
    // Kick the live material/shader preview for material/shader assets (resets
    // the rig for anything else).
    crate::material_viewer::open_material_preview(world, &asset);
}

/// Spawn the backdrop + content card and return the backdrop root.
fn build_overlay(commands: &mut Commands, fonts: &EmberFonts, asset: &AssetSummary) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
            GlobalZIndex(9600),
            FocusPolicy::Block,
            // OverlaySurface + a cursor probe so ember counts this as a modal
            // surface and confines wheel/pointer to it (panels behind go inert).
            OverlaySurface,
            RelativeCursorPosition::default(),
            Interaction::default(),
            ItemBackdrop,
            Name::new("item-overlay"),
        ))
        .id();

    let card = commands
        .spawn((
            Node {
                width: Val::Percent(92.0),
                max_width: Val::Px(640.0),
                max_height: Val::Percent(86.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            // Block so clicks inside the card never reach the backdrop-close.
            FocusPolicy::Block,
            Name::new("item-overlay-card"),
        ))
        .id();

    let header = build_header(commands, fonts, asset);
    let body = build_body(commands, fonts, asset);
    // `scroll_area` (px-capped) sizes to content; `scroll_view` flex-grows to
    // fill a fixed-height parent, and this card is content-sized (only
    // `max_height`), so it would collapse the body to zero height.
    let body_scroll = scroll_area(commands, body, 520.0);

    commands.entity(card).add_children(&[header, body_scroll]);
    commands.entity(root).add_child(card);
    root
}

/// The pinned header: the large main-image viewer (the currently-selected
/// gallery image) with a floating close X.
fn build_header(commands: &mut Commands, fonts: &EmberFonts, _asset: &AssetSummary) -> Entity {
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(240.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();

    // ── Static image gallery (non-model assets, or a model whose 3D preview
    // failed to load). Wrapped so its whole visibility is gated by
    // `model_viewer::show_gallery`, while the inner image keeps its own
    // swap-on-ready display logic. ──
    let gallery_wrap = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            display: Display::None,
            ..default()
        })
        .id();
    // Gallery shows when neither live preview is active: the model viewer says so
    // (non-model / failed) AND no material preview is on.
    bind_display(commands, gallery_wrap, |w| {
        crate::model_viewer::show_gallery(w.untracked())
            && !crate::material_viewer::material_active(&Rx::new(w.untracked()))
    });
    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            // Click the big preview to open it full-size in the lightbox.
            Interaction::default(),
            MainImageBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    // Bind to the resolved handle of the *selected* image, not a fixed URL: the
    // value changes both when the selection moves and when a handle finishes
    // loading, so the same swap-on-ready path covers both.
    bind_with(commands, img, selected_image_handle, apply_thumb);
    commands.entity(gallery_wrap).add_child(img);
    commands.entity(header).add_child(gallery_wrap);

    // ── Live 3D turntable (model / animation assets). Shows the offscreen RTT
    // once the model is framed; a placeholder covers the load. Letterboxed
    // (height-fit, 16:9) so the model isn't stretched by the header's ratio. ──
    let model_img = letterboxed_image(commands);
    bind_with(
        commands,
        model_img,
        crate::model_viewer::preview_image_handle,
        set_image_only,
    );
    bind_display(commands, model_img, crate::model_viewer::model_ready);
    commands.entity(header).add_child(model_img);

    // Loading placeholder for the 3D preview (self-hides once ready/failed).
    let model_note = loading_note(commands, fonts, "Loading 3D preview\u{2026}");
    bind_display(commands, model_note, crate::model_viewer::model_loading);
    commands.entity(header).add_child(model_note);

    // ── Live material/shader preview (materials & shaders assets). Same
    // letterboxed RTT treatment as the model turntable. ──
    let mat_img = letterboxed_image(commands);
    bind_with(
        commands,
        mat_img,
        crate::material_viewer::preview_image_handle,
        set_image_only,
    );
    bind_display(commands, mat_img, crate::material_viewer::material_ready);
    commands.entity(header).add_child(mat_img);

    // Loading placeholder for the material preview.
    let mat_note = loading_note(commands, fonts, "Compiling shader preview\u{2026}");
    bind_display(commands, mat_note, crate::material_viewer::material_loading);
    commands.entity(header).add_child(mat_note);

    // Floating close button (top-right), over the image.
    let close = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                right: Val::Px(8.0),
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(13.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            HoverTint::solid(
                Color::srgba(0.0, 0.0, 0.0, 0.5),
                Color::srgba(0.0, 0.0, 0.0, 0.72),
                Color::srgba(0.0, 0.0, 0.0, 0.72),
            ),
            Interaction::default(),
            ItemCloseBtn,
            Name::new("item-overlay-close"),
        ))
        .id();
    let x = icon_text(commands, &fonts.phosphor, "x", (240, 240, 245), 13.0);
    commands.entity(x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(x);
    commands.entity(header).add_child(close);
    header
}

/// A 16:9 height-fit image for a live render target, hidden until its binding
/// says the preview is ready. Unlike a gallery thumbnail this never toggles its
/// own `display` — the ready/loading gate owns that — so it gets its own setter.
fn letterboxed_image(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            ImageNode::default(),
            Node {
                height: Val::Percent(100.0),
                aspect_ratio: Some(16.0 / 9.0),
                display: Display::None,
                ..default()
            },
        ))
        .id()
}

fn set_image_only(w: &mut World, e: Entity, h: &Option<Handle<Image>>) {
    if let Some(h) = h {
        if let Some(mut n) = w.get_mut::<ImageNode>(e) {
            if n.image != *h {
                n.image = h.clone();
            }
        }
    }
}

/// A centered muted line covering the header while a live preview loads.
fn loading_note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let note = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(note).add_child(label);
    note
}

/// The scrollable body: title, meta, rating, install, description, comments.
fn build_body(commands: &mut Commands, fonts: &EmberFonts, asset: &AssetSummary) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .id();

    // Title.
    let name = commands
        .spawn((
            Text::new(asset.name.clone()),
            ui_font(&fonts.ui, 18.0),
            TextColor(rgb(text_primary())),
        ))
        .id();

    // Meta row: creator + category + downloads chips.
    let meta = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let by = commands
        .spawn((
            Text::new(format!("by {}", asset.creator_name)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let cat_chip = accent_chip(commands, fonts, HUE_STORE, Some("tag"), &asset.category);
    let dl_chip = accent_chip(
        commands,
        fonts,
        HUE_STORE,
        Some("download-simple"),
        &format!("{} downloads", asset.downloads),
    );
    commands.entity(meta).add_children(&[by, cat_chip, dl_chip]);

    // Rating row: five interactive stars + average/count + the viewer's vote.
    let rating = build_rating_row(commands, fonts);

    // Install / Get action, price carried on its label.
    let (label, hue) = if asset.price_credits == 0 {
        ("Get for free".to_string(), GREEN)
    } else {
        (format!("Buy ({} credits)", asset.price_credits), HUE_STORE)
    };
    let actions = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let install = accent_button(commands, fonts, hue, &label);
    commands.entity(install).insert(ItemInstallBtn);
    commands.entity(actions).add_child(install);

    // Any network error, surfaced quietly under the actions.
    let error = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb((224, 96, 96))),
        ))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<ItemOverlay>().and_then(|s| s.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<ItemOverlay>().map(|s| s.error.is_some()).unwrap_or(false)
    });

    // Description.
    let desc_label = section_label(commands, fonts, "About");
    let desc = commands
        .spawn((
            Text::new(if asset.description.trim().is_empty() {
                "No description provided.".to_string()
            } else {
                asset.description.clone()
            }),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();

    // Comments.
    let comments_label = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, comments_label, |w| {
        let n = w.get_resource::<ItemOverlay>().map(|s| s.comments.len()).unwrap_or(0);
        if n == 0 {
            "Comments".to_string()
        } else {
            format!("Comments ({n})")
        }
    });
    let comments_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    // Untokened: the modal is short-lived and the list small, so recomputing the
    // snapshot each frame is cheap and avoids any stale-token risk when comments
    // arrive or a new post lands.
    keyed_list(commands, comments_list, comments_snapshot);

    let composer = build_composer(commands, fonts);
    let div1 = divider(commands);
    let div2 = divider(commands);

    // Gallery + media previews. Each is self-hiding when its media type is
    // absent (a fresh asset with no `/media` shows just the fallback thumbnail).
    let strip = build_strip(commands);
    let audio = build_audio(commands, fonts);
    let video = build_video(commands, fonts);

    // Live material/shader controls — shape selector + auto-generated `@param`
    // sliders (self-hides unless the open asset is a material/shader).
    let mat_controls = crate::material_viewer::build_material_controls(commands, fonts);

    commands.entity(col).add_children(&[
        name,
        meta,
        strip,
        mat_controls,
        rating,
        actions,
        error,
        div1,
        desc_label,
        desc,
        audio,
        video,
        div2,
        comments_label,
        comments_list,
        composer,
    ]);
    col
}
