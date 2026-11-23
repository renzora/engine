//! Script functions a domain crate *declares* rather than writes.
//!
//! `renzora_physics` wants scripts to be able to say `apply_force(x, y, z)`.
//! It used to get that by taking a dependency on `mlua` and writing the
//! function by hand into every Lua state, which meant the physics crate
//! compiled a Lua interpreter and a second language would have meant a second
//! copy of every binding.
//!
//! It turned out that every one of the engine's extensions was the same four
//! lines: read the arguments, pack them into a `ScriptCommand::Action`, push
//! it. So the crate declares the shape instead and the backend builds the
//! function. `renzora_physics` no longer knows what Lua is, and a Wren backend
//! gets `apply_force` without `renzora_physics` knowing what Wren is either.
//!
//! ## Why this lives in the contract crate
//!
//! Because a plugin has to be able to declare script functions too, and a
//! plugin reaches exactly three crates: `bevy`, `renzora` and `renzora_ember`.
//! While this lived in `renzora_scripting` it was unreachable from `plugins/`,
//! so the one mechanism designed to let any crate extend the script API was
//! open only to crates compiled into the engine. `renzora_scripting` re-exports
//! all of it, so every existing `use renzora_scripting::extension::…` still
//! resolves.
//!
//! ## The three shapes
//!
//! [`BindingKind`] has exactly three variants because that is what the engine's
//! extensions needed: fire an action, read a reflected field, translate a
//! string. A fourth shape means a fourth variant, which is a change here and in
//! each language backend, so it should be worth it.

use bevy::prelude::*;

/// The type of one parameter of a declared binding.
///
/// [`ParamKind::Vec3`] consumes **three** numbers from the script call and
/// produces one [`renzora::ScriptActionValue::Vec3`] argument. That is not a
/// convenience: both shapes exist in the engine today — `apply_force(x, y, z)`
/// sends three separate float args named `x`, `y`, `z`, while
/// `nav_set_destination(x, y, z)` sends one Vec3 named `target` — and a binding
/// system that could only express one of them would not replace the
/// hand-written functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Float,
    Int,
    Bool,
    Str,
    Vec3,
}

impl ParamKind {
    /// How many script-level arguments this parameter consumes.
    pub fn arity(self) -> usize {
        match self {
            Self::Vec3 => 3,
            _ => 1,
        }
    }
}

/// One parameter of a declared binding.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// The key this becomes in the action's argument list.
    pub name: String,
    pub kind: ParamKind,
}

/// What a declared binding does when called.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingKind {
    /// Pack the arguments and emit a `ScriptCommand::Action`, which fires a
    /// [`ScriptAction`](crate::ScriptAction) the owning crate observes. Returns
    /// nothing.
    Action { action: String },
    /// Read a reflected field and return it.
    ///
    /// `component` and `field` may contain `{0}`, `{1}` … placeholders, which
    /// the backend substitutes with the call's arguments. That is what lets
    /// `get_animation_length(name)` be declared rather than written: it is
    /// `Read { component: "AnimatorReadState", field: "clip_lengths.{0}" }`.
    Read { component: String, field: String },
    /// Look the argument up in the localization table and return the result.
    Translate,
}

/// A script function a domain crate declares rather than writes.
///
/// This is what replaced `ScriptExtension::register_lua_functions`. Every one
/// of the engine's five extensions turned out to be sugar of exactly this
/// shape — pack the arguments, fire an action — so declaring it means the
/// domain crate stops linking a Lua interpreter and *every* language backend
/// gets the function for free. A Wren backend picks these up without
/// `renzora_physics` knowing Wren exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// The name scripts call, e.g. `"apply_force"`.
    pub name: String,
    pub kind: BindingKind,
    /// Parameters in call order.
    pub params: Vec<Param>,
    /// One-line description, for editor autocomplete. May be empty.
    pub doc: String,
}

/// Builds a [`Binding`].
///
/// A builder rather than inherent methods on `Binding` itself, so the struct
/// stays a plain description of the function and the fluent form stays
/// optional. It reads the way the hand-written functions did, which is the
/// point:
///
/// ```ignore
/// Bind::action("apply_force", "apply_force")
///     .arg("x", ParamKind::Float)
///     .arg("y", ParamKind::Float)
///     .arg("z", ParamKind::Float)
///     .doc("Apply a force in world space.")
///     .build()
/// ```
pub struct Bind(Binding);

impl Bind {
    /// A function that fires a `ScriptAction` and returns nothing.
    pub fn action(name: &str, action: &str) -> Self {
        Self(Binding {
            name: name.to_string(),
            kind: BindingKind::Action {
                action: action.to_string(),
            },
            params: Vec::new(),
            doc: String::new(),
        })
    }

    /// A function that reads a reflected field and returns it.
    ///
    /// `component` and `field` may contain `{0}`, `{1}` … placeholders, which
    /// the backend substitutes with the call's arguments — that is what lets
    /// `get_animation_length(name)` read `clip_lengths.{0}`.
    pub fn read(name: &str, component: &str, field: &str) -> Self {
        Self(Binding {
            name: name.to_string(),
            kind: BindingKind::Read {
                component: component.to_string(),
                field: field.to_string(),
            },
            params: Vec::new(),
            doc: String::new(),
        })
    }

    /// A function that looks its argument up in the localization table.
    pub fn translate(name: &str) -> Self {
        Self(Binding {
            name: name.to_string(),
            kind: BindingKind::Translate,
            params: Vec::new(),
            doc: String::new(),
        })
    }

    /// One parameter, in call order.
    pub fn arg(mut self, name: &str, kind: ParamKind) -> Self {
        self.0.params.push(Param {
            name: name.to_string(),
            kind,
        });
        self
    }

    /// Three numbers from the call, packed into one `Vec3` argument.
    pub fn vec3(self, name: &str) -> Self {
        self.arg(name, ParamKind::Vec3)
    }

    /// Three consecutive float arguments named `x`, `y`, `z` — the shape most
    /// of the physics bindings use, where the action wants three separate
    /// values rather than one vector.
    pub fn xyz(self) -> Self {
        self.arg("x", ParamKind::Float)
            .arg("y", ParamKind::Float)
            .arg("z", ParamKind::Float)
    }

    /// One-line description, for editor autocomplete.
    pub fn doc(mut self, doc: &str) -> Self {
        self.0.doc = doc.to_string();
        self
    }

    pub fn build(self) -> Binding {
        self.0
    }
}

/// A crate that adds script functions.
pub trait ScriptExtension: Send + Sync + 'static {
    /// For logs.
    fn name(&self) -> &str;

    /// The functions this crate contributes.
    fn bindings(&self) -> Vec<Binding>;
}

/// Every registered extension's bindings, merged.
#[derive(Resource, Default)]
pub struct ScriptExtensions {
    names: Vec<String>,
    bindings: Vec<Binding>,
    generation: u64,
}

impl ScriptExtensions {
    /// Register an extension's bindings.
    ///
    /// The extension object itself is not kept — only its bindings are, since
    /// there is nothing left to call it for. A duplicate function name is
    /// refused rather than shadowing, because "which crate won" would depend on
    /// plugin registration order.
    pub fn register(&mut self, ext: impl ScriptExtension) {
        let name = ext.name().to_string();
        let mut added = 0;
        for b in ext.bindings() {
            if let Some(existing) = self.bindings.iter().find(|e| e.name == b.name) {
                warn!(
                    "[scripting] `{}` from `{name}` collides with the one from an earlier \
                     extension and is ignored (kind {:?})",
                    b.name, existing.kind
                );
                continue;
            }
            self.bindings.push(b);
            added += 1;
        }
        info!("[scripting] extension `{name}` declared {added} function(s)");
        self.names.push(name);
        // Bumped even when nothing was added, so a backend that keyed a rebuild
        // on this cannot miss a later registration that did add something.
        self.generation += 1;
    }

    /// Every declared function.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Changes whenever [`Self::register`] runs. A language backend compares
    /// this against what it last built and rebuilds its function table on
    /// mismatch, rather than re-sending the whole list every frame.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Names of the registered extensions, for diagnostics.
    pub fn extension_names(&self) -> &[String] {
        &self.names
    }
}

/// Resolve `{0}`-style placeholders in a [`BindingKind::Read`] path with the
/// call's arguments.
///
/// Lives here rather than in any one backend so every language resolves a path
/// the same way: a Lua backend and a Wren backend disagreeing about what
/// `clip_lengths.{0}` means would be a genuinely miserable bug to find.
///
/// A placeholder with no matching argument is left as written, deliberately:
/// a path that visibly fails to resolve beats one that silently reads a
/// different field.
pub fn substitute(template: &str, args: &[String]) -> String {
    // Almost every template has no placeholder at all, so do not build a new
    // string for the common case.
    if !template.contains('{') {
        return template.to_string();
    }
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), a);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ext(&'static str, Vec<Binding>);
    impl ScriptExtension for Ext {
        fn name(&self) -> &str {
            self.0
        }
        fn bindings(&self) -> Vec<Binding> {
            self.1.clone()
        }
    }

    #[test]
    fn xyz_expands_to_three_float_parameters() {
        let b = Bind::action("apply_force", "apply_force").xyz().build();
        assert_eq!(b.params.len(), 3);
        assert_eq!(b.params[0].name, "x");
        assert_eq!(b.params[2].name, "z");
        assert!(b.params.iter().all(|p| p.kind == ParamKind::Float));
    }

    #[test]
    fn a_vec3_parameter_stays_one_argument() {
        let b = Bind::action("nav_set_destination", "nav_set_destination")
            .vec3("target")
            .build();
        assert_eq!(b.params.len(), 1);
        assert_eq!(b.params[0].kind, ParamKind::Vec3);
        // …but consumes three from the script call.
        assert_eq!(b.params[0].kind.arity(), 3);
    }

    #[test]
    fn registering_merges_bindings_and_bumps_the_generation() {
        let mut exts = ScriptExtensions::default();
        assert_eq!(exts.generation(), 0);

        exts.register(Ext("physics", vec![Bind::action("apply_force", "apply_force").xyz().build()]));
        assert_eq!(exts.bindings().len(), 1);
        assert_eq!(exts.generation(), 1);

        exts.register(Ext("nav", vec![Bind::action("nav_stop", "nav_clear_destination").build()]));
        assert_eq!(exts.bindings().len(), 2);
        assert_eq!(exts.generation(), 2);
        assert_eq!(exts.extension_names(), ["physics", "nav"]);
    }

    #[test]
    fn a_duplicate_function_name_is_refused_rather_than_shadowing() {
        let mut exts = ScriptExtensions::default();
        exts.register(Ext("a", vec![Bind::action("boom", "a_boom").build()]));
        exts.register(Ext("b", vec![Bind::action("boom", "b_boom").build()]));

        assert_eq!(exts.bindings().len(), 1);
        assert_eq!(
            exts.bindings()[0].kind,
            BindingKind::Action {
                action: "a_boom".into()
            },
            "the first registration must win, so behaviour does not depend on load order"
        );
    }

    #[test]
    fn the_generation_moves_even_when_a_registration_adds_nothing() {
        let mut exts = ScriptExtensions::default();
        exts.register(Ext("a", vec![Bind::action("boom", "a_boom").build()]));
        let g = exts.generation();
        exts.register(Ext("b", vec![Bind::action("boom", "b_boom").build()]));
        assert_ne!(exts.generation(), g);
    }

    #[test]
    fn placeholders_are_substituted_by_position() {
        assert_eq!(
            substitute("clip_lengths.{0}", &["run".to_string()]),
            "clip_lengths.run"
        );
        assert_eq!(
            substitute("{1}.{0}", &["b".to_string(), "a".to_string()]),
            "a.b"
        );
    }

    #[test]
    fn a_path_with_no_placeholder_is_returned_unchanged() {
        assert_eq!(substitute("grounded", &["ignored".to_string()]), "grounded");
        assert_eq!(substitute("a.b.c", &[]), "a.b.c");
    }

    #[test]
    fn a_placeholder_with_no_argument_is_left_alone_rather_than_blanked() {
        // Better a path that visibly fails to resolve than one that silently
        // reads the wrong field.
        assert_eq!(substitute("clip_lengths.{0}", &[]), "clip_lengths.{0}");
    }
}
