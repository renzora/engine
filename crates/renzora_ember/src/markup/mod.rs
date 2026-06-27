//! Markup runtime — author UI as hot-reloadable `.html` compiled into a `bevy_ui`
//! entity tree. Folded in from the former `renzora_hui` crate; ember is now the
//! single UI crate.
//!
//! Uses **only the parser** half of the vendored `bevy_hui` fork (under
//! `crates/bevy_hui/`) to read `.html` into a typed AST; [`loader`] then walks it
//! and spawns one entity per node with standard bevy_ui components attached
//! directly. **No bevy_hui runtime.** See `docs/renzora_markup.md`.

use bevy::prelude::*;

pub mod binding;
pub mod cursor;
pub mod decor;
pub mod dnd;
pub mod drag;
pub mod foreach;
pub mod input_field;
pub mod interactions;
pub mod loader;
pub mod lua_bridge;
pub mod provenance;
pub mod template;
pub mod transitions;
pub mod vector;
pub mod widgets;
pub mod writeback;

pub use provenance::MarkupSource;
pub use template::{HtmlTemplatePath, TemplateReloadRequests};

/// The markup runtime plugin (formerly `renzora_hui::HuiPlugin`). Registered via
/// `renzora::add!` at Runtime scope so it runs in both the editor viewport and
/// shipped games — anywhere markup UI is used. The lean export strips this whole
/// module (and `renzora_game_ui`/`bevy_hui`) via the `game_ui` feature; the
/// foundational `cursor_icon`/`icons` systems it once installed now live in
/// `EmberPlugin` so they run with or without markup.
#[derive(Default)]
pub struct MarkupPlugin;

impl Plugin for MarkupPlugin {
    fn build(&self, app: &mut App) {
        // Parser-side only: registers `HtmlTemplate` as an asset + its `.html`
        // loader. We do NOT add bevy_hui's Build/Transition/Binding runtime.
        app.add_plugins(bevy_hui::prelude::LoaderPlugin);

        app.init_resource::<renzora::ScriptUiInbox>()
            .add_observer(lua_bridge::handle_hui_spawn)
            .add_observer(lua_bridge::handle_hui_despawn)
            .add_observer(lua_bridge::handle_hui_hide)
            .add_observer(lua_bridge::handle_hui_show)
            .add_observer(lua_bridge::handle_quit);

        template::plugin(app);
        interactions::plugin(app);
        cursor::plugin(app);
        drag::plugin(app);
        dnd::plugin(app);
        binding::plugin(app);
        foreach::plugin(app);
        input_field::plugin(app);
        widgets::plugin(app);
        transitions::plugin(app);
        vector::plugin(app);
        // `cursor_icon` + `icons` are now installed by `EmberPlugin` (they're
        // foundational to all widgets, not just markup) — see ember/src/lib.rs.
    }
}

renzora::add!(MarkupPlugin);
