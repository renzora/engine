#define_import_path bevy_gaussian_splatting::bindings

#import bevy_pbr::prepass_bindings::PreviousViewUniforms
#import bevy_render::globals::Globals
#import bevy_render::view::View

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(0) @binding(2) var<uniform> previous_view_uniforms: PreviousViewUniforms;

@group(0) @binding(14) var<storage> visibility_ranges: array<vec4<f32>>;

struct GaussianUniforms {
    transform: mat4x4<f32>,
    global_opacity: f32,
    global_scale: f32,
    count: u32,
    count_root_ceil: u32,
    time: f32,
    time_start: f32,
    time_stop: f32,
    num_classes: u32,
    color_space: u32,
    min: vec4<f32>,
    max: vec4<f32>,
};
@group(1) @binding(0) var<uniform> gaussian_uniforms: GaussianUniforms;

#ifdef GAUSSIAN_3D_STRUCTURE
    #ifdef PACKED_F32
        struct Gaussian {
            @location(0) rotation: vec4<f32>,
            @location(1) position_visibility: vec4<f32>,
            @location(2) scale_opacity: vec4<f32>,
            sh: array<f32, #{SH_COEFF_COUNT}>,
        };

        #ifdef READ_WRITE_POINTS
            @group(2) @binding(0) var<storage, read_write> points: array<Gaussian>;
        #else
            @group(2) @binding(0) var<storage, read> points: array<Gaussian>;
        #endif

        #ifdef BINARY_GAUSSIAN_OP
            #ifdef READ_WRITE_POINTS
                @group(3) @binding(0) var<storage, read_write> rhs_points: array<Gaussian>;
            #else
                @group(3) @binding(0) var<storage, read> rhs_points: array<Gaussian>;
            #endif

            @group(4) @binding(0) var<storage, read_write> out_points: array<Gaussian>;
        #endif
    #endif

    #ifdef PLANAR_F32
        #ifdef READ_WRITE_POINTS
            @group(2) @binding(0) var<storage, read_write> position_visibility: array<vec4<f32>>;
        #else
            @group(2) @binding(0) var<storage, read> position_visibility: array<vec4<f32>>;
        #endif

        @group(2) @binding(1) var<storage, read> spherical_harmonics: array<array<f32, #{SH_COEFF_COUNT}>>;

        #ifdef BINARY_GAUSSIAN_OP
            #ifdef READ_WRITE_POINTS
                @group(3) @binding(0) var<storage, read_write> rhs_position_visibility: array<vec4<f32>>;
            #else
                @group(3) @binding(0) var<storage, read> rhs_position_visibility: array<vec4<f32>>;
            #endif

            @group(3) @binding(1) var<storage, read> rhs_spherical_harmonics: array<array<f32, #{SH_COEFF_COUNT}>>;

            #ifdef PRECOMPUTE_COVARIANCE_3D
                @group(3) @binding(2) var<storage, read> rhs_covariance_3d_opacity: array<array<f32, 8>>;
            #else
                @group(3) @binding(2) var<storage, read> rhs_rotation: array<vec4<f32>>;
                @group(3) @binding(3) var<storage, read> rhs_scale_opacity: array<vec4<f32>>;
            #endif
        #endif


        #ifdef BINARY_GAUSSIAN_OP
            @group(4) @binding(0) var<storage, read_write> out_position_visibility: array<vec4<f32>>;
            @group(4) @binding(1) var<storage, read_write> out_spherical_harmonics: array<array<f32, #{SH_COEFF_COUNT}>>;

            #ifdef PRECOMPUTE_COVARIANCE_3D
                @group(4) @binding(2) var<storage, read_write> out_covariance_3d_opacity: array<array<f32, 8>>;
            #else
                @group(4) @binding(2) var<storage, read_write> out_rotation: array<vec4<f32>>;
                @group(4) @binding(3) var<storage, read_write> out_scale_opacity: array<vec4<f32>>;
            #endif
        #endif

        #ifdef PRECOMPUTE_COVARIANCE_3D
            @group(2) @binding(2) var<storage, read> covariance_3d_opacity: array<array<f32, 8>>;
        #else
            @group(2) @binding(2) var<storage, read> rotation: array<vec4<f32>>;
            @group(2) @binding(3) var<storage, read> scale_opacity: array<vec4<f32>>;
        #endif
    #endif

    #ifdef PLANAR_F16
        #ifdef READ_WRITE_POINTS
            @group(2) @binding(0) var<storage, read_write> position_visibility: array<vec4<f32>>;
        #else
            @group(2) @binding(0) var<storage, read> position_visibility: array<vec4<f32>>;
        #endif

        @group(2) @binding(1) var<storage, read> spherical_harmonics: array<array<u32, #{HALF_SH_COEFF_COUNT}>>;

        #ifdef BINARY_GAUSSIAN_OP
            #ifdef READ_WRITE_POINTS
                @group(3) @binding(0) var<storage, read_write> rhs_position_visibility: array<vec4<f32>>;
            #else
                @group(3) @binding(0) var<storage, read> rhs_position_visibility: array<vec4<f32>>;
            #endif

            @group(3) @binding(1) var<storage, read> rhs_spherical_harmonics: array<array<u32, #{HALF_SH_COEFF_COUNT}>>;

            #ifdef PRECOMPUTE_COVARIANCE_3D
                @group(3) @binding(2) var<storage, read> rhs_covariance_3d_opacity: array<vec4<u32>>;
            #else
                @group(3) @binding(2) var<storage, read> rhs_rotation_scale_opacity: array<vec4<u32>>;
            #endif

            @group(4) @binding(0) var<storage, read_write> out_position_visibility: array<vec4<f32>>;
            @group(4) @binding(1) var<storage, read_write> out_spherical_harmonics: array<array<u32, #{HALF_SH_COEFF_COUNT}>>;

            #ifdef PRECOMPUTE_COVARIANCE_3D
                @group(4) @binding(2) var<storage, read_write> out_covariance_3d_opacity: array<vec4<u32>>;
            #else
                @group(4) @binding(2) var<storage, read_write> out_rotation_scale_opacity: array<vec4<u32>>;
            #endif
        #endif

        #ifdef PRECOMPUTE_COVARIANCE_3D
            @group(2) @binding(2) var<storage, read> covariance_3d_opacity: array<vec4<u32>>;
        #else
            @group(2) @binding(2) var<storage, read> rotation_scale_opacity: array<vec4<u32>>;
        #endif
    #endif

    #ifdef PLANAR_TEXTURE_F16
        @group(2) @binding(0) var position_visibility: texture_2d<f32>;

        #if SH_VEC4_PLANES == 1
            @group(2) @binding(1) var spherical_harmonics: texture_2d<u32>;
        #else
            @group(2) @binding(1) var spherical_harmonics: texture_2d_array<u32>;
        #endif

        #ifdef PRECOMPUTE_COVARIANCE_3D
            @group(2) @binding(2) var covariance_3d_opacity: texture_2d<u32>;
        #else
            @group(2) @binding(2) var rotation_scale_opacity: texture_2d<u32>;
        #endif

        #ifdef BINARY_GAUSSIAN_OP
            @group(3) @binding(0) var rhs_position_visibility: texture_2d<f32>;

            #if SH_VEC4_PLANES == 1
                @group(3) @binding(1) var rhs_spherical_harmonics: texture_2d<u32>;
            #else
                @group(3) @binding(1) var rhs_spherical_harmonics: texture_2d_array<u32>;
            #endif

            #ifdef PRECOMPUTE_COVARIANCE_3D
                @group(3) @binding(2) var rhs_covariance_3d_opacity: texture_2d<u32>;
            #else
                @group(3) @binding(2) var rhs_rotation_scale_opacity: texture_2d<u32>;
            #endif
        #endif
    #endif

    #ifdef PLANAR_TEXTURE_F32
        @group(2) @binding(0) var position_visibility: texture_2d<f32>;

        #if SH_VEC4_PLANES == 1
            @group(2) @binding(1) var spherical_harmonics: texture_2d<f32>;
        #else
            @group(2) @binding(1) var spherical_harmonics: texture_2d_array<f32>;
        #endif

        // TODO: support f32_cov3d_opacity texture

        @group(2) @binding(2) var rotation_scale_opacity: texture_2d<f32>;

        #ifdef BINARY_GAUSSIAN_OP
            @group(3) @binding(0) var rhs_position_visibility: texture_2d<f32>;

            #if SH_VEC4_PLANES == 1
                @group(3) @binding(1) var rhs_spherical_harmonics: texture_2d<f32>;
            #else
                @group(3) @binding(1) var rhs_spherical_harmonics: texture_2d_array<f32>;
            #endif

            @group(3) @binding(2) var rhs_rotation_scale_opacity: texture_2d<f32>;
        #endif
    #endif
#else ifdef GAUSSIAN_4D
    #ifdef PLANAR_F32
        #ifdef READ_WRITE_POINTS
            @group(2) @binding(0) var<storage, read_write> position_visibility: array<vec4<f32>>;
        #else
            @group(2) @binding(0) var<storage, read> position_visibility: array<vec4<f32>>;
        #endif

        @group(2) @binding(1) var<storage, read> spherindrical_harmonics: array<array<f32, #{SH_4D_COEFF_COUNT}>>;

        @group(2) @binding(2) var<storage, read> isotropic_rotations: array<array<f32, 8>>;
        @group(2) @binding(3) var<storage, read> scale_opacity: array<vec4<f32>>;
        @group(2) @binding(4) var<storage, read> timestamp_timescale: array<vec4<f32>>;

        #ifdef BINARY_GAUSSIAN_OP
            @group(3) @binding(0) var<storage, read> rhs_position_visibility: array<vec4<f32>>;
            @group(3) @binding(1) var<storage, read> rhs_spherindrical_harmonics: array<array<f32, #{SH_4D_COEFF_COUNT}>>;
            @group(3) @binding(2) var<storage, read> rhs_isotropic_rotations: array<array<f32, 8>>;
            @group(3) @binding(3) var<storage, read> rhs_scale_opacity: array<vec4<f32>>;
            @group(3) @binding(4) var<storage, read> rhs_timestamp_timescale: array<vec4<f32>>;
        #endif
    #endif
#endif

struct DrawIndirect {
    vertex_count: u32,
    instance_count: atomic<u32>,
    base_vertex: u32,
    base_instance: u32,
}

struct Entry {
    key: u32,
    value: u32,
}
