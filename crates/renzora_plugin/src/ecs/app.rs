//! `App`, `Plugin`, and the editor-panel surface.
//!
//! Bevy spells its schedule labels `Update`, `PostUpdate` and so on, and that
//! spelling is the point of this whole layer — so they are re-exported as
//! constants under exactly those names.
//!
//! Two "did it actually register?" channels run through `App`:
//! [`App::unresolved_component`] and [`App::rejected_system`]. `add!` turns
//! either into a refused load, because a plugin that comes up half-wired is
//! much harder to diagnose than one that refuses to come up at all.

use core::marker::PhantomData;

use crate::sys;

use super::commands::{Commands, Scene};
use super::component::{Component, Vec3};
use super::init::InitCtx;
use super::render::{render_thunk, RenderPass, IFACE};
use super::resource::Resource;
use super::system::{catch, materialize, IntoSystem, IntoSystems};

/// Which schedule a system runs in. Mirrors Bevy's schedule labels.
pub use crate::sys::Schedule;

// Bevy spells its schedule labels `Update`, `PostUpdate` and so on, and that
// spelling is the point of this whole layer — so they are re-exported as
// constants under exactly those names. They used to be enum variants, which
// `use` could import directly; associated constants cannot be, hence the
// explicit list.
#[allow(non_upper_case_globals)]
pub const First: Schedule = Schedule::First;
#[allow(non_upper_case_globals)]
pub const PreUpdate: Schedule = Schedule::PreUpdate;
#[allow(non_upper_case_globals)]
pub const Update: Schedule = Schedule::Update;
#[allow(non_upper_case_globals)]
pub const PostUpdate: Schedule = Schedule::PostUpdate;
#[allow(non_upper_case_globals)]
pub const Last: Schedule = Schedule::Last;

/// Mirrors `bevy::App` for the surface a plugin can reach.
pub struct App {
    ctx: InitCtx,
    /// The first system the host refused, if any. Kept so `add!` can fail the
    /// whole load rather than let a plugin come up half-wired.
    rejected: Option<sys::RegisterStatus>,
}

impl App {
    /// # Safety
    /// `iface` and `host` must be the values the host passed to init.
    pub unsafe fn new(iface: *const sys::Interface, host: *mut sys::Host) -> Self {
        IFACE.store(iface as *mut sys::Interface, core::sync::atomic::Ordering::Relaxed);
        Self {
            ctx: InitCtx {
                iface,
                host,
                cache: alloc::vec::Vec::new(),
                unresolved: None,
            },
            rejected: None,
        }
    }

    /// The type path of the first component that could not be resolved.
    ///
    /// `add!` turns this into [`sys::InitResult::Failed`]; it is exposed so a
    /// plugin hand-writing its own entry point can do the same.
    pub fn unresolved_component(&self) -> Option<&'static str> {
        self.ctx.unresolved
    }

    /// Register one system, or a tuple of them. Mirrors `bevy::App::add_systems`.
    ///
    /// ```ignore
    /// app.add_systems(Update, spin)
    ///    .add_systems(Update, (flock, steer, draw));
    /// ```
    ///
    /// A tuple means "all of these", exactly as in Bevy — it says nothing about
    /// order, and the ABI has no ordering yet, so they may run in any order or
    /// in parallel.
    pub fn add_systems<M, S: IntoSystems<M>>(&mut self, schedule: Schedule, systems: S) -> &mut Self {
        systems.add_to(self, schedule);
        self
    }

    pub(crate) fn add_one_system<M, S: IntoSystem<M>>(
        &mut self,
        schedule: Schedule,
        system: S,
    ) -> &mut Self {
        let (builder, entry, user) = system.build(&mut self.ctx);
        let descs: alloc::vec::Vec<sys::QueryDesc> = builder
            .queries
            .iter()
            .map(|terms| sys::QueryDesc {
                terms: terms.as_ptr(),
                term_count: terms.len(),
            })
            .collect();
        let desc = sys::SystemDesc {
            entry,
            schedule,
            queries: descs.as_ptr(),
            query_count: descs.len(),
            resources: builder.resources.as_ptr(),
            resource_count: builder.resources.len(),
            user,
            flags: 0,
        };
        // SAFETY: every pointer in `desc` outlives the call; the host copies
        // what it needs into its own plan.
        let status = unsafe { ((*self.ctx.iface).add_system)(self.ctx.host, &desc) };
        if status != sys::RegisterStatus::Ok && self.rejected.is_none() {
            self.rejected = Some(status);
        }
        self
    }

    /// Why the first refused system was refused, if any was.
    ///
    /// Worth checking in `build` alongside
    /// [`unresolved_component`](Self::unresolved_component): a system the host
    /// declined is a plugin that loads and silently does less than it says.
    pub fn rejected_system(&self) -> Option<sys::RegisterStatus> {
        self.rejected
    }

    /// Add a full-screen render pass.
    ///
    /// `fragment_wgsl` is shader **source**, not a path: a plugin has no
    /// `AssetServer` and no asset root the engine could resolve against. The
    /// host compiles it and pairs it with the engine's fullscreen vertex shader.
    ///
    /// The callback runs inside the render graph, once per view, in phase +
    /// `order` sequence.
    pub fn add_render_pass<F>(
        &mut self,
        id: &'static str,
        fragment_wgsl: &'static str,
        phase: sys::RenderPhase,
        order: f32,
        _callback: F,
    ) -> &mut Self
    where
        F: Fn(&mut RenderPass) + 'static,
    {
        let desc = sys::RenderPassDesc {
            id: sys::StrRef::new(id),
            fragment_wgsl: sys::StrRef::new(fragment_wgsl),
            phase,
            order,
            callback: render_thunk::<F>,
        };
        unsafe { ((*self.ctx.iface).add_render_pass)(self.ctx.host, &desc) };
        self
    }

    /// Register a parameterised full-screen effect.
    ///
    /// `T` is an ordinary plugin component: put one on a camera to enable the
    /// effect there, and its fields become the shader's uniform *and* the
    /// inspector's controls — one declaration, both jobs.
    ///
    /// The shader receives:
    ///
    /// ```wgsl
    /// @group(0) @binding(0) var screen_texture: texture_2d<f32>;
    /// @group(0) @binding(1) var texture_sampler: sampler;
    /// @group(0) @binding(2) var<uniform> settings: MySettings;
    /// ```
    ///
    /// `T` must be `#[repr(C)]` and its layout must match the shader's struct
    /// **exactly**. The trap to know about: WGSL aligns `vec3<f32>` to 16 bytes
    /// and Rust's `[f32; 3]` to 4, so
    ///
    /// ```ignore
    /// struct S { a: f32, pad: [f32; 3] }   // Rust: 16 bytes
    /// struct S { a: f32, pad: vec3<f32> }  // WGSL: 32 bytes
    /// ```
    ///
    /// disagree. Pad with scalars on both sides. wgpu rejects a mismatch and the
    /// engine escalates that to an unrecoverable GPU panic — it is not a warning
    /// you can ignore.
    ///
    /// Prefer this over [`App::add_render_pass`] for anything shaped like a
    /// screen effect: the host does extraction, the uniform upload, the bind
    /// group and the draw, so the plugin writes no render code at all.
    pub fn add_post_process<T: Component>(
        &mut self,
        id: &'static str,
        fragment_wgsl: &'static str,
        phase: sys::RenderPhase,
        order: f32,
    ) -> &mut Self {
        let settings = self.ctx.id_of::<T>();
        let desc = sys::PostProcessDesc {
            id: sys::StrRef::new(id),
            fragment_wgsl: sys::StrRef::new(fragment_wgsl),
            settings,
            settings_size: core::mem::size_of::<T>() as u64,
            phase,
            order,
        };
        unsafe { ((*self.ctx.iface).add_post_process)(self.ctx.host, &desc) };
        self
    }

    /// Create a mesh from a built-in primitive.
    ///
    /// Init-only, so build what you need in `Plugin::build` and keep the handle.
    /// One primitive spawned a thousand times shares a single asset.
    pub fn add_mesh(&mut self, primitive: sys::Primitive, size: Vec3) -> sys::AssetHandle {
        let desc = sys::MeshDesc { primitive, size };
        unsafe { ((*self.ctx.iface).add_mesh)(self.ctx.host, &desc) }
    }

    /// Upload an image the plugin generated.
    ///
    /// `data` must be exactly `width * height * bytes_per_pixel` — a short
    /// buffer is refused, not padded, because uploading one as a full texture
    /// reads past the plugin's heap into a GPU transfer.
    ///
    /// Init-only, like the other asset constructors. Contents can be replaced
    /// from a system with [`super::Images::write`]; dimensions and format
    /// cannot.
    pub fn add_image(
        &mut self,
        width: u32,
        height: u32,
        format: sys::ImageFormat,
        data: &[u8],
    ) -> sys::AssetHandle {
        let desc = sys::ImageDesc {
            width,
            height,
            format,
            data: data.as_ptr(),
            data_len: data.len(),
        };
        unsafe { ((*self.ctx.iface).add_image)(self.ctx.host, &desc) }
    }

    /// Register a custom shaded material, driven by one of the plugin's own
    /// components.
    ///
    /// ```ignore
    /// #[derive(Component)]
    /// #[repr(C)]
    /// pub struct Ripple { pub speed: f32, pub amplitude: f32 }
    ///
    /// let mat = app.add_material_shader::<Ripple>("ripple", WGSL, sys::AlphaMode::Blend);
    /// ```
    ///
    /// `T`'s bytes are uploaded as the uniform at `@group(3) @binding(0)`, so
    /// the parameters are described once — editable in the inspector, saved
    /// into scenes, readable by the plugin's own systems — rather than
    /// duplicated into a GPU-only struct.
    ///
    /// The shader supplies a `fragment` entry point only; the vertex stage is
    /// Bevy's. It is compiled through Bevy's pipeline, so naga_oil imports work
    /// — and `#import bevy_pbr::forward_io::VertexOutput` is required, since
    /// that is what the vertex stage hands over.
    ///
    /// Refused, with a log line, if `T` is larger than
    /// [`sys::MATERIAL_UNIFORM_CAP`]: the bind-group layout is fixed for the
    /// shared material type, and a uniform read past its buffer is undefined on
    /// the GPU rather than merely wrong.
    pub fn add_material_shader<T: Component>(
        &mut self,
        id: &'static str,
        wgsl: &'static str,
        alpha_mode: sys::AlphaMode,
        textures: &[sys::AssetHandle],
    ) -> sys::AssetHandle {
        let settings = self.ctx.id_of::<T>();
        let desc = sys::MaterialShaderDesc {
            id: sys::StrRef::new(id),
            wgsl: sys::StrRef::new(wgsl),
            settings,
            settings_size: core::mem::size_of::<T>() as u64,
            alpha_mode,
            textures: textures.as_ptr(),
            texture_count: textures.len(),
        };
        unsafe { ((*self.ctx.iface).add_material_shader)(self.ctx.host, &desc) }
    }

    /// Create a mesh from geometry the plugin generated itself.
    ///
    /// This is what lets a plugin be more than a consumer of built-in shapes —
    /// text meshes, procedural foliage, hair ribbons, water surfaces.
    ///
    /// ```ignore
    /// // A quad. Normals and UVs derived by the host.
    /// let quad = app.add_mesh_data(
    ///     &[Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0),
    ///       Vec3::new(1.0, 0.0, 1.0),   Vec3::new(-1.0, 0.0, 1.0)],
    ///     None, None,
    ///     Some(&[0, 1, 2, 0, 2, 3]),
    /// );
    /// ```
    ///
    /// `normals` and `uvs` may be `None` — the host computes normals from the
    /// faces and zeroes the UVs. `indices` may be `None` for an unindexed
    /// triangle list, where every three positions form one face.
    ///
    /// Everything is copied before this returns, so the slices may be locals.
    /// Anything inconsistent — an index past the end, a normal count that does
    /// not match the vertices, a position count that is not a whole number of
    /// triangles — is **refused**, returning an invalid handle and logging why,
    /// rather than being padded or clamped into a mesh that renders subtly wrong.
    pub fn add_mesh_data(
        &mut self,
        positions: &[Vec3],
        normals: Option<&[Vec3]>,
        uvs: Option<&[[f32; 2]]>,
        indices: Option<&[u32]>,
    ) -> sys::AssetHandle {
        let desc = sys::MeshDataDesc {
            positions: positions.as_ptr(),
            position_count: positions.len(),
            normals: normals.map_or(core::ptr::null(), |n| n.as_ptr()),
            normal_count: normals.map_or(0, |n| n.len()),
            uvs: uvs.map_or(core::ptr::null(), |u| u.as_ptr()),
            uv_count: uvs.map_or(0, |u| u.len()),
            indices: indices.map_or(core::ptr::null(), |i| i.as_ptr()),
            index_count: indices.map_or(0, |i| i.len()),
        };
        unsafe { ((*self.ctx.iface).add_mesh_data)(self.ctx.host, &desc) }
    }

    /// Create a standard PBR material. Init-only, like [`App::add_mesh`].
    pub fn add_material(&mut self, color: [f32; 4]) -> sys::AssetHandle {
        let desc = sys::MaterialDesc {
            color,
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0, 1.0],
        };
        unsafe { ((*self.ctx.iface).add_material)(self.ctx.host, &desc) }
    }

    /// Full control over the material's PBR parameters.
    pub fn add_material_pbr(
        &mut self,
        color: [f32; 4],
        metallic: f32,
        roughness: f32,
        emissive: [f32; 4],
    ) -> sys::AssetHandle {
        let desc = sys::MaterialDesc {
            color,
            metallic,
            roughness,
            emissive,
        };
        unsafe { ((*self.ctx.iface).add_material)(self.ctx.host, &desc) }
    }

    /// Register a plugin-owned component ahead of first use. Optional — a
    /// component named in a query is registered automatically.
    pub fn register_component<T: Component>(&mut self) -> &mut Self {
        self.ctx.id_of::<T>();
        self
    }

    /// Register `T` and insert its `Default` value. Mirrors `bevy::App::init_resource`.
    pub fn init_resource<T: Resource>(&mut self) -> &mut Self {
        self.ctx.resource_id_of::<T>();
        self
    }

    /// Register `T` and insert `value`. Mirrors `bevy::App::insert_resource`.
    pub fn insert_resource<T: Resource>(&mut self, value: T) -> &mut Self {
        let id = self.ctx.resource_id_of::<T>();
        if id.is_valid() {
            // The host copies the bytes out and takes ownership of them, so this
            // side must not also run the destructor.
            let value = core::mem::ManuallyDrop::new(value);
            unsafe {
                ((*self.ctx.iface).insert_resource)(
                    self.ctx.host,
                    id,
                    (&*value as *const T).cast(),
                    core::mem::size_of::<T>(),
                );
            }
        }
        self
    }

    /// Register an editor panel from markup. See [`sys::PanelDesc`] for the
    /// grammar.
    ///
    /// `handler` runs when an action widget fires and may be a plain fn or a
    /// non-capturing closure, the same rule systems follow and for the same
    /// reason: the host has nowhere to put a capture.
    pub fn add_panel<H: PanelHandler>(&mut self, panel: Panel<H>) -> &mut Self {
        let desc = sys::PanelDesc {
            id: sys::StrRef::new(panel.id),
            title: sys::StrRef::new(panel.title),
            icon: sys::StrRef::new(panel.icon),
            category: sys::StrRef::new(panel.category),
            markup: sys::StrRef::new(panel.scene.0),
            on_action: H::ENTRY,
            user: core::ptr::null_mut(),
        };
        // SAFETY: every `StrRef` points at a `'static` str, and the host copies
        // the markup before returning.
        let status = unsafe { ((*self.ctx.iface).add_panel)(self.ctx.host, &desc) };
        if status != sys::RegisterStatus::Ok && self.rejected.is_none() {
            self.rejected = Some(status);
        }
        self
    }

    /// Register a section on the Settings overlay's **Plugins** tab.
    ///
    /// Takes the same [`Panel`] a dock panel does, because a settings section is
    /// one — same id, title, icon, markup and action handler — that renders
    /// inside Settings instead of in the dock:
    ///
    /// ```ignore
    /// app.add_settings_section(
    ///     Panel::new("mychat", "AI Chat", bsn! { .. })
    ///         .icon("robot")
    ///         .on_action(on_settings_action),
    /// );
    /// ```
    ///
    /// Ids are ONE namespace across panels and sections, because
    /// [`Commands::set_panel_content`](crate::panel::PanelCommands::set_panel_content)
    /// resolves against one list — so a section may update itself the same way a
    /// panel does, and a section sharing an id with a panel is refused rather
    /// than silently applying its content to the wrong one.
    ///
    /// `.category()` groups sections in the Settings sidebar; it does not choose
    /// the tab, which is always Plugins.
    pub fn add_settings_section<H: PanelHandler>(&mut self, panel: Panel<H>) -> &mut Self {
        let desc = sys::PanelDesc {
            id: sys::StrRef::new(panel.id),
            title: sys::StrRef::new(panel.title),
            icon: sys::StrRef::new(panel.icon),
            category: sys::StrRef::new(panel.category),
            markup: sys::StrRef::new(panel.scene.0),
            on_action: H::ENTRY,
            user: core::ptr::null_mut(),
        };
        // SAFETY: as `add_panel` — every `StrRef` points at a `'static` str and
        // the host copies the markup before returning.
        let status = unsafe { ((*self.ctx.iface).add_settings_section)(self.ctx.host, &desc) };
        if status != sys::RegisterStatus::Ok && self.rejected.is_none() {
            self.rejected = Some(status);
        }
        self
    }

    /// Register a scripting language.
    ///
    /// The descriptor comes from the `script_backend!` macro, which owns the
    /// entry point's state — see [`crate::script`]. The extension array it
    /// returns alongside must stay alive across this call, which is why the two
    /// travel together rather than the macro handing back a bare descriptor
    /// pointing at a temporary.
    ///
    /// ```ignore
    /// renzora_plugin::script_backend!(LuaBackend);
    ///
    /// impl Plugin for LuaPlugin {
    ///     fn build(&self, app: &mut App) {
    ///         app.add_script_backend(script_backend::desc());
    ///     }
    /// }
    /// ```
    #[cfg(feature = "script")]
    pub fn add_script_backend(
        &mut self,
        backend: (sys::ScriptBackendDesc, alloc::vec::Vec<sys::Str256>),
    ) -> &mut Self {
        let (desc, extensions) = backend;
        // SAFETY: `extensions` is alive for this whole function, `desc` points
        // into it, and the host copies both name and extensions before
        // returning.
        let status = unsafe { ((*self.ctx.iface).add_script_backend)(self.ctx.host, &desc) };
        drop(extensions);
        if status != sys::RegisterStatus::Ok && self.rejected.is_none() {
            self.rejected = Some(status);
        }
        self
    }

    /// Register the audio backend.
    ///
    /// The descriptor comes from the `audio_backend!` macro, which owns the
    /// entry point's state — see [`crate::audio`]. A bare descriptor rather than
    /// the pair `add_script_backend` takes, because there is no borrowed
    /// extension array to keep alive: an audio backend claims no file types.
    ///
    /// Only one backend loads. A second registration is refused and logged by
    /// the host, because two backends would open the same output device and the
    /// user would hear both mixes at once.
    ///
    /// ```ignore
    /// renzora_plugin::audio_backend!(MyMixer);
    ///
    /// impl Plugin for MyAudioPlugin {
    ///     fn build(&self, app: &mut App) {
    ///         app.add_audio_backend(audio_backend::desc());
    ///     }
    /// }
    /// ```
    #[cfg(feature = "audio")]
    pub fn add_audio_backend(&mut self, desc: sys::AudioBackendDesc) -> &mut Self {
        // SAFETY: `desc` is alive for this call and the host copies the name
        // before returning; `state` and `entry` are passed through untouched.
        let status = unsafe { ((*self.ctx.iface).add_audio_backend)(self.ctx.host, &desc) };
        if status != sys::RegisterStatus::Ok && self.rejected.is_none() {
            self.rejected = Some(status);
        }
        self
    }

    /// Become the engine's HTTP client.
    ///
    /// The descriptor comes from the `net_backend!` macro, which owns the entry
    /// point's state — see [`crate::net`]. Registering makes every network
    /// request the engine wants to make — the marketplace, asset thumbnails,
    /// sign-in, the update check, a script's `http_get` — go through this
    /// plugin. Without one registered, the engine has no client at all and every
    /// such call reports that plainly rather than hanging.
    ///
    /// Only one backend loads. A second registration is refused and logged, for
    /// the reason `add_audio_backend` refuses one: there is no per-request key
    /// to choose by, and splitting a session's cookies and connection pool
    /// across two clients would break both.
    ///
    /// ```ignore
    /// renzora_plugin::net_backend!(MyClient);
    ///
    /// impl Plugin for MyHttpPlugin {
    ///     fn build(&self, app: &mut App) {
    ///         app.add_net_backend(net_backend::desc());
    ///     }
    /// }
    /// ```
    #[cfg(feature = "net")]
    pub fn add_net_backend(&mut self, desc: sys::NetBackendDesc) -> &mut Self {
        // SAFETY: `desc` is alive for this call and the host copies the name
        // before returning; `state` and `entry` are passed through untouched.
        let status = unsafe { ((*self.ctx.iface).add_net_backend)(self.ctx.host, &desc) };
        if status != sys::RegisterStatus::Ok && self.rejected.is_none() {
            self.rejected = Some(status);
        }
        self
    }
}

/// An editor panel, before registration.
///
/// A struct rather than seven positional arguments — a call site reading
/// `add_panel("flock", "Flock", "wind", "Plugins", MARKUP, on_action)` is a
/// puzzle at every future edit.
pub struct Panel<H> {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    /// The panel's contents, as a [`Scene`] — write it inline with
    /// [`bsn!`](crate::bsn) rather than parking it in a `const`.
    pub scene: Scene,
    pub on_action: H,
}

impl Panel<()> {
    /// A panel with no action widgets.
    ///
    /// ```ignore
    /// app.add_panel(Panel::new("flock", "Flock", bsn! {
    ///     Node { flex_direction: Column, row_gap: Px(6.0) }
    ///     Children [
    ///         Text("Flocking"),
    ///     ]
    /// }));
    /// ```
    pub fn new(id: &'static str, title: &'static str, scene: Scene) -> Self {
        Self {
            id,
            title,
            icon: "",
            category: "",
            scene,
            on_action: (),
        }
    }
}

impl<H> Panel<H> {
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = icon;
        self
    }

    pub fn category(mut self, category: &'static str) -> Self {
        self.category = category;
        self
    }

    /// Attach the handler for clicks on this panel's `PanelActionId` widgets.
    pub fn on_action<F: Fn(Action)>(self, handler: F) -> Panel<F> {
        Panel {
            id: self.id,
            title: self.title,
            icon: self.icon,
            category: self.category,
            scene: self.scene,
            on_action: handler,
        }
    }
}

/// What a fired action tells the plugin.
pub struct Action<'a> {
    name: &'a str,
    /// A toggle's 0 or 1, a slider's position, 0 for a button.
    pub value: f32,
    /// A text input's contents; empty for widgets that have no text. Borrowed
    /// for the duration of the handler — copy it to keep it.
    text: &'a str,
    /// Structural changes, same queue a system gets.
    pub commands: Commands<'a>,
}

impl Action<'_> {
    /// Which action fired — the `action` number from the widget's
    /// `PanelActionId`, as a string. A number because the ABI's field kinds can
    /// describe an `i32` and not a `String`; a string here because that is what
    /// the wire format carries and a future named form would slot straight in.
    pub fn name(&self) -> &str {
        self.name
    }

    /// Convenience for the usual `match`-free single-action panel.
    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }

    /// The widget's text — a text input's current contents, empty for anything
    /// without any.
    ///
    /// Borrowed from host memory for the duration of this call only, which is
    /// why it is a `&str` rather than a `String`: a plugin keeping the prompt
    /// has to say so by copying it. A text input fires its action on every
    /// change, so the usual shape is to stash this in a `static` and read it
    /// back when a Send button fires.
    pub fn text(&self) -> &str {
        self.text
    }
}

/// Supplies the `extern "C"` entry point for a panel's actions.
///
/// Implemented for `()` — no handler — and for any zero-sized `Fn(Action)`. The
/// ZST bound is the same one systems carry: the thunk reconstructs the callable
/// from nothing, so there is nothing for the host to own or free.
pub trait PanelHandler {
    const ENTRY: Option<sys::PanelActionEntry>;
}

impl PanelHandler for () {
    const ENTRY: Option<sys::PanelActionEntry> = None;
}

impl<F: Fn(Action) + 'static> PanelHandler for F {
    const ENTRY: Option<sys::PanelActionEntry> = Some(panel_thunk::<F>);
}

unsafe extern "C" fn panel_thunk<F: Fn(Action) + 'static>(
    action: *const sys::PanelAction,
) -> sys::SystemStatus {
    let a = &*action;
    let payload = Action {
        name: a.name.as_str(),
        value: a.value,
        text: a.text.as_str(),
        commands: Commands {
            sink: a.commands,
            _p: PhantomData,
        },
    };
    // A panic here would unwind out of an `extern "C"` call made from inside the
    // editor's own UI systems, which aborts the process — a bad button taking
    // the editor down with it.
    match catch(move || materialize::<F>()(payload)) {
        Ok(()) => sys::SystemStatus::Ok,
        Err(msg) => {
            if !a.iface.is_null() {
                ((*a.iface).log)(
                    core::ptr::null_mut(),
                    sys::LogLevel::Error,
                    sys::StrRef {
                        ptr: msg.as_ptr(),
                        len: msg.len(),
                    },
                );
            }
            sys::SystemStatus::Panicked
        }
    }
}

/// Mirrors `bevy::Plugin`.
pub trait Plugin {
    fn build(&self, app: &mut App);
}
