use std::marker::PhantomData;

use bevy::{prelude::*, reflect::GetTypeRegistration};

use crate::{GpuPlanarStorage, PlanarHandle, PlanarSync};

pub struct PlanarStoragePlugin<R> {
    phantom: PhantomData<fn() -> R>,
}
impl<R> Default for PlanarStoragePlugin<R> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<R: 'static> Plugin for PlanarStoragePlugin<R>
where
    R: PlanarSync + Default + GetTypeRegistration + Clone + Reflect,
    R::GpuPlanarType: GpuPlanarStorage,
{
    fn build(&self, app: &mut App) {
        app.register_type::<R>();

        app.register_type::<R::PlanarType>();
        app.register_type::<R::PlanarTypeHandle>();
        app.init_asset::<R::PlanarType>();
        app.register_asset_reflect::<R::PlanarType>();

        app.add_plugins(bevy::render::render_asset::RenderAssetPlugin::<
            R::GpuPlanarType,
        >::default());
        app.add_plugins(bevy::render::sync_component::SyncComponentPlugin::<
            R::PlanarTypeHandle,
        >::default());

        let render_app = app.sub_app_mut(bevy::render::RenderApp);
        render_app.add_systems(bevy::render::ExtractSchedule, extract_planar_handles::<R>);
        render_app.add_systems(
            bevy::render::Render,
            queue_gpu_storage_buffers::<R>.in_set(bevy::render::RenderSystems::PrepareBindGroups),
        );
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.init_resource::<PlanarStorageLayouts<R>>();
        }
    }
}

// TODO: migrate to PlanarLayouts<R: PlanarSync>
#[derive(bevy::prelude::Resource)]
pub struct PlanarStorageLayouts<R: PlanarSync>
where
    R::GpuPlanarType: GpuPlanarStorage,
{
    pub bind_group_layout: bevy::render::render_resource::BindGroupLayout,
    pub phantom: PhantomData<fn() -> R>,
}

impl<R: PlanarSync> FromWorld for PlanarStorageLayouts<R>
where
    R::GpuPlanarType: GpuPlanarStorage,
{
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<bevy::render::renderer::RenderDevice>();

        let read_only = true;
        let bind_group_layout = R::GpuPlanarType::bind_group_layout(render_device, read_only);

        Self {
            bind_group_layout,
            phantom: PhantomData,
        }
    }
}

#[derive(bevy::prelude::Component, Clone, Debug)]
pub struct PlanarStorageBindGroup<R: PlanarSync> {
    pub bind_group: bevy::render::render_resource::BindGroup,
    pub phantom: PhantomData<fn() -> R>,
}

fn extract_planar_handles<R>(
    mut commands: Commands,
    mut main_world: ResMut<bevy::render::MainWorld>,
) where
    R: PlanarSync + Default + Clone + Reflect,
    R::PlanarType: Asset,
    R::GpuPlanarType: GpuPlanarStorage,
{
    let mut planar_handles_query =
        main_world.query::<(bevy::render::sync_world::RenderEntity, &R::PlanarTypeHandle)>();

    for (entity, planar_handle) in planar_handles_query.iter(&main_world) {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(planar_handle.clone());
        }
    }
}

fn queue_gpu_storage_buffers<R>(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    render_device: ResMut<bevy::render::renderer::RenderDevice>,
    gpu_planars: Res<bevy::render::render_asset::RenderAssets<R::GpuPlanarType>>,
    bind_group_layout: Res<PlanarStorageLayouts<R>>,
    clouds: Query<
        (bevy::prelude::Entity, &R::PlanarTypeHandle),
        Without<PlanarStorageBindGroup<R>>,
    >,
) where
    R: PlanarSync + Default + Clone + Reflect,
    R::PlanarType: Asset,
    R::GpuPlanarType: GpuPlanarStorage,
{
    let layout = &bind_group_layout.bind_group_layout;

    for (entity, planar_handle) in clouds.iter() {
        if let Some(load_state) = asset_server.get_load_state(planar_handle.handle())
            && load_state.is_loading()
        {
            continue;
        }

        if gpu_planars.get(planar_handle.handle()).is_none() {
            continue;
        }

        let gpu_planar: &<R as PlanarSync>::GpuPlanarType =
            gpu_planars.get(planar_handle.handle()).unwrap();
        let bind_group = gpu_planar.bind_group(&render_device, layout);

        commands.entity(entity).insert(PlanarStorageBindGroup::<R> {
            bind_group,
            phantom: PhantomData,
        });
    }
}
