#![allow(dead_code)] // ShaderType derives emit unused check helpers
use core::time::Duration;
use std::marker::PhantomData;

use bevy::{
    asset::RenderAssetUsages,
    ecs::system::{SystemParamItem, lifetimeless::SRes},
    math::Vec3A,
    platform::time::Instant,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_asset::{PrepareAssetError, RenderAsset, RenderAssetPlugin},
        render_resource::*,
        renderer::RenderDevice,
    },
};
use bevy_interleave::prelude::*;
use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};
use static_assertions::assert_cfg;

use crate::{CloudSettings, camera::GaussianCamera, gaussian::interface::CommonCloud};

#[cfg(feature = "sort_bitonic")]
pub mod bitonic;

#[cfg(all(feature = "sort_radix", not(feature = "buffer_texture")))]
pub mod radix;

#[cfg(feature = "sort_rayon")]
pub mod rayon;

#[cfg(feature = "sort_std")]
pub mod std_sort; // rename to std_sort.rs to avoid name conflict with std crate

assert_cfg!(
    any(
        feature = "sort_radix",
        feature = "sort_rayon",
        feature = "sort_std",
    ),
    "no sort mode enabled",
);

#[derive(Component, Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub enum SortMode {
    None,

    #[cfg(all(feature = "sort_radix", not(feature = "buffer_texture")))]
    Radix,

    #[cfg(feature = "sort_rayon")]
    Rayon,

    #[cfg(feature = "sort_std")]
    Std,
}

impl Default for SortMode {
    #[allow(unreachable_code)]
    fn default() -> Self {
        #[cfg(all(feature = "sort_radix", not(feature = "buffer_texture")))]
        return Self::Radix;

        #[cfg(feature = "sort_rayon")]
        return Self::Rayon;

        #[cfg(feature = "sort_std")]
        return Self::Std;

        Self::None
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct SortConfig {
    pub period_ms: usize,
}

impl Default for SortConfig {
    fn default() -> Self {
        Self { period_ms: 1000 }
    }
}

#[derive(Default)]
pub struct SortPluginFlag;
impl Plugin for SortPluginFlag {
    fn build(&self, _app: &mut App) {}
}

// TODO: make this generic /w shared components
#[derive(Default)]
pub struct SortPlugin<R: PlanarSync> {
    phantom: PhantomData<R>,
}

impl<R: PlanarSync> Plugin for SortPlugin<R>
where
    R::PlanarType: CommonCloud,
    R::GpuPlanarType: GpuPlanarStorage,
{
    fn build(&self, app: &mut App) {
        #[cfg(all(feature = "sort_radix", not(feature = "buffer_texture")))]
        app.add_plugins(radix::RadixSortPlugin::<R>::default());

        #[cfg(feature = "sort_rayon")]
        app.add_plugins(rayon::RayonSortPlugin::<R>::default());

        #[cfg(feature = "sort_std")]
        app.add_plugins(std_sort::StdSortPlugin::<R>::default());

        app.add_systems(Update, auto_insert_sorted_entries::<R>);

        if app.is_plugin_added::<SortPluginFlag>() {
            debug!("sort plugin flag already added");
            return;
        }
        app.add_plugins(SortPluginFlag);

        app.register_type::<SortConfig>();
        app.init_resource::<SortConfig>();

        app.register_type::<SortedEntries>();
        app.register_type::<SortedEntriesHandle>();
        app.init_asset::<SortedEntries>();
        app.register_asset_reflect::<SortedEntries>();

        app.register_type::<SortTrigger>();
        app.add_plugins(ExtractComponentPlugin::<SortTrigger>::default());

        app.add_plugins(RenderAssetPlugin::<GpuSortedEntry>::default());

        app.add_systems(Update, (update_sort_trigger, update_sorted_entries_sizes));

        #[cfg(feature = "buffer_texture")]
        app.add_systems(PostUpdate, update_textures_on_change);
    }
}

#[derive(Component, ExtractComponent, Debug, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SortTrigger {
    pub camera_index: usize,
    pub needs_sort: bool,
    pub last_camera_position: Vec3A,
    pub last_sort_time: Option<Instant>,
}

#[allow(clippy::type_complexity)]
fn update_sort_trigger(
    mut commands: Commands,
    new_gaussian_cameras: Query<Entity, (With<Camera>, With<GaussianCamera>, Without<SortTrigger>)>,
    mut existing_sort_triggers: Query<(Entity, &GlobalTransform, &mut SortTrigger), With<Camera>>,
    sort_config: Res<SortConfig>,
) {
    for entity in new_gaussian_cameras.iter() {
        commands.entity(entity).insert(SortTrigger::default());
    }

    // renzora patch: assign `camera_index` as a dense 0..N over the LIVE
    // gaussian cameras instead of trusting `Camera::order`. Upstream asserted
    // `order >= 0` and used it verbatim, but `camera_index` selects one of the
    // `camera_count` per-camera chunks of every `SortedEntries` buffer — so a
    // negative order panicked the assert (editor render-to-texture cameras use
    // negative orders) and any sparse order set (0 and 5, say) overran the
    // chunk list at `chunks.nth()`. Entity order is arbitrary but consistent
    // within a frame, which is all the chunk indexing needs; re-densifying
    // every frame also heals index holes left by despawned cameras. An index
    // change forces a re-sort because the camera now owns a different chunk.
    let mut cameras: Vec<Entity> = existing_sort_triggers
        .iter()
        .map(|(entity, ..)| entity)
        .collect();
    cameras.sort_unstable();

    for (entity, camera_transform, mut sort_trigger) in existing_sort_triggers.iter_mut() {
        if let Ok(index) = cameras.binary_search(&entity)
            && sort_trigger.camera_index != index
        {
            sort_trigger.camera_index = index;
            sort_trigger.needs_sort = true;
        }

        match sort_trigger.last_sort_time.as_ref() {
            None => {
                sort_trigger.needs_sort = true;
                sort_trigger.last_sort_time = Some(Instant::now());
                continue;
            }
            Some(last_sort_time)
                if last_sort_time.elapsed()
                    < Duration::from_millis(sort_config.period_ms as u64) =>
            {
                continue;
            }
            Some(_) => {}
        }

        let camera_position = camera_transform.affine().translation;
        let camera_movement = sort_trigger.last_camera_position != camera_position;

        if camera_movement {
            sort_trigger.needs_sort = true;
            sort_trigger.last_sort_time = Some(Instant::now());
            sort_trigger.last_camera_position = camera_position;
        }
    }
}

#[cfg(feature = "buffer_texture")]
fn update_textures_on_change(
    mut images: ResMut<Assets<Image>>,
    mut ev_asset: MessageReader<AssetEvent<SortedEntries>>,
    sorted_entries_res: Res<Assets<SortedEntries>>,
) {
    for ev in ev_asset.read() {
        match ev {
            AssetEvent::Modified { id } => {
                let sorted_entries = sorted_entries_res.get(*id).unwrap();
                let mut image = images.get_mut(&sorted_entries.texture).unwrap();

                image.data = Some(bytemuck::cast_slice(sorted_entries.sorted.as_slice()).to_vec());
            }
            AssetEvent::Added { id: _ } => {}
            AssetEvent::Removed { id: _ } => {}
            AssetEvent::LoadedWithDependencies { id: _ } => {}
            AssetEvent::Unused { id: _ } => {}
        }
    }
}

#[allow(clippy::type_complexity)]
fn auto_insert_sorted_entries<R: PlanarSync>(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gaussian_clouds_res: Res<Assets<R::PlanarType>>,
    mut sorted_entries_res: ResMut<Assets<SortedEntries>>,
    gaussian_clouds: Query<
        (Entity, &R::PlanarTypeHandle, &CloudSettings),
        Without<SortedEntriesHandle>,
    >,
    gaussian_cameras: Query<Entity, (With<Camera>, With<GaussianCamera>)>,
    #[cfg(feature = "buffer_texture")] mut images: ResMut<Assets<Image>>,
) where
    R::PlanarType: CommonCloud,
{
    let camera_count = gaussian_cameras.iter().len();

    if camera_count == 0 {
        debug!("no gaussian cameras found");
        return;
    }

    for (entity, gaussian_cloud_handle, _settings) in gaussian_clouds.iter() {
        // // TODO: specialize vertex shader for sort mode (e.g. draw_indirect but no sort indirection)
        // if settings.sort_mode == SortMode::None {
        //     continue;
        // }

        if let Some(load_state) = asset_server.get_load_state(gaussian_cloud_handle.handle())
            && load_state.is_loading()
        {
            debug!("cloud asset is still loading");
            continue;
        }

        let cloud = gaussian_clouds_res.get(gaussian_cloud_handle.handle());
        if cloud.is_none() {
            debug!("cloud asset is not loaded");
            continue;
        }
        let cloud = cloud.unwrap();

        let sorted_entries = sorted_entries_res.add(SortedEntries::new(
            camera_count,
            cloud.len_sqrt_ceil().pow(2),
            #[cfg(feature = "buffer_texture")]
            &mut images,
        ));

        commands
            .entity(entity)
            .insert(SortedEntriesHandle(sorted_entries));
    }
}

fn update_sorted_entries_sizes(
    mut sorted_entries_res: ResMut<Assets<SortedEntries>>,
    sorted_entries: Query<&SortedEntriesHandle>,
    gaussian_cameras: Query<Entity, (With<Camera>, With<GaussianCamera>)>,
    #[cfg(feature = "buffer_texture")] mut images: ResMut<Assets<Image>>,
) {
    let camera_count: usize = gaussian_cameras.iter().len();

    for handle in sorted_entries.iter() {
        if camera_count == 0 {
            sorted_entries_res.remove(handle);
            continue;
        }

        if let Some(sorted_entries) = sorted_entries_res.get(handle)
            && sorted_entries.camera_count != camera_count
        {
            let new_entry = SortedEntries::new(
                camera_count,
                sorted_entries.entry_count,
                #[cfg(feature = "buffer_texture")]
                &mut images,
            );
            let _ = sorted_entries_res.insert(handle, new_entry);
        }
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct SortedEntriesHandle(pub Handle<SortedEntries>);

impl From<Handle<SortedEntries>> for SortedEntriesHandle {
    fn from(handle: Handle<SortedEntries>) -> Self {
        Self(handle)
    }
}

impl From<SortedEntriesHandle> for AssetId<SortedEntries> {
    fn from(handle: SortedEntriesHandle) -> Self {
        handle.0.id()
    }
}

impl From<&SortedEntriesHandle> for AssetId<SortedEntries> {
    fn from(handle: &SortedEntriesHandle) -> Self {
        handle.0.id()
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect, ShaderType, Pod, Zeroable)]
#[repr(C)]
pub struct SortEntry {
    pub key: u32,
    pub index: u32,
}

#[derive(Clone, Asset, Debug, Default, PartialEq, Reflect)]
pub struct SortedEntries {
    pub camera_count: usize,
    pub entry_count: usize,
    pub sorted: Vec<SortEntry>,

    #[cfg(feature = "buffer_texture")]
    pub texture: Handle<Image>,
}

impl SortedEntries {
    pub fn new(
        camera_count: usize,
        entry_count: usize,
        #[cfg(feature = "buffer_texture")] images: &mut Assets<Image>,
    ) -> Self {
        let sorted: Vec<SortEntry> = (0..camera_count)
            .flat_map(|_camera_idx| {
                (0..entry_count).map(|idx| SortEntry {
                    key: 1,
                    index: idx as u32,
                })
            })
            .collect();

        #[cfg(feature = "buffer_texture")]
        let mut sorted_entries = SortedEntries {
            camera_count,
            entry_count,
            sorted,
            texture: Handle::default(),
        };

        #[cfg(not(feature = "buffer_texture"))]
        let sorted_entries = SortedEntries {
            camera_count,
            entry_count,
            sorted,
        };

        #[cfg(feature = "buffer_texture")]
        {
            let side = (entry_count as f32).sqrt().ceil() as u32;
            let data = bytemuck::cast_slice(sorted_entries.sorted.as_slice()).to_vec();
            let mut image = Image::new(
                Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: camera_count as u32,
                },
                TextureDimension::D2,
                data,
                TextureFormat::Rg32Uint,
                RenderAssetUsages::default(),
            );
            image.texture_descriptor.usage =
                TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
            sorted_entries.texture = images.add(image);
        }

        sorted_entries
    }
}

impl RenderAsset for GpuSortedEntry {
    type SourceAsset = SortedEntries;
    type Param = SRes<RenderDevice>;

    fn prepare_asset(
        source: Self::SourceAsset,
        _: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
        _: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let sorted_entry_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("sorted_entry_buffer"),
            contents: bytemuck::cast_slice(source.sorted.as_slice()),
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST | BufferUsages::STORAGE,
        });

        let count = source.sorted.len();

        Ok(GpuSortedEntry {
            sorted_entry_buffer,
            count,

            #[cfg(feature = "buffer_texture")]
            texture: source.texture,
        })
    }

    fn asset_usage(_: &Self::SourceAsset) -> RenderAssetUsages {
        RenderAssetUsages::default()
    }
}

// TODO: support instancing and multiple cameras
//       separate entry_buffer_a binding into unique a bind group to optimize buffer updates
#[derive(Debug, Clone)]
pub struct GpuSortedEntry {
    pub sorted_entry_buffer: Buffer,
    pub count: usize,

    #[cfg(feature = "buffer_texture")]
    pub texture: Handle<Image>,
}
