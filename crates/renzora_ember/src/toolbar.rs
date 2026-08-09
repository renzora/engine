//! Registries for widgets a *different crate* mounts inside the viewport panel.
//!
//! There used to be a shared toolbar strip below the top bar, whose contents
//! followed whichever panel was the active dock tab. It is gone: a panel's tools
//! now live **inside that panel**, which is where the code editor always kept
//! its own, and where the material and blueprint graphs moved theirs. A tool row
//! that belongs to a panel but renders somewhere else has to answer "is my panel
//! visible?" every frame, and answers it in a bar that is the same distance from
//! every panel — the one it acts on included.
//!
//! What survives is the narrower problem the strip was also solving: the editor
//! **shell** needs to put things (Play, the scene tabs) into the **viewport**,
//! and it can't call the viewport to do it — `renzora_shell` depends on
//! `renzora_viewport`, not the other way round. So the shell registers a builder
//! here and the viewport panel builds whatever it finds, with no dependency
//! edge in the wrong direction.
//!
//! Both registries are `static` rather than resources because the viewport panel
//! is built from a panel-content closure that receives only `Commands` and
//! [`EmberFonts`] — there is no `World` in scope to read a resource from.
//! Registration happens at plugin-build time and the lists are only appended to,
//! so a plain `Mutex` costs nothing at the one point each is read.

use bevy::prelude::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::font::EmberFonts;

/// Builds one mounted widget (a button, a whole bar…) and returns its root
/// entity. Gets full `Commands` + fonts, so it can use any ember widget and any
/// reactive binding.
pub type ToolbarBuilder = Arc<dyn Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync>;

/// Widgets mounted at the trailing (right) edge of the **in-viewport** tool
/// strip — the one with Select / Move / Rotate / Scale on it.
static VIEWPORT_TRAILING: OnceLock<Mutex<Vec<ToolbarBuilder>>> = OnceLock::new();

fn viewport_trailing() -> &'static Mutex<Vec<ToolbarBuilder>> {
    VIEWPORT_TRAILING.get_or_init(|| Mutex::new(Vec::new()))
}

/// Add a widget to the right-hand end of every primary in-viewport tool strip.
pub fn register_viewport_tool_trailing<F>(build: F)
where
    F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
{
    if let Ok(mut items) = viewport_trailing().lock() {
        items.push(Arc::new(build));
    }
}

/// Build everything registered via [`register_viewport_tool_trailing`].
pub fn build_viewport_tool_trailing(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let builders: Vec<ToolbarBuilder> = viewport_trailing()
        .lock()
        .map(|items| items.clone())
        .unwrap_or_default();
    builders.iter().map(|b| b(commands, fonts)).collect()
}

/// Full-width bars mounted inside the primary viewport panel, between its tool
/// strip and the rendered scene — the shell's scene tabs are the one that lives
/// here.
static VIEWPORT_TOP_STRIP: OnceLock<Mutex<Vec<ToolbarBuilder>>> = OnceLock::new();

fn viewport_top_strip() -> &'static Mutex<Vec<ToolbarBuilder>> {
    VIEWPORT_TOP_STRIP.get_or_init(|| Mutex::new(Vec::new()))
}

/// Add a full-width bar inside the primary viewport panel, under its tool strip.
/// Bars stack in registration order.
pub fn register_viewport_top_strip<F>(build: F)
where
    F: Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
{
    if let Ok(items) = viewport_top_strip().lock().as_mut() {
        items.push(Arc::new(build));
    }
}

/// Build everything registered via [`register_viewport_top_strip`].
pub fn build_viewport_top_strip(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let builders: Vec<ToolbarBuilder> = viewport_top_strip()
        .lock()
        .map(|items| items.clone())
        .unwrap_or_default();
    builders.iter().map(|b| b(commands, fonts)).collect()
}
