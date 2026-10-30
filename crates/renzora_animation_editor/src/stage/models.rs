//! Which model is on the stage, and choosing another.
//!
//! The stage works on a model in the open scene (that is what the clip keys
//! and what plays in the game), but needing one there first meant finding it
//! in a hierarchy the workspace does not show, or leaving the workspace to
//! drop it in. So the choices are every model in the scene and every model
//! file in the project: picking a file that has no copy in the scene adds one
//! at the origin, as an undoable step, and puts it on the stage.

use bevy::prelude::*;

/// A model the stage could show.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Choice {
    /// A model already in the scene.
    Instance(Entity, String),
    /// A model file in the project with no copy in the scene yet.
    File(String),
}

impl Choice {
    pub(crate) fn label(&self) -> String {
        match self {
            Choice::Instance(_, name) => name.clone(),
            // With its folder: two files of the same name are common (a model
            // and its export, a copy per level), and the name alone cannot tell
            // them apart.
            Choice::File(rel) => match rel.rsplit_once('/') {
                Some((dir, file)) => format!("+ {file}  ({dir})"),
                None => format!("+ {rel}"),
            },
        }
    }
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// The project's model files, project-relative with forward slashes.
#[derive(Resource, Default)]
pub(crate) struct ProjectModels {
    pub(crate) files: Vec<String>,
    /// The project index's revision these were read at.
    revision: Option<u64>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<ProjectModels>().add_systems(
        Update,
        read_models.run_if(in_state(renzora_editor_framework::SplashState::Editor)).run_if(super::stage_mounted),
    );
}

/// Folders that hold build output, never a model to animate. The index leaves
/// out what the watcher ignores already (`target`, `.renzora`, hidden folders);
/// these are copies of models, not models. `terrain_clusters` is the virtual
/// geometry the editor builds for terrain chunks, one `.rvg` per chunk (the
/// asset browser hides it too): a terrain put thousands of them in this list.
const SKIP: [&str; 4] = ["dist", "build", "exports", "terrain_clusters"];

/// The project's model files, from the project index (see
/// `renzora::core::project_index`), whenever it changes: a model imported
/// while the workspace is open shows up without anything to press.
///
/// This walked the project itself every four seconds, on the main thread:
/// 79 ms in a project with a model library, the worst frame a profile of the
/// editor showed. The index is kept current by the watcher, so reading it is
/// a filter over a map and nobody walks a folder.
fn read_models(mut models: ResMut<ProjectModels>, index: Option<Res<renzora::core::ProjectIndex>>) {
    let Some(index) = index else { return };
    if models.revision == Some(index.revision()) {
        return;
    }
    let mut files: Vec<String> = index
        .with_extensions(&["rvg", "glb", "gltf"])
        .map(|f| f.path.as_str())
        .filter(|p| !p.split('/').any(|part| SKIP.contains(&part)))
        // The original a rig was built from is kept beside the model; it is
        // the way back, not a second model.
        .filter(|p| {
            let lower = p.to_lowercase();
            ![".unrigged.glb", ".unrigged.rvg", ".unshaped.glb", ".unshaped.rvg"].iter().any(|s| lower.ends_with(s))
        })
        .map(str::to_string)
        .collect();
    files.sort_by_key(|f| file_name(f).to_lowercase());
    models.files = files;
    models.revision = Some(index.revision());
}

/// Every model in the scene, then every model file with no copy in it.
pub(crate) fn choices(world: &World) -> Vec<Choice> {
    let mut instances: Vec<(Entity, String, String)> = Vec::new();
    if let Some(mut q) = world.try_query_filtered::<(Entity, &renzora::core::MeshInstanceData, Option<&Name>), Without<renzora::core::HideInHierarchy>>() {
        for (e, data, name) in q.iter(world) {
            let Some(path) = data.model_path.clone() else { continue };
            instances.push((e, name.map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("{e}")), path.replace('\\', "/")));
        }
    }
    instances.sort_by(|a, b| a.1.cmp(&b.1));
    let mut out: Vec<Choice> = instances.iter().map(|(e, n, _)| Choice::Instance(*e, n.clone())).collect();
    if let Some(models) = world.get_resource::<ProjectModels>() {
        for f in &models.files {
            if !instances.iter().any(|(_, _, p)| p == f) {
                out.push(Choice::File(f.clone()));
            }
        }
    }
    out
}

/// Put a choice on the stage: select it, adding it to the scene first when
/// it is a file.
pub(crate) fn choose(world: &mut World, choice: &Choice) {
    let entity = match choice {
        Choice::Instance(e, _) => *e,
        Choice::File(rel) => {
            let mut cmd = AddModel { entity: Entity::PLACEHOLDER, rel: rel.clone() };
            renzora_undo::UndoCommand::execute(&mut cmd, world);
            let e = cmd.entity;
            renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
            e
        }
    };
    if let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() {
        sel.set(Some(entity));
    }
}

/// Adding a model file to the scene, at the origin: a bare instance the
/// engine's rehydrate step fills in, the same path a saved scene takes when
/// it loads.
struct AddModel {
    entity: Entity,
    rel: String,
}

impl renzora_undo::UndoCommand for AddModel {
    fn label(&self) -> &str {
        "Add Model"
    }
    fn execute(&mut self, world: &mut World) {
        let stem = std::path::Path::new(&self.rel).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "model".into());
        let e = world
            .spawn((Transform::default(), Visibility::default(), renzora::core::MeshInstanceData { model_path: Some(self.rel.clone()) }))
            .id();
        let name = renzora::unique_entity_name(world, &stem, e);
        world.entity_mut(e).insert(Name::new(name));
        self.entity = e;
    }
    fn undo(&mut self, world: &mut World) {
        if let Ok(e) = world.get_entity_mut(self.entity) {
            e.despawn();
        }
    }
}
