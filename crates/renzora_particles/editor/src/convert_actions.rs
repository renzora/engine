//! The editor's ways into the converter ([`crate::convert`]).
//!
//! - **Assets panel, on a `.particle`:** "Convert to new particle system"
//!   writes the `.vfx` beside it and opens it in the effect editor.
//! - **Assets panel, on a folder:** "Convert all .particle files here", the same
//!   for every `.particle` directly in it, without opening each.
//! - **The Particle System's Effect slot:** a dropped `.particle` is converted
//!   and the `.vfx` assigned in its place ([`effect_for_dropped_path`]).
//!
//! Every route reports the same way: one console line per file, a console
//! warning per thing that did not carry over, and a toast saying what
//! happened. All of them name and place files by [`crate::convert::write_beside`],
//! which never overwrites or deletes anything.
//!
//! These are the explicit routes. Opening a project converts every `.particle`
//! that has no `.vfx` yet on its own ([`crate::legacy`]), so most users never
//! need them; they remain for a file added later, or for a fresh copy of one
//! whose `.vfx` has since been edited.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use renzora::core::CurrentProject;
use renzora::{AssetAction, AssetActionScope, RegisterAssetAction};
use renzora_editor_framework::{DocTabKind, Toasts};

use crate::convert::{self, Converted, Outcome};

const CATEGORY: &str = "Particles";

pub fn register(app: &mut App) {
    app.register_asset_action(
        AssetAction::new("convert_particle", AssetActionScope::File, &["particle"], convert_file_and_open)
            .label("assets.action.convert_particle", "Convert to new particle system")
            .icon("magic-wand"),
    );
    app.register_asset_action(
        AssetAction::new("convert_particles_folder", AssetActionScope::Folder, &["particle"], convert_folder)
            .label("assets.action.convert_particles_folder", "Convert all .particle files here")
            .icon("magic-wand"),
    );
}

fn file_name(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("effect").to_string()
}

/// Console lines for one conversion: what was written (or reused) and each
/// warning.
pub(crate) fn log(source: &str, converted: &Converted) {
    let target = file_name(&converted.path);
    let verb = match converted.outcome {
        Outcome::Written => "converted to",
        Outcome::Reused => "already converted as",
    };
    let line = format!("{source} {verb} {target}");
    if converted.warnings.is_empty() {
        renzora::console_log::console_success(CATEGORY, line);
    } else {
        renzora::console_log::console_info(CATEGORY, format!("{line}, with {} warning(s):", converted.warnings.len()));
        for w in &converted.warnings {
            renzora::console_log::console_warn(CATEGORY, format!("{source}: {w}"));
        }
    }
}

fn toast(world: &mut World, warn: bool, message: String) {
    if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
        if warn {
            toasts.warning(message);
        } else {
            toasts.success(message);
        }
    }
}

/// One file's toast: the new name, and a pointer to the console when
/// something did not carry over.
fn toast_one(world: &mut World, converted: &Converted) {
    let name = file_name(&converted.path);
    let n = converted.warnings.len();
    if n == 0 {
        toast(world, false, format!("Converted to {name}"));
    } else {
        toast(world, true, format!("Converted to {name}; {n} thing(s) did not carry over (see the Console)"));
    }
}

/// Open a converted file in the effect editor.
fn open(world: &mut World, path: &Path) {
    renzora_editor_framework::open_asset_tab(world, path, DocTabKind::Particle);
}

/// The Assets panel's "Convert to new particle system".
pub fn convert_file_and_open(world: &mut World, particle: &Path) {
    match convert::convert_beside(particle) {
        Ok(converted) => {
            log(&file_name(particle), &converted);
            toast_one(world, &converted);
            open(world, &converted.path);
        }
        Err(e) => {
            renzora::console_log::console_error(CATEGORY, format!("{}: {e}", file_name(particle)));
            toast(world, true, format!("Could not convert {}", file_name(particle)));
        }
    }
}

/// The Assets panel's "Convert all .particle files here". Not recursive: the
/// action is offered on the folder it says, and a project's whole tree in one
/// click is what the `convert_particles` example is for.
pub fn convert_folder(world: &mut World, dir: &Path) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("particle")))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let (mut written, mut reused, mut warned, mut failed) = (0, 0, 0, 0);
    for f in &files {
        match convert::convert_beside(f) {
            Ok(c) => {
                log(&file_name(f), &c);
                match c.outcome {
                    Outcome::Written => written += 1,
                    Outcome::Reused => reused += 1,
                }
                if !c.warnings.is_empty() {
                    warned += 1;
                }
            }
            Err(e) => {
                failed += 1;
                renzora::console_log::console_error(CATEGORY, format!("{}: {e}", file_name(f)));
            }
        }
    }
    let mut msg = format!("Converted {written} effect(s)");
    if reused > 0 {
        msg.push_str(&format!(", {reused} already converted"));
    }
    if failed > 0 {
        msg.push_str(&format!(", {failed} failed"));
    }
    if warned > 0 {
        msg.push_str(&format!("; {warned} with warnings (see the Console)"));
    }
    toast(world, failed > 0 || warned > 0, msg);
}

/// What the Effect slot should hold for a dropped path: a `.vfx` as is, a
/// `.particle` converted beside itself (or the earlier conversion reused) and
/// its `.vfx` in its place. Same path form as given (project-relative from a
/// drop), so the scene stores what it always stores. `None` when a conversion
/// failed, which leaves the slot unchanged.
pub fn effect_for_dropped_path(world: &mut World, path: &str) -> Option<String> {
    if !path.to_ascii_lowercase().ends_with(".particle") {
        return Some(path.to_string());
    }
    let abs = world
        .get_resource::<CurrentProject>()
        .map(|p| p.resolve_path(path))
        .unwrap_or_else(|| PathBuf::from(path));
    match convert::convert_beside(&abs) {
        Ok(converted) => {
            log(&file_name(&abs), &converted);
            toast_one(world, &converted);
            let name = file_name(&converted.path);
            let rel = match path.rfind(['/', '\\']) {
                Some(i) => format!("{}/{name}", &path[..i]),
                None => name,
            };
            Some(rel)
        }
        Err(e) => {
            renzora::console_log::console_error(CATEGORY, format!("{}: {e}", file_name(&abs)));
            toast(world, true, format!("Could not convert {}", file_name(&abs)));
            None
        }
    }
}
