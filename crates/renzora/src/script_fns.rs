//! Script functions a **plugin** declares.
//!
//! A crate in the workspace declares its script verbs by implementing
//! `renzora_scripting::extension::ScriptExtension`. A plugin cannot: that trait
//! is defined over `renzora_plugin::script::Binding`, and reaching it would
//! mean compiling the whole script wire codec (some 4,500 lines of encode and
//! decode) into this crate, which every other crate in the engine depends on.
//!
//! So this is the same declaration expressed as plain data. A plugin pushes
//! [`ScriptFn`]s into [`PluginScriptFns`]; `renzora_scripting` drains them and
//! registers them exactly as if a workspace crate had declared them. The
//! vocabulary below deliberately mirrors `Binding` one-for-one, because the
//! translation has to be lossless in both directions or a plugin's verbs would
//! be second-class next to the engine's.
//!
//! ## Why a queue and not a registry
//!
//! Draining rather than reading means a plugin loaded ten seconds after startup
//! contributes its functions just as one loaded during `App` build does, and a
//! plugin reloaded after an edit re-declares rather than duplicating. The
//! alternative, a registry read once, would have made "was this plugin present
//! when scripting initialised" decide whether its verbs exist, which is exactly
//! the kind of load-order dependence the shell registries were shaped to avoid.

use bevy::prelude::*;

/// The type of one declared argument.
///
/// Mirrors `renzora_plugin::script::ParamKind`. `Vec3` is one parameter that
/// consumes three script-level arguments, which is what lets
/// `parkour_move(x, y, z)` and `nav_set_destination(x, y, z)` both be
/// declarable when one wants three floats and the other wants a vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptArgKind {
    Float,
    Int,
    Bool,
    Str,
    Vec3,
}

impl ScriptArgKind {
    /// How many script-level arguments this consumes.
    pub fn arity(self) -> usize {
        match self {
            Self::Vec3 => 3,
            _ => 1,
        }
    }
}

/// One parameter of a declared function.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptArg {
    /// The key this becomes in the action's argument list.
    pub name: String,
    pub kind: ScriptArgKind,
}

/// What a declared function does when a script calls it.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptFnKind {
    /// Pack the arguments and fire a [`ScriptAction`](crate::ScriptAction) the
    /// plugin observes. Returns nothing.
    Action { action: String },
    /// Read a reflected field and return it.
    ///
    /// `component` and `field` may contain `{0}`, `{1}` … placeholders, which
    /// the backend substitutes with the call's arguments.
    Read { component: String, field: String },
    /// Look the argument up in the localization table and return the result.
    Translate,
}

/// A script function, declared rather than written.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptFn {
    /// The name scripts call, e.g. `"parkour_jump"`.
    pub name: String,
    pub kind: ScriptFnKind,
    /// Parameters in call order.
    pub args: Vec<ScriptArg>,
    /// One-line description, for editor autocomplete. May be empty.
    pub doc: String,
}

impl ScriptFn {
    /// A function that fires an action of the same name.
    pub fn action(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            kind: ScriptFnKind::Action {
                action: name.clone(),
            },
            name,
            args: Vec::new(),
            doc: String::new(),
        }
    }

    /// A function that reads a reflected field.
    pub fn read(name: impl Into<String>, component: impl Into<String>, field: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: ScriptFnKind::Read {
                component: component.into(),
                field: field.into(),
            },
            args: Vec::new(),
            doc: String::new(),
        }
    }

    /// Append a parameter.
    pub fn arg(mut self, name: impl Into<String>, kind: ScriptArgKind) -> Self {
        self.args.push(ScriptArg {
            name: name.into(),
            kind,
        });
        self
    }

    /// Append the three floats an `x`/`y`/`z` verb takes.
    pub fn xyz(self) -> Self {
        self.arg("x", ScriptArgKind::Float)
            .arg("y", ScriptArgKind::Float)
            .arg("z", ScriptArgKind::Float)
    }

    /// Set the one-line description.
    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }
}

/// Functions declared by plugins and not yet handed to the scripting system.
///
/// Push here from a plugin's `build`; `renzora_scripting` drains it. Absent
/// when the engine was built without scripting, so a plugin declaring verbs
/// into a game that cannot run scripts does nothing rather than failing.
#[derive(Resource, Default)]
pub struct PluginScriptFns {
    /// Grouped by the declaring plugin's name, which is what the scripting
    /// system logs and what a duplicate-name warning has to be able to blame.
    pub pending: Vec<(String, Vec<ScriptFn>)>,
}

impl PluginScriptFns {
    /// Declare a plugin's script functions.
    pub fn declare(&mut self, plugin: impl Into<String>, fns: Vec<ScriptFn>) {
        self.pending.push((plugin.into(), fns));
    }
}

/// `app.declare_script_fns("parkour", vec![…])` from a plugin's `build`.
pub trait DeclareScriptFns {
    fn declare_script_fns(&mut self, plugin: impl Into<String>, fns: Vec<ScriptFn>) -> &mut Self;
}

impl DeclareScriptFns for App {
    fn declare_script_fns(&mut self, plugin: impl Into<String>, fns: Vec<ScriptFn>) -> &mut Self {
        let mut pending = self
            .world_mut()
            .get_resource_or_insert_with(PluginScriptFns::default);
        pending.declare(plugin, fns);
        self
    }
}
