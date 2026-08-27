//! Grayscale post-process, converted from the C-ABI `plugins/grayscale`.
//!
//! Side by side, the two versions are a fair summary of what the native plugin
//! mechanism buys and what it costs.
//!
//! # What the C-ABI version does
//!
//! It links no Bevy at all. The effect is *declared* through a `#[repr(C)]`
//! struct and a descriptor handed across a function table:
//!
//! ```ignore
//! #[derive(Component)]
//! #[component(name = "Grayscale")]
//! #[repr(C)]
//! pub struct Grayscale { #[field(min = 0.0, max = 1.0)] pub intensity: f32, … }
//!
//! app.add_post_process::<Grayscale>("grayscale", WGSL, RenderPhase::LdrPost, 0.0);
//! ```
//!
//! The host copies those bytes straight into a uniform buffer, which is why that
//! crate carries a test asserting the struct matches the shader byte for byte —
//! a mismatch is not an error there, it is a wrong picture.
//!
//! # What this version does
//!
//! The same effect, but through Renzora's real [`PostProcessEffect`] trait with
//! Bevy's real types. `ShaderType` derives the uniform layout instead of it being
//! asserted by a test, the shader is an ordinary embedded asset instead of a
//! `&str` shipped across a boundary, and `Reflect` makes the component visible to
//! scripting and scene serialisation for free.
//!
//! # Which to use
//!
//! This does **not** replace the C-ABI one. That version is ~52 KB, loads into
//! any engine build however it was compiled, and can be linked *into* a lean
//! export — a shipped game can carry it. None of that is true here: a native
//! plugin links the shared images, so it runs in the editor and in a
//! dynamically-linked runtime, and nowhere else.
//!
//! For a post-process effect specifically, the C-ABI version is usually the
//! better choice — effects are exactly the kind of small, self-contained,
//! ship-with-the-game thing that mechanism was built for. This exists to show
//! that the native path reaches the same framework, and for effects that need
//! more than a uniform buffer and a fragment shader.

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

// Both are re-exported at the contract crate's root. Note that rustc's
// diagnostics call it `renzora_dylib` — the shared image's real crate name,
// which the `--extern renzora=` alias hides everywhere except error messages.
use renzora::postprocess::PostProcessEffect;
use renzora::AppEditorExt;

/// Where the embedded shader lands in the asset server's namespace.
///
/// `embedded_asset!` keys on the crate name and the path relative to `src/`, so
/// this string is not free-form — it has to match the file the macro embedded.
const SHADER: &str = "embedded://native_grayscale/grayscale.wgsl";

/// The effect's settings, and the uniform the shader reads.
///
/// `ShaderType` derives the GPU layout from the Rust declaration, so the struct
/// and the shader cannot disagree about offsets. That is the one real
/// improvement over the C-ABI version, which needs a test to assert the same
/// property because its bytes are memcpy'd across a boundary that knows nothing
/// about either side's types.
#[derive(Component, Clone, Copy, ShaderType, ExtractComponent, Reflect, Debug)]
// `Inspectable` is what satisfies `add_post_process`'s `InspectableComponent`
// bound — it generates the panel rows from the `#[field]` attributes below. The
// derive is re-exported from the contract crate, so a plugin reaches it without
// depending on the macro crate directly.
#[derive(renzora::Inspectable)]
// `Default` in the reflect list, not just the derive list: a scene saved before
// a field existed still loads, because `FromReflect` returns `None` for partial
// data and falls back to `ReflectDefault` for what is missing.
#[reflect(Component, Default)]
pub struct NativeGrayscale {
    /// How far towards grey, 0..1.
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub intensity: f32,
    /// Luma weights. Rec. 709, and not usually worth touching — they are fields
    /// rather than constants only because the shader reads them from the uniform,
    /// so they are hidden from the inspector like the C-ABI version hides them.
    #[field(skip)]
    pub luminance_r: f32,
    #[field(skip)]
    pub luminance_g: f32,
    #[field(skip)]
    pub luminance_b: f32,
}

impl Default for NativeGrayscale {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            luminance_r: 0.2126,
            luminance_g: 0.7152,
            luminance_b: 0.0722,
        }
    }
}

impl PostProcessEffect for NativeGrayscale {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct NativeGrayscalePlugin;

impl Plugin for NativeGrayscalePlugin {
    fn build(&self, app: &mut App) {
        // Embeds the WGSL into the binary and registers it with the asset server
        // under `embedded://<crate>/<path>`. A plugin cannot rely on a file next
        // to itself: the asset server resolves relative to the PROJECT's assets
        // directory, which has nothing to do with where a plugin was installed.
        embedded_asset!(app, "grayscale.wgsl");

        // Registers the type for reflection, installs the render pipeline, and
        // adds the inspector entry — all three, because an effect nobody can see
        // in the inspector is an effect nobody can turn on.
        app.add_post_process::<NativeGrayscale>();

        info!("[native-grayscale] registered as a native post-process effect");
    }
}

renzora::plugin!(NativeGrayscalePlugin);
