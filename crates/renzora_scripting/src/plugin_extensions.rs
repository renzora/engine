//! Turning a plugin's declared script functions into registered bindings.
//!
//! A workspace crate implements [`ScriptExtension`](crate::extension::ScriptExtension)
//! and hands over `Binding`s directly. A native plugin cannot: `Binding` is
//! defined in `renzora_plugin::script`, and putting that within a plugin's
//! reach would mean compiling the whole script wire codec into the contract
//! crate. So a plugin declares the same thing as plain data in
//! [`renzora::script_fns`], and this module translates.
//!
//! The translation is deliberately total. Every `ScriptFn` shape maps onto
//! exactly one `Binding` shape, so a plugin's verbs are indistinguishable from
//! the engine's once registered: same collision handling, same generation bump,
//! same autocomplete.
//!
//! # Why this drains every frame instead of running once
//!
//! Native plugins are not all present when `ScriptPlugin` builds. One installed
//! from the marketplace mid-session loads later, and one rebuilt after a source
//! edit loads again. A one-shot `Startup` drain would give the first case no
//! verbs at all and the second case stale ones, and both failures are silent:
//! the script calls a function that does not exist and the interpreter reports
//! an unknown global, which reads like a typo rather than a load-order bug.

use bevy::prelude::*;

use renzora::script_fns::{PluginScriptFns, ScriptArgKind, ScriptFn, ScriptFnKind};
use renzora_plugin::script::{Binding, BindingKind, Param, ParamKind};

use crate::extension::{ScriptExtension, ScriptExtensions};

/// One plugin's worth of declared functions, wearing the trait
/// [`ScriptExtensions::register`] expects.
struct DeclaredExtension {
    name: String,
    bindings: Vec<Binding>,
}

impl ScriptExtension for DeclaredExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn bindings(&self) -> Vec<Binding> {
        self.bindings.clone()
    }
}

fn kind_of(kind: ScriptArgKind) -> ParamKind {
    match kind {
        ScriptArgKind::Float => ParamKind::Float,
        ScriptArgKind::Int => ParamKind::Int,
        ScriptArgKind::Bool => ParamKind::Bool,
        ScriptArgKind::Str => ParamKind::Str,
        ScriptArgKind::Vec3 => ParamKind::Vec3,
    }
}

fn binding_of(f: ScriptFn) -> Binding {
    Binding {
        name: f.name,
        kind: match f.kind {
            ScriptFnKind::Action { action } => BindingKind::Action { action },
            ScriptFnKind::Read { component, field } => BindingKind::Read { component, field },
            ScriptFnKind::Translate => BindingKind::Translate,
        },
        params: f
            .args
            .into_iter()
            .map(|a| Param {
                name: a.name,
                kind: kind_of(a.kind),
            })
            .collect(),
        doc: f.doc,
    }
}

/// Register anything plugins have declared since the last run.
///
/// `Option<ResMut<..>>` because the resource only exists once some plugin has
/// declared something: a build with no such plugin never inserts it, and this
/// should cost a null check rather than force the resource into existence.
pub fn drain_plugin_script_fns(
    pending: Option<ResMut<PluginScriptFns>>,
    extensions: Option<ResMut<ScriptExtensions>>,
) {
    let (Some(mut pending), Some(mut extensions)) = (pending, extensions) else {
        return;
    };
    if pending.pending.is_empty() {
        return;
    }
    for (name, fns) in pending.pending.drain(..) {
        extensions.register(DeclaredExtension {
            name,
            bindings: fns.into_iter().map(binding_of).collect(),
        });
    }
}
