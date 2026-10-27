//! Converting a project's `.glb` models into `.rvg` models.
//!
//! Imports are stored as `.rvg` now: one file holding the model's scene and
//! its virtual geometry. A project from before that holds each model twice,
//! a `.glb` plus the sidecar `.rvg` the converter keeps beside it. Both keep
//! working, so nothing here runs on its own: when a project with `.glb`
//! models opens, the Console says so, and "Convert Models to RVG" in the
//! command palette does it after a confirmation.
//!
//! Each model is built into an `.rvg` of the same name (replacing its
//! sidecar), then an `AssetPathChanged` from the `.glb` to the `.rvg` is
//! fired, which is what a rename in the asset browser fires: every scene,
//! material and script that names the model is rewritten, and so is the open
//! scene in memory. Only then is the `.glb` deleted, so a model whose
//! conversion fails is left exactly as it was.
//!
//! The sidecar converter is paused while this runs. It writes `.rvg` files of
//! the same names, and a sidecar landing on a model a moment after it was
//! converted would leave the model with clusters and no scene.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use renzora::console_log::{console_info, console_warn};
use renzora::core::project_files::is_ignored;
use renzora::core::{AssetPathChanged, ConvertModelsRequested, CurrentProject};
use renzora_ember::font::EmberFonts;
use renzora_ember::UiInteraction;
use renzora_rvg::bake::{convert_to_model, find_models, Converted};

use crate::converter::Converter;

/// Where the console messages go.
const CATEGORY: &str = "Assets";

#[derive(Resource, Default)]
pub(crate) struct Migration {
    root: Option<PathBuf>,
    /// A look for `.glb` models, and whether the user asked for it (rather
    /// than it being the look taken when the project opened).
    scan: Option<(Task<Vec<PathBuf>>, bool)>,
    /// The confirmation on screen, and the models it is asking about.
    dialog: Option<(Entity, Vec<PathBuf>)>,
    queue: VecDeque<PathBuf>,
    running: Option<(PathBuf, Task<Result<Converted, String>>)>,
    tally: Tally,
}

#[derive(Default)]
struct Tally {
    converted: usize,
    kept: usize,
    bytes: u64,
    before: u64,
}

impl Migration {
    /// Whether models are being converted, during which the sidecar
    /// converter waits.
    pub(crate) fn busy(&self) -> bool {
        !self.queue.is_empty() || self.running.is_some()
    }
}

#[derive(Component)]
struct ConvertCancel;

#[derive(Component)]
struct ConvertConfirm;

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Migration>().add_systems(Update, migrate);
}

/// The models stored as `.glb` under `root`.
///
/// `exports/` is left out: it is where "Export with Clips" writes a `.glb` for
/// use outside the engine, and that one is wanted as a `.glb`.
fn glb_models(root: &Path) -> Vec<PathBuf> {
    find_models(root, |relative| {
        is_ignored(relative) || relative == "exports" || relative.starts_with("exports/")
    })
}

fn scan(root: PathBuf) -> Task<Vec<PathBuf>> {
    AsyncComputeTaskPool::get().spawn(async move { glb_models(&root) })
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

/// One pass of everything: following the project, the request, the
/// confirmation, and the conversion itself. Exclusive because finishing a
/// model fires `AssetPathChanged`, whose subscribers rewrite the open scene.
fn migrate(world: &mut World) {
    let root = world.get_resource::<CurrentProject>().map(|p| p.path.clone());
    let asked = world.remove_resource::<ConvertModelsRequested>().is_some();
    let mut m = world.resource_mut::<Migration>();

    if m.root != root {
        // A different project: the old one's list means nothing here. A model
        // already being built finishes writing its file, but its `.glb` is
        // kept, and converting again later picks it up as done.
        m.queue.clear();
        m.running = None;
        m.dialog = None;
        m.tally = Tally::default();
        m.root = root.clone();
        m.scan = root.clone().map(|r| (scan(r), false));
    }
    let Some(root) = root else { return };

    if asked {
        if m.busy() || m.dialog.is_some() {
            console_info(CATEGORY, "Models are already being converted to .rvg");
        } else {
            m.scan = Some((scan(root.clone()), true));
        }
    }

    finish_scan(world, &root);
    answer_dialog(world);
    pump(world, &root);
}

fn finish_scan(world: &mut World, root: &Path) {
    let mut m = world.resource_mut::<Migration>();
    let Some((task, asked)) = m.scan.as_mut() else { return };
    let Some(models) = block_on(future::poll_once(task)) else { return };
    let asked = *asked;
    m.scan = None;

    if models.is_empty() {
        if asked {
            console_info(CATEGORY, "Every model in this project is already stored as .rvg");
        }
        return;
    }
    if !asked {
        console_info(
            CATEGORY,
            format!(
                "{} model(s) in this project are stored as .glb, with their virtual geometry in a second file. \
                 Run \"Convert Models to RVG\" from the command palette (Ctrl+P) to store each as one .rvg.",
                models.len()
            ),
        );
        return;
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let listed: Vec<String> = models
        .iter()
        .take(6)
        .map(|p| p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/"))
        .collect();
    let more = models.len().saturating_sub(listed.len());
    let mut body = format!(
        "Convert {} model(s) from .glb to .rvg?\n\n{}{}\n\n\
         Scenes, materials and scripts that use them are updated to the new path, \
         and each .glb is deleted once its .rvg is written.",
        models.len(),
        listed.join("\n"),
        if more > 0 { format!("\nand {more} more") } else { String::new() },
    );
    if models.len() > 10 {
        body.push_str(" Large models take a few seconds each.");
    }
    let mut commands = world.commands();
    let (dialog, buttons) =
        renzora_ember::widgets::confirm_dialog(&mut commands, &fonts, "Convert Models to RVG", body, 480.0, 300.0, &["Cancel", "Convert"]);
    if let [cancel, confirm] = buttons[..] {
        commands.entity(cancel).insert(ConvertCancel);
        commands.entity(confirm).insert(ConvertConfirm);
    }
    world.flush();
    world.resource_mut::<Migration>().dialog = Some((dialog, models));
}

/// Start on a confirmation; forget the list on anything else. Escape, a
/// backdrop click and the title bar's × despawn the dialog without pressing
/// a button, which is a no.
fn answer_dialog(world: &mut World) {
    let Some(dialog) = world.resource::<Migration>().dialog.as_ref().map(|(e, _)| *e) else { return };
    let confirm = pressed::<ConvertConfirm>(world);
    let cancel = pressed::<ConvertCancel>(world);
    let gone = world.get_entity(dialog).is_err();
    if !confirm && !cancel && !gone {
        return;
    }
    if let Ok(e) = world.get_entity_mut(dialog) {
        e.despawn();
    }
    let Some((_, models)) = world.resource_mut::<Migration>().dialog.take() else { return };
    if !confirm {
        return;
    }
    // What the sidecar converter had queued is these same models.
    world.resource_mut::<Converter>().clear_queue();
    let mut m = world.resource_mut::<Migration>();
    m.tally = Tally::default();
    m.queue = models.into();
    console_info(CATEGORY, format!("Converting {} model(s) to .rvg", m.queue.len()));
}

fn pressed<T: Component>(world: &mut World) -> bool {
    world
        .query_filtered::<&UiInteraction, With<T>>()
        .iter(world)
        .any(|i| *i == UiInteraction::Pressed)
}

/// Collect a finished model and start the next, once the sidecar converter
/// has nothing in flight.
fn pump(world: &mut World, root: &Path) {
    let finished = {
        let mut m = world.resource_mut::<Migration>();
        match m.running.as_mut() {
            Some((_, task)) => match block_on(future::poll_once(task)) {
                Some(result) => m.running.take().map(|(glb, _)| (glb, result)),
                None => return,
            },
            None => None,
        }
    };
    if let Some((glb, result)) = finished {
        finish(world, root, &glb, result);
    }

    if world.resource::<Converter>().is_running() {
        return;
    }
    let mut m = world.resource_mut::<Migration>();
    let Some(next) = m.queue.pop_front() else { return };
    let path = next.clone();
    m.running = Some((next, AsyncComputeTaskPool::get().spawn(async move { convert_to_model(&path) })));
}

fn finish(world: &mut World, root: &Path, glb: &Path, result: Result<Converted, String>) {
    let name = glb.strip_prefix(root).unwrap_or(glb).to_string_lossy().replace('\\', "/");
    let (rvg, sizes) = match result {
        Ok(Converted::Written { rvg, bytes, before }) => (rvg, Some((bytes, before))),
        Ok(Converted::AlreadyConverted { rvg }) => (rvg, None),
        Ok(Converted::NameTaken { rvg }) => {
            console_warn(
                CATEGORY,
                format!(
                    "Kept {name}: {} is already a different model. Rename one of them and convert again.",
                    rvg.strip_prefix(root).unwrap_or(&rvg).display()
                ),
            );
            world.resource_mut::<Migration>().tally.kept += 1;
            report_if_done(world);
            return;
        }
        Err(e) => {
            console_warn(CATEGORY, format!("Kept {name} as .glb: {e}"));
            world.resource_mut::<Migration>().tally.kept += 1;
            report_if_done(world);
            return;
        }
    };

    // References first, while the `.glb` is still there to be found, then the
    // file itself.
    if let Some(project) = world.get_resource::<CurrentProject>() {
        let old = project.make_asset_relative(glb);
        let new = project.make_asset_relative(&rvg);
        world.trigger(AssetPathChanged { old, new, is_dir: false });
        world.flush();
    }
    if let Err(e) = std::fs::remove_file(glb) {
        console_warn(CATEGORY, format!("Converted {name}, but could not delete the .glb: {e}"));
    }
    let mut m = world.resource_mut::<Migration>();
    m.tally.converted += 1;
    if let Some((bytes, before)) = sizes {
        m.tally.bytes += bytes;
        m.tally.before += before;
    }
    report_if_done(world);
}

fn report_if_done(world: &mut World) {
    let m = world.resource::<Migration>();
    if m.busy() {
        return;
    }
    let Tally { converted, kept, bytes, before } = m.tally;
    let mut message = format!("Converted {converted} model(s) to .rvg");
    if before > 0 {
        message.push_str(&format!(" ({} on disk, was {})", megabytes(bytes), megabytes(before)));
    }
    if kept > 0 {
        message.push_str(&format!("; {kept} kept as .glb, see above"));
    }
    console_info(CATEGORY, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every model `.glb` is found except the ones written for use outside
    /// the engine, and the editor's own folders.
    #[test]
    fn exports_stay_glb() {
        let root = std::env::temp_dir().join(format!("renzora_vg_migrate_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["models/hero", "exports", ".renzora", "models/exports_old"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in ["models/hero/hero.glb", "exports/hero.glb", ".renzora/cache.glb", "models/exports_old/old.glb", "models/hero/hero.rvg"] {
            std::fs::write(root.join(file), b"").unwrap();
        }
        let found: Vec<String> = glb_models(&root)
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(found, ["models/exports_old/old.glb", "models/hero/hero.glb"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
