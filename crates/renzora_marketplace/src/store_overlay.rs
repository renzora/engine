//! The Marketplace as a full-window overlay rather than a docked panel.
//!
//! # Why it stopped being a panel
//!
//! A dock panel is for something you keep beside your work: the hierarchy, the
//! inspector, the console. The Marketplace is not that. You go to it, you find
//! a thing, you install it, and you leave — and while you are there you want the
//! whole window, because it is a grid of artwork. As a panel it was competing
//! for space with the viewport it exists to fill, and it needed a whole
//! workspace of its own to be usable at all.
//!
//! So it opens over everything, from a door in the chrome (the storefront icon
//! beside the gear) and from the asset browser's Import button — the two places
//! you are standing when you realise you need an asset you do not have.
//!
//! # How the chrome opens something it knows nothing about
//!
//! [`renzora::ShellActionItem`] — the shell draws a registered icon, and a press
//! writes [`renzora::ShellActionInvoked`] carrying the id. The id is the only
//! thing that crosses: no callback, no type, nothing the shell has to link. The
//! asset browser reaches the same overlay by writing the same message, which is
//! why neither of those crates has ever heard of this one.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora::RenzoraShellExt;
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::overlay_val;

/// The id the shell button and the asset browser both write. Defined in the
/// contract crate, because the asset browser writes it too and neither crate
/// should depend on the other to agree on a string.
pub use renzora::ACTION_MARKETPLACE as ACTION_ID;

/// Marks the overlay root, so a second press closes rather than stacks.
#[derive(Component)]
pub(crate) struct StoreOverlayRoot;

pub(crate) fn register(app: &mut App) {
    app.register_shell_action(renzora::ShellActionItem {
        id: ACTION_ID,
        icon: "storefront",
        // A function, not a string: registration happens during `App` assembly,
        // long before the chrome is built and before the user has had a chance
        // to change language.
        tooltip: || renzora::lang::t_or("marketplace.title", "Marketplace"),
        order: 0,
    });
    app.add_systems(
        Update,
        open_on_action.run_if(in_state(renzora::SplashState::Editor)),
    );
}

/// Open (or close) the overlay when the action fires.
fn open_on_action(
    mut invoked: MessageReader<renzora::ShellActionInvoked>,
    open: Query<Entity, With<StoreOverlayRoot>>,
    mut commands: Commands,
) {
    if !invoked.read().any(|m| m.0 == ACTION_ID) {
        return;
    }
    // Toggle. The overlay's own X, Escape and backdrop click all despawn it
    // through ember's `overlay_dismiss`, so this only has to handle the case
    // where the same door is used twice.
    if let Some(existing) = open.iter().next() {
        commands.entity(existing).despawn();
        return;
    }
    commands.queue(|world: &mut World| {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
            return;
        };
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            // Not fullscreen: a margin of dimmed editor around it is what says
            // "this is over your work, not instead of it", and it is what makes
            // the backdrop click an obvious way out.
            let (root, content) = overlay_val(
                &mut commands,
                &fonts,
                &renzora::lang::t_or("marketplace.title", "Marketplace"),
                Val::Percent(88.0),
                Val::Percent(88.0),
                true,
            );
            // Above the docked panels, below the item and install overlays it
            // opens (9600 / 9700) and the image lightbox (9900) — all of which
            // are raised *from* this one and must land on top of it.
            commands.entity(root).insert(GlobalZIndex(9400));
            commands.entity(root).insert(StoreOverlayRoot);
            let store = crate::native_store::build(&mut commands, &fonts);
            commands.entity(content).add_child(store);
        }
        queue.apply(world);
    });
}
