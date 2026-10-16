use std::hash::BuildHasher;

use bevy::{
    asset::{Assets, Handle},
    ecs::{change_detection::ResMut, resource::Resource},
    log::{debug, error, trace, warn},
    platform::collections::HashMap,
    shader::Shader,
};

/// The WESL module every other hanabi shader imports its shared types and
/// helpers from. The templates still say `#import bevy_hanabi::vfx_common::X`,
/// and the translator keeps that name, so the common shader must be
/// registered under exactly this path.
pub(crate) const VFX_COMMON_MODULE: &str = "bevy_hanabi::vfx_common";

/// Build a Bevy [`Shader`] from one of hanabi's shader templates, after its
/// `{{...}}` placeholders have been filled in.
///
/// The templates are written for naga_oil (`#import`, `#ifdef`,
/// `#define_import_path`), which Bevy 0.19 ran over every WGSL shader. Bevy
/// 0.20 hands a WGSL shader to wgpu untouched and preprocesses only WESL, so
/// the filled source is translated to WESL here, at runtime: translating the
/// files once by hand would break the placeholder substitution, which happens
/// on the text before any of this.
///
/// `module` is the WESL module path the shader is registered under, such as
/// `bevy_hanabi::vfx_indirect`. It becomes an `embedded://` path, which is
/// what makes Bevy register the shader as an importable module of that name.
/// Only [`VFX_COMMON_MODULE`] is actually imported by anything, but every
/// hanabi shader goes through the same naming so its path stays a valid
/// module path (a file path with a drive letter would not be).
pub(crate) fn translated_shader(module: &str, source: &str) -> Shader {
    let options = renzora_legacy_shaders::Options {
        known_modules: vec![VFX_COMMON_MODULE.into()],
        ..Default::default()
    };
    let translation = renzora_legacy_shaders::translate(source, &options);
    for d in &translation.diagnostics {
        match d.severity {
            renzora_legacy_shaders::Severity::Error => {
                error!("hanabi shader `{module}` line {}: {}", d.line, d.message)
            }
            renzora_legacy_shaders::Severity::Warning => {
                warn!("hanabi shader `{module}` line {}: {}", d.line, d.message)
            }
            renzora_legacy_shaders::Severity::Info => {
                trace!("hanabi shader `{module}` line {}: {}", d.line, d.message)
            }
        }
    }
    let path = format!("embedded://{}.wesl", module.replace("::", "/"));
    Shader::from_wesl(translation.source, path)
}

/// A WESL module path component from an arbitrary effect name: only ASCII
/// letters, digits and underscores, and never starting with a digit or a
/// double underscore (both of which WGSL rejects in an identifier).
fn module_component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("e_{cleaned}")
}

/// Cache of baked shaders variants.
///
/// Baked shader variants are shaders where the placeholders `{{PLACEHOLDER}}`
/// present in the WGSL template code have been replaced by actual WGSL code,
/// then translated to WESL (see [`translated_shader`]), making them a valid
/// shader from the point of view of the Bevy renderer.
#[derive(Default, Resource)]
pub struct ShaderCache {
    /// Map of allocated shader resources from their baked shader code.
    cache: HashMap<String, Handle<Shader>>,
}

impl ShaderCache {
    /// Get an existing baked shader variant, or insert it into the cache and
    /// allocate a new [`Shader`] resource for it.
    ///
    /// Returns the [`Shader`] resource associated with `source`.
    pub fn get_or_insert(
        &mut self,
        filename: &str,
        suffix: &str,
        source: &str,
        shaders: &mut ResMut<Assets<Shader>>,
    ) -> Handle<Shader> {
        if let Some(handle) = self.cache.get(source) {
            handle.clone()
        } else {
            let hash = bevy::platform::hash::FixedHasher.hash_one(source);
            // The hash keeps two variants of one effect apart: Bevy replaces
            // a module registered under a name that is already taken.
            let module = format!(
                "bevy_hanabi::generated::{}_{}_{}",
                module_component(filename),
                suffix,
                hash
            );
            let shader = translated_shader(&module, source);
            trace!(
                "Shader path={} import_path={:?} imports={:?}",
                shader.path,
                shader.import_path,
                shader.imports
            );
            let shader_path = shader.path.clone();
            let handle = shaders.add(shader);
            debug!(
                "Inserted new configured shader {}: {:?}\n{}",
                shader_path, handle, source
            );
            self.cache.insert(source.to_string(), handle.clone());
            handle
        }
    }
}
