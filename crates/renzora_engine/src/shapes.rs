//! The built-in shape palette — what the editor's Add Shape menu offers, and
//! what a `spawn_shape("cube")` resolves against.
//!
//! A table rather than a match: every entry is `(id, display name, category,
//! how to build the mesh, what colour it starts)`, and the registry is a
//! resource so a plugin can append to it. Adding a shape is one entry here and
//! one generator in [`procedural_meshes`](crate::procedural_meshes) — the
//! primitives Bevy already ships are built inline. New shapes go in
//! [`procedural_meshes_more`](crate::procedural_meshes_more) and get one
//! `shape!` line in [`register_more`].

use bevy::prelude::*;

use renzora::core::{ShapeEntry, ShapeRegistry};

use crate::procedural_meshes as pm;
use crate::procedural_meshes_more as pm2;

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

    register_more(&mut reg);
    reg
}

/// One registry entry per line for the second wave of shapes.
///
/// A macro because `create_mesh` is a plain `fn` pointer: a closure that
/// names its generator directly coerces to one, but a closure that captured
/// the generator from a loop variable would not. Ids are persisted in saved
/// scenes, so like the ones above they are never renamed or reused.
macro_rules! shape {
    ($reg:expr, $id:literal, $name:literal, $make:path, ($r:literal, $g:literal, $b:literal)) => {
        $reg.register(ShapeEntry {
            id: $id,
            name: $name,
            icon: "",
            category: "Shapes",
            create_mesh: |m| m.add($make()),
            default_color: Color::srgb($r, $g, $b),
        })
    };
}

fn register_more(reg: &mut ShapeRegistry) {
    // Primitives
    shape!(reg, "tetrahedron", "Tetrahedron", pm2::create_tetrahedron_mesh, (0.75, 0.45, 0.3));
    shape!(reg, "octahedron", "Octahedron", pm2::create_octahedron_mesh, (0.3, 0.6, 0.75));
    shape!(reg, "dodecahedron", "Dodecahedron", pm2::create_dodecahedron_mesh, (0.6, 0.4, 0.7));
    shape!(reg, "icosahedron", "Icosahedron", pm2::create_icosahedron_mesh, (0.35, 0.65, 0.5));
    shape!(reg, "rounded_cube", "Rounded Cube", pm2::create_rounded_cube_mesh, (0.8, 0.4, 0.3));
    shape!(reg, "disc", "Disc", pm2::create_disc_mesh, (0.45, 0.45, 0.5));
    shape!(reg, "star_prism", "Star", pm2::create_star_prism_mesh, (0.85, 0.7, 0.25));
    shape!(reg, "ellipsoid", "Ellipsoid", pm2::create_ellipsoid_mesh, (0.25, 0.5, 0.8));
    shape!(reg, "hex_prism", "Hexagonal Prism", pm2::create_hex_prism_mesh, (0.5, 0.5, 0.7));
    shape!(reg, "domed_cylinder", "Domed Cylinder", pm2::create_domed_cylinder_mesh, (0.3, 0.65, 0.45));

    // Blocks
    shape!(reg, "beveled_block", "Beveled Block", pm2::create_beveled_block_mesh, (0.6, 0.55, 0.5));
    shape!(reg, "step_block", "Step Block", pm2::create_step_block_mesh, (0.55, 0.55, 0.5));
    shape!(reg, "inner_corner_wedge", "Inner Corner Wedge", pm2::create_inner_corner_wedge_mesh, (0.6, 0.6, 0.5));
    shape!(reg, "outer_corner_wedge", "Outer Corner Wedge", pm2::create_outer_corner_wedge_mesh, (0.6, 0.6, 0.5));
    shape!(reg, "slope_2_1", "Slope 2:1", pm2::create_slope_2_1_mesh, (0.5, 0.55, 0.5));
    shape!(reg, "rounded_edge_block", "Rounded Edge Block", pm2::create_rounded_edge_block_mesh, (0.55, 0.5, 0.5));
    shape!(reg, "gear", "Gear", pm2::create_gear_mesh, (0.55, 0.55, 0.6));
    shape!(reg, "frame", "Frame", pm2::create_frame_mesh, (0.55, 0.5, 0.45));

    // Architecture
    shape!(reg, "column", "Column", pm2::create_column_mesh, (0.75, 0.72, 0.65));
    shape!(reg, "gable_roof", "Gable Roof", pm2::create_gable_roof_mesh, (0.6, 0.35, 0.3));
    shape!(reg, "hip_roof", "Hip Roof", pm2::create_hip_roof_mesh, (0.6, 0.35, 0.3));
    shape!(reg, "dome_shell", "Dome", pm2::create_dome_shell_mesh, (0.55, 0.6, 0.65));
    shape!(reg, "archway", "Archway", pm2::create_archway_mesh, (0.6, 0.55, 0.5));
    shape!(reg, "crenellated_wall", "Battlement Wall", pm2::create_crenellated_wall_mesh, (0.55, 0.55, 0.5));
    shape!(reg, "railing", "Railing", pm2::create_railing_mesh, (0.45, 0.45, 0.5));
    shape!(reg, "fence", "Fence", pm2::create_fence_mesh, (0.6, 0.5, 0.4));
    shape!(reg, "window_frame", "Window Frame", pm2::create_window_frame_mesh, (0.65, 0.6, 0.55));
    shape!(reg, "door_frame", "Door Frame", pm2::create_door_frame_mesh, (0.6, 0.5, 0.4));
    shape!(reg, "wall_corner", "Wall Corner", pm2::create_wall_corner_mesh, (0.55, 0.5, 0.5));
    shape!(reg, "floor_tile", "Floor Tile", pm2::create_floor_tile_mesh, (0.5, 0.5, 0.55));
    shape!(reg, "balcony", "Balcony", pm2::create_balcony_mesh, (0.55, 0.55, 0.6));

    // Stairs and ramps
    shape!(reg, "l_stairs", "L-Stairs", pm2::create_l_stairs_mesh, (0.5, 0.5, 0.6));
    shape!(reg, "u_stairs", "U-Stairs", pm2::create_u_stairs_mesh, (0.5, 0.5, 0.6));
    shape!(reg, "curved_ramp", "Curved Ramp", pm2::create_curved_ramp_mesh, (0.5, 0.55, 0.5));
    shape!(reg, "ladder", "Ladder", pm2::create_ladder_mesh, (0.6, 0.5, 0.35));
    shape!(reg, "slab_stairs", "Slab Stairs", pm2::create_slab_stairs_mesh, (0.5, 0.5, 0.55));

    // Pipes
    shape!(reg, "pipe_elbow", "Pipe Elbow", pm2::create_pipe_elbow_mesh, (0.4, 0.5, 0.6));
    shape!(reg, "pipe_tee", "Pipe Tee", pm2::create_pipe_tee_mesh, (0.4, 0.5, 0.6));
    shape!(reg, "pipe_cross", "Pipe Cross", pm2::create_pipe_cross_mesh, (0.4, 0.5, 0.6));
    shape!(reg, "pipe_flange", "Pipe Flange", pm2::create_pipe_flange_mesh, (0.45, 0.5, 0.6));
    shape!(reg, "channel_bend", "Channel Bend", pm2::create_channel_bend_mesh, (0.4, 0.6, 0.5));
    shape!(reg, "spring", "Spring", pm2::create_spring_mesh, (0.6, 0.6, 0.65));
    shape!(reg, "torus_knot", "Torus Knot", pm2::create_torus_knot_mesh, (0.7, 0.4, 0.6));

    // Props
    shape!(reg, "crate", "Crate", pm2::create_crate_mesh, (0.65, 0.5, 0.3));
    shape!(reg, "barrel", "Barrel", pm2::create_barrel_mesh, (0.55, 0.4, 0.25));
    shape!(reg, "platform", "Platform", pm2::create_platform_mesh, (0.5, 0.5, 0.55));
    shape!(reg, "bridge", "Bridge", pm2::create_bridge_mesh, (0.55, 0.45, 0.35));
    shape!(reg, "bench", "Bench", pm2::create_bench_mesh, (0.6, 0.45, 0.3));
    shape!(reg, "table", "Table", pm2::create_table_mesh, (0.6, 0.45, 0.3));
    shape!(reg, "pedestal", "Pedestal", pm2::create_pedestal_mesh, (0.7, 0.68, 0.62));
    shape!(reg, "rock", "Rock", pm2::create_rock_mesh, (0.5, 0.48, 0.45));
    shape!(reg, "sign_post", "Sign Post", pm2::create_sign_post_mesh, (0.6, 0.5, 0.35));
    shape!(reg, "lamp_post", "Lamp Post", pm2::create_lamp_post_mesh, (0.3, 0.32, 0.35));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// `register` drops a duplicate id without a word, so a copy-pasted entry
    /// would silently shadow nothing and lose the shape. Count them instead.
    #[test]
    fn every_builtin_id_is_unique_and_builds() {
        let reg = builtin_shapes();
        let ids: Vec<&str> = reg.iter().map(|e| e.id).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
        assert_eq!(ids.len(), 30 + 53, "a registration was dropped as a duplicate");
        let mut meshes = Assets::<Mesh>::default();
        for e in reg.iter() {
            let handle = (e.create_mesh)(&mut meshes);
            assert!(meshes.get(&handle).is_some(), "{} built no mesh", e.id);
        }
    }
}
