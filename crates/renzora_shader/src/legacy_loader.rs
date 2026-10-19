//! Keeps `.wgsl` files written for Bevy's old preprocessor loading.
//!
//! Bevy 0.20 hands a `.wgsl` file to the GPU exactly as written, so one that
//! still says `#import bevy_pbr::forward_io::VertexOutput` or `#ifdef
//! VERTEX_UVS_A` fails on its first directive. Plenty of those exist outside
//! this repository: theme folders, project shaders, the post-process effects
//! in a project's `assets/shaders`, and the shaders inside marketplace
//! plugins, all written for the naga_oil dialect the engine used until now.
//!
//! This loader replaces Bevy's for the `wgsl` extension. A file in the old
//! dialect is translated to WESL (`renzora::shader_source`) and loaded as WESL;
//! anything else is loaded exactly as Bevy's own loader would. Registering a
//! second loader for an extension makes Bevy log one "Duplicate AssetLoader"
//! warning at startup and then use the newest, which is this one.

use bevy::asset::{io::Reader, AssetLoader, AssetPath, LoadContext};
use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport, ShaderSettings};

#[derive(Default, TypePath)]
pub struct LegacyWgslLoader;

#[derive(Debug, thiserror::Error)]
pub enum LegacyWgslLoaderError {
    #[error("Could not load shader: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse shader: {0}")]
    Parse(#[from] std::string::FromUtf8Error),
}

impl AssetLoader for LegacyWgslLoader {
    type Asset = Shader;
    // Bevy's own settings type, so a `.meta` file written for Bevy's loader
    // still deserializes for this one.
    type Settings = ShaderSettings;
    type Error = LegacyWgslLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Shader, Self::Error> {
        // Same normalisation as Bevy's loader: the path uses `\` or `/`
        // inconsistently on Windows.
        let path = load_context.path().to_string().replace(std::path::MAIN_SEPARATOR, "/");
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let source = String::from_utf8(bytes)?;

        use renzora::shader_source::{dialect, Dialect};
        // A `.wgsl` holding WESL (an `import a::b;`, as a material shader
        // written since 0.20 does) goes to the WESL compiler like a `.wesl`
        // would; only one with neither imports nor directives is plain WGSL.
        let wesl_source = match dialect(&source, &path) {
            Dialect::Wgsl => return Ok(Shader::from_wgsl(source, path)),
            Dialect::Wesl => source,
            Dialect::Legacy => {
                let translation = renzora::shader_source::to_wesl(&source);
                renzora::shader_source::report(&path, &translation.diagnostics);
                translation.source
            }
        };
        let mut shader = Shader::from_wesl(wesl_source, path);
        // Numeric defs from a `.meta` were readable as `#{NAME}`; in WESL they
        // are `constants::NAME`, which the translation already wrote.
        shader.shader_defs = settings.shader_defs.clone();

        // What Bevy's loader does for a `.wesl`: load the files its relative
        // imports could name, so the compiler can resolve them.
        let candidates: Vec<String> = shader
            .imports
            .iter()
            .filter_map(|import| match import {
                ShaderImport::AssetPath(p) => Some(format!("{}.wesl", p.trim_start_matches('/'))),
                ShaderImport::Custom(_) => None,
            })
            .collect();
        for file in candidates {
            if load_context.read_asset_bytes(AssetPath::from(file.clone())).await.is_ok() {
                shader.file_dependencies.push(load_context.load(AssetPath::from(file)));
            }
        }
        Ok(shader)
    }

    fn extensions(&self) -> &[&str] {
        &["wgsl"]
    }
}
