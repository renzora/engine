use bevy::prelude::*;
use bevy_interleave::prelude::*;

use crate::{
    gaussian::{
        formats::{planar_3d::Gaussian3d, planar_4d::Gaussian4d},
        interface::CommonCloud,
    },
    io::codec::CloudCodec,
};

#[derive(Component, Debug, Default, Reflect)]
pub struct Select {
    pub indicies: Vec<usize>,
    pub completed: bool,
}

impl FromIterator<usize> for Select {
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let indicies = iter.into_iter().collect::<Vec<usize>>();
        Select {
            indicies,
            ..Default::default()
        }
    }
}

impl Select {
    pub fn invert(&mut self, cloud_size: usize) -> Select {
        let inverted = (0..cloud_size)
            .filter(|index| !self.indicies.contains(index))
            .collect::<Vec<usize>>();

        Select {
            indicies: inverted,
            completed: self.completed,
        }
    }
}

#[derive(Default)]
pub struct SelectPlugin;

impl Plugin for SelectPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Select>();

        app.add_message::<InvertSelectionEvent>();
        app.add_message::<SaveSelectionEvent>();

        app.add_plugins(CommonCloudSelectPlugin::<Gaussian3d>::default());
        app.add_plugins(CommonCloudSelectPlugin::<Gaussian4d>::default());
    }
}

#[derive(Default)]
pub struct CommonCloudSelectPlugin<R: PlanarSync>
where
    R::PlanarType: CommonCloud,
{
    _phantom: std::marker::PhantomData<R>,
}

impl<R: PlanarSync> Plugin for CommonCloudSelectPlugin<R>
where
    R::PlanarType: CloudCodec,
    R::PlanarType: CommonCloud,
{
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                apply_selection::<R>,
                invert_selection::<R>,
                save_selection::<R>,
            ),
        );
    }
}

fn apply_selection<R: PlanarSync>(
    asset_server: Res<AssetServer>,
    mut gaussian_clouds_res: ResMut<Assets<R::PlanarType>>,
    mut selections: Query<(Entity, &R::PlanarTypeHandle, &mut Select)>,
) where
    R::PlanarType: CommonCloud,
{
    for (_entity, cloud_handle, mut select) in selections.iter_mut() {
        if select.indicies.is_empty() || select.completed {
            continue;
        }

        if let Some(load_state) = asset_server.get_load_state(cloud_handle.handle())
            && load_state.is_loading()
        {
            continue;
        }

        let mut cloud = gaussian_clouds_res.get_mut(cloud_handle.handle()).unwrap();

        (0..cloud.len()).for_each(|index| {
            *cloud.visibility_mut(index) = 0.0;
        });

        select.indicies.iter().for_each(|index| {
            *cloud.visibility_mut(*index) = 1.0;
        });

        select.completed = true;
    }
}

#[derive(Message, Debug, Reflect)]
pub struct InvertSelectionEvent;

fn invert_selection<R: PlanarSync>(
    mut events: MessageReader<InvertSelectionEvent>,
    mut gaussian_clouds_res: ResMut<Assets<R::PlanarType>>,
    mut selections: Query<(Entity, &R::PlanarTypeHandle, &mut Select)>,
) where
    R::PlanarType: CommonCloud,
{
    if events.is_empty() {
        return;
    }
    events.clear();

    for (_entity, cloud_handle, mut select) in selections.iter_mut() {
        if select.indicies.is_empty() {
            continue;
        }

        let mut cloud = gaussian_clouds_res.get_mut(cloud_handle.handle()).unwrap();

        let mut new_indicies = Vec::with_capacity(cloud.len() - select.indicies.len());

        (0..cloud.len()).for_each(|index| {
            if cloud.visibility(index) == 0.0 {
                new_indicies.push(index);
            }

            *cloud.visibility_mut(index) = 1.0;
        });

        select.indicies.iter().for_each(|index| {
            *cloud.visibility_mut(*index) = 0.0;
        });

        select.indicies = new_indicies;
    }
}

#[derive(Message, Debug, Reflect)]
pub struct SaveSelectionEvent;

pub fn save_selection<R: PlanarSync>(
    mut events: MessageReader<SaveSelectionEvent>,
    mut gaussian_clouds_res: ResMut<Assets<R::PlanarType>>,
    mut selections: Query<(Entity, &R::PlanarTypeHandle, &Select)>,
) where
    R::PlanarType: CloudCodec,
    R::PlanarType: CommonCloud,
{
    if events.is_empty() {
        return;
    }
    events.clear();

    for (_entity, cloud_handle, select) in selections.iter_mut() {
        let cloud = gaussian_clouds_res.get_mut(cloud_handle.handle()).unwrap();

        let selected = cloud.subset(select.indicies.as_slice());

        selected.write_to_file("live_output.gcloud");
    }
}
