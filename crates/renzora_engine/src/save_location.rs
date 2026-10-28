//! Point `renzora::save` at the open project's save folder.
//!
//! The save store lives in the contract crate and knows nothing about
//! projects, so something with the `World` has to tell it where to write. The
//! folder is named for the game (`project.toml`'s `name`), which is only known
//! once the project has loaded, so this follows `CurrentProject` rather than
//! running once at startup.
//!
//! The same in the editor's Play and in a shipped game: Play runs the game as
//! its own process with the project loaded, so a playtest writes to the folder
//! the player's copy will use. That is deliberate. A save made while testing
//! is the quickest way to check the save menu, and a separate "editor saves"
//! folder would hide a bug that only shows where the real saves are.

use bevy::prelude::*;
use renzora::core::CurrentProject;

pub fn point_saves_at_project(project: Res<CurrentProject>) {
    let dir = renzora::save::default_directory(&project.config.name);
    match &dir {
        Some(dir) => info!("[save] slots go to {}", dir.display()),
        None => warn!("[save] this platform reports no per-user data folder; saving is unavailable"),
    }
    renzora::save::set_directory(dir);
}
