//! Compile-checks `grass.wgsl`.
//!
//! The grass pipeline is hand-written, and Bevy compiles its shaders at run time
//! — so a typo in the WGSL is not a build failure, it is grass that silently
//! doesn't render with one line in the log. That is a bad failure mode for a
//! shader nobody can eyeball without a GPU, so the source is parsed and
//! validated here instead, by the same front end (naga) that will compile it for
//! real.
//!
//! What this catches: syntax errors, type errors, bad swizzles, wrong argument
//! counts, entry points that don't match their declared IO. What it can't catch:
//! a mismatch between the WGSL's bindings and the Rust pipeline's bind group
//! layout, since only the driver sees both.

/// Bevy's `bevy_render::view::View`, cut down to the fields `grass.wgsl` uses.
///
/// The shader's `#import` is Bevy preprocessor syntax that naga doesn't speak,
/// so the import line is stripped and this is prepended in its place. Field
/// names are copied from `bevy_render`'s `view.wgsl` — if Bevy renames one, the
/// real shader breaks and this stub happily keeps compiling, so treat this as a
/// check on *our* code rather than on the import.
const VIEW_STUB: &str = "
struct View {
    clip_from_world: mat4x4<f32>,
    world_position: vec3<f32>,
};
";

fn preprocessed_grass_shader() -> String {
    let source = include_str!("grass.wgsl");
    let body: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("#import"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{VIEW_STUB}\n{body}")
}

#[test]
fn grass_shader_parses_and_validates() {
    let source = preprocessed_grass_shader();
    renzora::wgsl::check(&source).unwrap_or_else(|err| panic!("grass.wgsl: {err}"));
}

/// The pipeline names these entry points explicitly, so a rename in either place
/// is a pipeline that never builds.
#[test]
fn grass_shader_has_the_entry_points_the_pipeline_asks_for() {
    let source = preprocessed_grass_shader();
    let module = renzora::wgsl::parse(&source).expect("grass.wgsl should parse");
    let names: Vec<&str> = module
        .entry_points
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(names.contains(&"vertex"), "entry points: {names:?}");
    assert!(names.contains(&"fragment"), "entry points: {names:?}");
}

/// The blade strip is rebuilt from the vertex index, which only works if the
/// shader and [`super::instance::BLADE_SEGMENTS`] agree on how many segments
/// there are. They are declared in two languages, so nothing but a test links
/// them.
#[test]
fn shader_segment_count_matches_the_rust_constant() {
    let source = include_str!("grass.wgsl");
    let declared = source
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("const BLADE_SEGMENTS: u32 = ")?
                .strip_suffix("u;")?
                .parse::<u32>()
                .ok()
        })
        .expect("grass.wgsl should declare BLADE_SEGMENTS");
    assert_eq!(declared, super::instance::BLADE_SEGMENTS);
}

/// Every instance field the pipeline uploads has to be consumed by a matching
/// `@location`, in the order `VertexBufferLayout::from_vertex_formats` assigns
/// them — three `Float32x4`s at locations 0, 1 and 2.
#[test]
fn shader_declares_the_three_instance_attributes() {
    let source = include_str!("grass.wgsl");
    for (location, name) in [
        (0, "position_height"),
        (1, "width_phase_bend_var"),
        (2, "lean_rotation"),
    ] {
        let expected = format!("@location({location}) {name}: vec4<f32>");
        assert!(
            source.contains(&expected),
            "grass.wgsl is missing `{expected}`"
        );
    }
}
