//! Shape Library — built-in mesh primitives for the engine.
//!
//! Provides two plugins:
//! - [`ShapeRegistrationPlugin`] — registers all built-in shapes into [`ShapeRegistry`].
//!   Include this in **both** editor and runtime builds so scenes can rehydrate.
//! - [`ShapeLibraryPlugin`] — editor panel for browsing and spawning shapes.
//!   Requires the `editor` feature.

pub mod procedural_meshes;

use bevy::prelude::*;
use renzora::core::{ShapeEntry, ShapeRegistry};

// ============================================================================
// Built-in shape registration
// ============================================================================

/// Add icons to shapes already registered by the engine (editor only).
fn add_shape_icons(registry: &mut ShapeRegistry) {
    use egui_phosphor::regular;
    let icons: &[(&str, &str)] = &[
        ("cube", regular::CUBE), ("sphere", regular::GLOBE), ("cylinder", regular::CYLINDER),
        ("plane", regular::SQUARE), ("cone", regular::TRIANGLE), ("torus", regular::CIRCLE),
        ("capsule", regular::CYLINDER), ("hemisphere", regular::GLOBE),
        ("wedge", regular::TRIANGLE), ("stairs", regular::STAIRS), ("arch", regular::CIRCLE),
        ("half_cylinder", regular::CYLINDER), ("quarter_pipe", regular::POLYGON),
        ("corner", regular::POLYGON), ("wall", regular::WALL), ("ramp", regular::TRIANGLE),
        ("curved_wall", regular::WALL), ("doorway", regular::DOOR),
        ("window_wall", regular::FRAME_CORNERS), ("l_shape", regular::POLYGON),
        ("t_shape", regular::POLYGON), ("cross_shape", regular::PLUS),
        ("spiral_stairs", regular::SPIRAL), ("pillar", regular::COLUMNS),
        ("pipe", regular::PIPE), ("ring", regular::CIRCLE), ("funnel", regular::TRIANGLE),
        ("gutter", regular::CYLINDER), ("prism", regular::HEXAGON), ("pyramid", regular::DIAMOND),
    ];
    for (id, icon) in icons {
        if let Some(entry) = registry.get_mut(id) {
            entry.icon = icon;
        }
    }
}

#[allow(dead_code)]
fn register_builtin_shapes(registry: &mut ShapeRegistry) {
    use procedural_meshes as pm;

    use egui_phosphor::regular;

    macro_rules! icon {
        ($name:ident) => { regular::$name };
    }

    // Basic
    registry.register(ShapeEntry {
        id: "cube", name: "Cube", icon: icon!(CUBE), category: "Basic",
        create_mesh: |m| m.add(Cuboid::new(1.0, 1.0, 1.0)),
        default_color: Color::srgb(0.8, 0.3, 0.2),
    });
    registry.register(ShapeEntry {
        id: "sphere", name: "Sphere", icon: icon!(GLOBE), category: "Basic",
        create_mesh: |m| m.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
        default_color: Color::srgb(0.2, 0.5, 0.8),
    });
    registry.register(ShapeEntry {
        id: "cylinder", name: "Cylinder", icon: icon!(CYLINDER), category: "Basic",
        create_mesh: |m| m.add(Cylinder::new(0.5, 1.0)),
        default_color: Color::srgb(0.3, 0.7, 0.4),
    });
    registry.register(ShapeEntry {
        id: "plane", name: "Plane", icon: icon!(SQUARE), category: "Basic",
        create_mesh: |m| m.add(Plane3d::default().mesh().size(2.0, 2.0)),
        default_color: Color::srgb(0.35, 0.35, 0.35),
    });
    registry.register(ShapeEntry {
        id: "cone", name: "Cone", icon: icon!(TRIANGLE), category: "Basic",
        create_mesh: |m| m.add(Cone { radius: 0.5, height: 1.0 }),
        default_color: Color::srgb(0.7, 0.5, 0.2),
    });
    registry.register(ShapeEntry {
        id: "torus", name: "Torus", icon: icon!(CIRCLE), category: "Basic",
        create_mesh: |m| m.add(Torus { minor_radius: 0.15, major_radius: 0.35 }),
        default_color: Color::srgb(0.6, 0.3, 0.7),
    });
    registry.register(ShapeEntry {
        id: "capsule", name: "Capsule", icon: icon!(CYLINDER), category: "Basic",
        create_mesh: |m| m.add(Capsule3d::new(0.25, 0.5)),
        default_color: Color::srgb(0.3, 0.6, 0.6),
    });
    registry.register(ShapeEntry {
        id: "hemisphere", name: "Hemisphere", icon: icon!(GLOBE), category: "Basic",
        create_mesh: |m| m.add(pm::create_hemisphere_mesh(16)),
        default_color: Color::srgb(0.5, 0.4, 0.7),
    });

    // Level Building
    registry.register(ShapeEntry {
        id: "wedge", name: "Wedge", icon: icon!(TRIANGLE), category: "Level",
        create_mesh: |m| m.add(pm::create_wedge_mesh()),
        default_color: Color::srgb(0.6, 0.6, 0.5),
    });
    registry.register(ShapeEntry {
        id: "stairs", name: "Stairs", icon: icon!(STAIRS), category: "Level",
        create_mesh: |m| m.add(pm::create_stairs_mesh(6)),
        default_color: Color::srgb(0.5, 0.5, 0.6),
    });
    registry.register(ShapeEntry {
        id: "arch", name: "Arch", icon: icon!(CIRCLE), category: "Level",
        create_mesh: |m| m.add(pm::create_arch_mesh(16)),
        default_color: Color::srgb(0.6, 0.5, 0.4),
    });
    registry.register(ShapeEntry {
        id: "half_cylinder", name: "Half Cylinder", icon: icon!(CYLINDER), category: "Level",
        create_mesh: |m| m.add(pm::create_half_cylinder_mesh(16)),
        default_color: Color::srgb(0.5, 0.6, 0.5),
    });
    registry.register(ShapeEntry {
        id: "quarter_pipe", name: "Quarter Pipe", icon: icon!(POLYGON), category: "Level",
        create_mesh: |m| m.add(pm::create_quarter_pipe_mesh(16)),
        default_color: Color::srgb(0.55, 0.55, 0.5),
    });
    registry.register(ShapeEntry {
        id: "corner", name: "Corner", icon: icon!(POLYGON), category: "Level",
        create_mesh: |m| m.add(pm::create_corner_mesh()),
        default_color: Color::srgb(0.5, 0.5, 0.55),
    });
    registry.register(ShapeEntry {
        id: "wall", name: "Wall", icon: icon!(WALL), category: "Level",
        create_mesh: |m| m.add(Cuboid::new(1.0, 2.0, 0.1)),
        default_color: Color::srgb(0.55, 0.5, 0.5),
    });
    registry.register(ShapeEntry {
        id: "ramp", name: "Ramp", icon: icon!(TRIANGLE), category: "Level",
        create_mesh: |m| m.add(pm::create_ramp_mesh()),
        default_color: Color::srgb(0.5, 0.55, 0.5),
    });
    registry.register(ShapeEntry {
        id: "curved_wall", name: "Curved Wall", icon: icon!(WALL), category: "Level",
        create_mesh: |m| m.add(pm::create_curved_wall_mesh(16)),
        default_color: Color::srgb(0.55, 0.55, 0.55),
    });
    registry.register(ShapeEntry {
        id: "doorway", name: "Doorway", icon: icon!(DOOR), category: "Level",
        create_mesh: |m| m.add(pm::create_doorway_mesh()),
        default_color: Color::srgb(0.5, 0.5, 0.6),
    });
    registry.register(ShapeEntry {
        id: "window_wall", name: "Window Wall", icon: icon!(FRAME_CORNERS), category: "Level",
        create_mesh: |m| m.add(pm::create_window_wall_mesh()),
        default_color: Color::srgb(0.5, 0.55, 0.55),
    });
    registry.register(ShapeEntry {
        id: "l_shape", name: "L-Shape", icon: icon!(POLYGON), category: "Level",
        create_mesh: |m| m.add(pm::create_l_shape_mesh()),
        default_color: Color::srgb(0.55, 0.5, 0.55),
    });
    registry.register(ShapeEntry {
        id: "t_shape", name: "T-Shape", icon: icon!(POLYGON), category: "Level",
        create_mesh: |m| m.add(pm::create_t_shape_mesh()),
        default_color: Color::srgb(0.5, 0.55, 0.6),
    });
    registry.register(ShapeEntry {
        id: "cross_shape", name: "Cross", icon: icon!(PLUS), category: "Level",
        create_mesh: |m| m.add(pm::create_cross_shape_mesh()),
        default_color: Color::srgb(0.55, 0.55, 0.6),
    });
    registry.register(ShapeEntry {
        id: "spiral_stairs", name: "Spiral Stairs", icon: icon!(SPIRAL), category: "Level",
        create_mesh: |m| m.add(pm::create_spiral_stairs_mesh(16)),
        default_color: Color::srgb(0.5, 0.5, 0.55),
    });
    registry.register(ShapeEntry {
        id: "pillar", name: "Pillar", icon: icon!(COLUMNS), category: "Level",
        create_mesh: |m| m.add(pm::create_pillar_mesh()),
        default_color: Color::srgb(0.55, 0.5, 0.5),
    });

    // Curved
    registry.register(ShapeEntry {
        id: "pipe", name: "Pipe", icon: icon!(PIPE), category: "Curved",
        create_mesh: |m| m.add(pm::create_pipe_mesh(24)),
        default_color: Color::srgb(0.4, 0.5, 0.6),
    });
    registry.register(ShapeEntry {
        id: "ring", name: "Ring", icon: icon!(CIRCLE), category: "Curved",
        create_mesh: |m| m.add(pm::create_ring_mesh(24)),
        default_color: Color::srgb(0.5, 0.4, 0.6),
    });
    registry.register(ShapeEntry {
        id: "funnel", name: "Funnel", icon: icon!(TRIANGLE), category: "Curved",
        create_mesh: |m| m.add(pm::create_funnel_mesh(24)),
        default_color: Color::srgb(0.6, 0.4, 0.5),
    });
    registry.register(ShapeEntry {
        id: "gutter", name: "Gutter", icon: icon!(CYLINDER), category: "Curved",
        create_mesh: |m| m.add(pm::create_gutter_mesh(16)),
        default_color: Color::srgb(0.4, 0.6, 0.5),
    });

    // Advanced
    registry.register(ShapeEntry {
        id: "prism", name: "Prism", icon: icon!(HEXAGON), category: "Advanced",
        create_mesh: |m| m.add(pm::create_prism_mesh()),
        default_color: Color::srgb(0.5, 0.5, 0.7),
    });
    registry.register(ShapeEntry {
        id: "pyramid", name: "Pyramid", icon: icon!(DIAMOND), category: "Advanced",
        create_mesh: |m| m.add(pm::create_pyramid_mesh()),
        default_color: Color::srgb(0.7, 0.5, 0.5),
    });
}

mod panel;

/// Shape library plugin — registers built-in shapes and adds the shape browser panel.
#[derive(Default)]
pub struct ShapeLibraryPlugin;

impl Plugin for ShapeLibraryPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShapeLibraryPlugin");

        // Add icons to the shapes already registered by the engine
        add_shape_icons(&mut app.world_mut().resource_mut::<ShapeRegistry>());

        use renzora_editor_framework::AppEditorExt;
        app.register_panel(panel::ShapeLibraryPanel::default());
    }
}

renzora::add!(ShapeLibraryPlugin, Editor);
