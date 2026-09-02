//! Texture nodes — every way a graph reads a sampler: plain 2D, normal maps,
//! triplanar, explicit LOD / gradients, cubemaps, 2D arrays and 3D volumes.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_TEXTURE, CLR_TEXTURE};

pub static SAMPLE_TEXTURE: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample",
    display_name: "Sample Texture",
    category: CAT_TEXTURE,
    description: "Sample a 2D texture at UV coordinates",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("color", "Color", PinType::Color),
            PinTemplate::output("rgb", "RGB", PinType::Vec3),
            PinTemplate::output("r", "R", PinType::Float),
            PinTemplate::output("g", "G", PinType::Float),
            PinTemplate::output("b", "B", PinType::Float),
            PinTemplate::output("a", "Alpha", PinType::Float),
        ]
    },
    color: CLR_TEXTURE,
};

pub static SAMPLE_NORMAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_normal",
    display_name: "Sample Normal Map",
    category: CAT_TEXTURE,
    description: "Sample and decode a normal map texture. Decoding assumes the OpenGL convention (+Y green points up) — turn on Flip Green for a DirectX map, which stores green inverted and otherwise renders with its lighting flipped on one axis.",
    pins: || {
        vec![
            PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::input("strength", "Strength", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::input("flip_green", "Flip Green (DirectX)", PinType::Bool)
                .with_default(PinValue::Bool(false)),
            PinTemplate::output("normal", "Normal", PinType::Vec3),
        ]
    },
    color: [120, 120, 200],
};

pub static TRIPLANAR_SAMPLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/triplanar",
    display_name: "Triplanar Sample",
    category: CAT_TEXTURE,
    description: "Sample texture using triplanar projection (no UV seams)",
    pins: || {
        vec![
            PinTemplate::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("sharpness", "Sharpness", PinType::Float)
                .with_default(PinValue::Float(2.0)),
            PinTemplate::output("color", "Color", PinType::Color),
            PinTemplate::output("rgb", "RGB", PinType::Vec3),
        ]
    },
    color: CLR_TEXTURE,
};

pub static SAMPLE_TEXTURE_LOD: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_lod",
    display_name: "Sample Texture LOD",
    category: CAT_TEXTURE,
    description: "Sample a 2D texture at an explicit mip level (textureSampleLevel). Use to blur reflections/refractions with a Roughness-driven LOD, or to sample at mip 0 from inside a loop/branch where automatic derivatives aren't valid.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("lod", "LOD", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_TEXTURE_GRAD: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_grad",
    display_name: "Sample Texture Grad",
    category: CAT_TEXTURE,
    description: "Sample a 2D texture with explicit UV derivatives (textureSampleGrad). Fixes mip-selection bias when UVs are rotated, polar-warped, or otherwise transformed in ways that fool the default derivatives — produces crisp anisotropic filtering.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("ddx", "dUV/dx", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("ddy", "dUV/dy", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_CUBEMAP: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_cubemap",
    display_name: "Sample Cubemap",
    category: CAT_TEXTURE,
    description: "Sample a user-supplied cubemap texture along a direction vector. Separate from Environment Map Sample — this goes to a material-local cube slot so one graph can reference its own skybox / reflection-probe / stylized sky without disturbing the scene's IBL. LOD controls glossiness (0 = sharpest mip).",
    pins: || vec![
        PinTemplate::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("lod", "LOD", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_2D_ARRAY: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_2d_array",
    display_name: "Sample 2D Array",
    category: CAT_TEXTURE,
    description: "Sample a layered 2D texture array — UV picks the in-layer position, Layer Index picks which layer. Use for terrain layer stacks, asset-variant atlases (e.g. same character body with multiple skins), paletted materials, mask banks. Layer Index is rounded to the nearest integer layer.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
        PinTemplate::input("layer", "Layer", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};

pub static SAMPLE_3D: MaterialNodeDef = MaterialNodeDef {
    node_type: "texture/sample_3d",
    display_name: "Sample 3D Texture",
    category: CAT_TEXTURE,
    description: "Sample a volumetric (3D) texture at a UVW coordinate. Use for volume-fog density, caustic LUTs, precomputed scattering tables, 3D noise bakes, LUT color-grading tables. UVW in [0..1]^3 indexes directly into the volume.",
    pins: || vec![
        PinTemplate::input("uvw", "UVW", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
        PinTemplate::output("r", "R", PinType::Float),
        PinTemplate::output("g", "G", PinType::Float),
        PinTemplate::output("b", "B", PinType::Float),
        PinTemplate::output("a", "Alpha", PinType::Float),
    ],
    color: CLR_TEXTURE,
};
