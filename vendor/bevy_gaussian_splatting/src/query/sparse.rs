use bevy::prelude::*;
use bevy_interleave::prelude::PlanarHandle;
use kd_tree::{KdPoint, KdTree};
use typenum::consts::U3;

use crate::{
    PlanarGaussian3d, PlanarGaussian3dHandle, gaussian::interface::CommonCloud,
    query::select::Select,
};

#[derive(Clone, Copy)]
struct PositionPoint([f32; 3]);

impl KdPoint for PositionPoint {
    type Scalar = f32;
    type Dim = U3;

    fn at(&self, i: usize) -> Self::Scalar {
        self.0[i]
    }
}

#[derive(Component, Debug, Reflect)]
pub struct SparseSelect {
    pub radius: f32,
    pub neighbor_threshold: usize,
    pub completed: bool,
}

impl Default for SparseSelect {
    fn default() -> Self {
        Self {
            radius: 0.05,
            neighbor_threshold: 3,
            completed: false,
        }
    }
}

impl SparseSelect {
    pub fn select(&self, cloud: &PlanarGaussian3d) -> Select {
        let points = collect_points(cloud);
        let tree = KdTree::build_by_ordered_float(points.clone());

        points
            .iter()
            .enumerate()
            .filter(|(_idx, point)| {
                tree.within_radius(*point, self.radius).len() < self.neighbor_threshold
            })
            .map(|(idx, _point)| idx)
            .collect::<Select>()
    }
}

#[derive(Default)]
pub struct SparsePlugin;

impl Plugin for SparsePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SparseSelect>();
        app.add_systems(Update, select_sparse_handler);
    }
}

fn collect_points(cloud: &PlanarGaussian3d) -> Vec<PositionPoint> {
    cloud
        .position_iter()
        .map(|position| PositionPoint([position[0], position[1], position[2]]))
        .collect()
}

fn select_sparse_handler(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    gaussian_clouds_res: Res<Assets<PlanarGaussian3d>>,
    mut selections: Query<(Entity, &PlanarGaussian3dHandle, &mut SparseSelect)>,
) {
    for (entity, cloud_handle, mut select) in selections.iter_mut() {
        if let Some(load_state) = asset_server.get_load_state(cloud_handle.handle())
            && load_state.is_loading()
        {
            continue;
        }

        if select.completed {
            continue;
        }
        select.completed = true;

        let Some(cloud) = gaussian_clouds_res.get(cloud_handle.handle()) else {
            continue;
        };

        let points = collect_points(cloud);
        let tree = KdTree::build_by_ordered_float(points.clone());

        let new_selection = points
            .iter()
            .enumerate()
            .filter(|(_idx, point)| {
                tree.within_radius(*point, select.radius).len() < select.neighbor_threshold
            })
            .map(|(idx, _point)| idx)
            .collect::<Select>();

        commands
            .entity(entity)
            .remove::<Select>()
            .insert(new_selection);
    }
}
