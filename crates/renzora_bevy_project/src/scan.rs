//! Reading a Bevy crate's source well enough to rebuild it as a plugin.
//!
//! Three questions, all answered by looking at text:
//!
//! 1. which files are the crate's modules ([`modules`]), so the generated root
//!    can declare them by absolute path;
//! 2. where `fn main` is and what its body says ([`main_body`]);
//! 3. which of that body builds the `App` and which of it opens a window
//!    ([`app_construction`]).
//!
//! # Why text and not a parser
//!
//! The same reason the `add!` generator reads `add!(…)` as text: a real parser
//! means `syn`, `syn` means `proc-macro2` and `quote`, and those are three
//! crates in the engine's dependency graph to answer questions whose hard cases
//! are not syntactic anyway. What makes this tractable is that everything being
//! looked for is at a known nesting depth in a file whose shape is conventional
//! (`mod` at the top level, `App::new()` inside `fn main`), and [`Masked`]
//! makes "at the top level" something that can actually be counted, rather than
//! something a regex pretends to know.
//!
//! What it gives up is the general case, and it gives it up loudly rather than
//! quietly. A project whose `App` is assembled somewhere this cannot see gets a
//! message naming the escape hatch (`[package.metadata.renzora] plugins = [...]`),
//! not a game that loads with no systems in it.

/// Source with comments and literals blanked out, so offsets still line up.
///
/// Every search here is really "find this token where it is code", and the three
/// ways that goes wrong are a `//` comment, a string, and a `'a` lifetime that
/// looks like the start of a char literal. Blanking them to spaces rather than
/// deleting them means an offset found in the mask is an offset in the original,
/// so the two can be used together: search the mask, slice the source.
pub struct Masked {
    masked: String,
}

impl Masked {
    pub fn new(src: &str) -> Self {
        let bytes = src.as_bytes();
        let mut out = String::with_capacity(src.len());
        let mut i = 0usize;

        // Pushes one blanked byte per source byte, keeping newlines so that a
        // line-oriented diagnostic still points at the right line.
        macro_rules! blank {
            ($n:expr) => {
                for k in 0..$n {
                    out.push(if bytes[i + k] == b'\n' { '\n' } else { ' ' });
                }
                i += $n;
            };
        }

        while i < bytes.len() {
            let rest = &bytes[i..];
            if rest.starts_with(b"//") {
                let n = rest.iter().position(|b| *b == b'\n').unwrap_or(rest.len());
                blank!(n);
            } else if rest.starts_with(b"/*") {
                // Nested, because Rust's block comments nest and a naive scan to
                // the first `*/` ends a comment that is still open.
                let mut depth = 0usize;
                let mut n = 0usize;
                while n < rest.len() {
                    if rest[n..].starts_with(b"/*") {
                        depth += 1;
                        n += 2;
                    } else if rest[n..].starts_with(b"*/") {
                        depth -= 1;
                        n += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        n += 1;
                    }
                }
                blank!(n.min(rest.len()));
            } else if rest.starts_with(b"r\"") || starts_raw_string(rest) {
                let n = raw_string_len(rest);
                blank!(n);
            } else if rest[0] == b'"' {
                let n = string_len(rest);
                blank!(n);
            } else if rest[0] == b'\'' {
                match char_literal_len(rest) {
                    // A lifetime, not a literal. Left as code: `&'static str`
                    // has to keep reading as code or every brace after it is
                    // counted on the wrong side of a quote.
                    None => {
                        out.push('\'');
                        i += 1;
                    }
                    Some(n) => {
                        blank!(n);
                    }
                }
            } else {
                // Pushed by byte, not by char: the mask has to stay the same
                // length as the source, and a multi-byte char pushed whole would
                // still be the same length, but pushing bytes keeps the
                // invariant obvious and the indexing honest.
                let ch = src[i..].chars().next().unwrap_or(' ');
                out.push_str(&src[i..i + ch.len_utf8()]);
                i += ch.len_utf8();
            }
        }
        Masked { masked: out }
    }

    pub fn as_str(&self) -> &str {
        &self.masked
    }

    /// Byte offsets at which `needle` appears as a whole identifier in code.
    pub fn find_word(&self, needle: &str) -> Vec<usize> {
        let mut out = Vec::new();
        let b = self.masked.as_bytes();
        let mut from = 0usize;
        while let Some(rel) = self.masked[from..].find(needle) {
            let at = from + rel;
            let before_ok = at == 0 || !is_ident_byte(b[at - 1]);
            let after = at + needle.len();
            let after_ok = after >= b.len() || !is_ident_byte(b[after]);
            if before_ok && after_ok {
                out.push(at);
            }
            from = at + needle.len();
        }
        out
    }

    /// Brace nesting depth at `offset`, counting only `{}`.
    ///
    /// `{}` alone, not `()` or `[]`: the question every caller asks is "is this
    /// item at the crate's top level", and that is about blocks. A `mod` inside
    /// a parenthesised expression is not a thing.
    pub fn depth_at(&self, offset: usize) -> usize {
        let mut depth = 0i32;
        for b in self.masked.as_bytes()[..offset].iter() {
            match b {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
        }
        depth.max(0) as usize
    }

    /// Offset just past the delimiter matching the one that opens at `open`.
    ///
    /// `open` must point at the opening delimiter itself. Returns `None` for an
    /// unbalanced file, which is a file that would not compile anyway.
    pub fn match_delim(&self, open: usize) -> Option<usize> {
        let b = self.masked.as_bytes();
        let (o, c) = match b.get(open)? {
            b'{' => (b'{', b'}'),
            b'(' => (b'(', b')'),
            b'[' => (b'[', b']'),
            _ => return None,
        };
        let mut depth = 0usize;
        for (i, byte) in b.iter().enumerate().skip(open) {
            if *byte == o {
                depth += 1;
            } else if *byte == c {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
        }
        None
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn starts_raw_string(rest: &[u8]) -> bool {
    // `r#"`, `r##"`, … and the `br#"` byte-string forms.
    let rest = rest.strip_prefix(b"b").unwrap_or(rest);
    let Some(rest) = rest.strip_prefix(b"r") else {
        return false;
    };
    let hashes = rest.iter().take_while(|b| **b == b'#').count();
    rest.get(hashes) == Some(&b'"')
}

fn raw_string_len(rest: &[u8]) -> usize {
    let lead = if rest.starts_with(b"b") { 1 } else { 0 };
    let hashes = rest[lead + 1..].iter().take_while(|b| **b == b'#').count();
    let open = lead + 1 + hashes + 1; // b? r ##… "
    let mut close = Vec::with_capacity(hashes + 1);
    close.push(b'"');
    close.extend(std::iter::repeat_n(b'#', hashes));
    let mut i = open;
    while i < rest.len() {
        if rest[i..].starts_with(&close) {
            return i + close.len();
        }
        i += 1;
    }
    rest.len()
}

fn string_len(rest: &[u8]) -> usize {
    let mut i = 1usize;
    while i < rest.len() {
        match rest[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    rest.len()
}

/// Length of a char literal at `rest`, or `None` if this `'` opens a lifetime.
///
/// The distinction matters more than it looks: `&'a mut World` masked as a
/// literal swallows everything to the next quote, which is usually several
/// braces away, and every depth computed after it is wrong.
fn char_literal_len(rest: &[u8]) -> Option<usize> {
    if rest.get(1) == Some(&b'\\') {
        // An escape. Scan to the closing quote; `'\u{1F600}'` is why this is not
        // a fixed length.
        let mut i = 2usize;
        while i < rest.len() && i < 12 {
            if rest[i] == b'\'' {
                return Some(i + 1);
            }
            i += 1;
        }
        return None;
    }
    // One char then a quote is a literal; anything else beginning with an
    // identifier byte is a lifetime.
    let ch = core::str::from_utf8(&rest[1..rest.len().min(5)]).ok()?.chars().next()?;
    let n = 1 + ch.len_utf8();
    if rest.get(n) == Some(&b'\'') {
        return Some(n + 1);
    }
    None
}

/// One `mod foo;` declaration found at a file's top level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDecl {
    /// The module's name.
    pub name: String,
    /// Byte range of the whole item, from `pub`/`mod` through the `;`, so a
    /// caller can replace it. Any `#[cfg]` attributes above it are deliberately
    /// **outside** this range and survive the rewrite.
    pub span: (usize, usize),
}

/// Every `mod foo;` at the top level of `src`.
///
/// Top level only. A `mod` nested inside another `mod { … }` block in the same
/// file needs no rewriting: it resolves relative to the file it is written in,
/// which is where it already was.
pub fn modules(mask: &Masked) -> Vec<ModDecl> {
    let text = mask.as_str();
    let bytes = text.as_bytes();
    let mut out = Vec::new();

    for at in mask.find_word("mod") {
        if mask.depth_at(at) != 0 {
            continue;
        }
        // `mod` is only a module declaration when an identifier and a `;`
        // follow. `mod foo { … }` is inline and already resolved, and something
        // like `use a::mod_thing` never reaches here because `find_word`
        // requires a non-identifier byte on both sides.
        let mut i = at + 3;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && is_ident_byte(bytes[i]) {
            i += 1;
        }
        if i == start {
            continue;
        }
        let name = text[start..i].to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b';') {
            continue;
        }
        let end = i + 1;

        // Walk back over the visibility, so the replacement covers `pub mod x;`
        // and not just the `mod x;` inside it: an attribute has to precede the
        // whole item, and `pub #[path = "…"] mod x;` is not valid Rust.
        let item_start = item_start_before(text, at);
        out.push(ModDecl {
            name,
            span: (item_start, end),
        });
    }
    out
}

/// Back up from a keyword over the visibility and modifiers in front of it.
///
/// Stops at attributes on purpose: `#[cfg(feature = "x")] mod x;` must keep its
/// `cfg`, so the rewritten span begins after it.
fn item_start_before(text: &str, keyword_at: usize) -> usize {
    const MODIFIERS: &[&str] = &["pub", "async", "unsafe", "const", "default"];
    let bytes = text.as_bytes();
    let mut start = keyword_at;
    loop {
        let mut i = start;
        while i > 0 && bytes[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        // `pub(crate)` / `pub(in path)`: skip the parenthesised part first.
        if i > 0 && bytes[i - 1] == b')' {
            let mut depth = 0usize;
            let mut j = i;
            while j > 0 {
                j -= 1;
                match bytes[j] {
                    b')' => depth += 1,
                    b'(' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            i = j;
            while i > 0 && bytes[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
        }
        let word_end = i;
        while i > 0 && is_ident_byte(bytes[i - 1]) {
            i -= 1;
        }
        if i < word_end && MODIFIERS.contains(&&text[i..word_end]) {
            start = i;
            continue;
        }
        return start;
    }
}

/// Where a module's file actually is, given the file that declares it.
///
/// Both of Rust's layouts, checked in Rust's order: `foo.rs` beside the
/// declaring file, then `foo/mod.rs`. Returns `None` when neither exists, which
/// is a `#[cfg]`-ed-out module or a genuinely broken crate; the caller leaves
/// the declaration alone and lets `rustc` produce the real diagnostic.
pub fn module_file(declaring_file: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    // The directory a module's children live in: the file's own directory when
    // it is `lib.rs`/`main.rs`/`mod.rs`, and `<dir>/<stem>` otherwise. This is
    // Rust's rule and it is why `#[path]` on a nested module keeps working: the
    // path given is absolute, and `rustc` derives the child directory from it
    // exactly as it would have from the original location.
    let dir = declaring_file.parent()?;
    let stem = declaring_file.file_stem()?.to_str()?;
    let base = if matches!(stem, "lib" | "main" | "mod") {
        dir.to_path_buf()
    } else {
        dir.join(stem)
    };
    let flat = base.join(format!("{name}.rs"));
    if flat.is_file() {
        return Some(flat);
    }
    let nested = base.join(name).join("mod.rs");
    if nested.is_file() {
        return Some(nested);
    }
    None
}

/// The body of `fn main`, as a byte range covering the braces and their
/// contents, plus the range of the whole item so it can be removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainFn {
    /// The whole `fn main … { … }` item, including modifiers but not attributes.
    pub item: (usize, usize),
    /// Just the inside of the braces.
    pub body: (usize, usize),
}

/// Find `fn main` at the top level of `src`.
pub fn main_fn(mask: &Masked) -> Option<MainFn> {
    let text = mask.as_str();
    for at in mask.find_word("fn") {
        if mask.depth_at(at) != 0 {
            continue;
        }
        let after = text[at + 2..].trim_start();
        if !after.starts_with("main") {
            continue;
        }
        // `main` has to be the whole identifier: `fn main_menu()` is not it.
        let name_at = at + 2 + (text[at + 2..].len() - after.len());
        if text.as_bytes().get(name_at + 4).is_some_and(|b| is_ident_byte(*b)) {
            continue;
        }
        let open = text[name_at..].find('{')? + name_at;
        let close = mask.match_delim(open)?;
        return Some(MainFn {
            item: (item_start_before(text, at), close),
            body: (open + 1, close - 1),
        });
    }
    None
}

/// How the `App` in `fn main` is put together, rewritten to build into an `App`
/// the engine already owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConstruction {
    /// `fn main`'s body with `App::new()` redirected, `run()` neutered and the
    /// `DefaultPlugins` group removed.
    pub body: String,
    /// The plugin groups that were dropped, for the report. Being told that
    /// `DefaultPlugins` was skipped is the difference between "the editor
    /// ignored my window settings" and a mystery.
    pub dropped: Vec<String>,
}

/// Rewrite `fn main`'s body into something that configures a borrowed `App`.
///
/// Three substitutions, and the reason there are only three is that everything
/// else in a Bevy `main` is already exactly what we want to happen:
///
/// - **`App::new()` becomes the engine's `App`.** Every builder method takes
///   `&mut self` and returns `&mut Self`, so a chain written against an owned
///   `App` reads identically against a borrowed one, and `let mut app =
///   App::new();` followed by `app.add_systems(…)` works too: the binding is
///   simply a `&mut App` instead.
/// - **`.run()` becomes a no-op.** Not deleted: a `main` returning `AppExit`
///   ends in `.run()` as a tail expression, and deleting it leaves an expression
///   of the wrong type where a statement was expected.
/// - **`DefaultPlugins` is dropped.** The editor is already a Bevy app with a
///   window, a renderer and an asset server. Adding a second `WindowPlugin`
///   panics; adding a second `RenderPlugin` does considerably worse.
///
/// `None` when no `App::new()` is in the body at all: a `main` that calls a
/// helper returning a built `App`, most often. Substituting nothing would
/// silently configure a throwaway `App` and load a game with no systems in it,
/// so it refuses and the caller reports the escape hatch.
pub fn app_construction(src: &str, body: (usize, usize)) -> Option<AppConstruction> {
    let body_src = &src[body.0..body.1];
    let mask = Masked::new(body_src);

    // Collected as (start, end, replacement) against the body's own offsets, then
    // applied back to front so earlier edits do not move later offsets.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();

    let mut constructors = 0usize;
    for ctor in ["App::new", "App::default"] {
        for at in mask.find_word(ctor) {
            let Some(open) = mask.as_str()[at..].find('(').map(|o| at + o) else {
                continue;
            };
            let Some(end) = mask.match_delim(open) else { continue };
            constructors += 1;
            edits.push((at, end, "(&mut *__renzora_app)".to_string()));
        }
    }
    if constructors == 0 {
        return None;
    }

    for at in mask.find_word("run") {
        // `.run()`, not a free `run()` or a field named `run`.
        if !mask.as_str()[..at].trim_end().ends_with('.') {
            continue;
        }
        let Some(open) = mask.as_str()[at..].find('(').map(|o| at + o) else {
            continue;
        };
        let Some(end) = mask.match_delim(open) else { continue };
        // The whole call, `.` included, becomes the shim call.
        let dot = mask.as_str()[..at].trim_end().len() - 1;
        edits.push((dot, end, ".__renzora_no_run()".to_string()));
    }

    for at in mask.find_word("add_plugins") {
        let Some(open) = mask.as_str()[at..].find('(').map(|o| at + o) else {
            continue;
        };
        let Some(end) = mask.match_delim(open) else { continue };
        // The argument as written, with a tuple's own parentheses removed:
        // `add_plugins((A, B))` hands back `(A, B)`, whose commas are at depth
        // one, so splitting it whole finds no top-level comma and treats the
        // entire tuple as a single plugin named ``, which is how a
        // `(DefaultPlugins, MyPlugin)` tuple survived a pass that was looking
        // for exactly that.
        let arg = unwrap_tuple(&body_src[open + 1..end - 1]);
        let arg_mask = Masked::new(arg);
        let items = split_top_level(arg, &arg_mask);
        let total = items.len();
        let kept: Vec<&str> = items
            .into_iter()
            .filter(|item| {
                let group = group_head(item);
                let skip = matches!(group.as_str(), "DefaultPlugins" | "MinimalPlugins");
                if skip {
                    dropped.push(group);
                }
                !skip
            })
            .collect();
        if kept.len() == total {
            continue;
        }
        let dot = mask.as_str()[..at].trim_end();
        let dot = if dot.ends_with('.') { dot.len() - 1 } else { at };
        if kept.is_empty() {
            // Neutered, not deleted: the same trick as `.run()`, and for a
            // sharper reason.
            //
            // Deleting the call leaves whatever it was called on. In a chain
            // that is harmless (`App::new()` alone is still an expression), but
            // the other common shape is a statement:
            //
            //     let mut app = App::new();
            //     app.add_plugins(DefaultPlugins.set(..));   // deleted
            //     app.add_plugins(SolariPlugins);
            //
            // Deleting there leaves `app;`, which **moves** the `&mut App`,
            // and every later use fails with "borrow of moved value". Replacing
            // the call with a `&mut self -> &mut Self` no-op works in both
            // positions and moves nothing.
            edits.push((dot, end, ".__renzora_no_plugins()".to_string()));
        } else {
            // A mixed tuple: keep the rest, still as a tuple, because a
            // one-element tuple written `(X,)` is a tuple and `(X)` is not, and
            // `add_plugins` takes both, so the trailing comma is free insurance.
            edits.push((
                dot,
                end,
                format!(".add_plugins(({},))", kept.join(", ").trim_end_matches(',')),
            ));
        }
    }

    edits.sort_by_key(|(start, _, _)| *start);
    let mut out = body_src.to_string();
    for (start, end, replacement) in edits.into_iter().rev() {
        out.replace_range(start..end, &replacement);
    }
    dropped.sort();
    dropped.dedup();
    Some(AppConstruction { body: out, dropped })
}

/// Strip the parentheses from `(A, B)`, leaving anything else alone.
///
/// Only when the opening paren matches the *last* character, or
/// `foo(a, b).bar()` would lose the wrong pair and `split_top_level` would then
/// slice an expression in half.
fn unwrap_tuple(arg: &str) -> &str {
    let trimmed = arg.trim();
    if !trimmed.starts_with('(') {
        return arg;
    }
    let mask = Masked::new(trimmed);
    match mask.match_delim(0) {
        Some(end) if end == trimmed.len() => &trimmed[1..trimmed.len() - 1],
        _ => arg,
    }
}

/// Split a tuple's contents at top-level commas.
///
/// Returns the whole thing as one item when there are no top-level commas, which
/// is the `add_plugins(DefaultPlugins.set(…))` case and the one that matters
/// most.
fn split_top_level<'a>(src: &'a str, mask: &Masked) -> Vec<&'a str> {
    let bytes = mask.as_str().as_bytes();
    let mut depth = 0i32;
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b',' if depth == 0 => {
                out.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !src[start..].trim().is_empty() {
        out.push(&src[start..]);
    }
    out
}

/// The leading identifier of a plugin-group expression.
///
/// `DefaultPlugins.set(ImagePlugin::default_nearest())` is `DefaultPlugins`;
/// `bevy::DefaultPlugins` is too, because the question being asked is which
/// group this is and the path it was written with does not change the answer.
fn group_head(expr: &str) -> String {
    let expr = expr.trim();
    let head = expr
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
        .next()
        .unwrap_or("");
    head.rsplit("::").next().unwrap_or(head).to_string()
}

/// One `#[derive(Component)]` type, and where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDecl {
    /// The type's short name.
    pub name: String,
    /// 1-based line of the `struct` or `enum` keyword.
    pub line: u32,
    /// Does its derive list include `Reflect`?
    ///
    /// The difference between the inspector showing that an entity *holds* a
    /// `Vehicle` and showing what is *in* it. Only a reflected type can be
    /// registered, and only a registered type has fields the editor can read.
    pub reflect: bool,
}

/// Every publicly reachable `#[derive(Component)]` type in a file.
///
/// Feeds the generated label table, which is what lets the hierarchy show
/// `Player` and `Collectible` instead of `Entity 12`. Bevy keeps component type
/// names behind its `debug` feature and this engine builds without it, so the
/// names have to come from the source or from nowhere. See
/// [`renzora::core::bevy_project::ProjectComponentLabels`].
///
/// **Publicly reachable only.** The generated root names these as
/// `module::Type`, and a `struct Player;` private to its module cannot be named
/// from outside it. Skipping one costs a label; emitting one is a compile error
/// in generated code, which would take the whole project down over a cosmetic
/// feature.
pub fn components(mask: &Masked) -> Vec<ComponentDecl> {
    let text = mask.as_str();
    let mut out = Vec::new();

    for at in mask.find_word("derive") {
        // `#[derive(...)]`: the attribute's parentheses, not a `derive` written
        // anywhere else.
        let Some(open) = text[at..].find('(').map(|o| at + o) else {
            continue;
        };
        let Some(close) = mask.match_delim(open) else { continue };
        let derives: Vec<String> =
            split_top_level(&text[open + 1..close - 1], &Masked::new(&text[open + 1..close - 1]))
                .iter()
                .map(|d| d.trim().rsplit("::").next().unwrap_or("").trim().to_string())
                .collect();
        if !derives.iter().any(|d| d == "Component") {
            continue;
        }
        // Whether the type can be reached through Bevy's reflection, which is
        // what decides if the inspector can show its *fields* rather than only
        // its name. Read here rather than assumed, because the generated root
        // emits `register_type::<T>()` for these and only these: emitting it for
        // a type that does not derive `Reflect` is a compile error in generated
        // code the author cannot edit.
        let reflect = derives.iter().any(|d| d == "Reflect");
        // Walk forward to the item itself: past the derive attribute's own
        // closing `]` (which `match_delim` on the *parens* stops short of), then
        // past any further attributes: `#[require(Transform)]` sits between the
        // derive and the struct often enough to matter.
        let mut i = close;
        loop {
            let rest = text[i..].trim_start();
            i = text.len() - rest.len();
            if rest.starts_with(']') {
                i += 1;
                continue;
            }
            if rest.starts_with('#') {
                let Some(open) = text[i..].find('[').map(|o| i + o) else {
                    break;
                };
                let Some(end) = mask.match_delim(open) else { break };
                i = end;
                continue;
            }
            break;
        }
        // `pub struct X` / `pub(crate) enum X`, and nothing else: a derive on an
        // item this cannot name is one to leave alone.
        let rest = &text[i..];
        let Some(vis_end) = rest.strip_prefix("pub") else { continue };
        let vis_end = if vis_end.starts_with('(') {
            let open = i + 3;
            match mask.match_delim(open) {
                Some(end) => &text[end..],
                None => continue,
            }
        } else {
            vis_end
        };
        let decl = vis_end.trim_start();
        let Some(after_kw) = decl.strip_prefix("struct").or_else(|| decl.strip_prefix("enum"))
        else {
            continue;
        };
        let name: String = after_kw
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        // Generic components cannot be named without their parameters, and a
        // label is not worth inventing one.
        if name.is_empty() || after_kw.trim_start()[name.len()..].starts_with('<') {
            continue;
        }
        if !out.iter().any(|d: &ComponentDecl| d.name == name) {
            // 1-based, counted from the item rather than the `#[derive]` above
            // it, so the jump lands on `struct Vehicle` rather than on an
            // attribute the reader has to look past.
            let line = 1 + text[..i].bytes().filter(|b| *b == b'\n').count() as u32;
            out.push(ComponentDecl { name, line, reflect });
        }
    }
    out
}

/// Every `impl Plugin for X` in a file, as the type name.
///
/// The fallback when there is no `fn main` to read: a crate that is only a
/// library, which is a shape that exists and cannot say what its entry point is
/// any other way. Offered to the user rather than used: a library may define a
/// dozen plugins and adding all of them is not what anybody meant.
pub fn plugin_impls(mask: &Masked) -> Vec<String> {
    let text = mask.as_str();
    let mut out = Vec::new();
    for at in mask.find_word("impl") {
        let rest = &text[at + 4..];
        let Some(for_at) = rest.find(" for ") else { continue };
        let head = rest[..for_at].trim();
        // Generic parameters on the impl itself (`impl<T> Plugin for …`) are
        // skipped: a generic plugin cannot be named in a generated list without
        // knowing what to instantiate it with.
        if head.starts_with('<') {
            continue;
        }
        if head.rsplit("::").next().unwrap_or(head) != "Plugin" {
            continue;
        }
        let ty = rest[for_at + 5..]
            .trim_start()
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
            .next()
            .unwrap_or("")
            .to_string();
        if !ty.is_empty() && !out.contains(&ty) {
            out.push(ty);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_strings_do_not_hide_or_invent_code() {
        let src = r#"
// mod commented_out;
/* mod also_out; */
const S: &str = "mod in_a_string;";
mod real;
"#;
        let mask = Masked::new(src);
        let mods = modules(&mask);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "real");
    }

    /// The bug this exists to prevent: `&'static` read as an open char literal
    /// swallows the rest of the file and every depth after it is wrong.
    #[test]
    fn a_lifetime_is_not_a_char_literal() {
        let src = "fn f(x: &'static str) {}\nmod real;\n";
        let mask = Masked::new(src);
        assert_eq!(modules(&mask).len(), 1);
        // The brace of `f` opened and closed, so the module is at depth 0.
        assert_eq!(mask.depth_at(src.find("mod real").unwrap()), 0);
    }

    #[test]
    fn a_nested_mod_is_left_alone_but_a_visible_one_is_claimed_whole() {
        let src = "pub(crate) mod outer;\nmod wrapper { mod inner; }\n";
        let mask = Masked::new(src);
        let mods = modules(&mask);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "outer");
        // The span starts at `pub`, not at `mod`, or the generated attribute
        // would land between the visibility and the keyword.
        assert_eq!(&src[mods[0].span.0..mods[0].span.1], "pub(crate) mod outer;");
    }

    #[test]
    fn an_attribute_above_a_module_survives_the_rewrite() {
        let src = "#[cfg(feature = \"x\")]\nmod gated;\n";
        let mask = Masked::new(src);
        let m = &modules(&mask)[0];
        assert_eq!(&src[m.span.0..m.span.1], "mod gated;");
    }

    #[test]
    fn the_canonical_bevy_main_is_rewritten() {
        let src = r#"
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins((CorePlugin, HudPlugin))
        .run();
}
"#;
        let mask = Masked::new(src);
        let main = main_fn(&mask).expect("fn main");
        let app = app_construction(src, main.body).expect("an App");
        assert!(app.body.contains("(&mut *__renzora_app)"));
        assert!(!app.body.contains("DefaultPlugins"));
        assert!(app.body.contains("add_plugins((CorePlugin, HudPlugin))"));
        assert!(app.body.contains("__renzora_no_run()"));
        assert_eq!(app.dropped, vec!["DefaultPlugins".to_string()]);
    }

    /// The other common shape: a binding rather than one chain. It works for
    /// free, because `app` simply becomes a `&mut App`.
    #[test]
    fn a_let_bound_app_is_rewritten_too() {
        let src = "fn main() {\n    let mut app = App::new();\n    app.add_plugins(MinimalPlugins);\n    app.insert_resource(Score(0));\n    app.run();\n}\n";
        let mask = Masked::new(src);
        let main = main_fn(&mask).expect("fn main");
        let app = app_construction(src, main.body).expect("an App");
        assert!(app.body.contains("let mut app = (&mut *__renzora_app);"));
        assert!(!app.body.contains("MinimalPlugins"));
        assert!(app.body.contains("insert_resource(Score(0))"));
    }

    /// The shape that broke a real project: `add_plugins` as a whole statement
    /// on a `let`-bound app, with more statements after it. Deleting the call
    /// left `app;`, which moves the binding.
    #[test]
    fn dropping_a_statement_add_plugins_does_not_move_the_binding() {
        let src = "fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin::default()));
    app.add_plugins(SolariPlugins);
    app.run();
}
";
        let mask = Masked::new(src);
        let main = main_fn(&mask).expect("fn main");
        let app = app_construction(src, main.body).expect("an App");
        assert!(!app.body.contains("DefaultPlugins"));
        // The statement must still have a receiver-consuming call on it, or
        // `app;` moves the `&mut App` out from under everything after it.
        assert!(app.body.contains("app.__renzora_no_plugins();"));
        assert!(app.body.contains("app.add_plugins(SolariPlugins);"));
    }

    /// A mixed tuple keeps everything that was not the group being dropped.
    #[test]
    fn a_tuple_holding_defaultplugins_keeps_its_other_members() {
        let src = "fn main() { App::new().add_plugins((DefaultPlugins, MyPlugin, Other)).run(); }";
        let mask = Masked::new(src);
        let main = main_fn(&mask).expect("fn main");
        let app = app_construction(src, main.body).expect("an App");
        assert!(app.body.contains("MyPlugin"));
        assert!(app.body.contains("Other"));
        assert!(!app.body.contains("DefaultPlugins"));
    }

    /// The case that must fail loudly: the `App` is built somewhere else, so
    /// rewriting this body would configure a throwaway `App` and load a game
    /// with no systems in it.
    #[test]
    fn a_main_that_delegates_is_refused_rather_than_half_rewritten() {
        let src = "fn main() { build_app().run(); }";
        let mask = Masked::new(src);
        let main = main_fn(&mask).expect("fn main");
        assert!(app_construction(src, main.body).is_none());
    }

    #[test]
    fn fn_main_menu_is_not_fn_main() {
        let src = "fn main_menu() {}\nfn main() { App::new().run(); }\n";
        let mask = Masked::new(src);
        let main = main_fn(&mask).expect("fn main");
        assert!(src[main.item.0..main.item.1].starts_with("fn main()"));
    }

    #[test]
    fn public_components_are_collected_and_unnameable_ones_are_not() {
        let src = r#"
#[derive(Component)]
pub struct Player;

#[derive(Debug, Clone, Component, Default)]
pub struct Collectible { pub phase: f32 }

#[derive(Component)]
#[require(Transform)]
pub enum Mode { A, B }

#[derive(Component)]
pub(crate) struct Internal;

// Private to its module, so `module::Hidden` does not resolve from the root.
#[derive(Component)]
struct Hidden;

// Generic: cannot be named without its parameter.
#[derive(Component)]
pub struct Holder<T>(pub T);

// Not a component at all.
#[derive(Resource)]
pub struct Score(pub u32);
"#;
        let mask = Masked::new(src);
        let found: Vec<String> = components(&mask).into_iter().map(|d| d.name).collect();
        assert_eq!(found, vec!["Player", "Collectible", "Mode", "Internal"]);

        // Lines point at the declaration, not at the `#[derive]` above it.
        let decls = components(&mask);
        let player = decls.iter().find(|d| d.name == "Player").expect("Player");
        assert_eq!(src.lines().nth(player.line as usize - 1).unwrap().trim(), "pub struct Player;");
    }

    #[test]
    fn plugin_impls_are_found_and_generic_ones_are_skipped() {
        let src = "impl Plugin for MyPlugin { }\nimpl<T> Plugin for Generic<T> { }\nimpl bevy::app::Plugin for Second {}\n";
        let mask = Masked::new(src);
        assert_eq!(plugin_impls(&mask), vec!["MyPlugin", "Second"]);
    }
}
