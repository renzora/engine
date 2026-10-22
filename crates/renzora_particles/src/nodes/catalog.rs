//! The catalog table. One entry per node type; see the parent module for what
//! the fields mean and [`crate::compile`] for what each node computes.

use super::{NodeDef, OutputDef, PinDef, PinDefault, PinKind, PropDef, PropKind};
use crate::ir::{Attribute, ValueType};

const F: ValueType = ValueType::Float;
const V2: ValueType = ValueType::Vec2;
const V3: ValueType = ValueType::Vec3;
const C: ValueType = ValueType::Color;

const fn float(name: &'static str, label: &'static str, v: f32) -> PinDef {
    PinDef {
        name,
        label,
        kind: PinKind::Fixed(F),
        default: PinDefault::Value([v, 0.0, 0.0, 0.0]),
    }
}

const fn vec2(name: &'static str, label: &'static str, x: f32, y: f32) -> PinDef {
    PinDef {
        name,
        label,
        kind: PinKind::Fixed(V2),
        default: PinDefault::Value([x, y, 0.0, 0.0]),
    }
}

const fn vec3(name: &'static str, label: &'static str, x: f32, y: f32, z: f32) -> PinDef {
    PinDef {
        name,
        label,
        kind: PinKind::Fixed(V3),
        default: PinDefault::Value([x, y, z, 0.0]),
    }
}

const fn color(name: &'static str, label: &'static str, c: [f32; 4]) -> PinDef {
    PinDef {
        name,
        label,
        kind: PinKind::Fixed(C),
        default: PinDefault::Value(c),
    }
}

/// A generic input. Its default is a Float, so an unconnected generic pin with
/// no inline value is a scalar and the node's type comes from its other pins.
const fn any(name: &'static str, label: &'static str, v: f32) -> PinDef {
    PinDef {
        name,
        label,
        kind: PinKind::Generic,
        default: PinDefault::Value([v, v, v, v]),
    }
}

const fn attr(name: &'static str, label: &'static str, ty: ValueType, a: Attribute) -> PinDef {
    PinDef {
        name,
        label,
        kind: PinKind::Fixed(ty),
        default: PinDefault::Attr(a),
    }
}

const fn out(name: &'static str, label: &'static str, ty: ValueType) -> OutputDef {
    OutputDef {
        name,
        label,
        kind: PinKind::Fixed(ty),
    }
}

const fn out_any(name: &'static str, label: &'static str) -> OutputDef {
    OutputDef {
        name,
        label,
        kind: PinKind::Generic,
    }
}

const SALT: PinDef = float("salt", "Salt", 0.0);
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const SHAPE_OUT: &[OutputDef] = &[out("position", "Position", V3), out("direction", "Direction", V3)];
const VALUE_F: &[OutputDef] = &[out("value", "Value", F)];
const VALUE_V3: &[OutputDef] = &[out("value", "Value", V3)];
const VALUE_ANY: &[OutputDef] = &[out_any("value", "Value")];
const FORCE_OUT: &[OutputDef] = &[out("force", "Force", V3)];
const UNARY_ANY: &[PinDef] = &[any("value", "Value", 0.0)];
const BINARY_ANY: &[PinDef] = &[any("a", "A", 0.0), any("b", "B", 0.0)];
const BINARY_ANY_ONE: &[PinDef] = &[any("a", "A", 1.0), any("b", "B", 1.0)];
const KILL_PROP: PropDef = PropDef {
    name: "kill",
    label: "Kill On Hit",
    kind: PropKind::Bool(false),
};
const WORLD_PROP: PropDef = PropDef {
    name: "world",
    label: "World Space",
    kind: PropKind::Bool(false),
};
const COLLIDE_PROPS: &[PropDef] = &[KILL_PROP, WORLD_PROP];
const COLLIDE_CONTAIN_PROPS: &[PropDef] = &[
    PropDef {
        name: "inside",
        label: "Inside",
        kind: PropKind::Bool(false),
    },
    KILL_PROP,
    WORLD_PROP,
];

const fn node(
    node_type: &'static str,
    display_name: &'static str,
    category: &'static str,
    description: &'static str,
    inputs: &'static [PinDef],
    outputs: &'static [OutputDef],
) -> NodeDef {
    NodeDef {
        node_type,
        display_name,
        category,
        description,
        inputs,
        outputs,
        props: &[],
    }
}

const fn with_props(mut def: NodeDef, props: &'static [PropDef]) -> NodeDef {
    def.props = props;
    def
}

pub static NODES: &[NodeDef] = &[
    // ── Sink ────────────────────────────────────────────────────────────────
    node(
        "output/particle",
        "Particle Output",
        "Output",
        "What the emitter does. Spawn pins run once per step, init pins once per particle at \
         birth, update pins every step for every particle.",
        &[
            float("spawn_rate", "Spawn Rate", 10.0),
            float("burst_count", "Burst Count", 0.0),
            float("burst_interval", "Burst Interval", 1.0),
            float("burst_cycles", "Burst Cycles", 1.0),
            float("lifetime", "Lifetime", 1.0),
            vec3("position", "Position", 0.0, 0.0, 0.0),
            vec3("velocity", "Velocity", 0.0, 0.0, 0.0),
            vec2("size", "Size", 0.1, 0.1),
            float("rotation", "Rotation", 0.0),
            float("angular_velocity", "Angular Velocity", 0.0),
            color("color", "Color", WHITE),
            vec3("acceleration", "Acceleration", 0.0, 0.0, 0.0),
            vec3("gravity", "Gravity", 0.0, 0.0, 0.0),
            float("drag", "Drag", 0.0),
            vec2("size_over_life", "Size Over Life", 1.0, 1.0),
            color("color_over_life", "Color Over Life", WHITE),
            float("max_speed", "Max Speed", 0.0),
            float("emission", "Emission", 1.0),
        ],
        &[],
    ),
    // ── Shapes ──────────────────────────────────────────────────────────────
    node(
        "shape/point",
        "Point",
        "Shape",
        "Every particle starts at the origin; direction is a random unit vector.",
        &[],
        SHAPE_OUT,
    ),
    with_props(
        node(
            "shape/sphere",
            "Sphere",
            "Shape",
            "A random point in (or on) a sphere; direction points outward.",
            &[float("radius", "Radius", 1.0)],
            SHAPE_OUT,
        ),
        &[PropDef {
            name: "surface",
            label: "Surface Only",
            kind: PropKind::Bool(false),
        }],
    ),
    node(
        "shape/box",
        "Box",
        "Shape",
        "A random point in a box centred on the origin; direction points outward.",
        &[vec3("size", "Size", 1.0, 1.0, 1.0)],
        SHAPE_OUT,
    ),
    node(
        "shape/cone",
        "Cone",
        "Shape",
        "A random point on a disc of the given radius in the XZ plane; direction is a random \
         unit vector within the cone's half-angle (degrees) around +Y.",
        &[float("angle", "Angle", 25.0), float("radius", "Radius", 0.5)],
        SHAPE_OUT,
    ),
    with_props(
        node(
            "shape/circle",
            "Circle",
            "Shape 2D",
            "A random point in (or on) a circle in the XY plane; direction points outward.",
            &[float("radius", "Radius", 1.0)],
            SHAPE_OUT,
        ),
        &[PropDef {
            name: "edge",
            label: "Edge Only",
            kind: PropKind::Bool(false),
        }],
    ),
    node(
        "shape/rect",
        "Rectangle",
        "Shape 2D",
        "A random point in a rectangle in the XY plane centred on the origin; direction points \
         outward.",
        &[vec2("size", "Size", 1.0, 1.0)],
        SHAPE_OUT,
    ),
    // ── Inputs ──────────────────────────────────────────────────────────────
    node("input/age", "Age", "Input", "Seconds since the particle was born.", &[], VALUE_F),
    node("input/life", "Life", "Input", "Age over lifetime: 0 at birth, 1 at death.", &[], VALUE_F),
    node("input/lifetime", "Lifetime", "Input", "The particle's lifetime in seconds.", &[], VALUE_F),
    node("input/position", "Position", "Input", "The particle's position.", &[], VALUE_V3),
    node("input/velocity", "Velocity", "Input", "The particle's velocity.", &[], VALUE_V3),
    node(
        "input/index",
        "Index",
        "Input",
        "The particle's spawn index, counting from 0.",
        &[],
        VALUE_F,
    ),
    node(
        "input/particle_random",
        "Particle Random",
        "Input",
        "A random number in 0..1 that stays the same for the particle's whole life. Change the \
         salt for an independent one.",
        &[SALT],
        VALUE_F,
    ),
    node(
        "input/emitter_time",
        "Emitter Time",
        "Input",
        "Seconds into the current cycle of the effect.",
        &[],
        VALUE_F,
    ),
    node("input/delta_time", "Delta Time", "Input", "This step's length in seconds.", &[], VALUE_F),
    node(
        "input/emitter_velocity",
        "Emitter Velocity",
        "Input",
        "World-space velocity of the entity. Add it to the spawn velocity to inherit motion.",
        &[],
        VALUE_V3,
    ),
    with_props(
        node(
            "input/parameter",
            "Parameter",
            "Input",
            "A value a script sets by name while the effect plays (vfx_set_param). Until one does, \
             it is Value: a float, or a vector or colour if you give it one. Any number of nodes may \
             share a name; one call sets them all.",
            &[any("value", "Value", 0.0)],
            VALUE_ANY,
        ),
        &[PropDef {
            name: "name",
            label: "Name",
            kind: PropKind::Text("param"),
        }],
    ),
    node(
        "input/wind",
        "Wind",
        "Input",
        "The world wind in m/s, gusts included, plus a cross-wind wobble from its turbulence that \
         differs per particle. Strength is the speed over 12 m/s (a fresh breeze). Dead calm \
         when the scene has no wind.",
        &[float("scale", "Scale", 1.0), float("frequency", "Wobble Hz", WIND_WOBBLE_HZ)],
        &[out("velocity", "Velocity", V3), out("strength", "Strength", F)],
    ),
    // ── Random ──────────────────────────────────────────────────────────────
    node(
        "random/range",
        "Random Range",
        "Random",
        "A random float between min and max, fixed per particle.",
        &[float("min", "Min", 0.0), float("max", "Max", 1.0), SALT],
        VALUE_F,
    ),
    node(
        "random/vec3_range",
        "Random Vec3",
        "Random",
        "A random vector between min and max per component, fixed per particle.",
        &[
            vec3("min", "Min", -1.0, -1.0, -1.0),
            vec3("max", "Max", 1.0, 1.0, 1.0),
            SALT,
        ],
        VALUE_V3,
    ),
    node(
        "random/unit_vector",
        "Random Direction",
        "Random",
        "A uniformly distributed random unit vector, fixed per particle.",
        &[SALT],
        VALUE_V3,
    ),
    node(
        "random/color_between",
        "Random Color",
        "Random",
        "A random colour on the line between two colours, fixed per particle.",
        &[color("a", "A", WHITE), color("b", "B", WHITE), SALT],
        &[out("color", "Color", C)],
    ),
    // ── Math ────────────────────────────────────────────────────────────────
    node("math/float", "Float", "Math", "A constant float.", &[float("value", "Value", 0.0)], VALUE_F),
    node(
        "math/vec3",
        "Vec3",
        "Math",
        "A constant vector.",
        &[vec3("value", "Value", 0.0, 0.0, 0.0)],
        VALUE_V3,
    ),
    node(
        "math/color",
        "Color",
        "Math",
        "A constant colour.",
        &[color("value", "Value", WHITE)],
        &[out("value", "Value", C)],
    ),
    node("math/add", "Add", "Math", "a + b", BINARY_ANY, VALUE_ANY),
    node("math/subtract", "Subtract", "Math", "a - b", BINARY_ANY, VALUE_ANY),
    node("math/multiply", "Multiply", "Math", "a * b", BINARY_ANY_ONE, VALUE_ANY),
    node(
        "math/divide",
        "Divide",
        "Math",
        "a / b. Division by zero gives zero.",
        BINARY_ANY_ONE,
        VALUE_ANY,
    ),
    node("math/min", "Min", "Math", "The smaller of a and b, per component.", BINARY_ANY, VALUE_ANY),
    node("math/max", "Max", "Math", "The larger of a and b, per component.", BINARY_ANY, VALUE_ANY),
    node(
        "math/lerp",
        "Lerp",
        "Math",
        "a + (b - a) * t",
        &[any("a", "A", 0.0), any("b", "B", 1.0), any("t", "T", 0.5)],
        VALUE_ANY,
    ),
    node(
        "math/clamp",
        "Clamp",
        "Math",
        "Keep value between min and max.",
        &[any("value", "Value", 0.0), any("min", "Min", 0.0), any("max", "Max", 1.0)],
        VALUE_ANY,
    ),
    node("math/abs", "Abs", "Math", "Absolute value.", UNARY_ANY, VALUE_ANY),
    node("math/negate", "Negate", "Math", "-value", UNARY_ANY, VALUE_ANY),
    node("math/sin", "Sin", "Math", "Sine, radians.", UNARY_ANY, VALUE_ANY),
    node("math/cos", "Cos", "Math", "Cosine, radians.", UNARY_ANY, VALUE_ANY),
    node(
        "math/smoothstep",
        "Smoothstep",
        "Math",
        "0 below edge0, 1 above edge1, a smooth curve between.",
        &[
            any("edge0", "Edge 0", 0.0),
            any("edge1", "Edge 1", 1.0),
            any("value", "Value", 0.5),
        ],
        VALUE_ANY,
    ),
    node(
        "math/remap",
        "Remap",
        "Math",
        "Map value from in_min..in_max onto out_min..out_max (not clamped).",
        &[
            any("value", "Value", 0.0),
            any("in_min", "In Min", 0.0),
            any("in_max", "In Max", 1.0),
            any("out_min", "Out Min", 0.0),
            any("out_max", "Out Max", 1.0),
        ],
        VALUE_ANY,
    ),
    node("math/length", "Length", "Math", "Length of a vector.", UNARY_ANY, VALUE_F),
    node(
        "math/normalize",
        "Normalize",
        "Math",
        "Unit vector in the same direction; zero stays zero.",
        UNARY_ANY,
        VALUE_ANY,
    ),
    node("math/dot", "Dot", "Math", "Dot product.", BINARY_ANY, VALUE_F),
    node(
        "math/compose_vec3",
        "Compose Vec3",
        "Math",
        "Build a vector from three floats.",
        &[float("x", "X", 0.0), float("y", "Y", 0.0), float("z", "Z", 0.0)],
        VALUE_V3,
    ),
    node(
        "math/split_vec3",
        "Split Vec3",
        "Math",
        "Take a vector apart.",
        &[vec3("value", "Value", 0.0, 0.0, 0.0)],
        &[out("x", "X", F), out("y", "Y", F), out("z", "Z", F)],
    ),
    // ── Curves ──────────────────────────────────────────────────────────────
    with_props(
        node(
            "curve/curve",
            "Curve",
            "Curve",
            "Sample a curve at t (the particle's life unless wired). Keys carry their easing.",
            &[attr("t", "T", F, Attribute::Life)],
            VALUE_F,
        ),
        &[PropDef {
            name: "points",
            label: "Points",
            kind: PropKind::Curve,
        }],
    ),
    with_props(
        node(
            "curve/gradient",
            "Gradient",
            "Curve",
            "Sample a colour gradient at t (the particle's life unless wired).",
            &[attr("t", "T", F, Attribute::Life)],
            &[out("color", "Color", C)],
        ),
        &[PropDef {
            name: "stops",
            label: "Stops",
            kind: PropKind::Gradient,
        }],
    ),
    // ── Forces ──────────────────────────────────────────────────────────────
    node(
        "force/curl_noise",
        "Curl Noise",
        "Force",
        "Swirling turbulence that neither bunches particles up nor thins them out. Octaves add \
         finer swirls on top, each Lacunarity times the frequency and Persistence times the \
         strength of the last. Wire into acceleration.",
        &[
            attr("position", "Position", V3, Attribute::Position),
            float("strength", "Strength", 1.0),
            float("frequency", "Frequency", 1.0),
            vec3("scroll", "Scroll", 0.0, 0.0, 0.0),
            float("octaves", "Octaves", 1.0),
            float("lacunarity", "Lacunarity", 2.0),
            float("persistence", "Persistence", 0.5),
        ],
        FORCE_OUT,
    ),
    node(
        "force/speed_limit",
        "Speed Limit",
        "Force",
        "Brakes a particle faster than Limit back down to it, at Rate per second (higher is \
         firmer). Slower particles are left alone. For a hard cap use the output's Max Speed.",
        &[
            attr("velocity", "Velocity", V3, Attribute::Velocity),
            float("limit", "Limit", 5.0),
            float("rate", "Rate", 8.0),
        ],
        FORCE_OUT,
    ),
    node(
        "force/drag",
        "Drag",
        "Force",
        "Air resistance: linear drag plus quadratic drag that grows with speed, so a fast \
         particle slows sharply and settles at a terminal speed. Stable at any strength.",
        &[
            attr("velocity", "Velocity", V3, Attribute::Velocity),
            float("linear", "Linear", 0.0),
            float("quadratic", "Quadratic", 0.5),
        ],
        FORCE_OUT,
    ),
    node(
        "force/attractor",
        "Attractor",
        "Force",
        "Pull toward a point (negative strength pushes). Falloff weakens it with distance \
         squared. Never pulls a particle past the point in one step, so it settles there at any \
         frame rate. For real gravity use Gravity Well.",
        &[
            attr("position", "Position", V3, Attribute::Position),
            vec3("point", "Point", 0.0, 0.0, 0.0),
            float("strength", "Strength", 1.0),
            float("falloff", "Falloff", 0.0),
        ],
        FORCE_OUT,
    ),
    node(
        "force/vortex",
        "Vortex",
        "Force",
        "Spin around an axis through a centre point, faster further out.",
        &[
            attr("position", "Position", V3, Attribute::Position),
            vec3("center", "Center", 0.0, 0.0, 0.0),
            vec3("axis", "Axis", 0.0, 1.0, 0.0),
            float("strength", "Strength", 1.0),
        ],
        FORCE_OUT,
    ),
    node(
        "force/wind",
        "Wind Force",
        "Force",
        "Pulls the particle's velocity toward the world wind: (wind - velocity) * coupling. Light \
         particles (smoke) take a high coupling and ride the wind; heavy ones (embers) a low one \
         and barely lean. It is drag through moving air, so use it instead of the output's drag. \
         Height Falloff fades the wind toward y = 0 below that height (0 is off).",
        &[
            attr("position", "Position", V3, Attribute::Position),
            attr("velocity", "Velocity", V3, Attribute::Velocity),
            attr("wind", "Wind", V3, Attribute::Wind),
            float("coupling", "Coupling", 1.0),
            float("height_falloff", "Height Falloff", 0.0),
        ],
        FORCE_OUT,
    ),
    node(
        "force/gravity_well",
        "Gravity Well",
        "Force",
        "Inverse-square pull toward Point: Strength is the pull at 1 m, a quarter of it at 2 m \
         (negative pushes). Within Softening it eases off to nothing at the centre, so nothing \
         is flung out, and Max Force caps it (0 is no cap). Stable at any frame rate.",
        &[
            attr("position", "Position", V3, Attribute::Position),
            attr("velocity", "Velocity", V3, Attribute::Velocity),
            vec3("point", "Point", 0.0, 0.0, 0.0),
            float("strength", "Strength", 10.0),
            float("softening", "Softening", 0.25),
            float("max_force", "Max Force", 0.0),
        ],
        FORCE_OUT,
    ),
    node(
        "force/orbit",
        "Orbit",
        "Force",
        "Circles particles around Axis through Center at Speed radians per second, holding them \
         at their distance. Grip is how fast they reach that speed (per second). With a Radius, \
         Radial Pull draws them onto a ring of it without overshooting. Stable at any frame rate.",
        &[
            attr("position", "Position", V3, Attribute::Position),
            attr("velocity", "Velocity", V3, Attribute::Velocity),
            vec3("center", "Center", 0.0, 0.0, 0.0),
            vec3("axis", "Axis", 0.0, 1.0, 0.0),
            float("speed", "Speed", 1.0),
            float("grip", "Grip", 4.0),
            float("radius", "Radius", 0.0),
            float("radial_pull", "Radial Pull", 4.0),
        ],
        FORCE_OUT,
    ),
    node(
        "force/conform_sphere",
        "Conform To Sphere",
        "Force",
        "Draws particles within Influence of a sphere's surface onto it at up to Max Speed, then \
         holds them there while they slide around it: Hanabi's conform to sphere. Attraction is \
         the pull, multiplied by Sticky inside a Shell of that half-thickness so a fast particle \
         sticks rather than overshoots. Wire last into acceleration.",
        &[
            attr("position", "Position", V3, Attribute::Position),
            attr("velocity", "Velocity", V3, Attribute::Velocity),
            vec3("center", "Center", 0.0, 0.0, 0.0),
            float("radius", "Radius", 1.0),
            float("influence", "Influence", 3.0),
            float("attraction", "Attraction", 5.0),
            float("max_speed", "Max Speed", 2.0),
            float("sticky", "Sticky", CONFORM_STICKY),
            float("shell", "Shell", CONFORM_SHELL),
        ],
        FORCE_OUT,
    ),
    // ── Collision ───────────────────────────────────────────────────────────
    // Sinks: they act by being in the graph and have nothing to wire out. See
    // `crate::ir::ColliderIr`.
    with_props(
        node(
            "collide/plane",
            "Collide Plane",
            "Collision",
            "An infinite plane through Point, facing Normal, in the effect's own space. Particles \
             stay on the side the normal points to and bounce off (or die with Kill On Hit). Acts \
             without being wired.",
            &[
                vec3("point", "Point", 0.0, 0.0, 0.0),
                vec3("normal", "Normal", 0.0, 1.0, 0.0),
                float("bounce", "Bounce", 0.4),
                float("friction", "Friction", 0.3),
            ],
            &[],
        ),
        COLLIDE_PROPS,
    ),
    with_props(
        node(
            "collide/sphere",
            "Collide Sphere",
            "Collision",
            "A solid ball particles bounce off, or with Inside a bowl that keeps them in. In the \
             effect's own space. Acts without being wired.",
            &[
                vec3("center", "Center", 0.0, 0.0, 0.0),
                float("radius", "Radius", 1.0),
                float("bounce", "Bounce", 0.4),
                float("friction", "Friction", 0.3),
            ],
            &[],
        ),
        COLLIDE_CONTAIN_PROPS,
    ),
    with_props(
        node(
            "collide/box",
            "Collide Box",
            "Collision",
            "An axis-aligned solid box particles bounce off, or with Inside a room that keeps them \
             in. In the effect's own space. Acts without being wired.",
            &[
                vec3("center", "Center", 0.0, 0.0, 0.0),
                vec3("size", "Size", 1.0, 1.0, 1.0),
                float("bounce", "Bounce", 0.4),
                float("friction", "Friction", 0.3),
            ],
            &[],
        ),
        COLLIDE_CONTAIN_PROPS,
    ),
    with_props(
        node(
            "collide/scene",
            "Collide Scene",
            "Collision",
            "The scene's own physics colliders: the level, props, anything with a collider. \
             Particles bounce off them (or die with Kill On Hit); Particle Radius keeps a fat one's \
             edge on the surface. Runs on the CPU, so an emitter using it stays there. Acts without \
             being wired.",
            &[
                float("radius", "Particle Radius", 0.0),
                float("bounce", "Bounce", 0.4),
                float("friction", "Friction", 0.3),
            ],
            &[],
        ),
        &[KILL_PROP],
    ),
    // ── Colour ──────────────────────────────────────────────────────────────
    node(
        "color/blackbody",
        "Blackbody",
        "Color",
        "The colour a body glows at a temperature in kelvin: 800 deep red, 1800 orange, 3500 \
         yellow-white, 6500 white. Intensity scales the colour (above 1 for HDR glow); alpha is 1.",
        &[float("temperature", "Kelvin", 1800.0), float("intensity", "Intensity", 1.0)],
        &[out("color", "Color", C)],
    ),
];

/// The default cross-wind wobble rate of the wind nodes, in Hz. Slow enough to
/// read as eddies drifting past rather than a vibration.
pub const WIND_WOBBLE_HZ: f32 = 0.6;

/// Conform To Sphere's Sticky and Shell defaults: Hanabi's own, for a
/// `ConformToSphereModifier` built without them, so a converted attractor
/// behaves as the old one did.
pub const CONFORM_STICKY: f32 = 2.0;
pub const CONFORM_SHELL: f32 = 0.1;
