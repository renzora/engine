//! Output nodes — the terminal of every graph. One per domain: the full PBR
//! surface, a terrain splat layer, vegetation (surface + vertex offset), and
//! unlit.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_OUTPUT, CLR_OUTPUT};

pub static OUTPUT_SURFACE: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/surface",
    display_name: "Surface Output",
    category: CAT_OUTPUT,
    description: "Full PBR surface material output — maps 1:1 onto StandardMaterial. Connect specular_transmission + ior for glass/water; clearcoat + clearcoat_roughness for car paint; anisotropy_strength + anisotropy_rotation for brushed metal / hair; diffuse_transmission + thickness for foliage / skin. Disconnected pins leave StandardMaterial defaults intact.",
    pins: || vec![
        // Core PBR
        PinTemplate::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([0.8, 0.8, 0.8, 1.0])),
        PinTemplate::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("normal", "Normal", PinType::Vec3),
        // Parallax occlusion mapping. `displacement` is a HEIGHT: white/1.0 is
        // the peak, black/0.0 the valley — the convention every PBR texture set
        // ships (and the inverse of Bevy's own `depth_map`, which is why a
        // connected displacement pin always compiles through codegen instead of
        // the StandardMaterial fast path). Only a *connection* does anything: a
        // constant height is the same surface everywhere and has no relief to
        // march through. `displacement_scale` is read as a literal, not an
        // expression — it is a loop constant, resolved before the graph body runs.
        PinTemplate::input("displacement", "Displacement (Height)", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("displacement_scale", "Displacement Scale", PinType::Float).with_default(PinValue::Float(0.05)),
        PinTemplate::input("emissive", "Emissive", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::input("ao", "Ambient Occlusion (AO)", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("reflectance", "Reflectance", PinType::Vec3).with_default(PinValue::Vec3([0.5, 0.5, 0.5])),

        // Transmission (refraction — connect for glass, water, ice)
        PinTemplate::input("specular_transmission", "Specular Transmission", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("diffuse_transmission", "Diffuse Transmission", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("thickness", "Thickness", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("ior", "Index of Refraction", PinType::Float).with_default(PinValue::Float(1.5)),
        PinTemplate::input("attenuation_distance", "Attenuation Distance", PinType::Float).with_default(PinValue::Float(1.0e37)),
        PinTemplate::input("attenuation_color", "Attenuation Color", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),

        // Clearcoat (second specular layer — car paint, lacquer)
        PinTemplate::input("clearcoat", "Clearcoat", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("clearcoat_roughness", "Clearcoat Roughness", PinType::Float).with_default(PinValue::Float(0.5)),

        // Anisotropy (directional specular — brushed metal, hair)
        PinTemplate::input("anisotropy_strength", "Anisotropy Strength", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("anisotropy_rotation", "Anisotropy Rotation", PinType::Float).with_default(PinValue::Float(0.0)),
    ],
    color: CLR_OUTPUT,
};

pub static OUTPUT_TERRAIN_LAYER: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/terrain_layer",
    display_name: "Terrain Layer Output",
    category: CAT_OUTPUT,
    description: "Terrain layer material (blended via splatmap)",
    pins: || {
        vec![
            PinTemplate::input("base_color", "Base Color", PinType::Color)
                .with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
            PinTemplate::input("metallic", "Metallic", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("roughness", "Roughness", PinType::Float)
                .with_default(PinValue::Float(0.5)),
            PinTemplate::input("normal", "Normal", PinType::Vec3),
            PinTemplate::input("height", "Height", PinType::Float)
                .with_default(PinValue::Float(0.5)),
        ]
    },
    color: CLR_OUTPUT,
};

pub static OUTPUT_VEGETATION: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/vegetation",
    display_name: "Vegetation Output",
    category: CAT_OUTPUT,
    description: "PBR surface + vertex displacement",
    pins: || {
        vec![
            PinTemplate::input("base_color", "Base Color", PinType::Color)
                .with_default(PinValue::Color([0.2, 0.5, 0.1, 1.0])),
            PinTemplate::input("metallic", "Metallic", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("roughness", "Roughness", PinType::Float)
                .with_default(PinValue::Float(0.7)),
            PinTemplate::input("normal", "Normal", PinType::Vec3),
            PinTemplate::input("emissive", "Emissive", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::input("ao", "Ambient Occlusion (AO)", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("vertex_offset", "Vertex Offset", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        ]
    },
    color: CLR_OUTPUT,
};

pub static OUTPUT_UNLIT: MaterialNodeDef = MaterialNodeDef {
    node_type: "output/unlit",
    display_name: "Unlit Output",
    category: CAT_OUTPUT,
    description: "Unlit color output (no lighting)",
    pins: || {
        vec![
            PinTemplate::input("color", "Color", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
        ]
    },
    color: CLR_OUTPUT,
};
