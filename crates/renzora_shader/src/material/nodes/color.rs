//! Color nodes — constants, space conversions, grading and blend modes.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_COLOR, CLR_COLOR};

pub static COLOR_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/constant",
    display_name: "Color",
    category: CAT_COLOR,
    description: "Constant color value",
    pins: || {
        vec![
            PinTemplate::output("color", "Color", PinType::Color),
            PinTemplate::output("rgb", "RGB", PinType::Vec3),
            PinTemplate::output("r", "R", PinType::Float),
            PinTemplate::output("g", "G", PinType::Float),
            PinTemplate::output("b", "B", PinType::Float),
            PinTemplate::output("a", "A", PinType::Float),
        ]
    },
    color: CLR_COLOR,
};

pub static FLOAT_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/float",
    display_name: "Float",
    category: CAT_COLOR,
    description: "Constant float value",
    pins: || vec![PinTemplate::output("value", "Value", PinType::Float)],
    color: CLR_COLOR,
};

pub static VEC2_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/vec2",
    display_name: "Vec2",
    category: CAT_COLOR,
    description: "Constant Vec2 value",
    pins: || vec![PinTemplate::output("value", "Value", PinType::Vec2)],
    color: CLR_COLOR,
};

pub static VEC3_CONSTANT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/vec3",
    display_name: "Vec3",
    category: CAT_COLOR,
    description: "Constant Vec3 value",
    pins: || vec![PinTemplate::output("value", "Value", PinType::Vec3)],
    color: CLR_COLOR,
};

pub static COLOR_LERP: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/lerp",
    display_name: "Color Lerp",
    category: CAT_COLOR,
    description: "Blend between two colors",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Color)
                .with_default(PinValue::Color([0.0, 0.0, 0.0, 1.0])),
            PinTemplate::input("b", "B", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
            PinTemplate::output("color", "Color", PinType::Color),
        ]
    },
    color: CLR_COLOR,
};

pub static COSINE_PALETTE: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/cosine_palette",
    display_name: "Cosine Palette",
    category: CAT_COLOR,
    description: "IQ cosine color palette: a + b * cos(2π(c*t + d))",
    pins: || {
        vec![
            PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("a", "Bias", PinType::Vec3)
                .with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
            PinTemplate::input("b", "Amplitude", PinType::Vec3)
                .with_default(PinValue::Vec3([0.5, 0.5, 0.5])),
            PinTemplate::input("c", "Frequency", PinType::Vec3)
                .with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
            PinTemplate::input("d", "Phase", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.33, 0.67])),
            PinTemplate::output("color", "Color", PinType::Vec3),
        ]
    },
    color: CLR_COLOR,
};

pub static FRESNEL: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/fresnel",
    display_name: "Fresnel",
    category: CAT_COLOR,
    description: "View-angle dependent effect (water edges, rim light)",
    pins: || {
        vec![
            PinTemplate::input("power", "Power", PinType::Float).with_default(PinValue::Float(5.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_COLOR,
};

pub static SRGB_TO_LINEAR: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/srgb_to_linear",
    display_name: "sRGB → Linear",
    category: CAT_COLOR,
    description: "Convert sRGB-encoded color to linear (piecewise)",
    pins: || {
        vec![
            PinTemplate::input("color", "Color", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::output("result", "Result", PinType::Color),
        ]
    },
    color: CLR_COLOR,
};

pub static LINEAR_TO_SRGB: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/linear_to_srgb",
    display_name: "Linear → sRGB",
    category: CAT_COLOR,
    description: "Convert linear color to sRGB (piecewise)",
    pins: || {
        vec![
            PinTemplate::input("color", "Color", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::output("result", "Result", PinType::Color),
        ]
    },
    color: CLR_COLOR,
};

pub static RGB_TO_HSV: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/rgb_to_hsv",
    display_name: "RGB → HSV",
    category: CAT_COLOR,
    description: "Convert RGB to HSV (hue/saturation/value)",
    pins: || {
        vec![
            PinTemplate::input("rgb", "RGB", PinType::Vec3)
                .with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
            PinTemplate::output("hsv", "HSV", PinType::Vec3),
            PinTemplate::output("h", "H", PinType::Float),
            PinTemplate::output("s", "S", PinType::Float),
            PinTemplate::output("v", "V", PinType::Float),
        ]
    },
    color: CLR_COLOR,
};

pub static HSV_TO_RGB: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/hsv_to_rgb",
    display_name: "HSV → RGB",
    category: CAT_COLOR,
    description: "Convert HSV to RGB",
    pins: || {
        vec![
            PinTemplate::input("hsv", "HSV", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 1.0, 1.0])),
            PinTemplate::output("rgb", "RGB", PinType::Vec3),
        ]
    },
    color: CLR_COLOR,
};

pub static HUE_SHIFT: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/hue_shift",
    display_name: "Hue Shift",
    category: CAT_COLOR,
    description: "Rotate the hue of an RGB color by a given amount (0-1)",
    pins: || {
        vec![
            PinTemplate::input("rgb", "RGB", PinType::Vec3)
                .with_default(PinValue::Vec3([1.0, 0.0, 0.0])),
            PinTemplate::input("shift", "Shift", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("rgb", "RGB", PinType::Vec3),
        ]
    },
    color: CLR_COLOR,
};

pub static LUMINANCE: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/luminance",
    display_name: "Luminance",
    category: CAT_COLOR,
    description: "Rec.709 luminance of an RGB color",
    pins: || {
        vec![
            PinTemplate::input("rgb", "RGB", PinType::Vec3)
                .with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
            PinTemplate::output("value", "Value", PinType::Float),
        ]
    },
    color: CLR_COLOR,
};

pub static GAMMA: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/gamma",
    display_name: "Gamma",
    category: CAT_COLOR,
    description: "Apply pow(color, gamma) per channel",
    pins: || {
        vec![
            PinTemplate::input("color", "Color", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::input("gamma", "Gamma", PinType::Float).with_default(PinValue::Float(2.2)),
            PinTemplate::output("result", "Result", PinType::Color),
        ]
    },
    color: CLR_COLOR,
};

pub static BRIGHTNESS_CONTRAST: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/brightness_contrast",
    display_name: "Brightness / Contrast",
    category: CAT_COLOR,
    description: "Adjust brightness (additive) and contrast (around 0.5 gray)",
    pins: || {
        vec![
            PinTemplate::input("color", "Color", PinType::Color)
                .with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
            PinTemplate::input("brightness", "Brightness", PinType::Float)
                .with_default(PinValue::Float(0.0)),
            PinTemplate::input("contrast", "Contrast", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Color),
        ]
    },
    color: CLR_COLOR,
};

pub static SATURATION: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/saturation",
    display_name: "Saturation",
    category: CAT_COLOR,
    description: "Adjust saturation (0 = greyscale, 1 = original, >1 = supersaturated)",
    pins: || {
        vec![
            PinTemplate::input("color", "Color", PinType::Color)
                .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            PinTemplate::input("saturation", "Saturation", PinType::Float)
                .with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Color),
        ]
    },
    color: CLR_COLOR,
};

pub static BLEND: MaterialNodeDef = MaterialNodeDef {
    node_type: "color/blend",
    display_name: "Blend",
    category: CAT_COLOR,
    description: "Blend mode composite. Mode: 0=normal, 1=multiply, 2=screen, 3=overlay, 4=add, 5=subtract, 6=soft-light, 7=hard-light, 8=difference, 9=divide",
    pins: || vec![
        PinTemplate::input("base", "Base", PinType::Color).with_default(PinValue::Color([0.5, 0.5, 0.5, 1.0])),
        PinTemplate::input("blend", "Blend", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::input("opacity", "Opacity", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("mode", "Mode", PinType::Float).with_default(PinValue::Int(0)),
        PinTemplate::output("result", "Result", PinType::Color),
    ],
    color: CLR_COLOR,
};
