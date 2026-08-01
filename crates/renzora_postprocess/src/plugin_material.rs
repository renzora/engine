//! Custom shaded materials for standalone plugins.
//!
//! A plugin cannot define a Bevy `Material`: that is a Rust type with a derived
//! `AsBindGroup`, and the plugin has no Bevy. So there is exactly one material
//! type here — [`PluginMaterial`] — and every plugin material is an *instance*
//! of it carrying its own shader handle and its own block of uniform bytes.
//!
//! ## Why one type with a fixed uniform size
//!
//! Bevy decides a material's bind-group layout **once per type**, not per
//! instance: `AsBindGroup::bind_group_layout_entries` is a static function. A
//! plugin's uniform is whatever struct it declared, so the layout cannot be
//! derived from it — it has to be reserved. Hence
//! [`MATERIAL_UNIFORM_CAP`](renzora_plugin::sys::MATERIAL_UNIFORM_CAP): one
//! uniform buffer of that size, and a plugin uses as much of it as its settings
//! component needs. Registration refuses anything larger rather than letting the
//! shader read past the buffer, which is undefined on the GPU rather than merely
//! wrong.
//!
//! The **shader**, unlike the layout, can vary per instance —
//! [`Material::specialize`] receives the pipeline descriptor and the material's
//! key, so each instance points the vertex and fragment stages at its own
//! module. That is the whole trick that makes one type serve every plugin.
//!
//! ## Where the uniform bytes come from
//!
//! The same place a post-process effect's do: a component the plugin declared.
//! `collect_material_settings` copies its bytes out of the world each frame and
//! into every material instance that names it, so the parameters are described
//! once — inspector-editable, scene-serialised, readable by the plugin's own
//! systems — instead of duplicated into a GPU-only struct.

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, BindingResources,
    OwnedBindingResource, RenderPipelineDescriptor, ShaderStages,
    SpecializedMeshPipelineError, UnpreparedBindGroup,
};
use bevy::render::renderer::RenderDevice;
use bevy::shader::{Shader, ShaderRef};

use renzora_plugin::host::{CustomMaterialApplier, PendingMaterials};
use renzora_plugin::sys::MATERIAL_UNIFORM_CAP;

/// One plugin-defined material.
///
/// `uniform` is a fixed-size block regardless of how much the plugin declared —
/// see the module doc on why the layout cannot follow the settings struct.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct PluginMaterial {
    /// This instance's WGSL module. Applied in `specialize`, which is what lets
    /// one Rust type render every plugin's shader.
    pub shader: Handle<Shader>,
    /// Size of the settings component, so the per-frame copy reads exactly that
    /// many bytes. Reading the full uniform capacity instead would run off the
    /// end of any component smaller than it.
    pub settings_size: usize,
    /// Raw uniform bytes, refreshed each frame from the settings component.
    pub uniform: [u8; MATERIAL_UNIFORM_CAP as usize],
    pub alpha_mode: AlphaMode,
    /// Component the bytes come from — read by `collect_material_settings`,
    /// never by the GPU.
    pub settings: ComponentId,
    /// Bound from `@group(3) @binding(1)` upward, each followed by its sampler.
    pub textures: Vec<Handle<Image>>,
}

/// Per-instance pipeline key: the shader to specialize to.
///
/// `AsBindGroup::Data` is what Bevy carries from the main world into
/// specialization, so the handle rides across here rather than being looked up
/// again in the render world.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PluginMaterialKey {
    shader: Handle<Shader>,
}

impl bevy::render::render_resource::AsBindGroup for PluginMaterial {
    type Data = PluginMaterialKey;
    /// The uploaded images, plus Bevy's fallback for the slots a material did
    /// not fill — see [`Self::unprepared_bind_group`] for why every slot must be
    /// filled with something.
    type Param = (
        bevy::ecs::system::lifetimeless::SRes<
            bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>,
        >,
        bevy::ecs::system::lifetimeless::SRes<bevy::render::texture::FallbackImage>,
    );

    fn label() -> &'static str {
        "plugin_material"
    }

    /// Carries the shader handle into specialization — that is what lets one
    /// Rust type render every plugin's module.
    ///
    /// Texture count is deliberately *not* in the key. The layout declares all
    /// [`MAX_MATERIAL_TEXTURES`](renzora_plugin::sys::MAX_MATERIAL_TEXTURES)
    /// slots whatever a material actually binds, so two materials with different
    /// texture counts have identical layouts and can share a pipeline.
    fn bind_group_data(&self) -> Self::Data {
        PluginMaterialKey {
            shader: self.shader.clone(),
        }
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        render_device: &RenderDevice,
        param: &mut bevy::ecs::system::SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        use bevy::render::render_resource::{BufferInitDescriptor, BufferUsages};
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("plugin_material_uniform"),
            contents: &self.uniform,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let mut bindings = vec![(0, OwnedBindingResource::Buffer(buffer))];

        // **Every slot the layout declares must be bound, including the ones
        // this material does not use.** wgpu requires the bind group and its
        // layout to agree exactly; a material with one texture against a layout
        // of four is a validation error and a hard render-thread abort, not a
        // tail that is quietly ignored. So the unused slots get Bevy's fallback
        // image — the same one its own optional-texture materials bind.
        //
        // Texture then sampler, from binding 1 upward. `RetryNextUpdate` rather
        // than an error when an image has not reached the GPU yet: that is the
        // normal state for the first frames after creation, and failing outright
        // would drop the material permanently.
        let (images, fallback) = param;
        for i in 0..renzora_plugin::sys::MAX_MATERIAL_TEXTURES {
            let gpu = match self.textures.get(i) {
                Some(handle) => match images.get(handle) {
                    Some(gpu) => gpu,
                    None => return Err(AsBindGroupError::RetryNextUpdate),
                },
                // Always `d2`: `add_image` only creates `TextureDimension::D2`.
                None => &fallback.d2,
            };
            let base = 1 + i as u32 * 2;
            bindings.push((
                base,
                OwnedBindingResource::TextureView(
                    bevy::render::render_resource::TextureViewDimension::D2,
                    gpu.texture_view.clone(),
                ),
            ));
            bindings.push((
                base + 1,
                OwnedBindingResource::Sampler(
                    bevy::render::render_resource::SamplerBindingType::Filtering,
                    gpu.sampler.clone(),
                ),
            ));
        }
        Ok(UnpreparedBindGroup {
            bindings: BindingResources(bindings),
        })
    }

    fn bind_group_layout_entries(
        _render_device: &RenderDevice,
        _force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        use bevy::render::render_resource::binding_types::{
            sampler, texture_2d, uniform_buffer_sized,
        };
        use bevy::render::render_resource::{
            BindGroupLayoutEntries, SamplerBindingType, TextureSampleType,
        };
        // The layout is fixed for the shared material type, so it always
        // declares the maximum — the same trade the uniform cap makes. A
        // material binding fewer textures does *not* leave the tail unbound;
        // `unprepared_bind_group` fills the rest with the fallback image,
        // because wgpu rejects a bind group whose count differs from its layout.
        let mut entries = BindGroupLayoutEntries::single(
            ShaderStages::VERTEX_FRAGMENT,
            uniform_buffer_sized(false, core::num::NonZeroU64::new(MATERIAL_UNIFORM_CAP)),
        )
        .to_vec();
        for i in 0..renzora_plugin::sys::MAX_MATERIAL_TEXTURES {
            let base = 1 + i as u32 * 2;
            let mut t = texture_2d(TextureSampleType::Float { filterable: true })
                .build(base, ShaderStages::VERTEX_FRAGMENT);
            t.binding = base;
            entries.push(t);
            let mut sm = sampler(SamplerBindingType::Filtering)
                .build(base + 1, ShaderStages::VERTEX_FRAGMENT);
            sm.binding = base + 1;
            entries.push(sm);
        }
        entries
    }
}

impl Material for PluginMaterial {
    /// Bevy's own mesh vertex shader, deliberately.
    ///
    /// A plugin supplies a fragment shader only. Writing a vertex stage would
    /// mean hand-rolling the skinning, morph targets and instance-indexed model
    /// transform out of `@group(0)`/`@group(1)` — version-fragile work that has
    /// nothing to do with the material's own look, and that every plugin would
    /// have to redo. Almost every custom material is a fragment.
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    /// Replaced per instance in `specialize`, which is what lets one Rust type
    /// serve every plugin's shader.
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // **Only the main pass.** Bevy calls this same function for the prepass
        // and the shadow pass too, and their vertex stages emit a different
        // `VertexOutput` — the prepass puts UVs at location 0 where the forward
        // path puts world position. Overriding the fragment there hands the
        // shader a struct that does not match what feeds it, which wgpu rejects
        // while creating the pipeline: an unrecoverable abort, not a bad frame.
        // Bevy's own source flags this hazard in `PrepassPipelineSpecializer`.
        //
        // The pipeline key cannot tell us which pass this is, so the label has
        // to: `MeshPipelineKey::DEPTH_PREPASS` is set on the *main* pass key as
        // well, to signal that a prepass exists, so testing the bits identifies
        // both. Matched positively — an unrecognised pass keeps Bevy's shader,
        // which is the safe direction for whatever it adds next.
        //
        // Leaving the other passes alone is also what we want on the merits.
        // The prepass still runs Bevy's depth/normal/motion-vector fragment for
        // this material, so SSAO, TAA, motion blur and DOF see plugin geometry
        // like any other. Opting out of the prepass entirely would have fixed
        // the crash and quietly broken all four.
        let main_pass = descriptor
            .label
            .as_deref()
            .is_some_and(|label| label.ends_with("_mesh_pipeline"));
        if !main_pass {
            return Ok(());
        }

        // Fragment only — the vertex stage stays Bevy's, so a plugin's shader
        // is just its `@fragment fn fragment(in: VertexOutput)`.
        //
        // Unlike a post-process shader, this one is compiled through Bevy's
        // normal pipeline, so naga_oil is available and
        // `#import bevy_pbr::forward_io::VertexOutput` works. That import is in
        // fact required: `VertexOutput` is what the vertex stage above hands
        // over, and its layout is Bevy's to define.
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = key.bind_group_data.shader.clone();
        }
        Ok(())
    }
}

/// Build the real material assets from what plugins registered.
///
/// Runs in `finish`, like the post-process bridge and for the same reason: by
/// then every plugin's `build` has run and the loader has mapped the cdylibs.
pub fn build_plugin_materials(app: &mut App) {
    let pending = app
        .world_mut()
        .remove_resource::<PendingMaterials>()
        .map(|p| p.0)
        .unwrap_or_default();
    if pending.is_empty() {
        return;
    }

    let mut created: Vec<(usize, Handle<PluginMaterial>)> = Vec::new();
    {
        let world = app.world_mut();
        let Some(mut materials) = world.get_resource_mut::<Assets<PluginMaterial>>() else {
            warn!("[plugin] materials ignored — this build has no renderer");
            return;
        };
        for m in &pending {
            let handle = materials.add(PluginMaterial {
                shader: m.shader.clone(),
                settings_size: (m.settings_size as usize).min(MATERIAL_UNIFORM_CAP as usize),
                uniform: [0; MATERIAL_UNIFORM_CAP as usize],
                textures: m.textures.clone(),
                alpha_mode: match m.alpha_mode {
                    renzora_plugin::sys::AlphaMode::Mask => AlphaMode::Mask(0.5),
                    renzora_plugin::sys::AlphaMode::Blend => AlphaMode::Blend,
                    _ => AlphaMode::Opaque,
                },
                settings: m.settings,
            });
            created.push((m.slot, handle));
        }
    }

    // Keep the built assets indexed by the slot the plugin already holds. They
    // live here rather than in `PluginAssets` because that store is typed to
    // `StandardMaterial` and this crate is the only one that can name
    // `PluginMaterial`.
    let mut table = BuiltPluginMaterials::default();
    for (slot, handle) in created {
        if table.0.len() <= slot {
            table.0.resize(slot + 1, None);
        }
        table.0[slot] = Some(handle);
    }
    app.insert_resource(table);
    app.insert_resource(CustomMaterialApplier(apply_custom_material));
}

/// Plugin materials, indexed by the asset-handle slot the plugin was given.
#[derive(Resource, Default)]
pub struct BuiltPluginMaterials(pub Vec<Option<Handle<PluginMaterial>>>);

/// Attach a plugin material to a spawned mesh.
///
/// Registered as the [`CustomMaterialApplier`] so `spawn_mesh` can finish a
/// spawn without `renzora_plugin` ever naming [`PluginMaterial`].
fn apply_custom_material(world: &mut World, entity: Entity, slot: usize) {
    let handle = world
        .get_resource::<BuiltPluginMaterials>()
        .and_then(|t| t.0.get(slot).cloned().flatten());
    let Some(handle) = handle else {
        error!("[plugin] spawn_mesh named custom material slot {slot}, which was never built");
        return;
    };
    if let Ok(mut e) = world.get_entity_mut(entity) {
        e.insert(MeshMaterial3d(handle));
    }
}

/// Copy each material's settings component out of the world into its uniform.
///
/// The same shape `collect_effect_settings` uses for post-process: find the
/// first entity carrying the component and take its bytes. A material is a
/// global look, not a per-entity one, so one source is the right model — the
/// same assumption the effect path already makes.
pub fn collect_material_settings(world: &mut World) {
    let wanted: Vec<(AssetId<PluginMaterial>, ComponentId, usize)> = {
        let Some(materials) = world.get_resource::<Assets<PluginMaterial>>() else {
            return;
        };
        materials
            .iter()
            .map(|(id, m)| (id, m.settings, m.settings_size))
            .collect()
    };
    if wanted.is_empty() {
        return;
    }

    let mut updates = Vec::new();
    for (id, component, size) in wanted {
        if size == 0 {
            continue;
        }
        let found = world.iter_entities().find_map(|e| {
            e.get_by_id(component).ok().map(|p| unsafe {
                // SAFETY: `size` is the size the plugin registered this
                // component with, clamped to the uniform cap. Reading the full
                // cap instead would run past the end of any smaller component —
                // a heap over-read, and the bytes past it would land in the
                // uniform as garbage.
                std::slice::from_raw_parts(p.as_ptr(), size).to_vec()
            })
        });
        if let Some(bytes) = found {
            updates.push((id, bytes));
        }
    }

    let Some(mut materials) = world.get_resource_mut::<Assets<PluginMaterial>>() else {
        return;
    };
    for (id, bytes) in updates {
        if let Some(mut m) = materials.get_mut(id) {
            let n = bytes.len().min(m.uniform.len());
            // Only touch the asset when the bytes actually differ — every write
            // marks the material changed, which re-prepares its bind group on
            // the render side.
            if m.uniform[..n] != bytes[..n] {
                m.uniform[..n].copy_from_slice(&bytes[..n]);
            }
        }
    }
}

/// Installs the material type and its per-frame uniform refresh.
pub struct PluginMaterialPlugin;

impl Plugin for PluginMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PluginMaterial>::default())
            .add_systems(PostUpdate, collect_material_settings);
    }

    fn finish(&self, app: &mut App) {
        build_plugin_materials(app);
    }
}
