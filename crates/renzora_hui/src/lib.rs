//! Renzora HUI — author UI as hot-reloadable markup (`.html`) compiled into a
//! `bevy_ui` entity tree.
//!
//! Uses **only the parser** half of the vendored `bevy_hui` fork (under
//! `crates/bevy_hui/`) to read `.html` files into a typed AST
//! (`HtmlTemplate`/`XNode`/`StyleAttr`). Renzora's own [`loader`] then walks
//! that AST and spawns one entity per markup node with standard bevy_ui
//! components (`Node`, `BackgroundColor`, `Text`, `TextFont`, …) attached
//! directly. **No bevy_hui runtime** — no `HtmlNode`, no per-frame style
//! re-assertion, no `retain::<KeepComps>()` strip. See
//! `docs/renzora_markup.md` for the architecture.

use bevy::prelude::*;

pub mod binding;
pub mod cursor;
pub mod drag;
pub mod interactions;
pub mod loader;
pub mod lua_bridge;
pub mod provenance;
pub mod template;
pub mod writeback;

pub use provenance::MarkupSource;
pub use template::HtmlTemplatePath;

#[cfg(feature = "editor")]
pub mod editor;

#[cfg(feature = "editor")]
pub mod inspector;

#[derive(Default)]
pub struct HuiPlugin;

impl Plugin for HuiPlugin {
    fn build(&self, app: &mut App) {
        // Parser-side only: registers `HtmlTemplate` as an asset and its loader
        // so `.html` files load into a typed AST. We do **not** add
        // bevy_hui's `BuildPlugin`/`TransitionPlugin`/`BindingPlugin`/etc. —
        // those are the runtime we're replacing.
        app.add_plugins(bevy_hui::prelude::LoaderPlugin);

        // Markup callbacks (`on_press="start_game"`, etc.) get attached as
        // bevy_hui's `OnUiPress`/`OnUiEnter`/... components by the loader, and
        // `interactions::forward_ui_interactions` watches `Changed<Interaction>`
        // to push them into `ScriptUiInbox` for every script's `on_ui` hook.
        app.init_resource::<renzora::ScriptUiInbox>()
            .add_observer(lua_bridge::handle_hui_spawn)
            .add_observer(lua_bridge::handle_hui_despawn)
            .add_observer(lua_bridge::handle_hui_hide)
            .add_observer(lua_bridge::handle_hui_show)
            .add_observer(lua_bridge::handle_quit);

        // The path → entity-tree loader and the markup → script interaction
        // bridge. Components used to be auto-registered by file stem; now
        // every reuse is via `<node template="path">`, so there's no separate
        // registry — paths resolve through `AssetServer` like any other asset.
        template::plugin(app);
        interactions::plugin(app);
        cursor::plugin(app);
        drag::plugin(app);
        binding::plugin(app);

        // Editor-only: hierarchy preset, hierarchy icons, and the bevy_ui
        // component inspectors with markup writeback.
        #[cfg(feature = "editor")]
        app.add_plugins((editor::HuiEditorPlugin, inspector::HuiInspectorPlugin));
    }
}

renzora::add!(HuiPlugin);
