//! Landscape nodes: what a terrain's material is built from.
//!
//! A terrain is textured by one ordinary Surface graph. These nodes are the
//! part of it that knows about the terrain: texture coordinates that run on
//! across chunk edges, the painted weight of each named layer, and the blend
//! that mixes the layers by those weights. On a mesh that is not a terrain they
//! still compile and draw sensibly: the coordinates are world `xz`, and every
//! weight is zero except the first layer's, which is one.
//!
//! Pins cannot be added at run time in this graph model, so Layer Blend
//! declares eight layers' worth of fixed pins and ignores the ones with nothing
//! wired into their `layer_N` input. Each layer's name and blend type are
//! String pins (edited in the node inspector, never wired), which is how
//! parameter nodes carry their names too.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_LANDSCAPE, CLR_LANDSCAPE};

/// How many layers one Layer Blend node mixes, and how many weight channels a
/// terrain stores. The two are the same number on purpose: a material can
/// never name more layers than a terrain can paint.
pub const LANDSCAPE_MAX_LAYERS: usize = 8;

pub static LANDSCAPE_LAYER_COORDS: MaterialNodeDef = MaterialNodeDef {
    node_type: "landscape/layer_coords",
    display_name: "Landscape Layer Coords",
    category: CAT_LANDSCAPE,
    description: "Texture coordinates across the whole terrain, in metres from its corner: Mapping Scale is the ground one repeat covers. Unlike a chunk's own UVs they never restart at a chunk edge. On a mesh that is not a terrain they are world X/Z.",
    pins: || {
        vec![
            PinTemplate::input("scale", "Mapping Scale (m)", PinType::Float)
                .with_default(PinValue::Float(4.0)),
            PinTemplate::input("rotation", "Rotation (deg)", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("pan", "Pan (m)", PinType::Vec2)
                .with_default(PinValue::Vec2([0.0, 0.0])),
            PinTemplate::output("uv", "UV", PinType::Vec2),
        ]
    },
    color: CLR_LANDSCAPE,
};

pub static LANDSCAPE_LAYER_BLEND: MaterialNodeDef = MaterialNodeDef {
    node_type: "landscape/layer_blend",
    display_name: "Layer Blend",
    category: CAT_LANDSCAPE,
    description: "Mixes up to eight named layers by the weights painted on the terrain. Wire one value per layer (a colour, a normal or a number); unwired layers are ignored. Blend is Weight (an even mix), Height (the taller layer wins where two meet, using its Height input) or Alpha (laid over the rest by its weight). Auto Weight paints a layer on by rule, for example rock above a slope, on top of what was painted by hand. The names are what the terrain's Paint tab lists.",
    pins: layer_blend_pins,
    color: CLR_LANDSCAPE,
};

pub static LANDSCAPE_LAYER_SAMPLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "landscape/layer_sample",
    display_name: "Layer Sample",
    category: CAT_LANDSCAPE,
    description: "The painted weight of one named layer, 0 to 1, to drive anything else: wet sand by the shore, darker grass under trees, a puddle mask. A name no Layer Blend uses is a layer of its own, and still paintable.",
    pins: || {
        vec![
            PinTemplate::input("name", "Layer Name", PinType::String)
                .with_default(PinValue::String("Layer 1".to_string())),
            PinTemplate::output("weight", "Weight", PinType::Float),
        ]
    },
    color: CLR_LANDSCAPE,
};

pub static LANDSCAPE_SLOPE: MaterialNodeDef = MaterialNodeDef {
    node_type: "landscape/slope",
    display_name: "Landscape Slope",
    category: CAT_LANDSCAPE,
    description: "The ground's slope in degrees from flat (0 is level, 90 a wall) and its height in world metres: what auto rules such as rock on cliffs or snow on peaks are built from.",
    pins: || {
        vec![
            PinTemplate::output("degrees", "Slope (deg)", PinType::Float),
            PinTemplate::output("height", "Height (m)", PinType::Float),
            PinTemplate::output("normal_y", "Normal Y", PinType::Float),
        ]
    },
    color: CLR_LANDSCAPE,
};

/// Eight layers of `name`, `blend`, `layer`, `height` and `auto` pins, then
/// the node-wide height blending. Pin names are `<field>_<index>`.
fn layer_blend_pins() -> Vec<PinTemplate> {
    let mut pins = Vec::with_capacity(LANDSCAPE_MAX_LAYERS * 5 + 5);
    for i in 0..LANDSCAPE_MAX_LAYERS {
        let n = i + 1;
        pins.push(
            PinTemplate::input(&format!("name_{i}"), &format!("Layer {n} Name"), PinType::String)
                .with_default(PinValue::String(format!("Layer {n}"))),
        );
        pins.push(
            PinTemplate::input(&format!("blend_{i}"), &format!("Layer {n} Blend (Weight/Height/Alpha)"), PinType::String)
                .with_default(PinValue::String("Weight".to_string())),
        );
        pins.push(PinTemplate::input(&format!("layer_{i}"), &format!("Layer {n}"), PinType::Vec4));
        pins.push(
            PinTemplate::input(&format!("height_{i}"), &format!("Layer {n} Height"), PinType::Float)
                .with_default(PinValue::Float(0.5)),
        );
        pins.push(
            PinTemplate::input(&format!("auto_{i}"), &format!("Layer {n} Auto Weight"), PinType::Float)
                .with_default(PinValue::Float(0.0)),
        );
    }
    pins.push(
        PinTemplate::input("height_blending", "Height Blending", PinType::Float)
            .with_default(PinValue::Float(0.8)),
    );
    pins.push(PinTemplate::output("result", "Result", PinType::Vec4));
    pins.push(PinTemplate::output("rgb", "RGB", PinType::Vec3));
    pins.push(PinTemplate::output("normal", "Normal", PinType::Vec3));
    pins.push(PinTemplate::output("value", "Value", PinType::Float));
    pins
}
