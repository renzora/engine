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
    scanned: Option<std::time::Instant>,
    root: Option<std::path::PathBuf>,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<ProjectModels>().add_systems(
        Update,
        scan_project.run_if(in_state(renzora_editor_framework::SplashState::Editor)).run_if(super::stage_mounted),
    );
}

/// Folders that hold build output or editor state, never a model to animate.
const SKIP: [&str; 6] = ["target", "dist", "node_modules", "build", "exports", ".renzora"];

/// Re-read the project's model files every few seconds, so a model imported
/// while the workspace is open shows up without anything to press.
fn scan_project(mut models: ResMut<ProjectModels>, project: Option<Res<renzora::core::CurrentProject>>) {
    let Some(project) = project else { return };
    let fresh = models.root.as_deref() == Some(project.path.as_path()) && models.scanned.is_some_and(|t| t.elapsed().as_secs_f32() < 4.0);
    if fresh {
        return;
    }
    let mut files = Vec::new();
    let mut stack = vec![project.path.clone()];
    // Bounded, so a project folder with a huge tree in it never stalls a frame.
    let mut visited = 0;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > 4000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if !name.starts_with('.') && !SKIP.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            let lower = name.to_lowercase();
            // The original a rig was built from is kept beside the model;
            // it is the way back, not a second model.
            if (lower.ends_with(".glb") || lower.ends_with(".gltf")) && !lower.ends_with(".unrigged.glb") {
                if let Ok(rel) = path.strip_prefix(&project.path) {
                    files.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    files.sort_by_key(|f| file_name(f).to_lowercase());
    models.files = files;
    models.scanned = Some(std::time::Instant::now());
    models.root = Some(project.path.clone());
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
