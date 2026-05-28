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

pub mod components;
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

        // Markup callbacks (e.g. `<button on_press="start_game">`) with no
        // Rust binding fall through to scripts' `on_ui` hook. The bridge will
        // be re-attached to our own `MarkupOnPress` interaction in Phase D.
        app.init_resource::<renzora::ScriptUiInbox>()
            .add_observer(lua_bridge::handle_hui_spawn);

        // The path → entity-tree loader, and the component-template registry
        // (`assets/ui/components/*.html` indexed by file stem so `<menu_button>`
        // can be resolved by the loader).
        components::plugin(app);
        template::plugin(app);

        // Editor-only: hierarchy preset, hierarchy icons, and the bevy_ui
        // component inspectors with markup writeback.
        #[cfg(feature = "editor")]
        app.add_plugins((editor::HuiEditorPlugin, inspector::HuiInspectorPlugin));
    }
}

renzora::add!(HuiPlugin);
