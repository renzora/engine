//! Scripting binding for localization — exposes `tr("key")`.
//!
//! `tr` is a pure read of the shared translation table (active language →
//! English → key), so it needs no `ScriptCommand`/observer plumbing. Scripts
//! use it to localize any text they push into game UI, e.g.
//! `set_text(label, tr("hud.score"))`.
//!
//! It gets its own [`BindingKind`](renzora_scripting::extension::BindingKind)
//! rather than riding on the reflected-field read, because the translation
//! table is not a component — there is no entity to read it from.

use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct LangScriptExtension;

impl ScriptExtension for LangScriptExtension {
    fn name(&self) -> &str {
        "localization"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Bind::translate("tr")
            .arg("key", ParamKind::Str)
            .doc("Translate a key into the active language.")
            .build()]
    }
}
