// use bevy::prelude::*;
// use bevy::render::{
//     render_graph::{Node, NodeRunError, RenderGraphContext, RenderLabel},
//     renderer::RenderContext,
//     RenderApp,
// };
// use bevy::asset::LoadState;
// use bevy::core_pipeline::core_3d::graph::Core3d;

// use crate::{
//     gaussian::cloud::PlanarGaussian3dHandle,
//     render::GpuCloud,
//     CloudSettings,
// };

// #[derive(Component, Default,)]
// pub struct GaussianSliceData {
//     pub data: Vec<f32>,
//     pub changed_start: usize,
//     pub changed_count: usize,
// }

// impl GaussianSliceData {
//     pub fn mark_changed(&mut self, start: usize, count: usize) {
//         self.changed_start = start;
//         self.changed_count = count;
//     }

//     pub fn clear_changed(&mut self) {
//         self.changed_start = 0;
//         self.changed_count = 0;
//     }

//     pub fn has_changed(&self) -> bool {
//         self.changed_count > 0
//     }

//     pub fn changed_slice(&self) -> &[u8] {
//         let end = self.changed_start + self.changed_count;
//         bytemuck::cast_slice(&self.data[self.changed_start..end])
//     }
// }

// #[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
// pub struct GaussianSliceLabel;

// pub struct GaussianSlicePlugin;

// impl Plugin for GaussianSlicePlugin {
//     fn build(&self, app: &mut App) {
//         if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
//             render_app
//                 .add_render_graph_node::<GaussianSliceNode>(
//                     Core3d,
//                     GaussianSliceLabel,
//                 )
//                 .add_render_graph_edge(
//                     Core3d,
//                     GaussianSliceLabel,
//                     crate::sort::GaussianSliceLabel,
//                 );
//         }
//     }
// }

// pub struct GaussianSliceNode {
//     query_gaussian: QueryState<(&'static PlanarGaussian3dHandle, &'static CloudSettings)>,
//     initialized: bool,
// }

// impl FromWorld for GaussianSliceNode {
//     fn from_world(world: &mut World) -> Self {
//         GaussianSliceNode {
//             query_gaussian: world.query(),
//             initialized: true,
//         }
//     }
// }

// impl Node for GaussianSliceNode {
//     fn update(&mut self, world: &mut World) {
//         self.query_gaussian.update_archetypes(world);
//     }

//     fn run(
//         &self,
//         _graph: &mut RenderGraphContext,
//         render_context: &mut RenderContext,
//         world: &World,
//     ) -> Result<(), NodeRunError> {
//         if !self.initialized {
//             return Ok(());
//         }

//         let slice_data = world.get_resource::<GaussianSliceData>();
//         if slice_data.is_none() || !slice_data.unwrap().has_changed() {
//             return Ok(());
//         }
//         let slice_data = slice_data.unwrap();

//         let gaussian_clouds = world.get_resource::<RenderAssets<GpuCloud>>().ok_or(NodeRunError::MissingResource)?;
//         let asset_server = world.get_resource::<AssetServer>().ok_or(NodeRunError::MissingResource)?;

//         for (cloud_handle, settings) in self.query_gaussian.iter_manual(world) {
//             if Some(LoadState::Loaded) != asset_server.get_load_state(cloud_handle) {
//                 continue;
//             }

//             let gpu_cloud = if let Some(g) = gaussian_clouds.get(cloud_handle) {
//                 g
//             } else {
//                 continue;
//             };

//             let queue = &render_context.queue;
//             let offset_bytes = slice_data.changed_start * std::mem::size_of::<f32>();
//             queue.write_buffer(&gpu_cloud.gaussian_buffer, offset_bytes as u64, slice_data.changed_slice());
//         }

//         Ok(())
//     }
// }
