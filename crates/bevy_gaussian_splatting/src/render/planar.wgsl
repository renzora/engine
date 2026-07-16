#define_import_path bevy_gaussian_splatting::planar

#import bevy_gaussian_splatting::bindings::gaussian_uniforms

#ifdef GAUSSIAN_3D_STRUCTURE
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::bindings::{
            position_visibility,
            spherical_harmonics,
            covariance_3d_opacity,
        }

        #ifdef BINARY_GAUSSIAN_OP
            #import bevy_gaussian_splatting::bindings::{
                rhs_position_visibility,
                rhs_spherical_harmonics,
                rhs_covariance_3d_opacity,
                out_position_visibility,
                out_spherical_harmonics,
                out_covariance_3d_opacity,
            }
        #endif
    #else
        #import bevy_gaussian_splatting::bindings::{
            position_visibility,
            spherical_harmonics,
            rotation,
            rotation_scale_opacity,
            scale_opacity,
        }

        #ifdef BINARY_GAUSSIAN_OP
            #import bevy_gaussian_splatting::bindings::{
                rhs_position_visibility,
                rhs_spherical_harmonics,
                out_position_visibility,
                out_spherical_harmonics,
            }

            #ifdef PLANAR_F16
                #import bevy_gaussian_splatting::bindings::rhs_rotation_scale_opacity
                #import bevy_gaussian_splatting::bindings::out_rotation_scale_opacity
            #endif

            #ifdef PLANAR_F32
                #import bevy_gaussian_splatting::bindings::{
                    rhs_rotation,
                    rhs_scale_opacity,
                    out_rotation,
                    out_scale_opacity,
                }
            #endif
        #endif
    #endif

    #import bevy_gaussian_splatting::spherical_harmonics::{
        spherical_harmonics_lookup,
        srgb_to_linear,
    }
#else ifdef GAUSSIAN_4D
    #import bevy_gaussian_splatting::bindings::{
        position_visibility,
        spherindrical_harmonics,
        isotropic_rotations,
        scale_opacity,
        timestamp_timescale,
    }

    #ifdef BINARY_GAUSSIAN_OP
        #import bevy_gaussian_splatting::bindings::{
            rhs_position_visibility,
            rhs_spherindrical_harmonics,
            rhs_isotropic_rotations,
            rhs_scale_opacity,
            rhs_timestamp_timescale,
        }
    #endif

    #import bevy_gaussian_splatting::spherical_harmonics::srgb_to_linear
    #import bevy_gaussian_splatting::spherindrical_harmonics::spherindrical_harmonics_lookup
#endif

fn planar_position(value: vec4<f32>) -> vec3<f32> {
    return value.xyz;
}

fn planar_visibility(value: vec4<f32>) -> f32 {
    return value.w;
}

fn convert_sh_color_to_linear(color: vec3<f32>) -> vec3<f32> {
    if gaussian_uniforms.color_space == 1u {
        return color;
    }

    return srgb_to_linear(color);
}

#ifdef GAUSSIAN_3D_STRUCTURE
    fn planar_color_from_sh(
        ray_direction: vec3<f32>,
        sh: array<f32, #{SH_COEFF_COUNT}>,
    ) -> vec3<f32> {
        let color = spherical_harmonics_lookup(ray_direction, sh);
        return convert_sh_color_to_linear(color);
    }

    fn planar_scale_from(scale_opacity: vec4<f32>) -> vec3<f32> {
        return scale_opacity.xyz;
    }

    fn planar_opacity_from(scale_opacity: vec4<f32>) -> f32 {
        return scale_opacity.w;
    }

    #ifdef PLANAR_F16
        fn planar_f16_decode_sh(
            raw: array<u32, #{HALF_SH_COEFF_COUNT}>,
        ) -> array<f32, #{SH_COEFF_COUNT}> {
            var coefficients: array<f32, #{SH_COEFF_COUNT}>;

            for (var i = 0u; i < #{HALF_SH_COEFF_COUNT}u; i = i + 1u) {
                let values = unpack2x16float(raw[i]);

                coefficients[i * 2u] = values[0];
                coefficients[i * 2u + 1u] = values[1];
            }

            return coefficients;
        }

        #ifdef PRECOMPUTE_COVARIANCE_3D
            fn planar_f16_decode_covariance(raw: vec4<u32>) -> array<f32, 6> {
                let c0 = unpack2x16float(raw.x);
                let c1 = unpack2x16float(raw.y);
                let c2 = unpack2x16float(raw.z);

                var cov3d: array<f32, 6>;

                cov3d[0] = c0.y;
                cov3d[1] = c0.x;
                cov3d[2] = c1.y;
                cov3d[3] = c1.x;
                cov3d[4] = c2.y;
                cov3d[5] = c2.x;

                return cov3d;
            }

            fn planar_f16_opacity(raw: vec4<u32>) -> f32 {
                return unpack2x16float(raw.w).y;
            }
        #else
            fn planar_f16_decode_rotation(raw: vec4<u32>) -> vec4<f32> {
                let q0 = unpack2x16float(raw.x);
                let q1 = unpack2x16float(raw.y);

                return vec4<f32>(
                    q0.yx,
                    q1.yx,
                );
            }

            fn planar_f16_decode_scale(raw: vec4<u32>) -> vec3<f32> {
                let s0 = unpack2x16float(raw.z);
                let s1 = unpack2x16float(raw.w);

                return vec3<f32>(
                    s0.yx,
                    s1.y,
                );
            }

            fn planar_f16_opacity(raw: vec4<u32>) -> f32 {
                return unpack2x16float(raw.w).x;
            }
        #endif

        fn get_color(
            index: u32,
            ray_direction: vec3<f32>,
        ) -> vec3<f32> {
            let sh = planar_f16_decode_sh(spherical_harmonics[index]);
            return planar_color_from_sh(ray_direction, sh);
        }

        fn get_position(index: u32) -> vec3<f32> {
            return planar_position(position_visibility[index]);
        }

        fn get_spherical_harmonics(index: u32) -> array<f32, #{SH_COEFF_COUNT}> {
            return planar_f16_decode_sh(spherical_harmonics[index]);
        }

        #ifdef PRECOMPUTE_COVARIANCE_3D
            fn get_cov3d(index: u32) -> array<f32, 6> {
                return planar_f16_decode_covariance(covariance_3d_opacity[index]);
            }
        #else
            fn get_rotation(index: u32) -> vec4<f32> {
                return planar_f16_decode_rotation(rotation_scale_opacity[index]);
            }

            fn get_scale(index: u32) -> vec3<f32> {
                return planar_f16_decode_scale(rotation_scale_opacity[index]);
            }
        #endif

        fn get_opacity(index: u32) -> f32 {
            #ifdef PRECOMPUTE_COVARIANCE_3D
                return planar_f16_opacity(covariance_3d_opacity[index]);
            #else
                return planar_f16_opacity(rotation_scale_opacity[index]);
            #endif
        }

        fn get_visibility(index: u32) -> f32 {
            return planar_visibility(position_visibility[index]);
        }

        #ifdef BINARY_GAUSSIAN_OP
            fn get_rhs_color(
                index: u32,
                ray_direction: vec3<f32>,
            ) -> vec3<f32> {
                let sh = planar_f16_decode_sh(rhs_spherical_harmonics[index]);
                return planar_color_from_sh(ray_direction, sh);
            }

            fn get_rhs_position(index: u32) -> vec3<f32> {
                return planar_position(rhs_position_visibility[index]);
            }

            fn get_rhs_spherical_harmonics(index: u32) -> array<f32, #{SH_COEFF_COUNT}> {
                return planar_f16_decode_sh(rhs_spherical_harmonics[index]);
            }

            #ifdef PRECOMPUTE_COVARIANCE_3D
                fn get_rhs_cov3d(index: u32) -> array<f32, 6> {
                    return planar_f16_decode_covariance(rhs_covariance_3d_opacity[index]);
                }
            #else
                fn get_rhs_rotation(index: u32) -> vec4<f32> {
                    return planar_f16_decode_rotation(rhs_rotation_scale_opacity[index]);
                }

                fn get_rhs_scale(index: u32) -> vec3<f32> {
                    return planar_f16_decode_scale(rhs_rotation_scale_opacity[index]);
                }
            #endif

            fn get_rhs_opacity(index: u32) -> f32 {
                #ifdef PRECOMPUTE_COVARIANCE_3D
                    return planar_f16_opacity(rhs_covariance_3d_opacity[index]);
                #else
                    return planar_f16_opacity(rhs_rotation_scale_opacity[index]);
                #endif
            }

            fn get_rhs_visibility(index: u32) -> f32 {
                return planar_visibility(rhs_position_visibility[index]);
            }

            fn set_output_position_visibility(
                index: u32,
                position: vec3<f32>,
                visibility: f32,
            ) {
                out_position_visibility[index] = vec4<f32>(position, visibility);
            }

            fn set_output_spherical_harmonics(
                index: u32,
                sh: array<f32, #{SH_COEFF_COUNT}>,
            ) {
                for (var i = 0u; i < #{HALF_SH_COEFF_COUNT}; i = i + 1u) {
                    let base = i * 2u;
                    out_spherical_harmonics[index][i] = pack2x16float(vec2<f32>(
                        sh[base],
                        sh[base + 1u],
                    ));
                }
            }

            #ifdef PRECOMPUTE_COVARIANCE_3D
                fn planar_f16_encode_covariance(
                    cov: array<f32, 6>,
                    opacity: f32,
                ) -> vec4<u32> {
                    return vec4<u32>(
                        pack2x16float(vec2<f32>(cov[1], cov[0])),
                        pack2x16float(vec2<f32>(cov[3], cov[2])),
                        pack2x16float(vec2<f32>(cov[5], cov[4])),
                        pack2x16float(vec2<f32>(0.0, opacity)),
                    );
                }

                fn set_output_covariance(
                    index: u32,
                    cov: array<f32, 6>,
                    opacity: f32,
                ) {
                    out_covariance_3d_opacity[index] = planar_f16_encode_covariance(cov, opacity);
                }
            #else
                fn planar_f16_encode_rotation_scale_opacity(
                    rotation: vec4<f32>,
                    scale: vec3<f32>,
                    opacity: f32,
                ) -> vec4<u32> {
                    return vec4<u32>(
                        pack2x16float(vec2<f32>(rotation.y, rotation.x)),
                        pack2x16float(vec2<f32>(rotation.w, rotation.z)),
                        pack2x16float(vec2<f32>(scale.y, scale.x)),
                        pack2x16float(vec2<f32>(opacity, scale.z)),
                    );
                }

                fn set_output_transform(
                    index: u32,
                    rotation: vec4<f32>,
                    scale: vec3<f32>,
                    opacity: f32,
                ) {
                    out_rotation_scale_opacity[index] = planar_f16_encode_rotation_scale_opacity(
                        rotation,
                        scale,
                        opacity,
                    );
                }
            #endif

        #endif
    #else ifdef PLANAR_F32
        fn get_color(
            index: u32,
            ray_direction: vec3<f32>,
        ) -> vec3<f32> {
            return planar_color_from_sh(ray_direction, spherical_harmonics[index]);
        }

        fn get_position(index: u32) -> vec3<f32> {
            return planar_position(position_visibility[index]);
        }

        fn get_spherical_harmonics(index: u32) -> array<f32, #{SH_COEFF_COUNT}> {
            return spherical_harmonics[index];
        }

        fn get_rotation(index: u32) -> vec4<f32> {
            return rotation[index];
        }

        fn get_scale(index: u32) -> vec3<f32> {
            return planar_scale_from(scale_opacity[index]);
        }

        fn get_opacity(index: u32) -> f32 {
            return planar_opacity_from(scale_opacity[index]);
        }

        fn get_visibility(index: u32) -> f32 {
            return planar_visibility(position_visibility[index]);
        }

        #ifdef BINARY_GAUSSIAN_OP
            fn get_rhs_color(
                index: u32,
                ray_direction: vec3<f32>,
            ) -> vec3<f32> {
                return planar_color_from_sh(ray_direction, rhs_spherical_harmonics[index]);
            }

            fn get_rhs_position(index: u32) -> vec3<f32> {
                return planar_position(rhs_position_visibility[index]);
            }

            fn get_rhs_spherical_harmonics(index: u32) -> array<f32, #{SH_COEFF_COUNT}> {
                return rhs_spherical_harmonics[index];
            }

            fn get_rhs_rotation(index: u32) -> vec4<f32> {
                return rhs_rotation[index];
            }

            fn get_rhs_scale(index: u32) -> vec3<f32> {
                return planar_scale_from(rhs_scale_opacity[index]);
            }

            fn get_rhs_opacity(index: u32) -> f32 {
                return planar_opacity_from(rhs_scale_opacity[index]);
            }

            fn get_rhs_visibility(index: u32) -> f32 {
                return planar_visibility(rhs_position_visibility[index]);
            }

            fn set_output_position_visibility(
                index: u32,
                position: vec3<f32>,
                visibility: f32,
            ) {
                out_position_visibility[index] = vec4<f32>(position, visibility);
            }

            fn set_output_spherical_harmonics(
                index: u32,
                sh: array<f32, #{SH_COEFF_COUNT}>,
            ) {
                for (var i = 0u; i < #{SH_COEFF_COUNT}; i = i + 1u) {
                    out_spherical_harmonics[index][i] = sh[i];
                }
            }

            #ifdef PRECOMPUTE_COVARIANCE_3D
                fn set_output_covariance(
                    index: u32,
                    cov: array<f32, 6>,
                    opacity: f32,
                ) {
                    out_covariance_3d_opacity[index][0] = cov[0];
                    out_covariance_3d_opacity[index][1] = cov[1];
                    out_covariance_3d_opacity[index][2] = cov[2];
                    out_covariance_3d_opacity[index][3] = cov[3];
                    out_covariance_3d_opacity[index][4] = cov[4];
                    out_covariance_3d_opacity[index][5] = cov[5];
                    out_covariance_3d_opacity[index][6] = 0.0;
                    out_covariance_3d_opacity[index][7] = opacity;
                }
            #else
                fn set_output_transform(
                    index: u32,
                    rotation: vec4<f32>,
                    scale: vec3<f32>,
                    opacity: f32,
                ) {
                    out_rotation[index] = rotation;
                    out_scale_opacity[index] = vec4<f32>(scale, opacity);
                }
            #endif

        #endif
    #endif
#else ifdef GAUSSIAN_4D
    fn planar4d_color_from_sh(
        ray_direction: vec3<f32>,
        dir_t: f32,
        sh: array<f32, #{SH_4D_COEFF_COUNT}>,
    ) -> vec3<f32> {
        let color = spherindrical_harmonics_lookup(ray_direction, dir_t, sh);
        return convert_sh_color_to_linear(color);
    }

    fn planar4d_isotropic_rotations(raw: array<f32, 8>) -> mat2x4<f32> {
        let r1x = raw[0];
        let r1y = raw[1];
        let r1z = raw[2];
        let r1w = raw[3];

        let r2x = raw[4];
        let r2y = raw[5];
        let r2z = raw[6];
        let r2w = raw[7];

        return mat2x4<f32>(
            r1x, r1y, r1z, r1w,
            r2x, r2y, r2z, r2w,
        );
    }

    fn planar4d_scale_from(scale_opacity: vec4<f32>) -> vec3<f32> {
        return scale_opacity.xyz;
    }

    fn planar4d_opacity_from(scale_opacity: vec4<f32>) -> f32 {
        return scale_opacity.w;
    }

    fn planar4d_timestamp_from(timestamp_timescale: vec4<f32>) -> f32 {
        return timestamp_timescale.x;
    }

    fn planar4d_time_scale_from(timestamp_timescale: vec4<f32>) -> f32 {
        return timestamp_timescale.y;
    }

    #ifdef PLANAR_F32
        fn get_color(
            index: u32,
            dir_t: f32,
            ray_direction: vec3<f32>,
        ) -> vec3<f32> {
            return planar4d_color_from_sh(ray_direction, dir_t, spherindrical_harmonics[index]);
        }

        fn get_isotropic_rotations(index: u32) -> mat2x4<f32> {
            return planar4d_isotropic_rotations(isotropic_rotations[index]);
        }

        fn get_scale(index: u32) -> vec3<f32> {
            return planar4d_scale_from(scale_opacity[index]);
        }

        fn get_opacity(index: u32) -> f32 {
            return planar4d_opacity_from(scale_opacity[index]);
        }

        fn get_position(index: u32) -> vec3<f32> {
            return planar_position(position_visibility[index]);
        }

        fn get_visibility(index: u32) -> f32 {
            return planar_visibility(position_visibility[index]);
        }

        fn get_spherindrical_harmonics(index: u32) -> array<f32, #{SH_4D_COEFF_COUNT}> {
            return spherindrical_harmonics[index];
        }

        fn get_timestamp(index: u32) -> f32 {
            return planar4d_timestamp_from(timestamp_timescale[index]);
        }

        fn get_time_scale(index: u32) -> f32 {
            return planar4d_time_scale_from(timestamp_timescale[index]);
        }

        #ifdef BINARY_GAUSSIAN_OP
            fn get_rhs_color(
                index: u32,
                dir_t: f32,
                ray_direction: vec3<f32>,
            ) -> vec3<f32> {
                return planar4d_color_from_sh(ray_direction, dir_t, rhs_spherindrical_harmonics[index]);
            }

            fn get_rhs_isotropic_rotations(index: u32) -> mat2x4<f32> {
                return planar4d_isotropic_rotations(rhs_isotropic_rotations[index]);
            }

            fn get_rhs_scale(index: u32) -> vec3<f32> {
                return planar4d_scale_from(rhs_scale_opacity[index]);
            }

            fn get_rhs_opacity(index: u32) -> f32 {
                return planar4d_opacity_from(rhs_scale_opacity[index]);
            }

            fn get_rhs_position(index: u32) -> vec3<f32> {
                return planar_position(rhs_position_visibility[index]);
            }

            fn get_rhs_visibility(index: u32) -> f32 {
                return planar_visibility(rhs_position_visibility[index]);
            }

            fn get_rhs_spherindrical_harmonics(index: u32) -> array<f32, #{SH_4D_COEFF_COUNT}> {
                return rhs_spherindrical_harmonics[index];
            }

            fn get_rhs_timestamp(index: u32) -> f32 {
                return planar4d_timestamp_from(rhs_timestamp_timescale[index]);
            }

            fn get_rhs_time_scale(index: u32) -> f32 {
                return planar4d_time_scale_from(rhs_timestamp_timescale[index]);
            }
        #endif
    #endif

    // TODO: PLANAR_F16 for GAUSSIAN_4D
#endif
