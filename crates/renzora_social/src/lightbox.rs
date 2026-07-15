//! Image lightbox — click a post image to view it near-fullscreen in a dimmed
//! overlay. Click anywhere (or press Esc) to dismiss. Images come through the
//! same [`crate::avatars::AvatarCache`] the thumbnails use, so an image that's
//! already on screen opens instantly.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::avatars::{absolute_url, AvatarCache, AvatarUrl};

/// Put on any clickable image (with `Interaction`) to open it in the lightbox.
/// Holds the image URL (relative URLs resolve against the API base).
#[derive(Component)]
pub(crate) struct LightboxImage(pub String);

/// The open lightbox overlay root, if any.
#[derive(Resource, Default)]
pub(crate) struct Lightbox(Option<Entity>);

#[derive(Component)]
pub(crate) struct LightboxRoot;

pub(crate) fn open_clicks(
    mut commands: Commands,
    mut lightbox: ResMut<Lightbox>,
    clicks: Query<(&Interaction, &LightboxImage), Changed<Interaction>>,
) {
    for (i, img) in &clicks {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Some(root) = lightbox.0.take() {
            commands.entity(root).try_despawn();
        }
        let url = absolute_url(&img.0);
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
                    padding: UiRect::all(Val::Px(28.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
                GlobalZIndex(9600),
                FocusPolicy::Block,
                Interaction::default(),
                bevy::ui::RelativeCursorPosition::default(),
                renzora_ember::widgets::OverlaySurface,
                // `request_avatars` fetches every on-screen AvatarUrl, so the
                // full-size image downloads through the shared cache.
                AvatarUrl(url.clone()),
                LightboxRoot,
                Name::new("image-lightbox"),
            ))
            .id();
        // Auto-sized (image intrinsic size, aspect preserved) but clamped to
        // the viewport; hidden until the cache has the texture.
        let img_e = commands
            .spawn((
                ImageNode::default(),
                Node {
                    max_width: Val::Percent(94.0),
                    max_height: Val::Percent(94.0),
                    display: Display::None,
                    ..default()
                },
                FocusPolicy::Pass,
            ))
            .id();
        renzora_ember::reactive::bind_with(
            &mut commands,
            img_e,
            move |w| w.get_resource::<AvatarCache>().and_then(|c| c.get(&url)),
            |w, e, h: &Option<Handle<Image>>| {
                if let Some(h) = h {
                    if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                        if n.image != *h {
                            n.image = h.clone();
                        }
                    }
                    if let Some(mut node) = w.get_mut::<Node>(e) {
                        if node.display != Display::Flex {
                            node.display = Display::Flex;
                        }
                    }
                }
            },
        );
        commands.entity(root).add_child(img_e);
        lightbox.0 = Some(root);
    }
}

pub(crate) fn close_clicks(
    mut commands: Commands,
    mut lightbox: ResMut<Lightbox>,
    keys: Res<ButtonInput<KeyCode>>,
    backdrops: Query<&Interaction, (With<LightboxRoot>, Changed<Interaction>)>,
) {
    let Some(root) = lightbox.0 else { return };
    let clicked = backdrops.iter().any(|i| *i == Interaction::Pressed);
    if clicked || keys.just_pressed(KeyCode::Escape) {
        commands.entity(root).try_despawn();
        lightbox.0 = None;
    }
}
