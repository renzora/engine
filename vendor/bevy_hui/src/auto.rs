use crate::prelude::*;
use bevy::{
    asset::{LoadState, LoadedFolder},
    prelude::*,
};

/// Add folders to autoload.
/// Any template will register as a component with filename.
/// On startup, the folder has to load first and is not available
/// you will have to check the `AutoLoadState`.
#[derive(Resource, Clone)]
pub struct HuiAutoLoadPlugin {
    auto_load_dirs: Vec<&'static str>,
    folders: Vec<Handle<LoadedFolder>>,
}

impl HuiAutoLoadPlugin {
    /// Paths start at your project root. Does not have to be part
    /// of bevy assets folder.
    pub fn new(dirs: &[&'static str]) -> Self {
        Self {
            auto_load_dirs: dirs.to_vec(),
            folders: vec![],
        }
    }
}

#[derive(States, Debug, Hash, PartialEq, Eq, Clone, Default)]
pub enum AutoLoadState {
    #[default]
    Loading,
    Finished,
}

impl Plugin for HuiAutoLoadPlugin {
    fn build(&self, app: &mut App) {
        let server = app.world().resource::<AssetServer>();
        let mut cfg = self.clone();

        for dir in self.auto_load_dirs.iter() {
            let h = server.load_folder(dir.to_owned());
            cfg.folders.push(h);
        }

        app.init_state::<AutoLoadState>();
        app.insert_resource(cfg);
        app.add_systems(
            Update,
            check_loading_state.run_if(in_state(AutoLoadState::Loading)),
        );
        app.add_systems(First, watch_autoload_dirs);
    }
}

fn check_loading_state(
    config: Res<HuiAutoLoadPlugin>,
    mut next: ResMut<NextState<AutoLoadState>>,
    server: Res<AssetServer>,
) {
    if config.folders.iter().all(|h| {
        std::mem::discriminant(&server.load_state(h)) == std::mem::discriminant(&LoadState::Loaded)
    }) {
        next.set(AutoLoadState::Finished);
    }
}

fn watch_autoload_dirs(
    mut events: MessageReader<AssetEvent<LoadedFolder>>,
    config: Res<HuiAutoLoadPlugin>,
    mut comps: HtmlComponents,
    folders: Res<Assets<LoadedFolder>>,
) {
    let Some(AssetEvent::LoadedWithDependencies { id }) = events.read().next() else {
        return;
    };

    for folder_handle in config.folders.iter() {
        if folder_handle.id() != *id {
            continue;
        }

        let Some(folder) = folders.get(folder_handle) else {
            warn!("auto load folder does not exists");
            continue;
        };

        for file_handle in folder.handles.iter().cloned() {
            match file_handle.try_typed::<HtmlTemplate>() {
                Ok(template) => {
                    let Some(name) = template
                        .path()
                        .map(|p| p.path().file_stem().unwrap_or_default())
                        .map(|s| s.to_string_lossy().to_string())
                    else {
                        warn!("template has no readable name");
                        continue;
                    };

                    comps.register(name.to_string(), template);
                    info!("registered HTML-component `{name}`");
                }
                Err(_) => {}
            }
        }
    }
}
