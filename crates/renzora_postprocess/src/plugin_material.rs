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

use bevy::asset::uuid_handle;
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
    type Param = ();

    fn label() -> &'static str {
        "plugin_material"
    }

    /// Carries the shader handle into specialization — that is what lets one
    /// Rust type render every plugin's module.
    fn bind_group_data(&self) -> Self::Data {
        PluginMaterialKey {
            shader: self.shader.clone(),
        }
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        render_device: &RenderDevice,
        _param: &mut bevy::ecs::system::SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        use bevy::render::render_resource::{BufferInitDescriptor, BufferUsages};
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("plugin_material_uniform"),
            contents: &self.uniform,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        Ok(UnpreparedBindGroup {
            bindings: BindingResources(vec![(0, OwnedBindingResource::Buffer(buffer))]),
        })
    }

    fn bind_group_layout_entries(
        _render_device: &RenderDevice,
        _force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        use bevy::render::render_resource::binding_types::uniform_buffer_sized;
        use bevy::render::render_resource::BindGroupLayoutEntries;
        BindGroupLayoutEntries::single(
            ShaderStages::VERTEX_FRAGMENT,
            uniform_buffer_sized(false, core::num::NonZeroU64::new(MATERIAL_UNIFORM_CAP)),
        )
        .to_vec()
    }
}

impl Material for PluginMaterial {
    fn vertex_shader() -> ShaderRef {
        // Replaced per instance in `specialize`. A placeholder rather than
        // `Default`, because `Default` would silently fall back to Bevy's own
        // mesh shader and render the plugin's material as plain PBR — which
        // looks like the shader "not working" rather than like a broken handle.
        ShaderRef::Handle(uuid_handle!("00000000-0000-0000-0000-000000000000"))
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(uuid_handle!("00000000-0000-0000-0000-000000000000"))
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
        // The per-instance shader, applied to both stages. A plugin ships one
        // module defining `vertex` and `fragment`, which keeps a material to a
        // single file.
        descriptor.vertex.shader = key.bind_group_data.shader.clone();
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
