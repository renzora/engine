//! How the code editor colours a language a plugin brings.
//!
//! The editor knows a handful of formats by heart (Rust, WGSL, TOML, ...), but
//! a scripting language is a plugin, so the editor cannot know in advance which
//! ones a project will have. A language plugin describes its syntax here and
//! the editor colours its files with it: install the JavaScript plugin and
//! `.js` and `.ts` light up; uninstall it and they go back to plain text, which
//! is right, because nothing could run them any more.
//!
//! It is a description rather than a callback so that a plugin needs no
//! tokenizer of its own and the editor keeps one, with one set of fixes (the
//! `.5` freeze of issue #84 was in exactly that code). Everything is
//! `&'static`: a plugin's image is never unloaded, so its string tables live as
//! long as the editor does.

use bevy::prelude::{App, Resource};

/// One language's syntax, as the code editor needs it.
///
/// Build it from [`CodeSyntax::new`] with struct update syntax, setting only
/// what differs:
///
/// ```ignore
/// app.add_code_syntax(CodeSyntax {
///     keywords: &["function", "return", "if", "else"],
///     line_comment: Some("//"),
///     block_comment: Some(("/*", "*/")),
///     ..CodeSyntax::new("JavaScript", &["js"])
/// });
/// ```
#[derive(Clone, Copy, Debug)]
pub struct CodeSyntax {
    /// What the status bar calls the language.
    pub name: &'static str,
    /// File extensions, lowercase and without the dot.
    pub extensions: &'static [&'static str],
    /// Words coloured as keywords.
    pub keywords: &'static [&'static str],
    /// Words coloured as types.
    pub types: &'static [&'static str],
    /// Colour every capitalised identifier as a type, for languages where that
    /// is the naming convention (classes, interfaces, enums).
    pub caps_are_types: bool,
    /// The token that starts a comment running to the end of the line. Also
    /// what Ctrl+/ inserts.
    pub line_comment: Option<&'static str>,
    /// The tokens that open and close a comment that may span lines.
    pub block_comment: Option<(&'static str, &'static str)>,
    /// Characters that open and close a string on one line, such as `"'`.
    pub quotes: &'static str,
    /// Delimiters of strings that may span lines, such as `` ` `` or `"""`.
    /// Checked before [`Self::quotes`], so `"""` wins over `"`.
    pub multiline_strings: &'static [&'static str],
    /// Characters an identifier may contain besides letters, digits and `_`,
    /// such as `$` in JavaScript.
    pub identifier_chars: &'static str,
}

impl CodeSyntax {
    /// A language with the given name and extensions, no keywords, no comments
    /// and `"`/`'` strings: the starting point for struct update syntax.
    pub const fn new(name: &'static str, extensions: &'static [&'static str]) -> Self {
        Self {
            name,
            extensions,
            keywords: &[],
            types: &[],
            caps_are_types: false,
            line_comment: None,
            block_comment: None,
            quotes: "\"'",
            multiline_strings: &[],
            identifier_chars: "",
        }
    }
}

/// Every syntax plugins have registered, newest last.
#[derive(Resource, Default, Clone, Debug)]
pub struct CodeSyntaxes(pub Vec<CodeSyntax>);

impl CodeSyntaxes {
    /// The syntax for a file extension (lowercase, no dot). The newest
    /// registration wins, so a plugin can replace an earlier one.
    pub fn for_extension(&self, ext: &str) -> Option<&CodeSyntax> {
        self.0.iter().rev().find(|s| s.extensions.contains(&ext))
    }
}

/// Register a language's syntax with the code editor.
pub trait AppCodeSyntaxExt {
    /// Colour files with `syntax.extensions` by `syntax` in the code editor.
    ///
    /// Harmless in a game, which has no editor: the resource is simply never
    /// read. A plugin's syntax takes precedence over the editor's built-in one
    /// for the same extension.
    fn add_code_syntax(&mut self, syntax: CodeSyntax) -> &mut Self;
}

impl AppCodeSyntaxExt for App {
    fn add_code_syntax(&mut self, syntax: CodeSyntax) -> &mut Self {
        self.world_mut()
            .get_resource_or_insert_with(CodeSyntaxes::default)
            .0
            .push(syntax);
        self
    }
}
