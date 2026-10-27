//! Editor half of virtual geometry: the converter, and the inspector entry for
//! keeping a model as ordinary geometry.
//!
//! The converter keeps an up-to-date `.rvg` beside every `.glb` in the open
//! project, in the background, so models render as virtual geometry without
//! anyone asking. When a project opens, every model is checked; after that, a
//! model that is added or re-imported is checked again when the file watcher
//! reports it. A file is current when its header records the source's hash and
//! this builder's version, so a re-import, an edited GLB or a builder upgrade
//! rebuilds it, and nothing else does.
//!
//! Until its `.rvg` exists a model renders as ordinary geometry, so the work
//! being in the background costs nothing but time: the switch happens live,
//! with no reload, when the file lands (see `RvgUpdated`).
//!
//! The project-wide switch is `[rendering] virtual_geometry` in `project.toml`.
//! Off, nothing is converted and existing files are left alone.
//!
//! All of that is for models stored as `.glb`, which is how a project from
//! before imports became `.rvg` holds them. [`migrate`] turns those into
//! `.rvg` models, when asked to.

mod converter;
mod migrate;

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry, StandardGeometry};

fn standard_geometry_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "standard_geometry",
        display_name: "Standard Geometry",
        icon: "cube",
        category: "rendering",
        has_fn: |world, entity| world.get::<StandardGeometry>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(StandardGeometry);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<StandardGeometry>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    }
}

#[derive(Default)]
pub struct VirtualGeometryEditorPlugin;

impl Plugin for VirtualGeometryEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] VirtualGeometryEditorPlugin");
        app.register_inspector(standard_geometry_entry());
        converter::build(app);
        migrate::build(app);
    }
}

renzora::add!(VirtualGeometryEditorPlugin, Editor);
