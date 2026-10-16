//! End-to-end translations of the shapes real shaders take.

use super::*;

fn wesl(src: &str) -> String {
    let t = translate(src, &Options::default());
    assert!(!t.has_errors(), "{:#?}", t.diagnostics);
    t.source
}

#[test]
fn plain_wgsl_is_not_legacy() {
    assert!(!is_legacy("@fragment fn f() -> @location(0) vec4<f32> { return vec4(1.0); }\n"));
    assert!(is_legacy("#import bevy_pbr::forward_io::VertexOutput\n"));
    assert!(is_legacy("@group(#{MATERIAL_BIND_GROUP}) @binding(0) var t: texture_2d<f32>;\n"));
}

/// Bevy's own `extended_material` example, before and after its hand port.
#[test]
fn extended_material_matches_bevys_own_port() {
    let src = "\
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct MyExtendedMaterial {
    quantize_steps: u32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    _webgl2_padding_8b: u32,
    _webgl2_padding_12b: u32,
#endif
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> my_extended_material: MyExtendedMaterial;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var out: FragmentOutput;
#ifdef PREPASS_PIPELINE
    out = deferred_output(in, pbr_input);
#else
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
";
    let out = wesl(src);
    for expected in [
        "import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;",
        "import bevy_pbr::render::pbr_functions::alpha_discard;",
        "@if(PREPASS_PIPELINE) import bevy_pbr::prepass::io::VertexOutput;",
        "@if(PREPASS_PIPELINE) import bevy_pbr::deferred::functions::deferred_output;",
        "@if(!PREPASS_PIPELINE) import bevy_pbr::render::forward_io::FragmentOutput;",
        "@if(SIXTEEN_BYTE_ALIGNMENT) _webgl2_padding_8b: u32,",
        "@if(SIXTEEN_BYTE_ALIGNMENT) _webgl2_padding_12b: u32,",
        "@group(constants::MATERIAL_BIND_GROUP) @binding(100)",
        "@if(PREPASS_PIPELINE) out = deferred_output(in, pbr_input);",
        "@if(!PREPASS_PIPELINE) out.color = apply_pbr_lighting(pbr_input);",
        "@if(!PREPASS_PIPELINE) out.color = main_pass_post_lighting_processing(pbr_input, out.color);",
    ] {
        assert!(out.contains(expected), "missing `{expected}` in:\n{out}");
    }
    // Imports come before every declaration.
    let last_import = out.rfind("import ").unwrap();
    assert!(out.find("struct").unwrap() > last_import, "{out}");
    assert!(!out.contains('#'), "{out}");
}

/// Statements are annotated one by one, never wrapped in a block, so a
/// variable declared under `#ifdef` is still in scope after it.
#[test]
fn conditional_declarations_stay_in_scope() {
    let out = wesl("\
fn f() -> f32 {
#ifdef A
    let x = 1.0;
#else
    let x = 2.0;
#endif
    return x;
}
");
    assert!(out.contains("@if(A) let x = 1.0;"), "{out}");
    assert!(out.contains("@if(!A) let x = 2.0;"), "{out}");
    assert!(!out.contains("@if(A) {"), "{out}");
}

#[test]
fn nested_conditions_are_joined() {
    let out = wesl("\
#ifdef A
fn a() {}
#ifdef B
fn ab() {}
#endif
#endif
");
    assert!(out.contains("@if(A)\nfn a() {}"), "{out}");
    assert!(out.contains("@if(A && B)\nfn ab() {}"), "{out}");
}

/// The fields of a vertex input struct, from Renzora's `viewport_debug.wgsl`.
#[test]
fn struct_fields_and_parameters() {
    let out = wesl("\
struct Vertex {
    @location(0) position: vec3<f32>,
#ifdef VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
};
fn g(
    a: f32,
#ifdef B
    b: f32,
#endif
) {}
");
    assert!(out.contains("@if(VERTEX_NORMALS) @location(1) normal: vec3<f32>,"), "{out}");
    assert!(out.contains("@if(VERTEX_UVS_A) @location(2) uv: vec2<f32>,"), "{out}");
    assert!(out.contains("@if(B) b: f32,"), "{out}");
}

#[test]
fn if_else_chains_inside_a_branch_stay_whole() {
    let out = wesl("\
fn f(x: f32) -> f32 {
    var y = x;
#ifdef A
    if (x > 0.0) {
        y = 1.0;
    } else if (x < 0.0) {
        y = -1.0;
    } else {
        y = 0.0;
    }
#endif
    return y;
}
");
    assert!(out.contains("@if(A) if (x > 0.0) {"), "{out}");
    assert_eq!(out.matches("@if(").count(), 1, "{out}");
}

#[test]
fn an_if_before_a_chain_is_closed_before_it() {
    let out = wesl("\
fn f(x: f32) -> f32 {
    if (x > 0.0) {
        return 1.0;
    }
#ifdef A
    return 2.0;
#endif
    return 0.0;
}
");
    assert!(out.contains("@if(A) return 2.0;"), "{out}");
    assert!(!out.contains("@if(A) if"), "{out}");
}

/// A chain that splits an expression has no `@if` spelling; the statement it
/// splits is written out per flag value instead, and nothing else is copied.
#[test]
fn a_split_expression_expands_its_declaration() {
    let t = translate(
        "\
fn f(a: f32, b: f32) -> f32 {
    return a
#ifdef ADD_B
        + b
#endif
    ;
}
fn untouched() {}
",
        &Options::default(),
    );
    assert!(!t.has_errors(), "{:#?}", t.diagnostics);
    let out = t.source;
    assert_eq!(out.matches("fn f(").count(), 1, "{out}");
    assert!(out.contains("@if(!ADD_B) return a\n    ;"), "{out}");
    assert!(out.contains("@if(ADD_B) return a\n        + b\n    ;"), "{out}");
    assert_eq!(out.matches("fn untouched").count(), 1, "{out}");
    // Reported at the `#ifdef` the author wrote, not a line of the copy.
    assert!(t.diagnostics.iter().any(|d| d.severity == Severity::Info && d.line == 3), "{:?}", t.diagnostics);
}

/// From Bevy 0.19's `pbr_fragment.wgsl`: conditional arguments inside a call,
/// in a function with many other conditionals. Only the call is copied.
#[test]
fn conditional_arguments_copy_only_the_call() {
    let out = wesl("\
fn f(slot: u32) {
    var uv = vec2(0.0);
#ifdef VERTEX_UVS_A
    uv = parallaxed_uv(
#ifdef BINDLESS
        material_array[slot].depth,
#else
        material.depth,
#endif
        uv,
    );
#endif
#ifdef VERTEX_UVS_B
    let b = 1.0;
#endif
}
");
    assert!(out.contains("@if(VERTEX_UVS_A && BINDLESS) uv = parallaxed_uv(\n        material_array[slot].depth,"), "{out}");
    assert!(out.contains("@if(VERTEX_UVS_A && !BINDLESS) uv = parallaxed_uv(\n        material.depth,"), "{out}");
    assert!(out.contains("@if(VERTEX_UVS_B) let b = 1.0;"), "{out}");
    assert_eq!(out.matches("var uv").count(), 1, "{out}");
}

#[test]
fn local_defines_and_shadowed_imports() {
    // smaa.wgsl: a flag defined by one preset and tested later.
    let out = wesl("\
#ifdef PRESET_LOW
#define NO_DIAG
#endif
fn f() {
#ifndef NO_DIAG
    diag();
#endif
}
");
    assert!(out.contains("@if(!(NO_DIAG || PRESET_LOW)) diag();"), "{out}");

    // tonemapping.wgsl: an unconditional define makes its tests constant.
    let out = wesl("#define PASS\nfn f() {\n#ifdef PASS\n    a();\n#endif\n}\n");
    assert!(out.contains("    a();") && !out.contains("@if"), "{out}");

    // A define with a value is a const, and `#NAME` reads it.
    let out = wesl("#define STEPS 4u\nfn f() -> u32 { return #STEPS; }\n");
    assert!(out.contains("const STEPS = 4u;"), "{out}");
    assert!(out.contains("return STEPS;"), "{out}");

    // gradient.wgsl: an import the file also declares itself.
    let out = wesl("#import bevy_render::color_operations::{mix_hsv, convert_to_linear_rgba}\nfn convert_to_linear_rgba() {}\n");
    assert!(out.contains("import bevy_render::color_operations::mix_hsv;"), "{out}");
    assert!(!out.contains("color_operations::convert_to_linear_rgba"), "{out}");
}

/// Bevy's `globals.wgsl`: a last member without its comma, inside a chain.
#[test]
fn last_member_without_a_comma() {
    let out = wesl("struct G {\n    frame_count: u32,\n#ifdef SIXTEEN_BYTE_ALIGNMENT\n    _pad: f32\n#endif\n};\n");
    assert!(out.contains("@if(SIXTEEN_BYTE_ALIGNMENT) _pad: f32"), "{out}");
}

/// gaussian splatting's `#else #ifdef X` and `#else SOME_COMMENT`.
#[test]
fn else_spellings() {
    let out = wesl("struct S {\n#ifdef A\n    a: f32,\n#else #ifdef B\n    b: f32,\n#else NOTE\n    c: f32,\n#endif\n}\n");
    assert!(out.contains("@if(A) a: f32,"), "{out}");
    assert!(out.contains("@if(!A && B) b: f32,"), "{out}");
    assert!(out.contains("@if(!A && !B) c: f32,"), "{out}");
}

/// Two branches that open the same function differently (a common way to
/// vary a signature) are another split; each version is kept whole.
#[test]
fn alternative_function_headers() {
    let out = wesl("\
#ifdef WIDE
fn f(x: vec4<f32>) -> vec4<f32> {
#else
fn f(x: f32) -> f32 {
#endif
    return x;
}
");
    assert!(out.contains("@if(WIDE)\nfn f(x: vec4<f32>) -> vec4<f32> {\n    return x;\n}"), "{out}");
    assert!(out.contains("@if(!WIDE)\nfn f(x: f32) -> f32 {\n    return x;\n}"), "{out}");
}

#[test]
fn module_name_is_reported_and_removed() {
    let t = translate("#define_import_path my_game::noise\nfn n() -> f32 { return 0.0; }\n", &Options::default());
    assert_eq!(t.module_name.as_deref(), Some("my_game::noise"));
    assert!(!t.source.contains("define_import_path"));
}

#[test]
fn renamed_defs_follow_bevy() {
    let out = wesl("fn f() {\n#ifdef OIT_ENABLED\n    oit();\n#endif\n}\n");
    assert!(out.contains("@if(MATERIAL_OIT_ENABLED) oit();"), "{out}");
}

#[test]
fn switch_cases_and_loops() {
    let out = wesl("\
fn f(m: u32) -> f32 {
    var r = 0.0;
    switch m {
        case 0u: { r = 1.0; }
#ifdef EXTRA
        case 1u: { r = 2.0; }
#endif
        default: { r = 3.0; }
    }
    for (var i = 0u; i < 4u; i++) {
#ifdef DOUBLE
        r = r * 2.0;
#endif
    }
    return r;
}
");
    assert!(out.contains("@if(EXTRA) case 1u: { r = 2.0; }"), "{out}");
    assert!(out.contains("@if(DOUBLE) r = r * 2.0;"), "{out}");
}

/// From Bevy 0.19's own shaders, each of which the first verification run
/// caught.
#[test]
fn shapes_found_in_bevys_own_shaders() {
    // tonemapping/lut_bindings.wgsl: the brace-less substitution.
    let out = wesl("@group(0) @binding(#TONEMAPPING_LUT_TEXTURE_BINDING_INDEX) var t: texture_3d<f32>;\n");
    assert!(out.contains("@binding(constants::TONEMAPPING_LUT_TEXTURE_BINDING_INDEX)"), "{out}");
    assert!(is_legacy("@binding(#X) var t: f32;\n"));
    assert!(!is_legacy("var t: f32; // issue #42\n"));

    // tonemapping_shared.wgsl: `#if 0` around GLSL, which must not survive.
    let out = wesl("fn f(v: f32) -> f32 {\n#if 0\n    float c = v;\n    return c;\n#else\n    return v;\n#endif\n}\n");
    assert!(!out.contains("float c"), "{out}");
    assert!(out.contains("    return v;"), "{out}");
    assert!(!out.contains("@if"), "{out}");

    // ...and the same file's `#ifdef 0`, a def nobody can define.
    let out = wesl("fn f() {\n#ifdef 0\n    float c;\n#endif\n}\n");
    assert!(!out.contains("float c"), "{out}");

    // pbr_lighting.wgsl: the comma inside a template list is not a separator.
    let out = wesl("struct S {\n#ifdef A\n    layers: array<L, 2>,\n#else\n    layers: array<L, 1>,\n#endif\n    p: vec3<f32>,\n}\n");
    assert!(out.contains("@if(A) layers: array<L, 2>,"), "{out}");
    assert!(out.contains("@if(!A) layers: array<L, 1>,"), "{out}");
    assert_eq!(out.matches("@if").count(), 2, "{out}");

    // transmission.wgsl: a WGSL-style `;` after the import list.
    let out = wesl("#import bevy_render::view::View;\nvar<uniform> view: View;\n");
    assert!(out.contains("import bevy_render::view::View;\n"), "{out}");

    // robust_contrast_adaptive_sharpening.wgsl: a doubled semicolon.
    let out = wesl("fn f() {\n#ifdef A\n    var n = 1.0;;\n#endif\n}\n");
    assert!(out.contains("@if(A) var n = 1.0;;"), "{out}");
    assert!(!out.contains("@if(A) ;"), "{out}");
}

/// Every `.material` saved before 0.20 embeds this call in the two-argument
/// form Bevy 0.20 replaced.
#[test]
fn old_alpha_discard_calls_take_the_new_arguments() {
    let src = "\
#import bevy_pbr::pbr_functions
fn f() {
    pbr_input.material.base_color = pbr_functions::alpha_discard(pbr_input.material, pbr_input.material.base_color);
}
";
    let t = translate(src, &Options::default());
    assert!(
        t.source.contains(
            "bevy_pbr::render::pbr_functions::alpha_discard(pbr_input.material.flags, \
             pbr_input.material.alpha_cutoff, pbr_input.material.base_color)"
        ),
        "{}",
        t.source
    );
    assert!(t.diagnostics.iter().any(|d| d.message.contains("alpha_discard")));

    // The 0.20 form is left alone.
    let out = wesl("fn f() { let c = alpha_discard(m.flags, m.alpha_cutoff, c0); }\n");
    assert!(out.contains("alpha_discard(m.flags, m.alpha_cutoff, c0)"), "{out}");
}

#[test]
fn unbalanced_directives_are_reported_not_fatal() {
    let t = translate("#ifdef A\nfn f() {}\n", &Options::default());
    assert!(t.has_errors());
    assert!(t.source.contains("fn f() {}"));
}
