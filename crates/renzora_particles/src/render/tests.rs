//! The rendering features without a GPU: mesh particles (templates, the CPU
//! copies, the GPU pool mesh and the orientation twin), every blend mode,
//! shadows and motion vectors, lighting and screen-space size as they reach
//! the uniform, project materials on any particle, the file format of all of
//! them, and which backend each runs on. The shaders themselves are compiled
//! by `tests/shaders.rs`.

use bevy::math::{Affine3A, Mat3, Vec2, Vec3};
use bevy::mesh::{Mesh, PrimitiveTopology, VertexAttributeValues};

use super::material::{alpha_mode_3d, blend_code, ParticleMaterialParams, ATTRIBUTE_PARTICLE_LOCAL, ATTRIBUTE_PARTICLE_VELOCITY};
use super::mesh_particles::{gpu_pool_mesh, gpu_pool_slots, mesh_rotation, model_path, MeshTemplate};
use super::write_mesh_particles;
use crate::asset::*;
use crate::compile::compile_graph;
use crate::gpu::select::choose_in as choose;
use crate::gpu::{ActiveBackend, GpuCaps};
use crate::sim::FrameInput;
use crate::tests::{f, sim_of, simple_effect, v3};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn new_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
}

fn f3(mesh: &Mesh, id: impl Into<bevy::mesh::MeshVertexAttributeId>) -> Vec<[f32; 3]> {
    match mesh.attribute(id) {
        Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
        other => panic!("not a vec3 attribute: {other:?}"),
    }
}

/// Three particles at known places with a known size.
fn three_particles() -> crate::sim::EffectSim {
    let asset = simple_effect(
        &[
            ("spawn_rate", f(0.0)),
            ("burst_count", f(3.0)),
            ("lifetime", f(10.0)),
            ("size", PinValue::Vec2([2.0, 2.0])),
            ("velocity", v3(0.0, 4.0, 0.0)),
        ],
        vec![],
        vec![],
    );
    let mut sim = sim_of(&asset);
    sim.step(0.01, &FrameInput::default());
    let p = &mut sim.emitters[0].particles;
    for (i, x) in [-5.0, 0.0, 5.0].into_iter().enumerate() {
        p.position[i] = Vec3::new(x, 0.0, 0.0);
    }
    sim
}

// ── The file ────────────────────────────────────────────────────────────────

#[test]
fn the_new_render_fields_default_and_are_not_written_until_set() {
    let plain = simple_effect(&[], vec![], vec![]);
    let text = write_vfx(&plain).unwrap();
    for field in ["mesh", "orient", "lighting", "material", "alpha_cutoff", "screen_space_size", "shadows"] {
        assert!(!text.contains(field), "{field} written while default:\n{text}");
    }
    // A file from before the fields existed loads with them at their defaults.
    let old = parse_vfx("(emitters: [(render: (blend: Additive))])").unwrap();
    let r = &old.emitters[0].render;
    assert_eq!(r.mesh, None);
    assert_eq!(r.orient, MeshOrient::Tumble);
    assert_eq!(r.lighting, Lighting::default());
    assert_eq!(r.alpha_cutoff, DEFAULT_ALPHA_CUTOFF);
    assert!(!r.screen_space_size);
    assert_eq!(r.shadows, ParticleShadows::Auto);

    let mut set = plain.clone();
    let r = &mut set.emitters[0].render;
    r.mesh = Some(ParticleMesh::Asset("models/rock.glb".into()));
    r.orient = MeshOrient::Axis([0.0, 0.0, 1.0]);
    r.lighting = Lighting { lit: true, roughness: 0.3, metallic: 1.0 };
    r.material = Some("materials/rock.material".into());
    r.blend = BlendMode::AlphaMask;
    r.alpha_cutoff = 0.25;
    r.screen_space_size = true;
    r.shadows = ParticleShadows::On;
    let back = parse_vfx(&write_vfx(&set).unwrap()).unwrap();
    assert_eq!(back.emitters[0].render, set.emitters[0].render);
}

/// The new blends and the shadow setting are plain variants in the file, so
/// they round-trip, and an older engine's file with neither loads as before.
#[test]
fn the_new_blends_and_shadows_round_trip() {
    for blend in [BlendMode::Multiply, BlendMode::AlphaToCoverage] {
        for shadows in [ParticleShadows::Auto, ParticleShadows::On, ParticleShadows::Off] {
            let mut effect = simple_effect(&[], vec![], vec![]);
            effect.emitters[0].render.blend = blend;
            effect.emitters[0].render.shadows = shadows;
            let text = write_vfx(&effect).unwrap();
            let back = parse_vfx(&text).unwrap();
            assert_eq!(back.emitters[0].render.blend, blend, "{text}");
            assert_eq!(back.emitters[0].render.shadows, shadows, "{text}");
        }
    }
    let r = &parse_vfx("(emitters: [(render: (blend: Multiply, shadows: Off))])").unwrap().emitters[0].render;
    assert_eq!((r.blend, r.shadows), (BlendMode::Multiply, ParticleShadows::Off));
}

// ── Shadows ─────────────────────────────────────────────────────────────────

#[test]
fn solid_blends_cast_shadows_by_default_and_the_setting_overrides() {
    let with = |blend, shadows| RenderSettings { blend, shadows, ..Default::default() };
    for blend in [BlendMode::Opaque, BlendMode::AlphaMask, BlendMode::AlphaToCoverage] {
        assert!(with(blend, ParticleShadows::Auto).casts_shadows(Dimension::ThreeD), "{blend:?}");
        assert!(!with(blend, ParticleShadows::Off).casts_shadows(Dimension::ThreeD), "{blend:?}");
    }
    for blend in [BlendMode::Alpha, BlendMode::Additive, BlendMode::Premultiplied, BlendMode::Multiply] {
        assert!(!with(blend, ParticleShadows::Auto).casts_shadows(Dimension::ThreeD), "{blend:?}");
        assert!(with(blend, ParticleShadows::On).casts_shadows(Dimension::ThreeD), "{blend:?}");
    }
    assert!(!with(BlendMode::Opaque, ParticleShadows::On).casts_shadows(Dimension::TwoD), "2D has no shadow maps");
}

// ── Motion vectors ──────────────────────────────────────────────────────────

#[test]
fn the_motion_uniform_carries_the_rate_and_the_space() {
    let r = RenderSettings::default();
    let p = ParticleMaterialParams::new(&r, false);
    assert_eq!((p.motion.x, p.motion.y), (1.0, 0.0), "playing, local");
    let p = p.in_space(SimulationSpace::World);
    assert_eq!(p.motion.y, 1.0, "a world emitter's previous position ignores the entity");
    let mut playback = crate::component::ParticlePlayback::default();
    assert_eq!(super::playback_rate(&playback), playback.time_scale.max(0.0));
    playback.time_scale = 2.0;
    assert_eq!(super::playback_rate(&playback), 2.0);
    playback.pause();
    assert_eq!(super::playback_rate(&playback), 0.0, "paused particles do not move");
    playback.editor_clock = Some(0.25);
    assert_eq!(super::playback_rate(&playback), 0.25, "an editor clock plays whatever the state says");
    playback.editor_clock = Some(f32::NAN);
    assert_eq!(super::playback_rate(&playback), 0.0);
}

/// Every vertex carries its particle's velocity in mesh space, whatever the
/// facing, for motion vectors and the material graph; a ribbon point has
/// none, and the column is still as long as the mesh.
#[test]
fn every_vertex_carries_its_particles_velocity() {
    let sim = three_particles();
    let particles = &sim.emitters[0].particles;
    for facing in [Facing::CameraBillboard, Facing::FixedAxis([0.0, 1.0, 0.0])] {
        let mut mesh = new_mesh();
        let render = RenderSettings { facing, ..Default::default() };
        // An entity scaled by 2: the velocity is in its (mesh) space.
        super::write_mesh(&mut mesh, particles, &render, Affine3A::from_scale(Vec3::splat(0.5)));
        let v = f3(&mesh, ATTRIBUTE_PARTICLE_VELOCITY);
        assert_eq!(v.len(), mesh.count_vertices());
        assert!(close(v[0][1], 2.0), "{facing:?}: {:?}", v[0]);
    }
    let ribbon = RenderSettings { facing: Facing::Ribbon, ..Default::default() };
    let mut mesh = new_mesh();
    super::write_mesh(&mut mesh, particles, &ribbon, Affine3A::IDENTITY);
    assert_eq!(f3(&mesh, ATTRIBUTE_PARTICLE_VELOCITY).len(), mesh.count_vertices(), "padded to the ribbon's vertices");
}

// ── Blend modes ─────────────────────────────────────────────────────────────

#[test]
fn opaque_and_mask_write_depth_draw_in_their_phases_and_never_sort() {
    use bevy::prelude::AlphaMode;
    assert_eq!(alpha_mode_3d(BlendMode::Opaque, 0.5), AlphaMode::Opaque);
    assert_eq!(alpha_mode_3d(BlendMode::AlphaMask, 0.3), AlphaMode::Mask(0.3));
    assert_eq!(alpha_mode_3d(BlendMode::Alpha, 0.3), AlphaMode::Blend);
    assert_eq!(alpha_mode_3d(BlendMode::Multiply, 0.3), AlphaMode::Multiply);
    assert_eq!(alpha_mode_3d(BlendMode::AlphaToCoverage, 0.3), AlphaMode::AlphaToCoverage);
    assert!(BlendMode::Opaque.writes_depth() && BlendMode::AlphaMask.writes_depth());
    assert!(BlendMode::AlphaToCoverage.writes_depth(), "coverage is a mask with soft edges");
    assert!(!BlendMode::Alpha.writes_depth() && !BlendMode::Additive.writes_depth() && !BlendMode::Multiply.writes_depth());
    assert!(BlendMode::AlphaMask.uses_cutoff() && BlendMode::AlphaToCoverage.uses_cutoff() && !BlendMode::Opaque.uses_cutoff());
    // The shader reads these as integers (`blend_mode()`).
    let codes: Vec<f32> = [
        BlendMode::Alpha,
        BlendMode::Additive,
        BlendMode::Premultiplied,
        BlendMode::Opaque,
        BlendMode::AlphaMask,
        BlendMode::Multiply,
        BlendMode::AlphaToCoverage,
    ]
    .map(blend_code)
    .to_vec();
    assert_eq!(codes, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    // Depth orders the solid ones and multiplication commutes, so none sort.
    for blend in [BlendMode::Opaque, BlendMode::AlphaMask, BlendMode::AlphaToCoverage, BlendMode::Multiply] {
        let r = RenderSettings { sort: true, blend, ..Default::default() };
        assert!(!r.sorts(Dimension::ThreeD), "{blend:?} needs no sort");
    }
    let r = RenderSettings { blend: BlendMode::AlphaMask, alpha_cutoff: 0.7, ..Default::default() };
    assert!(close(ParticleMaterialParams::new(&r, false).surface.w, 0.7));
    // A multiply layer is a filter, never lit.
    let lit = RenderSettings { blend: BlendMode::Multiply, lighting: Lighting { lit: true, ..Default::default() }, ..Default::default() };
    assert_eq!(ParticleMaterialParams::new(&lit, false).surface.x, 0.0);
}

#[cfg(feature = "render_2d")]
#[test]
fn the_2d_pipeline_has_the_depth_written_modes_too() {
    use super::material::{alpha_mode_2d, Particle2dKey};
    use bevy::sprite_render::AlphaMode2d;
    assert_eq!(alpha_mode_2d(BlendMode::Opaque, 0.5), AlphaMode2d::Opaque);
    assert_eq!(alpha_mode_2d(BlendMode::AlphaMask, 0.4), AlphaMode2d::Mask(0.4));
    assert_eq!(alpha_mode_2d(BlendMode::Premultiplied, 0.4), AlphaMode2d::Blend);
    assert_eq!(alpha_mode_2d(BlendMode::AlphaToCoverage, 0.4), AlphaMode2d::Mask(0.4), "2D has no coverage mode");
    assert_eq!(alpha_mode_2d(BlendMode::Multiply, 0.4), AlphaMode2d::Blend);
    assert!(!Particle2dKey::of(BlendMode::Opaque).premultiplied, "opaque must not get the premultiplied blend state");
    assert!(Particle2dKey::of(BlendMode::Additive).premultiplied);
    let multiply = Particle2dKey::of(BlendMode::Multiply);
    assert!(multiply.multiply && !multiply.premultiplied);
    let state = super::material::blend_state_2d(multiply).expect("multiply has its own blend state");
    assert_eq!(state.color.src_factor, bevy::render::render_resource::BlendFactor::Dst);
    assert!(super::material::blend_state_2d(Particle2dKey::of(BlendMode::Alpha)).is_none());
}

// ── The uniform ─────────────────────────────────────────────────────────────

#[test]
fn the_uniform_carries_mesh_lighting_and_pixel_sizes() {
    let mesh = RenderSettings {
        mesh: Some(ParticleMesh::Cube),
        orient: MeshOrient::Velocity,
        lighting: Lighting { lit: true, roughness: 0.25, metallic: 0.5 },
        screen_space_size: true,
        ..Default::default()
    };
    let p = ParticleMaterialParams::for_dimension(&mesh, false, Dimension::ThreeD);
    assert_eq!(p.shape.x, 1.0, "mesh particles");
    assert_eq!(p.shape.y, 1.0, "sizes in pixels");
    assert_eq!(p.mode.y, 1.0, "a velocity orientation reads the velocity through the facing slot");
    assert_eq!(p.mode.z, 2.0, "an untextured mesh is solid, not the soft dot");
    assert_eq!((p.surface.x, p.surface.y, p.surface.z), (1.0, 0.25, 0.5));

    let axis = RenderSettings { orient: MeshOrient::Axis([0.0, 0.0, 2.0]), ..mesh.clone() };
    let p = ParticleMaterialParams::for_dimension(&axis, true, Dimension::ThreeD);
    assert_eq!(p.mode.y, 2.0);
    assert_eq!(p.axis.z, 2.0, "the GPU fetch reads the orientation axis from the uniform");
    assert_eq!(p.mode.z, 1.0);

    // A 2D effect has no meshes and no lights: the settings do not reach it.
    let p = ParticleMaterialParams::for_dimension(&mesh, false, Dimension::TwoD);
    assert_eq!((p.shape.x, p.surface.x, p.mode.y), (0.0, 0.0, 0.0));
    assert_eq!(p.shape.y, 1.0, "2D sizes in pixels too");
    // Additive colour is light, and is never lit.
    let additive = RenderSettings { blend: BlendMode::Additive, ..mesh };
    assert_eq!(ParticleMaterialParams::new(&additive, false).surface.x, 0.0);
}

#[test]
fn a_mesh_wins_over_the_ribbon_facing_and_is_3d_only() {
    let r = RenderSettings { facing: Facing::Ribbon, mesh: Some(ParticleMesh::Sphere), ..Default::default() };
    assert!(r.draws_mesh(Dimension::ThreeD));
    assert!(!r.draws_ribbon(Dimension::ThreeD));
    assert!(!r.draws_mesh(Dimension::TwoD));
    assert!(r.draws_ribbon(Dimension::TwoD));
    assert_eq!(r.effective_facing(Dimension::ThreeD), Facing::CameraBillboard);
    assert_eq!(r.effective_facing(Dimension::TwoD), Facing::Ribbon);
    let blank = RenderSettings { material: Some("  ".into()), ..r.clone() };
    assert_eq!(blank.material_path(Dimension::ThreeD), None, "a blank material path is no material");
    // Quads, ribbons and meshes all wear one in 3D; 2D draws through its
    // own pipeline.
    let quad = RenderSettings { material: Some(" m.material ".into()), mesh: None, ..Default::default() };
    assert_eq!(quad.material_path(Dimension::ThreeD), Some("m.material"));
    assert_eq!(quad.mesh_material(Dimension::ThreeD), None, "the old mesh-only reading still says no mesh");
    let ribbon = RenderSettings { facing: Facing::Ribbon, ..quad.clone() };
    assert_eq!(ribbon.material_path(Dimension::ThreeD), Some("m.material"));
    assert_eq!(quad.material_path(Dimension::TwoD), None);
}

// ── Templates ───────────────────────────────────────────────────────────────

#[test]
fn every_built_in_shape_is_a_unit_sized_template() {
    for shape in ParticleMesh::BUILT_IN {
        let t = MeshTemplate::built_in(&shape).unwrap_or_else(|| panic!("{shape:?} has no template"));
        let n = t.vertex_count();
        assert!(n >= 3, "{shape:?}");
        assert_eq!((t.normals.len(), t.uvs.len(), t.tangents.len()), (n, n, n), "{shape:?}");
        assert!(t.indices.len() % 3 == 0 && t.indices.iter().all(|&i| (i as usize) < n), "{shape:?}");
        assert!(t.radius > 0.3 && t.radius <= 0.87, "{shape:?} is unit sized: radius {}", t.radius);
        assert!(t.normals.iter().all(|n| close(n.length(), 1.0)), "{shape:?} normals");
    }
    assert!(MeshTemplate::built_in(&ParticleMesh::Asset("a.glb".into())).is_none());
}

#[test]
fn a_template_refuses_what_it_cannot_draw_and_fills_in_what_is_missing() {
    let mut lines = Mesh::new(PrimitiveTopology::LineList, bevy::asset::RenderAssetUsages::default());
    lines.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 2]);
    assert!(MeshTemplate::from_mesh(&lines).is_err());
    assert!(MeshTemplate::from_mesh(&new_mesh()).is_err(), "no positions");

    // A bare triangle: normals are computed, UVs zeroed, indices made up.
    let mut bare = new_mesh();
    bare.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
    let t = MeshTemplate::from_mesh(&bare).unwrap();
    assert_eq!(t.indices, [0, 1, 2]);
    assert!(t.normals.iter().all(|n| close(n.z.abs(), 1.0)), "{:?}", t.normals);
    assert_eq!(t.uvs, [Vec2::ZERO; 3]);
    assert!(close(t.radius, 1.0));

    let mut bad = bare.clone();
    bad.insert_indices(bevy::mesh::Indices::U32(vec![0, 1, 7]));
    assert!(MeshTemplate::from_mesh(&bad).is_err(), "an index past the vertices");
}

#[test]
fn a_model_path_names_its_first_mesh_unless_it_names_one() {
    assert_eq!(model_path("models/rock.glb"), "models/rock.glb#Mesh0/Primitive0");
    assert_eq!(model_path(" models/rock.glb#Mesh2/Primitive1 "), "models/rock.glb#Mesh2/Primitive1");
}

// ── The CPU copies ──────────────────────────────────────────────────────────

#[test]
fn mesh_particles_write_one_copy_of_the_mesh_per_particle() {
    let sim = three_particles();
    let cube = MeshTemplate::built_in(&ParticleMesh::Cube).unwrap();
    let render = RenderSettings { mesh: Some(ParticleMesh::Cube), ..Default::default() };
    let mut mesh = new_mesh();
    let bounds = write_mesh_particles(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY, None, &cube)
        .expect("drawn");
    let n = cube.vertex_count();
    assert_eq!(mesh.count_vertices(), 3 * n);
    assert_eq!(mesh.indices().map(|i| i.len()), Some(3 * cube.indices.len()));
    let local = f3(&mesh, ATTRIBUTE_PARTICLE_LOCAL);
    let normal = f3(&mesh, Mesh::ATTRIBUTE_NORMAL);
    let center = f3(&mesh, Mesh::ATTRIBUTE_POSITION);
    assert_eq!(local[n + 5], cube.positions[5].to_array(), "the second copy repeats the mesh");
    assert_eq!(normal[5], cube.normals[5].to_array());
    assert_eq!(center[n], [0.0, 0.0, 0.0], "every vertex of a copy carries its particle's centre");
    // The UV rides in the corner, which sheet_uv turns back into (u, v).
    let Some(VertexAttributeValues::Float32x2(corner)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
    let uv = cube.uvs[3];
    assert_eq!(corner[3], [uv.x - 0.5, 0.5 - uv.y]);
    // Padded by the mesh's radius at the particle's size (2 m).
    let pad = 2.0 * cube.radius;
    assert!(bounds.min().x <= -5.0 - pad + 1e-4 && bounds.max().x >= 5.0 + pad - 1e-4, "{bounds:?}");

    // Nothing alive: one degenerate triangle that still has the mesh layout.
    let empty = crate::sim::ParticleBuffer::default();
    assert!(write_mesh_particles(&mut mesh, &empty, &render, Affine3A::IDENTITY, None, &cube).is_none());
    assert_eq!(mesh.count_vertices(), 3);
    assert_eq!(f3(&mesh, ATTRIBUTE_PARTICLE_LOCAL).len(), 3);

    // A quad write afterwards drops the mesh columns again.
    super::write_mesh(&mut mesh, &sim.emitters[0].particles, &RenderSettings::default(), Affine3A::IDENTITY);
    assert!(mesh.attribute(ATTRIBUTE_PARTICLE_LOCAL).is_none());
}

#[test]
fn a_velocity_oriented_mesh_carries_the_velocity_and_its_stretch() {
    let sim = three_particles();
    let cube = MeshTemplate::built_in(&ParticleMesh::Cube).unwrap();
    let render =
        RenderSettings { mesh: Some(ParticleMesh::Cube), orient: MeshOrient::Velocity, stretch: 0.5, ..Default::default() };
    let mut mesh = new_mesh();
    write_mesh_particles(&mut mesh, &sim.emitters[0].particles, &render, Affine3A::IDENTITY, None, &cube).unwrap();
    let axis = f3(&mesh, super::ATTRIBUTE_PARTICLE_AXIS);
    assert!(close(axis[0][1], 4.0), "{:?}", axis[0]);
    let Some(VertexAttributeValues::Float32x4(params)) = mesh.attribute(super::ATTRIBUTE_PARTICLE_PARAMS) else { panic!() };
    // 2 m plus 4 m/s for half a second.
    assert!(close(params[0][1], 4.0), "{:?}", params[0]);
}

// ── Orientation ─────────────────────────────────────────────────────────────

fn orthonormal(m: Mat3) -> bool {
    (m.transpose() * m).abs_diff_eq(Mat3::IDENTITY, 1e-4) && close(m.determinant(), 1.0)
}

#[test]
fn a_tumble_is_a_different_rotation_per_particle_that_spins_with_rotation() {
    let a = mesh_rotation(Facing::CameraBillboard, Vec3::ZERO, 0.1, 0.0);
    let b = mesh_rotation(Facing::CameraBillboard, Vec3::ZERO, 0.7, 0.0);
    let spun = mesh_rotation(Facing::CameraBillboard, Vec3::ZERO, 0.1, 1.0);
    assert!(orthonormal(a) && orthonormal(b) && orthonormal(spun));
    assert!(!a.abs_diff_eq(b, 1e-3), "two particles start differently");
    assert!(!a.abs_diff_eq(Mat3::IDENTITY, 1e-3), "a tumble starts at a random orientation");
    assert!(!a.abs_diff_eq(spun, 1e-3), "rotation spins it");
    assert!(a.abs_diff_eq(mesh_rotation(Facing::CameraBillboard, Vec3::ZERO, 0.1, 0.0), 0.0), "stable per seed");
}

#[test]
fn velocity_and_axis_orientations_put_the_mesh_up_along_them() {
    for dir in [Vec3::new(1.0, 2.0, -0.5), Vec3::X, -Vec3::Y, Vec3::Z * 3.0] {
        for rotation in [0.0, 1.3] {
            let m = mesh_rotation(Facing::VelocityAligned, dir, 0.4, rotation);
            assert!(orthonormal(m), "{dir} {rotation}");
            assert!((m * Vec3::Y).abs_diff_eq(dir.normalize(), 1e-4), "+Y along {dir}");
        }
    }
    // Rotation rolls about the axis.
    let a = mesh_rotation(Facing::FixedAxis([0.0, 1.0, 0.0]), Vec3::Y, 0.4, 0.0);
    let b = mesh_rotation(Facing::FixedAxis([0.0, 1.0, 0.0]), Vec3::Y, 0.4, std::f32::consts::FRAC_PI_2);
    assert!(a.abs_diff_eq(Mat3::IDENTITY, 1e-5));
    assert!((b * Vec3::X).abs_diff_eq(-Vec3::Z, 1e-4), "{}", b * Vec3::X);
    // No velocity: upright rather than NaN.
    assert!(mesh_rotation(Facing::VelocityAligned, Vec3::ZERO, 0.4, 0.0).is_finite());
}

/// `mesh_rotation` is the Rust twin of the shader's `mesh_orientation`, for
/// the project-material path; this holds the shader to the streams and
/// constants the twin uses, so the two paths turn a particle the same way.
#[test]
fn the_shader_orientation_uses_the_twins_streams_and_constants() {
    let shader = include_str!("../shaders/particle3d.wesl");
    for line in [
        "return hash12(vec2<f32>(seed * 4096.0, k));",
        "let base = axis_angle(random_axis(seed, 1.0), seed_random(seed, 3.0) * 6.2831853);",
        "return axis_angle(random_axis(seed, 4.0), rotation) * base;",
        "return align_y(dir) * axis_angle(vec3<f32>(0.0, 1.0, 0.0), rotation);",
        "if abs(dir.x) > 0.9 {",
    ] {
        assert!(shader.contains(line), "the shader's orientation has drifted from mesh_rotation: {line}");
    }
}

// ── Project materials ───────────────────────────────────────────────────────

/// A resolved material becomes the particle version: the emitter's blend in
/// place of the material's alpha mode, the emitter's render route in place
/// of the material's method, both faces drawn, and the graph's slots,
/// parameters, compiled shader and G-buffer branch carried over.
#[cfg(feature = "material_graph")]
#[test]
fn a_resolved_material_becomes_its_particle_version() {
    use super::material::RenderRoute;
    use super::surface::{cpu_material, particle_base, ParticleSurfaceKey, ParticleSurfaceSlot};
    use crate::asset::RenderMethod;
    use bevy::material::OpaqueRendererMethod;
    use bevy::prelude::{AlphaMode, Handle, StandardMaterial};
    use renzora_shader::material::surface_ext::SurfaceGraphExt;

    let base = StandardMaterial { alpha_mode: AlphaMode::Opaque, double_sided: true, ..Default::default() };
    let forward = RenderRoute::new(RenderMethod::Forward);
    let quad = particle_base(base.clone(), BlendMode::Alpha, 0.5, false, forward);
    assert_eq!(quad.alpha_mode, AlphaMode::Blend, "the emitter's blend wins");
    assert_eq!(quad.opaque_render_method, OpaqueRendererMethod::Forward);
    assert_eq!(quad.cull_mode, None);
    assert!(!quad.double_sided, "a quad's normal already faces the view");
    let mesh = particle_base(base.clone(), BlendMode::AlphaToCoverage, 0.5, true, RenderRoute::new(RenderMethod::Auto));
    assert_eq!(mesh.alpha_mode, AlphaMode::AlphaToCoverage);
    assert_eq!(mesh.opaque_render_method, OpaqueRendererMethod::Auto, "a solid particle follows the project");
    assert!(mesh.double_sided, "a mesh copy flips its back faces' normals");

    let uuid = bevy::asset::uuid::Uuid::from_u128(7);
    let mut graph =
        SurfaceGraphExt { shader_uuid: Some(uuid), texture_2: Some(Handle::default()), writes_gbuffer: true, ..Default::default() };
    graph.params.slots[3] = bevy::math::Vec4::splat(0.25);
    let r = RenderSettings { blend: BlendMode::Opaque, ..Default::default() };
    let params = ParticleMaterialParams::new(&r, false);
    let slot = ParticleSurfaceSlot::new(params, BlendMode::Opaque, 0.5, false, None, None);
    assert_eq!(slot.route, RenderRoute::new(RenderMethod::Auto), "a solid slot follows the project by default");
    let material = cpu_material(&slot, base.clone(), Some(&graph));
    assert_eq!(material.extension.shader_uuid, Some(uuid));
    assert!(material.extension.texture_2.is_some() && material.extension.texture_0.is_none());
    assert_eq!(material.extension.params.slots[3], bevy::math::Vec4::splat(0.25));
    assert_eq!(material.extension.particle, params);
    assert_eq!(material.base.opaque_render_method, OpaqueRendererMethod::Auto);
    let key = ParticleSurfaceKey::from(&material.extension);
    assert_eq!(key, ParticleSurfaceKey { shader_uuid: Some(uuid), writes_gbuffer: true, mesh: false, method: RenderMethod::Auto });

    // A transparent slot is forward whatever the emitter asked.
    let blended = ParticleSurfaceSlot::new(params, BlendMode::Alpha, 0.5, false, None, None);
    assert_eq!(blended.route.method, RenderMethod::Forward);
    // The key follows the route, so the two routes get pipelines of their own.
    let pinned = ParticleSurfaceSlot::new(params, BlendMode::Opaque, 0.5, false, None, None).with_route(forward);
    let pinned = cpu_material(&pinned, base, Some(&graph));
    assert_eq!(pinned.base.opaque_render_method, OpaqueRendererMethod::Forward);
    assert_ne!(ParticleSurfaceKey::from(&pinned.extension), key);
}

/// `render_method` stays out of a file until it is set, so every effect saved
/// before it existed (each built-in preset among them) re-saves unchanged and
/// reads as `Auto`, and each value survives a save.
#[test]
fn the_render_method_round_trips_and_old_files_stay_unchanged() {
    use crate::asset::{parse_vfx, write_vfx, RenderMethod};
    for preset in crate::presets::PRESETS {
        let asset = parse_vfx(preset.source).unwrap();
        assert!(asset.emitters.iter().all(|e| e.render.render_method == RenderMethod::Auto), "{}", preset.name);
        let text = write_vfx(&asset).unwrap();
        assert!(!text.contains("render_method"), "{}: written while Auto:\n{text}", preset.name);
    }
    let old = r#"(emitters: [(graph: (nodes: [(id: 1, node_type: "output/particle")]), render: (blend: Opaque))])"#;
    let asset = parse_vfx(old).expect("an old file parses");
    assert_eq!(asset.emitters[0].render.render_method, RenderMethod::Auto);
    for method in [RenderMethod::Forward, RenderMethod::Deferred, RenderMethod::Auto] {
        let mut set = asset.clone();
        set.emitters[0].render.render_method = method;
        let text = write_vfx(&set).unwrap();
        assert_eq!(text.contains("render_method"), method != RenderMethod::Auto, "{method:?}:\n{text}");
        assert_eq!(parse_vfx(&text).unwrap(), set, "{method:?} round-trips");
    }
}

/// A route asks for one method and gets what the cameras allow: `Auto` and
/// `Forward` as asked, `Deferred` only while every active 3D camera can draw
/// it, and a transparent blend `Forward` whatever the emitter says.
#[test]
fn a_render_route_resolves_against_the_cameras() {
    use super::material::RenderRoute;
    use crate::asset::RenderMethod;

    let deferred = RenderRoute::new(RenderMethod::Deferred);
    assert_eq!(deferred.method, RenderMethod::Forward, "nobody has checked the cameras yet");
    assert_eq!(deferred.resolved(true).method, RenderMethod::Deferred);
    assert_eq!(deferred.resolved(true).resolved(false).method, RenderMethod::Forward, "a forward camera demotes it again");
    for m in [RenderMethod::Auto, RenderMethod::Forward] {
        assert_eq!(RenderRoute::new(m).resolved(false).method, m);
        assert_eq!(RenderRoute::new(m).resolved(true).method, m);
    }
    let mut r = RenderSettings { blend: BlendMode::Opaque, render_method: RenderMethod::Deferred, ..Default::default() };
    assert_eq!(RenderRoute::of(&r).requested, RenderMethod::Deferred);
    for blend in [BlendMode::Alpha, BlendMode::Additive, BlendMode::Premultiplied, BlendMode::Multiply] {
        r.blend = blend;
        assert_eq!(r.effective_render_method(), RenderMethod::Forward, "{blend:?}");
        assert_eq!(RenderRoute::of(&r).resolved(true).method, RenderMethod::Forward, "{blend:?}");
    }
    for blend in [BlendMode::AlphaMask, BlendMode::AlphaToCoverage] {
        r.blend = blend;
        assert_eq!(r.effective_render_method(), RenderMethod::Deferred, "{blend:?}");
    }
}

// ── The GPU pool mesh ───────────────────────────────────────────────────────

#[test]
fn the_gpu_pool_mesh_is_one_copy_per_slot_with_the_slot_in_z() {
    let t = MeshTemplate::built_in(&ParticleMesh::Tetrahedron).unwrap();
    let mesh = gpu_pool_mesh(&t, 5);
    let n = t.vertex_count();
    assert_eq!(mesh.count_vertices(), 5 * n);
    let pos = f3(&mesh, Mesh::ATTRIBUTE_POSITION);
    assert_eq!(pos[0][2], 0.0);
    assert_eq!(pos[4 * n][2], 4.0);
    assert_eq!(f3(&mesh, ATTRIBUTE_PARTICLE_LOCAL)[3 * n + 1], t.positions[1].to_array());
    // A pool past the vertex budget draws as many slots as fit.
    let cube = MeshTemplate::built_in(&ParticleMesh::Cube).unwrap();
    assert_eq!(gpu_pool_slots(&cube, 1000), 1000);
    let fit = gpu_pool_slots(&cube, crate::gpu::layout::MAX_GPU_CAPACITY);
    assert!(fit < crate::gpu::layout::MAX_GPU_CAPACITY && fit as usize * cube.vertex_count() <= super::mesh_particles::MAX_GPU_POOL_VERTICES);
}

// ── Backends ────────────────────────────────────────────────────────────────

#[test]
fn mesh_particles_run_on_the_gpu_with_or_without_a_project_material() {
    let caps = GpuCaps::all();
    let mut e = simple_effect(&[], vec![], vec![]).emitters[0].clone();
    e.capacity = 100_000;
    let c = compile_graph(&e.graph).unwrap();
    e.render.mesh = Some(ParticleMesh::Cube);
    e.render.lighting.lit = true;
    e.render.blend = BlendMode::Opaque;
    e.render.screen_space_size = true;
    assert_eq!(choose(&e, &c, false, false, &caps).active, ActiveBackend::Gpu, "meshes, lighting, opaque and pixels all run there");
    // The Ribbon facing does not apply to a mesh, so it does not block it.
    e.render.facing = Facing::Ribbon;
    assert_eq!(choose(&e, &c, false, false, &caps).active, ActiveBackend::Gpu);
    // A project material no longer keeps an emitter on the CPU: the surface
    // pipeline fetches from the pool like the particle shader does.
    e.render.material = Some("rock.material".into());
    let choice = choose(&e, &c, false, false, &caps);
    assert_eq!(choice.active, ActiveBackend::Gpu, "{:?}", choice.reason);
    assert!(crate::gpu::select::gpu_blockers(&e, &c, false, false, &caps).is_empty());
    // Nor on quads, in any of the new blends.
    e.render.mesh = None;
    e.render.facing = Facing::CameraBillboard;
    for blend in [BlendMode::Multiply, BlendMode::AlphaToCoverage, BlendMode::Premultiplied] {
        e.render.blend = blend;
        assert_eq!(choose(&e, &c, false, false, &caps).active, ActiveBackend::Gpu, "{blend:?}");
    }
}
