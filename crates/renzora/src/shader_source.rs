//! Turning shader text into a Bevy [`Shader`], whichever dialect it is in.
//!
//! Bevy 0.20 compiles shaders with WESL and dropped naga_oil, the preprocessor
//! that understood `#import`, `#ifdef` and `#{MATERIAL_BIND_GROUP}`. A shader
//! written that way is still the norm in plenty of places this engine loads
//! from: `.material` files saved by earlier versions embed one, theme shaders
//! and project `.wgsl` files were written that way, and so were the shaders in
//! marketplace plugins. [`shader`] keeps all of them working by translating
//! the old dialect to WESL on the way in (`renzora_legacy_shaders`), so no
//! caller has to know which dialect it was handed.
//!
//! It lives in the contract crate so a native plugin building a shader at
//! runtime gets the same treatment as the engine.

use bevy::log::{error, warn};
use bevy::shader::Shader;

pub use renzora_legacy_shaders::{Diagnostic, Severity, Translation};

/// What a source turned out to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    /// Plain WGSL: no imports or conditionals, handed to the GPU as it is.
    Wgsl,
    /// WESL, compiled by Bevy's WESL compiler.
    Wesl,
    /// The naga_oil dialect Bevy used before 0.20, translated to WESL here.
    Legacy,
}

/// Which dialect `source` is written in. `path` breaks the tie for a source
/// with no directives at all: plain WGSL is valid WESL, so only the file name
/// says whether its author meant one.
pub fn dialect(source: &str, path: &str) -> Dialect {
    if renzora_legacy_shaders::is_legacy(source) {
        Dialect::Legacy
    } else if path.ends_with(".wesl") || has_wesl_import(source) {
        Dialect::Wesl
    } else {
        Dialect::Wgsl
    }
}

/// An `import a::b;` statement, the one WESL construct that plain WGSL lacks
/// and that makes the source unusable without the WESL compiler.
fn has_wesl_import(source: &str) -> bool {
    source.lines().any(|line| {
        let t = line.trim_start();
        t.starts_with("import ") && t.split("//").next().is_some_and(|code| code.contains("::"))
    })
}

/// The source as WESL (translated when it is legacy), plus what the
/// translator had to say. Plain WGSL and WESL come back unchanged with no
/// diagnostics.
pub fn to_wesl(source: &str) -> Translation {
    if renzora_legacy_shaders::is_legacy(source) {
        renzora_legacy_shaders::translate(source, &renzora_legacy_shaders::Options::default())
    } else {
        Translation {
            source: source.to_string(),
            module_name: None,
            diagnostics: Vec::new(),
        }
    }
}

/// Build a [`Shader`] from source in any dialect. `path` names it in logs and,
/// for WESL, is its module path, which is what a relative `import super::x;`
/// resolves against.
///
/// A legacy source is translated and its warnings logged once here, so a
/// shader that needs a hand fix says so instead of silently rendering wrong.
pub fn shader(source: impl Into<String>, path: impl Into<String>) -> Shader {
    let source = source.into();
    let path = path.into();
    match dialect(&source, &path) {
        Dialect::Wgsl => Shader::from_wgsl(source, path),
        Dialect::Wesl => Shader::from_wesl(source, wesl_path(&path)),
        Dialect::Legacy => {
            let t = to_wesl(&source);
            report(&path, &t.diagnostics);
            Shader::from_wesl(t.source, wesl_path(&path))
        }
    }
}

/// The project files a WESL shader's relative imports could live in, for a
/// shader built in memory rather than loaded by the asset server.
///
/// Bevy's shader loader loads the files a `.wesl` imports so the compiler can
/// find them; a shader made with [`shader`] skips the loader, so its caller
/// does the same: load each of these that exists and push the handle into
/// `Shader::file_dependencies`, which also keeps it alive. Each import yields
/// two candidates, since `import super::noise::fbm;` may name the item `fbm`
/// in `noise.wesl` or the module `noise/fbm.wesl`.
pub fn wesl_import_files(shader: &Shader) -> Vec<String> {
    if !matches!(shader.source, bevy::shader::Source::Wesl(_)) {
        return Vec::new();
    }
    shader
        .imports
        .iter()
        .filter_map(|import| match import {
            bevy::shader::ShaderImport::AssetPath(path) => {
                Some(format!("{}.wesl", path.trim_start_matches('/')))
            }
            bevy::shader::ShaderImport::Custom(_) => None,
        })
        .collect()
}

/// Log what the translator could not do cleanly. Info-level notes (a statement
/// written out per flag, say) are expected and stay quiet.
pub fn report(path: &str, diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        match d.severity {
            Severity::Info => {}
            Severity::Warning => warn!("shader `{path}` line {}: {}", d.line, d.message),
            Severity::Error => error!("shader `{path}` line {}: {}", d.line, d.message),
        }
    }
}

/// Bevy takes a WESL shader's module path from its asset path, minus the
/// extension, so a translated `foo.wgsl` would collide with nothing but read
/// oddly in errors. Naming it `.wesl` keeps the two dialects apart in logs.
fn wesl_path(path: &str) -> String {
    match path.strip_suffix(".wgsl") {
        Some(stem) => format!("{stem}.wesl"),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_wgsl_stays_wgsl() {
        assert_eq!(dialect("@fragment fn f() {}", "a.wgsl"), Dialect::Wgsl);
    }

    #[test]
    fn a_wesl_file_or_import_is_wesl() {
        assert_eq!(dialect("@fragment fn f() {}", "a.wesl"), Dialect::Wesl);
        assert_eq!(
            dialect("import bevy_pbr::render::forward_io::VertexOutput;\n", "a.wgsl"),
            Dialect::Wesl
        );
    }

    #[test]
    fn naga_oil_directives_are_legacy_and_translate() {
        let src = "#import bevy_pbr::forward_io::VertexOutput\n\
                   @fragment fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n\
                   #ifdef VERTEX_COLORS\n    return in.color;\n#else\n    return vec4<f32>(1.0);\n#endif\n}\n";
        assert_eq!(dialect(src, "a.wgsl"), Dialect::Legacy);
        let t = to_wesl(src);
        assert!(t.source.contains("import bevy_pbr::render::forward_io::VertexOutput;"), "{}", t.source);
        assert!(t.source.contains("@if(VERTEX_COLORS)"), "{}", t.source);
    }
}
