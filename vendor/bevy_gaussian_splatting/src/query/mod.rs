use bevy::prelude::*;

#[cfg(feature = "query_raycast")]
pub mod raycast;

#[cfg(feature = "query_select")]
pub mod select;

#[cfg(feature = "query_sparse")]
pub mod sparse;

#[derive(Default)]
pub struct QueryPlugin;

impl Plugin for QueryPlugin {
    #[allow(unused)]
    fn build(&self, app: &mut App) {
        #[cfg(feature = "query_raycast")]
        app.add_plugins(raycast::RaycastSelectionPlugin);

        #[cfg(feature = "query_select")]
        app.add_plugins(select::SelectPlugin);

        #[cfg(feature = "query_sparse")]
        app.add_plugins(sparse::SparsePlugin);
    }
}
