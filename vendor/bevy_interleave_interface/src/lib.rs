pub mod storage;
// pub mod texture;

pub trait PlanarHandle<T>
where
    Self: bevy::ecs::component::Component,
    Self: Clone,
    Self: Default,
    Self: bevy::reflect::FromReflect,
    Self: bevy::reflect::GetTypeRegistration,
    Self: bevy::reflect::Reflect,
    Self: bevy::render::sync_component::SyncComponent<bevy::render::RenderApp>,
    T: bevy::asset::Asset,
{
    fn handle(&self) -> &bevy::asset::Handle<T>;
}

// TODO: migrate to PlanarSync
pub trait PlanarSync
where
    Self: Default,
    Self: Send,
    Self: Sync,
    Self: bevy::reflect::Reflect,
    Self: 'static,
{
    type PackedType; // Self
    type PlanarType: Planar<PackedType = Self::PackedType>;
    type PlanarTypeHandle: PlanarHandle<Self::PlanarType>;
    type GpuPlanarType: GpuPlanar<PackedType = Self::PackedType, PlanarType = Self::PlanarType>;
}

pub trait GpuPlanar
where
    Self: bevy::render::render_asset::RenderAsset<SourceAsset = Self::PlanarType>,
{
    type PackedType;
    type PlanarType;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn len(&self) -> usize;
}

// #[cfg(feature = "debug_gpu")]
// pub debug_gpu: PlanarType,
// TODO: when `debug_gpu` feature is enabled, add a function to access the main -> render world copied asset (for ease of test writing)
pub trait GpuPlanarStorage
where
    Self: GpuPlanar,
    Self: bevy::render::render_asset::RenderAsset<SourceAsset = Self::PlanarType>,
{
    fn draw_indirect_buffer(&self) -> &bevy::render::render_resource::Buffer;

    fn bind_group(
        &self,
        render_device: &bevy::render::renderer::RenderDevice,
        layout: &bevy::render::render_resource::BindGroupLayout,
    ) -> bevy::render::render_resource::BindGroup;

    fn bind_group_layout(
        render_device: &bevy::render::renderer::RenderDevice,
        read_only: bool,
    ) -> bevy::render::render_resource::BindGroupLayout;
}

pub trait GpuPlanarTexture
where
    Self: GpuPlanar,
    Self: bevy::render::render_asset::RenderAsset<SourceAsset = Self::PlanarType>,
{
    fn bind_group(
        &self,
        render_device: &bevy::render::renderer::RenderDevice,
        gpu_images: &bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>,
        layout: &bevy::render::render_resource::BindGroupLayout,
    ) -> bevy::render::render_resource::BindGroup;

    fn bind_group_layout(
        render_device: &bevy::render::renderer::RenderDevice,
    ) -> bevy::render::render_resource::BindGroupLayout;

    fn get_asset_handles(&self) -> Vec<bevy::asset::Handle<bevy::image::Image>>;
}

// TODO: find a better name, PlanarTexture is implemented on the packed type
pub trait PlanarTexture
where
    Self: PlanarSync,
{
    // note: planar texture's gpu type utilizes bevy's image render asset
    fn prepare(
        images: &mut bevy::asset::Assets<bevy::image::Image>,
        planar: &Self::PlanarType,
    ) -> Self::GpuPlanarType;

    fn get_asset_handles(&self) -> Vec<bevy::asset::Handle<bevy::image::Image>>;
}

pub trait ReflectInterleaved {
    type PackedType;

    fn min_binding_sizes() -> &'static [usize];
    fn ordered_field_names() -> &'static [&'static str];
}

pub trait Planar
where
    Self: bevy::asset::Asset,
    Self: bevy::reflect::GetTypeRegistration,
    Self: bevy::reflect::FromReflect,
    // Bevy main: `FromReflect` no longer implies `Reflect`, which
    // `register_asset_reflect` still needs.
    Self: bevy::reflect::Reflect,
{
    type PackedType;

    fn get(&self, index: usize) -> Self::PackedType;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn len(&self) -> usize;
    fn set(&mut self, index: usize, value: Self::PackedType);
    fn to_interleaved(&self) -> Vec<Self::PackedType>;

    fn from_interleaved(packed: Vec<Self::PackedType>) -> Self
    where
        Self: Sized;

    fn subset(&self, indices: &[usize]) -> Self;
}
