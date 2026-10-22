//! The Shape Library's categories: which group each built-in shape sits in,
//! and the colour that group's name is drawn in.
//!
//! Kept here rather than in the registry's `category`, because that string is
//! also what Add Entity groups by and what a `SpawnCategoryScope` filters on;
//! splitting "Shapes" into six there would reshuffle a menu and a scope that
//! have nothing to do with how this panel wants to browse. A shape a plugin
//! registers falls into [`OTHER`] until it is listed below.

/// Groups in display order: (name, colour).
pub(crate) const GROUPS: &[(&str, (u8, u8, u8))] = &[
    ("Primitives", (143, 176, 255)),
    ("Blocks", (240, 178, 122)),
    ("Architecture", (127, 209, 168)),
    ("Stairs and ramps", (196, 155, 242)),
    ("Pipes", (111, 200, 214)),
    ("Props", (230, 150, 170)),
    (OTHER, (184, 186, 196)),
];

/// The group for anything not listed in [`group_of`].
pub(crate) const OTHER: &str = "Other";

/// The group a shape id belongs to.
pub(crate) fn group_of(id: &str) -> &'static str {
    match id {
        "cube" | "sphere" | "cylinder" | "plane" | "cone" | "torus" | "capsule" | "hemisphere"
        | "prism" | "pyramid" | "ring" | "tetrahedron" | "octahedron" | "dodecahedron"
        | "icosahedron" | "rounded_cube" | "disc" | "star_prism" | "ellipsoid" | "hex_prism"
        | "domed_cylinder" => "Primitives",
        "wedge" | "half_cylinder" | "quarter_pipe" | "corner" | "l_shape" | "t_shape"
        | "cross_shape" | "pillar" | "beveled_block" | "step_block" | "inner_corner_wedge"
        | "outer_corner_wedge" | "slope_2_1" | "rounded_edge_block" | "gear" | "frame" => "Blocks",
        "wall" | "curved_wall" | "doorway" | "window_wall" | "arch" | "column" | "gable_roof"
        | "hip_roof" | "dome_shell" | "archway" | "crenellated_wall" | "railing" | "fence"
        | "window_frame" | "door_frame" | "wall_corner" | "floor_tile" | "balcony" => "Architecture",
        "stairs" | "spiral_stairs" | "ramp" | "l_stairs" | "u_stairs" | "curved_ramp" | "ladder"
        | "slab_stairs" => "Stairs and ramps",
        "pipe" | "funnel" | "gutter" | "pipe_elbow" | "pipe_tee" | "pipe_cross" | "pipe_flange"
        | "channel_bend" | "spring" | "torus_knot" => "Pipes",
        "crate" | "barrel" | "platform" | "bridge" | "bench" | "table" | "pedestal" | "rock"
        | "sign_post" | "lamp_post" => "Props",
        _ => OTHER,
    }
}

/// A group's name colour.
pub(crate) fn color_of(group: &str) -> (u8, u8, u8) {
    GROUPS.iter().find(|(g, _)| *g == group).map(|(_, c)| *c).unwrap_or((184, 186, 196))
}
