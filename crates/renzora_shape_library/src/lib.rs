//! Shape Library — the editor panel for browsing and spawning built-in shapes.
//!
//! The shapes themselves are registered by `renzora_engine`, into the
//! [`ShapeRegistry`] in the contract crate, because a scene has to be able to
//! rehydrate a shape in a shipped game where this crate is not present. All this
//! crate does is add the phosphor icons — which only the editor has a use for —
//! and draw the browser.
//!
//! It used to register the shapes too, from its own verbatim copy of the
//! generators. That copy went unused the moment the engine took registration
//! over, and by the time it was deleted the two had already drifted: the live
//! sphere is a UV sphere and the dead one was still an icosphere, with the seam
//! artefact the live one's comment describes.

use bevy::prelude::*;
use renzora::core::ShapeRegistry;

mod groups;
mod panel;
mod thumbs;

/// Add icons to shapes already registered by the engine (editor only).
fn add_shape_icons(registry: &mut ShapeRegistry) {
    // Phosphor icon names (kebab-case), resolved to glyphs by the panel.
    let icons: &[(&str, &str)] = &[
        ("cube", "cube"),
        ("sphere", "globe"),
        ("cylinder", "cylinder"),
        ("plane", "square"),
        ("cone", "triangle"),
        ("torus", "circle"),
        ("capsule", "cylinder"),
        ("hemisphere", "globe"),
        ("wedge", "triangle"),
        ("stairs", "stairs"),
        ("arch", "circle"),
        ("half_cylinder", "cylinder"),
        ("quarter_pipe", "polygon"),
        ("corner", "polygon"),
        ("wall", "wall"),
        ("ramp", "triangle"),
        ("curved_wall", "wall"),
        ("doorway", "door"),
        ("window_wall", "frame-corners"),
        ("l_shape", "polygon"),
        ("t_shape", "polygon"),
        ("cross_shape", "plus"),
        ("spiral_stairs", "spiral"),
        ("pillar", "columns"),
        ("pipe", "pipe"),
        ("ring", "circle"),
        ("funnel", "triangle"),
        ("gutter", "cylinder"),
        ("prism", "hexagon"),
        ("pyramid", "diamond"),
        ("tetrahedron", "triangle"),
        ("octahedron", "diamond"),
        ("dodecahedron", "pentagon"),
        ("icosahedron", "polygon"),
        ("rounded_cube", "cube"),
        ("disc", "disc"),
        ("star_prism", "star"),
        ("ellipsoid", "egg"),
        ("hex_prism", "hexagon"),
        ("domed_cylinder", "cylinder"),
        ("beveled_block", "cube"),
        ("step_block", "steps"),
        ("inner_corner_wedge", "triangle"),
        ("outer_corner_wedge", "triangle"),
        ("slope_2_1", "triangle"),
        ("rounded_edge_block", "square"),
        ("gear", "gear-six"),
        ("frame", "frame-corners"),
        ("column", "columns"),
        ("gable_roof", "house"),
        ("hip_roof", "house-line"),
        ("dome_shell", "globe-hemisphere-west"),
        ("archway", "door-open"),
        ("crenellated_wall", "castle-turret"),
        ("railing", "rows"),
        ("fence", "rows"),
        ("window_frame", "frame-corners"),
        ("door_frame", "door"),
        ("wall_corner", "wall"),
        ("floor_tile", "squares-four"),
        ("balcony", "building"),
        ("l_stairs", "stairs"),
        ("u_stairs", "stairs"),
        ("curved_ramp", "spiral"),
        ("ladder", "ladder-simple"),
        ("slab_stairs", "steps"),
        ("pipe_elbow", "pipe"),
        ("pipe_tee", "pipe"),
        ("pipe_cross", "plus"),
        ("pipe_flange", "pipe"),
        ("channel_bend", "pipe"),
        ("spring", "spiral"),
        ("torus_knot", "infinity"),
        ("crate", "package"),
        ("barrel", "database"),
        ("platform", "stack-simple"),
        ("bridge", "bridge"),
        ("bench", "armchair"),
        ("table", "table"),
        ("pedestal", "trophy"),
        ("rock", "mountains"),
        ("sign_post", "signpost"),
        ("lamp_post", "lamp"),
    ];
    for (id, icon) in icons {
        if let Some(entry) = registry.get_mut(id) {
            entry.icon = icon;
        }
    }
}

/// Shape library plugin — icons for the built-in shapes, plus the browser panel.
#[derive(Default)]
pub struct ShapeLibraryPlugin;

impl Plugin for ShapeLibraryPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShapeLibraryPlugin");

        // Add icons to the shapes already registered by the engine
        add_shape_icons(&mut app.world_mut().resource_mut::<ShapeRegistry>());

        app.add_plugins(panel::ShapeLibraryPanel);
    }
}

renzora::add!(ShapeLibraryPlugin, Editor);
