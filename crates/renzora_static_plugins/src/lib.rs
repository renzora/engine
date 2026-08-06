//! The C-ABI plugins linked into this binary.
//!
//! **This file is generated.** The version checked into the repo returns an
//! empty list; the lean exporter overwrites it (and the manifest beside it)
//! inside `target/export-src/`, the throwaway workspace copy an export compiles.
//! Editing it here changes nothing about an export — it only sets what a
//! `--features static_plugins` build of the dev tree would link, which is
//! nothing.
//!
//! ## Why a generated crate rather than a generated `mod`
//!
//! The plugins are separate cargo packages outside the workspace. Linking one in
//! means adding a `path` dependency, which means editing a manifest — and the
//! manifest that gains 60 plugin dependencies should be the one crate whose only
//! job is to have them, not `renzora_app`'s. Keeping the generated surface to
//! "one crate, wholly rewritten" also means an export never has to parse and
//! patch a file a human maintains.
//!
//! See `renzora_plugin::static_link` for what this buys and what it costs.

use renzora_plugin::static_link::StaticPlugin;

/// Every plugin compiled into this binary, in the order the exporter listed
/// them. Empty in the dev tree.
pub fn plugins() -> Vec<StaticPlugin> {
    Vec::new()
}
