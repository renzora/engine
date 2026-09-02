//! Procedural nodes — the noise family (UV and triplanar), patterns, gradients,
//! and the height-to-normal / parallax derivations built on them.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_PROCEDURAL, CLR_PROCEDURAL};

pub static NOISE_PERLIN: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_perlin",
    display_name: "Perlin Noise",
    category: CAT_PROCEDURAL,
    description: "Smooth gradient noise",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float)
                .with_default(PinValue::Float(10.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_SIMPLEX: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_simplex",
    display_name: "Simplex Noise",
    category: CAT_PROCEDURAL,
    description: "Fast gradient noise with fewer directional artifacts",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float)
                .with_default(PinValue::Float(10.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_VORONOI: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_voronoi",
    display_name: "Voronoi",
    category: CAT_PROCEDURAL,
    description: "Cell/Worley noise with F1, F2, edge-distance and cell-id outputs",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(5.0)),
            PinTemplate::output("distance", "F1 (nearest)", PinType::Float),
            PinTemplate::output("f2", "F2 (2nd nearest)", PinType::Float),
            PinTemplate::output("edge", "Edge Distance", PinType::Float),
            PinTemplate::output("cell_id", "Cell ID", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_FBM: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_fbm",
    display_name: "FBM Noise",
    category: CAT_PROCEDURAL,
    description: "Fractal Brownian Motion (layered noise)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(5.0)),
            PinTemplate::input("octaves", "Octaves", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::input("lacunarity", "Lacunarity", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::input("persistence", "Persistence", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static CHECKERBOARD: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/checkerboard",
    display_name: "Checkerboard",
    category: CAT_PROCEDURAL,
    description: "Alternating pattern",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(8.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static GRADIENT: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient",
    display_name: "Gradient",
    category: CAT_PROCEDURAL,
    description: "Linear gradient (0-1) along U or V",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("u", "U", PinType::Float),
            PinTemplate::output("v", "V", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static BRICK: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/brick",
    display_name: "Brick",
    category: CAT_PROCEDURAL,
    description: "Brick/tile pattern with mortar",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Vec2)
                .with_default(PinValue::Vec2([4.0, 8.0])),
            PinTemplate::input("mortar", "Mortar Width", PinType::Float)
                .with_default(PinValue::Float(0.05)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NORMAL_FROM_HEIGHT: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/normal_from_height",
    display_name: "Normal From Height (tangent)",
    category: CAT_PROCEDURAL,
    description: "Derive a tangent-space normal from a height value via screen-space derivatives. Output is in tangent frame (Z = up). For materials that plug into a pbr_input.N hook and expect world-space, use `world_normal_from_height` instead.",
    pins: || vec![
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_PROCEDURAL,
};

pub static WORLD_NORMAL_FROM_HEIGHT: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/world_normal_from_height",
    display_name: "World Normal From Height",
    category: CAT_PROCEDURAL,
    description: "Derive a WORLD-space perturbed normal from a height value, reconstructing a tangent frame per-fragment from the screen-space derivatives of world_position. Works on any surface orientation (horizontal lake, tilted river, sculpted terrain). Feed directly into Surface Output's `normal` pin for water / stone / procedural displacement.",
    pins: || vec![
        PinTemplate::input("height", "Height", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_PROCEDURAL,
};

pub static DOMAIN_WARP: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/domain_warp",
    display_name: "Domain Warp",
    category: CAT_PROCEDURAL,
    description: "Distort UV coordinates using FBM noise as an offset vector. Produces organic cloud / marble / fluid shapes.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Noise Scale", PinType::Float).with_default(PinValue::Float(1.5)),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.35)),
        PinTemplate::input("offset", "Offset", PinType::Vec2).with_default(PinValue::Vec2([5.2, 1.3])),
        PinTemplate::output("uv", "Warped UV", PinType::Vec2),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_RIDGED: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_ridged",
    display_name: "Ridged FBM",
    category: CAT_PROCEDURAL,
    description: "Ridged multifractal — sharp crests for cumulus billows, mountain ridges, cracks",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(4.0)),
            PinTemplate::input("octaves", "Octaves", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::input("lacunarity", "Lacunarity", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::input("persistence", "Persistence", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_TURBULENCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_turbulence",
    display_name: "Turbulence",
    category: CAT_PROCEDURAL,
    description: "Fire / smoke / turbulent flow (|noise| accumulated across octaves)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(4.0)),
            PinTemplate::input("octaves", "Octaves", PinType::Float)
                .with_default(PinValue::Float(5.0)),
            PinTemplate::input("lacunarity", "Lacunarity", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::input("persistence", "Persistence", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_BILLOW: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_billow",
    display_name: "Billow Noise",
    category: CAT_PROCEDURAL,
    description:
        "Puffy, rounded shapes (|noise|² accumulated) — great for cumulus clouds and stone pores",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(4.0)),
            PinTemplate::input("octaves", "Octaves", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::input("lacunarity", "Lacunarity", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::input("persistence", "Persistence", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_WHITE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_white",
    display_name: "White Noise",
    category: CAT_PROCEDURAL,
    description: "Uncorrelated random values at every UV coordinate (grain, sparkle)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float)
                .with_default(PinValue::Float(50.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_CURL: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_curl",
    display_name: "Curl Noise",
    category: CAT_PROCEDURAL,
    description:
        "Divergence-free 2D flow field — ideal for fluid-like advection and swirly UV distortion",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(3.0)),
            PinTemplate::input("epsilon", "Epsilon", PinType::Float)
                .with_default(PinValue::Float(0.01)),
            PinTemplate::output("flow", "Flow", PinType::Vec2),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_RADIAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_radial",
    display_name: "Radial Gradient",
    category: CAT_PROCEDURAL,
    description: "0 at center → 1 at `radius`, with soft falloff",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("center", "Center", PinType::Vec2)
                .with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("radius", "Radius", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("softness", "Softness", PinType::Float)
                .with_default(PinValue::Float(0.3)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_LINEAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_linear",
    display_name: "Linear Gradient",
    category: CAT_PROCEDURAL,
    description: "Gradient along a direction (angle in radians, 0 = +x)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("angle", "Angle", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("center", "Center", PinType::Vec2)
                .with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_ANGULAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_angular",
    display_name: "Angular Gradient",
    category: CAT_PROCEDURAL,
    description: "0-1 sweeping around a center point (for pie / compass / clock effects)",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("center", "Center", PinType::Vec2)
                .with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("offset", "Start Offset", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static GRADIENT_DIAMOND: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/gradient_diamond",
    display_name: "Diamond Gradient",
    category: CAT_PROCEDURAL,
    description: "Diamond-shaped falloff (L1 / Manhattan distance) around a center",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("center", "Center", PinType::Vec2)
                .with_default(PinValue::Vec2([0.5, 0.5])),
            PinTemplate::input("size", "Size", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static BUMP_OFFSET: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/bump_offset",
    display_name: "Bump Offset",
    category: CAT_PROCEDURAL,
    description:
        "Simple parallax: displace UVs along view vector by a height value. Cheap fake depth.",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("height", "Height", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("reference", "Reference", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("strength", "Strength", PinType::Float)
                .with_default(PinValue::Float(0.05)),
            PinTemplate::output("uv", "Offset UV", PinType::Vec2),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_FBM: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_fbm",
    display_name: "Triplanar FBM",
    category: CAT_PROCEDURAL,
    description: "World-space FBM projected onto X/Y/Z planes and blended by world normal. No UV, no seams, works on any mesh topology (spheres, terrain, sculpts).",
    pins: || vec![
        PinTemplate::input("scale",       "Scale",       PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("octaves",     "Octaves",     PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity",  "Lacunarity",  PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("sharpness",   "Sharpness",   PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_RIDGED: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_ridged",
    display_name: "Triplanar Ridged",
    category: CAT_PROCEDURAL,
    description: "Ridged FBM sampled triplanar — seamless mountain/cumulus ridges on any topology.",
    pins: || {
        vec![
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("octaves", "Octaves", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::input("lacunarity", "Lacunarity", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::input("persistence", "Persistence", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("sharpness", "Sharpness", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_TURBULENCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_turbulence",
    display_name: "Triplanar Turbulence",
    category: CAT_PROCEDURAL,
    description: "Turbulence noise sampled triplanar — seamless fire/smoke/flow on any topology.",
    pins: || {
        vec![
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("octaves", "Octaves", PinType::Float)
                .with_default(PinValue::Float(5.0)),
            PinTemplate::input("lacunarity", "Lacunarity", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::input("persistence", "Persistence", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("sharpness", "Sharpness", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_BILLOW: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_billow",
    display_name: "Triplanar Billow",
    category: CAT_PROCEDURAL,
    description: "Billow noise sampled triplanar — seamless puffy cumulus / stone-pore shapes on any topology.",
    pins: || vec![
        PinTemplate::input("scale",       "Scale",       PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("octaves",     "Octaves",     PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::input("lacunarity",  "Lacunarity",  PinType::Float).with_default(PinValue::Float(2.0)),
        PinTemplate::input("persistence", "Persistence", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("sharpness",   "Sharpness",   PinType::Float).with_default(PinValue::Float(4.0)),
        PinTemplate::output("value", "Value", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};

pub static NOISE_TRIPLANAR_VORONOI: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/noise_triplanar_voronoi",
    display_name: "Triplanar Voronoi",
    category: CAT_PROCEDURAL,
    description:
        "Voronoi cells sampled triplanar — seamless cracked-surface / cell pattern on any mesh.",
    pins: || {
        vec![
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(3.0)),
            PinTemplate::input("sharpness", "Sharpness", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::output("distance", "F1", PinType::Float),
            PinTemplate::output("cell_id", "Cell ID", PinType::Float),
        ]
    },
    color: CLR_PROCEDURAL,
};

pub static HEX_TILE: MaterialNodeDef = MaterialNodeDef {
    node_type: "procedural/hex_tile",
    display_name: "Hex Tile UV",
    category: CAT_PROCEDURAL,
    description: "Hexagonal anti-tiling: decomposes UV space into hex cells, randomly rotates each cell's UV, and blends three overlapping hex samples together. Feed the output UV into a Sample Texture node to break up visible repetition on a single tiled texture. The `variation` pin controls random rotation strength (0 = plain tiling, 1 = maximum scramble).",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("variation", "Variation", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("uv1", "UV A", PinType::Vec2),
        PinTemplate::output("uv2", "UV B", PinType::Vec2),
        PinTemplate::output("uv3", "UV C", PinType::Vec2),
        PinTemplate::output("w1", "Weight A", PinType::Float),
        PinTemplate::output("w2", "Weight B", PinType::Float),
        PinTemplate::output("w3", "Weight C", PinType::Float),
    ],
    color: CLR_PROCEDURAL,
};
