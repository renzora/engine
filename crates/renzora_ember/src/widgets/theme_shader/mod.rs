//! Themeable UI shaders — animated fragment effects (and now images) painted
//! *behind* editor chrome and panels as `UiMaterial`s, where **each shader's
//! source and image come from the active theme's own folder**, and **each surface
//! can run a different shader/image**.
//!
//! A theme is a folder (`themes/<Name>/`) holding its `theme.toml`, its shaders
//! (`shaders/*.wgsl`), its images and its fonts. `[effects]` maps surfaces to
//! shader files; `[images]` maps surfaces to image files. The shell reads each
//! file and hands the WGSL source ([`set_surface_shader`]) + the loaded image
//! ([`set_surface_image`]) per surface. A surface with an image but no shader uses
//! the built-in image shader (just displays the picture); a surface with a custom
//! shader can sample the image at `@group(1) @binding(1)`.
//!
//! ## How an arbitrary on-disk shader reaches the GPU
//!
//! Bevy resolves a `UiMaterial`'s shader **once**, per material *type*, into a
//! `Handle<Shader>` at pipeline init — it cannot vary per instance/path at
//! runtime. So each surface has its OWN material type (`surface_mat!`) with its
//! own fixed shader handle; the theme's wgsl source is overwritten into that
//! handle's `Shader` asset and Bevy respecializes. Independent handles ⇒ surfaces
//! can run completely different shaders at once.
//!
//! Uniform/texture contract a theme shader honours: `@group(1) @binding(0)` =
//! `params` (x = time) + bg/accent/fg colors; `@binding(1)` = the theme image
//! (`texture_2d<f32>`); `@binding(2)` = its sampler. Fixed layout, so a downloaded
//! theme can't declare arbitrary bindings (kept sandboxable).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, RwLock};

use bevy::asset::{uuid_handle, AssetId, RenderAssetUsages};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::{Shader, ShaderRef};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::theme::{accent, text_primary, window_bg};

/// Built-in animated fallback (matrix rain), compiled in.
const BUILTIN_WGSL: &str = include_str!("matrix_rain.wgsl");
/// Built-in image-display shader (cover-fits the theme image), for surfaces that
/// set an image but no shader of their own.
const IMAGE_WGSL: &str = include_str!("theme_image.wgsl");

/// Change-keys for the two built-in sources (custom shaders hash to >= 2).
const BUILTIN_KEY: u64 = 0;
const IMAGE_KEY: u64 = 1;

/// A chrome/panel surface that can host a themeable shader/image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeSurface {
    TopBar,
    DocTabs,
    StatusBar,
    Panel,
    PanelHeader,
}

impl ThemeSurface {
    pub const ALL: [ThemeSurface; 5] = [
        Self::TopBar,
        Self::DocTabs,
        Self::StatusBar,
        Self::Panel,
        Self::PanelHeader,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "top_bar" | "topbar" | "top-bar" => Some(Self::TopBar),
            "doc_tabs" | "doctabs" | "tabs" | "document_tabs" => Some(Self::DocTabs),
            "status_bar" | "statusbar" | "status" => Some(Self::StatusBar),
            "panel" | "panels" => Some(Self::Panel),
            "panel_header" | "panelheader" | "header" | "tab_bar" | "tabbar" => {
                Some(Self::PanelHeader)
            }
            _ => None,
        }
    }
}

/// Marks a chrome/panel node as a themeable shader surface.
#[derive(Component, Clone, Copy)]
pub struct ThemeShaderSurface {
    pub surface: ThemeSurface,
}

/// Which WGSL a surface runs.
#[derive(Clone)]
pub enum SurfaceSource {
    /// Built-in animated fallback (matrix rain).
    Builtin,
    /// Built-in image display (just shows the surface's theme image).
    Image,
    /// A theme's own shader source.
    Custom(String),
}

// ── Per-surface registries (process-wide, runtime-safe) ───────────────────────

struct SurfaceState {
    key: u64,
    source: SurfaceSource,
}

static SURFACES: LazyLock<RwLock<HashMap<ThemeSurface, SurfaceState>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static SURFACE_IMAGES: LazyLock<RwLock<HashMap<ThemeSurface, Handle<Image>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Set a surface's shader. `Some((key, source))` enables it; `None` turns it off.
pub fn set_surface_shader(surface: ThemeSurface, req: Option<(u64, SurfaceSource)>) {
    if let Ok(mut g) = SURFACES.write() {
        match req {
            Some((key, source)) => {
                g.insert(surface, SurfaceState { key, source });
            }
            None => {
                g.remove(&surface);
            }
        }
    }
}

/// Bind (or clear) a surface's theme image. Cleared ⇒ the default white texture.
pub fn set_surface_image(surface: ThemeSurface, image: Option<Handle<Image>>) {
    if let Ok(mut g) = SURFACE_IMAGES.write() {
        match image {
            Some(h) => {
                g.insert(surface, h);
            }
            None => {
                g.remove(&surface);
            }
        }
    }
}

fn surface_state(surface: ThemeSurface) -> Option<(u64, SurfaceSource)> {
    SURFACES.read().ok().and_then(|g| g.get(&surface).map(|s| (s.key, s.source.clone())))
}

fn surface_image(surface: ThemeSurface) -> Option<Handle<Image>> {
    SURFACE_IMAGES.read().ok().and_then(|g| g.get(&surface).cloned())
}

/// Hash a shader source into the change-key `set_surface_shader` expects. Never
/// returns a built-in key (0/1) so custom source can't masquerade as a built-in.
pub fn shader_key(source: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    let k = h.finish();
    if k <= IMAGE_KEY {
        IMAGE_KEY + 1
    } else {
        k
    }
}

fn rgb_f32((r, g, b): (u8, u8, u8)) -> Vec4 {
    Vec4::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

/// The 1×1 white texture every surface material binds until the theme provides a
/// real image (a texture binding can't be empty).
#[derive(Resource)]
struct ThemeDefaultImage(Handle<Image>);

/// Common behaviour every per-surface material shares.
trait SurfaceMat: UiMaterial {
    const SURFACE: ThemeSurface;
    fn shader_handle() -> Handle<Shader>;
    fn make(time: f32, image: Handle<Image>) -> Self;
    fn refresh(&mut self, time: f32);
    fn image_id(&self) -> AssetId<Image>;
    fn set_image(&mut self, image: Handle<Image>);
}

/// Define a per-surface material: fixed uniform layout + a theme image binding,
/// each with its own shader handle.
macro_rules! surface_mat {
    ($name:ident, $handle:ident, $surface:expr, $uuid:literal) => {
        const $handle: Handle<Shader> = uuid_handle!($uuid);

        #[derive(Asset, TypePath, AsBindGroup, Clone)]
        struct $name {
            #[uniform(0)]
            params: Vec4,
            #[uniform(0)]
            bg: Vec4,
            #[uniform(0)]
            accent: Vec4,
            #[uniform(0)]
            fg: Vec4,
            #[texture(1)]
            #[sampler(2)]
            image: Handle<Image>,
        }

        impl UiMaterial for $name {
            fn fragment_shader() -> ShaderRef {
                ShaderRef::Handle($handle)
            }
        }

        impl SurfaceMat for $name {
            const SURFACE: ThemeSurface = $surface;
            fn shader_handle() -> Handle<Shader> {
                $handle
            }
            fn make(time: f32, image: Handle<Image>) -> Self {
                let mut m = Self {
                    params: Vec4::ZERO,
                    bg: Vec4::ONE,
                    accent: Vec4::ONE,
                    fg: Vec4::ONE,
                    image,
                };
                m.refresh(time);
                m
            }
            fn refresh(&mut self, time: f32) {
                self.params = Vec4::new(time, 0.0, 0.0, 0.0);
                self.bg = rgb_f32(window_bg());
                self.accent = rgb_f32(accent());
                self.fg = rgb_f32(text_primary());
            }
            fn image_id(&self) -> AssetId<Image> {
                self.image.id()
            }
            fn set_image(&mut self, image: Handle<Image>) {
                self.image = image;
            }
        }
    };
}

surface_mat!(TopBarMat, TOP_BAR_SHADER, ThemeSurface::TopBar, "b7e6a3c2-9d41-4f8a-8c10-6e2f5a9b41d0");
surface_mat!(DocTabsMat, DOC_TABS_SHADER, ThemeSurface::DocTabs, "c1a2b3d4-1122-4a3b-8c4d-5e6f7a8b9c01");
surface_mat!(StatusBarMat, STATUS_BAR_SHADER, ThemeSurface::StatusBar, "d2b3c4e5-2233-4b4c-9d5e-6f7a8b9c0d12");
surface_mat!(PanelMat, PANEL_SHADER, ThemeSurface::Panel, "e3c4d5f6-3344-4c5d-ae6f-7a8b9c0d1e23");
surface_mat!(PanelHeaderMat, PANEL_HEADER_SHADER, ThemeSurface::PanelHeader, "f4d5e6a7-4455-4d6e-bf70-8b9c0d1e2f34");

pub(crate) struct ThemeShaderPlugin;

impl Plugin for ThemeShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiMaterialPlugin::<TopBarMat>::default(),
            UiMaterialPlugin::<DocTabsMat>::default(),
            UiMaterialPlugin::<StatusBarMat>::default(),
            UiMaterialPlugin::<PanelMat>::default(),
            UiMaterialPlugin::<PanelHeaderMat>::default(),
        ));
        app.add_systems(Startup, seed_default_image);
        app.add_systems(
            Update,
            (
                drive_surface::<TopBarMat>,
                drive_surface::<DocTabsMat>,
                drive_surface::<StatusBarMat>,
                drive_surface::<PanelMat>,
                drive_surface::<PanelHeaderMat>,
            ),
        );
    }
}

/// Create the 1×1 white default texture all surface materials start with.
fn seed_default_image(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let img = Image::new_fill(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    commands.insert_resource(ThemeDefaultImage(images.add(img)));
}

/// Keep one surface's shader source, image and material in sync with the active
/// theme, then attach / detach its material on the matching surface entities.
fn drive_surface<M: SurfaceMat>(
    mut commands: Commands,
    time: Res<Time>,
    mut shaders: ResMut<Assets<Shader>>,
    mut mats: ResMut<Assets<M>>,
    default_img: Option<Res<ThemeDefaultImage>>,
    mut shared: Local<Option<Handle<M>>>,
    mut applied: Local<u64>,
    unattached: Query<(Entity, &ThemeShaderSurface), Without<MaterialNode<M>>>,
    attached: Query<(Entity, &ThemeShaderSurface), With<MaterialNode<M>>>,
) {
    let Some(default_img) = default_img else {
        return; // default texture not seeded yet
    };

    // Lazily seed the built-in shader + the one shared material on first run.
    let handle = match shared.as_ref() {
        Some(h) => h.clone(),
        None => {
            let _ = shaders.insert(
                &M::shader_handle(),
                Shader::from_wgsl(BUILTIN_WGSL, "renzora_ember/theme_surface.wgsl"),
            );
            let h = mats.add(M::make(time.elapsed_secs(), default_img.0.clone()));
            *shared = Some(h.clone());
            h
        }
    };

    let state = surface_state(M::SURFACE);

    // Swap this surface's shader source if it changed.
    let want_key = state.as_ref().map(|s| s.0).unwrap_or(BUILTIN_KEY);
    if *applied != want_key {
        let source = match state.as_ref().map(|s| &s.1) {
            Some(SurfaceSource::Custom(s)) => s.clone(),
            Some(SurfaceSource::Image) => IMAGE_WGSL.to_string(),
            _ => BUILTIN_WGSL.to_string(),
        };
        let _ = shaders.insert(
            &M::shader_handle(),
            Shader::from_wgsl(source, "renzora_ember/theme_surface.wgsl"),
        );
        *applied = want_key;
    }

    // Animate, recolor, and bind the surface's image (or the default) once/frame.
    let want_image = surface_image(M::SURFACE).unwrap_or_else(|| default_img.0.clone());
    if let Some(mut m) = mats.get_mut(&handle) {
        m.refresh(time.elapsed_secs());
        if m.image_id() != want_image.id() {
            m.set_image(want_image);
        }
    }

    // Attach the shared material where this surface is enabled; drop it elsewhere.
    let enabled = state.is_some();
    for (e, surf) in &unattached {
        if surf.surface == M::SURFACE && enabled {
            commands.entity(e).try_insert(MaterialNode(handle.clone()));
        }
    }
    for (e, surf) in &attached {
        if surf.surface == M::SURFACE && !enabled {
            commands.entity(e).remove::<MaterialNode<M>>();
        }
    }
}
