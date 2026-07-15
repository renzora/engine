//! Image lightbox for the marketplace item overlay — click the big preview image
//! to view it near-fullscreen in a dimmed overlay, click anywhere (or press Esc)
//! to dismiss. Mirrors the social feed's [`renzora_social` lightbox], but pulls
//! textures from [`HubThumbs`] and stacks *above* the item-detail overlay.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use renzora::SplashState;

use crate::thumbs::HubThumbs;

/// The open lightbox overlay root, if any. One at a time.
#[derive(Resource, Default)]
pub(crate) struct HubLightbox {
    pub(crate) root: Option<Entity>,
}

#[derive(Component)]
struct HubLightboxRoot;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<HubLightbox>();
    app.add_systems(Update, close_clicks.run_if(in_state(SplashState::Editor)));
}

/// Open a full-screen preview of `url`, above everything (incl. the item
/// overlay). Exclusive-world so it can request the texture and spawn in one shot.
pub(crate) fn open(world: &mut World, url: String) {
    if url.is_empty() {
        return;
    }
    // Make sure the texture is loaded / loading (it usually already is).
    if let Some(mut thumbs) = world.get_resource_mut::<HubThumbs>() {
        thumbs.request(&url);
    }
    // Replace any lightbox already up.
    if let Some(old) = world.get_resource::<HubLightbox>().and_then(|s| s.root) {
        if let Ok(e) = world.get_entity_mut(old) {
            e.despawn();
        }
    }
    let mut queue = CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        build(&mut commands, &url)
    };
    queue.apply(world);
    if let Some(mut lb) = world.get_resource_mut::<HubLightbox>() {
        lb.root = Some(root);
    }
}

fn build(commands: &mut Commands, url: &str) -> Entity {
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.9)),
            // Above the item overlay (9600) so it reads as a modal over the modal.
            GlobalZIndex(9900),
            FocusPolicy::Block,
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_ember::widgets::OverlaySurface,
            HubLightboxRoot,
            Name::new("hub-image-lightbox"),
        ))
        .id();
    // Auto-sized (intrinsic aspect) but clamped to the viewport; hidden until the
    // texture is ready.
    let img = commands
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
    let u = url.to_string();
    renzora_ember::reactive::bind_with(
        commands,
        img,
        move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&u)),
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
    commands.entity(root).add_child(img);
    root
}

/// Dismiss on a backdrop press or Escape.
fn close_clicks(
    mut commands: Commands,
    mut lightbox: ResMut<HubLightbox>,
    keys: Res<ButtonInput<KeyCode>>,
    backdrops: Query<&Interaction, (With<HubLightboxRoot>, Changed<Interaction>)>,
) {
    let Some(root) = lightbox.root else {
        return;
    };
    let clicked = backdrops.iter().any(|i| *i == Interaction::Pressed);
    if clicked || keys.just_pressed(KeyCode::Escape) {
        commands.entity(root).try_despawn();
        lightbox.root = None;
    }
}
