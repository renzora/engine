//! The built-in shape palette — what the editor's Add Shape menu offers, and
//! what a `spawn_shape("cube")` resolves against.
//!
//! A table rather than a match: every entry is `(id, display name, category,
//! how to build the mesh, what colour it starts)`, and the registry is a
//! resource so a plugin can append to it. Adding a shape is one entry here and
//! one generator in [`procedural_meshes`](crate::procedural_meshes) — the
//! primitives Bevy already ships are built inline.

use bevy::prelude::*;

use renzora::core::{ShapeEntry, ShapeRegistry};

use crate::procedural_meshes as pm;

/// Build the registry the runtime inserts at startup.
pub(crate) fn builtin_shapes() -> ShapeRegistry {
    let mut reg = ShapeRegistry::default();

    // Basic
    reg.register(ShapeEntry {
        id: "cube",
        name: "Cube",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(Cuboid::new(1.0, 1.0, 1.0)),
        default_color: Color::srgb(0.8, 0.3, 0.2),
    });
    reg.register(ShapeEntry {
        id: "sphere",
        name: "Sphere",
        icon: "",
        category: "Shapes",
        // A UV sphere, not an icosphere. Bevy's `ico` tessellation has
        // no clean UV layout: its seam runs in a zigzag around the
        // icosahedron's triangle edges, which any tiling texture — the
        // default blockout grid included — draws as a visible jagged
        // scar down one side. `uv` gives the ordinary lat/long
        // parametrization with a single straight seam, at a vertex
        // count in the same ballpark.
        create_mesh: |m| m.add(Sphere::new(0.5).mesh().uv(32, 18)),
        default_color: Color::srgb(0.2, 0.5, 0.8),
    });
    reg.register(ShapeEntry {
        id: "cylinder",
        name: "Cylinder",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(Cylinder::new(0.5, 1.0)),
        default_color: Color::srgb(0.3, 0.7, 0.4),
    });
    reg.register(ShapeEntry {
        id: "plane",
        name: "Plane",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(Plane3d::default().mesh().size(2.0, 2.0)),
        default_color: Color::srgb(0.35, 0.35, 0.35),
    });
    reg.register(ShapeEntry {
        id: "cone",
        name: "Cone",
        icon: "",
        category: "Shapes",
        create_mesh: |m| {
            m.add(Cone {
                radius: 0.5,
                height: 1.0,
            })
        },
        default_color: Color::srgb(0.7, 0.5, 0.2),
    });
    reg.register(ShapeEntry {
        id: "torus",
        name: "Torus",
        icon: "",
        category: "Shapes",
        create_mesh: |m| {
            m.add(Torus {
                minor_radius: 0.15,
                major_radius: 0.35,
            })
        },
        default_color: Color::srgb(0.6, 0.3, 0.7),
    });
    reg.register(ShapeEntry {
        id: "capsule",
        name: "Capsule",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(Capsule3d::new(0.25, 0.5)),
        default_color: Color::srgb(0.3, 0.6, 0.6),
    });
    reg.register(ShapeEntry {
        id: "hemisphere",
        name: "Hemisphere",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_hemisphere_mesh(16)),
        default_color: Color::srgb(0.5, 0.4, 0.7),
    });

    // Level
    reg.register(ShapeEntry {
        id: "wedge",
        name: "Wedge",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_wedge_mesh()),
        default_color: Color::srgb(0.6, 0.6, 0.5),
    });
    reg.register(ShapeEntry {
        id: "stairs",
        name: "Stairs",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_stairs_mesh(6)),
        default_color: Color::srgb(0.5, 0.5, 0.6),
    });
    reg.register(ShapeEntry {
        id: "arch",
        name: "Arch",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_arch_mesh(16)),
        default_color: Color::srgb(0.6, 0.5, 0.4),
    });
    reg.register(ShapeEntry {
        id: "half_cylinder",
        name: "Half Cylinder",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_half_cylinder_mesh(16)),
        default_color: Color::srgb(0.5, 0.6, 0.5),
    });
    reg.register(ShapeEntry {
        id: "quarter_pipe",
        name: "Quarter Pipe",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_quarter_pipe_mesh(16)),
        default_color: Color::srgb(0.55, 0.55, 0.5),
    });
    reg.register(ShapeEntry {
        id: "corner",
        name: "Corner",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_corner_mesh()),
        default_color: Color::srgb(0.5, 0.5, 0.55),
    });
    reg.register(ShapeEntry {
        id: "wall",
        name: "Wall",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(Cuboid::new(1.0, 2.0, 0.1)),
        default_color: Color::srgb(0.55, 0.5, 0.5),
    });
    reg.register(ShapeEntry {
        id: "ramp",
        name: "Ramp",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_ramp_mesh()),
        default_color: Color::srgb(0.5, 0.55, 0.5),
    });
    reg.register(ShapeEntry {
        id: "curved_wall",
        name: "Curved Wall",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_curved_wall_mesh(16)),
        default_color: Color::srgb(0.55, 0.55, 0.55),
    });
    reg.register(ShapeEntry {
        id: "doorway",
        name: "Doorway",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_doorway_mesh()),
        default_color: Color::srgb(0.5, 0.5, 0.6),
    });
    reg.register(ShapeEntry {
        id: "window_wall",
        name: "Window Wall",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_window_wall_mesh()),
        default_color: Color::srgb(0.5, 0.55, 0.55),
    });
    reg.register(ShapeEntry {
        id: "l_shape",
        name: "L-Shape",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_l_shape_mesh()),
        default_color: Color::srgb(0.55, 0.5, 0.55),
    });
    reg.register(ShapeEntry {
        id: "t_shape",
        name: "T-Shape",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_t_shape_mesh()),
        default_color: Color::srgb(0.5, 0.55, 0.6),
    });
    reg.register(ShapeEntry {
        id: "cross_shape",
        name: "Cross",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_cross_shape_mesh()),
        default_color: Color::srgb(0.55, 0.55, 0.6),
    });
    reg.register(ShapeEntry {
        id: "spiral_stairs",
        name: "Spiral Stairs",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_spiral_stairs_mesh(16)),
        default_color: Color::srgb(0.5, 0.5, 0.55),
    });
    reg.register(ShapeEntry {
        id: "pillar",
        name: "Pillar",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_pillar_mesh()),
        default_color: Color::srgb(0.55, 0.5, 0.5),
    });

    // Curved
    reg.register(ShapeEntry {
        id: "pipe",
        name: "Pipe",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_pipe_mesh(24)),
        default_color: Color::srgb(0.4, 0.5, 0.6),
    });
    reg.register(ShapeEntry {
        id: "ring",
        name: "Ring",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_ring_mesh(24)),
        default_color: Color::srgb(0.5, 0.4, 0.6),
    });
    reg.register(ShapeEntry {
        id: "funnel",
        name: "Funnel",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_funnel_mesh(24)),
        default_color: Color::srgb(0.6, 0.4, 0.5),
    });
    reg.register(ShapeEntry {
        id: "gutter",
        name: "Gutter",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_gutter_mesh(16)),
        default_color: Color::srgb(0.4, 0.6, 0.5),
    });

    // Advanced
    reg.register(ShapeEntry {
        id: "prism",
        name: "Prism",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_prism_mesh()),
        default_color: Color::srgb(0.5, 0.5, 0.7),
    });
    reg.register(ShapeEntry {
        id: "pyramid",
        name: "Pyramid",
        icon: "",
        category: "Shapes",
        create_mesh: |m| m.add(pm::create_pyramid_mesh()),
        default_color: Color::srgb(0.7, 0.5, 0.5),
    });

    reg
}
