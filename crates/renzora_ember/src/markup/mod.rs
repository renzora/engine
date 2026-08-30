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

/// Starter contents for a new `.html` UI template.
///
/// Owned by the crate that parses the format, so the three places that create
/// one — the Assets panel's New menu, the hierarchy's Attach menu, and the UI
/// Template slot's "+" — write the same file.
///
/// `boilerplate` off still writes the `<template>` root: markup without it does
/// not parse, so "minimal" is a skeleton rather than an empty file. On, it is a
/// laid-out panel with a heading and a button, because an empty `<node>` renders
/// as nothing at all and a canvas that shows nothing is indistinguishable from
/// one that is broken.
pub fn starter_template(boilerplate: bool) -> String {
    if !boilerplate {
        return "<template>\n    <node></node>\n</template>\n".to_string();
    }
    concat!(
        "<template>\n",
        "    <node\n",
        "        width=\"100%\"\n",
        "        height=\"100%\"\n",
        "        display=\"flex\"\n",
        "        flex_direction=\"column\"\n",
        "        align_items=\"center\"\n",
        "        justify_content=\"center\"\n",
        "        row_gap=\"12px\"\n",
        "    >\n",
        "        <text font_size=\"28\" font_color=\"#FFFFFF\">Your UI</text>\n",
        "\n",
        "        <button\n",
        "            padding=\"10px 22px\"\n",
        "            border_radius=\"6px\"\n",
        "            background=\"#5B9CF5\"\n",
        "            hover:background=\"#7BB0FF\"\n",
        "        >\n",
        "            <text font_size=\"14\" font_color=\"#FFFFFF\">Play</text>\n",
        "        </button>\n",
        "    </node>\n",
        "</template>\n",
    )
    .to_string()
}

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
