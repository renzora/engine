//! Scene nodes — what the material can learn about the frame around it: the
//! depth, normal and motion-vector prepasses, screen UVs, refraction offsets and
//! the environment cubemap.

use crate::material::graph::{PinTemplate, PinType, PinValue};

use super::{MaterialNodeDef, CAT_SCENE, CLR_SCENE};

pub static PIXEL_DEPTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/pixel_depth",
    display_name: "Pixel Depth",
    category: CAT_SCENE,
    description: "Linear view-space depth of this fragment (distance from camera in scene units).",
    pins: || vec![PinTemplate::output("depth", "Depth", PinType::Float)],
    color: CLR_SCENE,
};

pub static SCENE_DEPTH: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/scene_depth",
    display_name: "Scene Depth",
    category: CAT_SCENE,
    description: "Reads the opaque-pass depth buffer at this fragment (linear view space). Requires DepthPrepass on the camera; returns a large value when prepass is unavailable.",
    pins: || vec![
        PinTemplate::output("depth", "Depth", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static SCENE_DEPTH_FADE: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/depth_fade",
    display_name: "Scene Depth Fade",
    category: CAT_SCENE,
    description: "Proximity fade to nearest opaque surface: 0 at contact, 1 when scene is at least `distance` units behind. For shoreline foam, soft intersection, etc.",
    pins: || vec![
        PinTemplate::input("distance", "Distance", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("fade", "Fade", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static SCENE_NORMAL: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/scene_normal",
    display_name: "Scene Normal",
    category: CAT_SCENE,
    description: "Reads the world-space normal from Bevy's normal prepass at this fragment. Requires NormalPrepass on the camera; returns +Y when prepass is unavailable. Useful for wetness masks, surface-aware post-effects, and edge detection.",
    pins: || vec![
        PinTemplate::output("normal", "Normal", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

pub static MOTION_VECTOR: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/motion_vector",
    display_name: "Motion Vector",
    category: CAT_SCENE,
    description: "Reads the per-fragment screen-space motion vector (Δ NDC since last frame) from Bevy's motion vector prepass. Requires MotionVectorPrepass on the camera. Magnitude drives motion-sensitive effects — motion blur masks, speed lines, velocity-warped distortion.",
    pins: || vec![
        PinTemplate::output("velocity", "Velocity", PinType::Vec2),
        PinTemplate::output("speed", "Speed", PinType::Float),
    ],
    color: CLR_SCENE,
};

pub static REFRACTION_UV_OFFSET: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/refraction_uv_offset",
    display_name: "Refraction UV Offset",
    category: CAT_SCENE,
    description: "Compute a screen-UV offset for refraction based on a distorting normal and strength. Feed into a Scene Color node (Phase D.2) or sample a custom render target.",
    pins: || vec![
        PinTemplate::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 1.0])),
        PinTemplate::input("strength", "Strength", PinType::Float).with_default(PinValue::Float(0.05)),
        PinTemplate::output("offset", "UV Offset", PinType::Vec2),
    ],
    color: CLR_SCENE,
};

pub static SCREEN_UV: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/screen_uv",
    display_name: "Screen UV",
    category: CAT_SCENE,
    description:
        "Fragment's screen-space UV (0,0 top-left → 1,1 bottom-right). For screen-space effects.",
    pins: || vec![PinTemplate::output("uv", "UV", PinType::Vec2)],
    color: CLR_SCENE,
};

pub static SCENE_COLOR_STUB: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/scene_color",
    display_name: "Scene Color (stub)",
    category: CAT_SCENE,
    description: "NOT IMPLEMENTED — Bevy doesn't expose a grab-pass texture to custom Material trait shaders without a custom render graph node. Returns magenta as a placeholder. Needs Phase D.2 render-graph work to enable.",
    pins: || vec![
        PinTemplate::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.5, 0.5])),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

pub static ENV_MAP_SAMPLE: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/env_map_sample",
    display_name: "Environment Map Sample",
    category: CAT_SCENE,
    description: "Sample the scene's environment cubemap at a given world-space direction and LOD. Works with both manually-loaded skyboxes (`Skybox` component) and Bevy's procedural atmosphere (dynamic sky baked to env map each frame). LOD 0 = sharpest mip, higher = blurrier (matches roughness-based reflections).",
    pins: || vec![
        PinTemplate::input("direction", "Direction", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("mip_level", "Mip Level", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_SCENE,
};

pub static ENV_MAP_REFLECT: MaterialNodeDef = MaterialNodeDef {
    node_type: "scene/env_map_reflect",
    display_name: "Environment Map Reflect",
    category: CAT_SCENE,
    description: "Compute reflection from view direction off world_normal and sample the environment cubemap — classic mirror/glossy reflection. LOD controls glossiness (0 = perfect mirror, higher = matte).",
    pins: || vec![
        PinTemplate::input("normal", "Normal", PinType::Vec3).with_default(PinValue::Vec3([0.0, 1.0, 0.0])),
        PinTemplate::input("mip_level", "Mip Level", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("color", "Color", PinType::Color),
        PinTemplate::output("rgb", "RGB", PinType::Vec3),
    ],
    color: CLR_SCENE,
};
