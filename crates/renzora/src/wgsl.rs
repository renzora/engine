//! The one place WGSL meets naga in this workspace.
//!
//! Self-contained sources only — a shader using `#import` is naga_oil
//! preprocessing, not WGSL, and belongs to a composer (the material
//! validator in `renzora_shader` keeps one).

use naga::front::wgsl::ParseError;
use naga::valid::{Capabilities, ModuleInfo, ValidationError, ValidationFlags, Validator};
use naga::{Module, WithSpan};

/// Parse self-contained WGSL into a naga module.
pub fn parse(source: &str) -> Result<Module, ParseError> {
    naga::front::wgsl::parse_str(source)
}

/// Validate a module the way wgpu will. The flags and capabilities live here
/// so no call site decides them alone — four of them once did, identically.
///
/// The error comes boxed: naga's `WithSpan` carries the whole codespan, and an
/// Err that fat costs every caller a `result_large_err` warning.
pub fn validate(module: &Module) -> Result<ModuleInfo, Box<WithSpan<ValidationError>>> {
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(module)
        .map_err(Box::new)
}

/// [`parse`] + [`validate`], with the compiler's own rendering against
/// `source`. The common case for a test that only cares pass/fail.
pub fn check(source: &str) -> Result<Module, String> {
    let module = parse(source).map_err(|err| err.emit_to_string(source))?;
    validate(&module).map_err(|err| err.emit_to_string(source))?;
    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_module_checks() {
        check("@vertex fn vertex() -> @builtin(position) vec4<f32> { return vec4<f32>(0.0); }")
            .expect("a minimal vertex shader must pass");
    }

    #[test]
    fn an_invalid_module_names_the_problem() {
        let err = check("fn f() -> f32 { return no_such_symbol; }")
            .expect_err("an undeclared symbol must fail");
        assert!(err.contains("no_such_symbol"), "got: {err}");
    }
}
