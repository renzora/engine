//! The Community panels. Each module registers its shell metadata + content
//! builder + systems.

pub(crate) mod chat;
pub(crate) mod feed;
pub(crate) mod friends;
pub(crate) mod learn;
pub(crate) mod notifications;
pub(crate) mod onboarding;
pub(crate) mod profile;
pub(crate) mod teams;
pub(crate) mod wallet;

use bevy::prelude::*;

pub(crate) fn register(app: &mut App) {
    friends::register(app);
    chat::register(app);
    // Notifications is data-only now (no panel) — see the module doc.
    notifications::register(app);
    feed::register(app);
    profile::register(app);
    // Teams is data + a section embedded in the Friends panel (no panel of its
    // own) — see the module doc.
    teams::register(app);
    learn::register(app);
    wallet::register(app);
    onboarding::register(app);
}
