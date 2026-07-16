#define_import_path bevy_gaussian_splatting::spherindrical_harmonics

#import bevy_gaussian_splatting::bindings::gaussian_uniforms
#import bevy_gaussian_splatting::spherical_harmonics::{
    shc,
    spherical_harmonics_lookup,
}

const PI = radians(180.0);

fn spherindrical_harmonics_lookup(
    ray_direction: vec3<f32>,
    dir_t: f32,
    sh: array<f32, #{SH_4D_COEFF_COUNT}>,
) -> vec3<f32> {
    var color = vec3<f32>(0.5);

    // TODO: reinterpret sh as vec3<f32>
    color += shc[ 0] * vec3<f32>(sh[0], sh[1], sh[2]);

#if SH_DEGREE > 0
    let x = ray_direction.x;
    let y = ray_direction.y;
    let z = ray_direction.z;

    let l1m1 = shc[1] * y;
    let l1m0 = shc[2] * z;
    let l1p1 = shc[3] * x;

    color += l1m1 * vec3<f32>(sh[ 3], sh[ 4], sh[ 5]);
    color += l1m0 * vec3<f32>(sh[ 6], sh[ 7], sh[ 8]);
    color += l1p1 * vec3<f32>(sh[ 9], sh[10], sh[11]);
#endif

#if SH_DEGREE > 1
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;

    let l2m2 = shc[4] * xy;
    let l2m1 = shc[5] * yz;
    let l2m0 = shc[6] * (2.0 * zz - xx - yy);
    let l2p1 = shc[7] * xz;
    let l2p2 = shc[8] * (xx - yy);

    color += l2m2 * vec3<f32>(sh[12], sh[13], sh[14]);
    color += l2m1 * vec3<f32>(sh[15], sh[16], sh[17]);
    color += l2m0 * vec3<f32>(sh[18], sh[19], sh[20]);
    color += l2p1 * vec3<f32>(sh[21], sh[22], sh[23]);
    color += l2p2 * vec3<f32>(sh[24], sh[25], sh[26]);
#endif

#if SH_DEGREE > 2
    let l3m3 = shc[9] * y * (3.0 * xx - yy);
    let l3m2 = shc[10] * z * xy;
    let l3m1 = shc[11] * y * (4.0 * zz - xx - yy);
    let l3m0 = shc[12] * z * (2.0 * zz - 3.0 * xx - 3.0 * yy);
    let l3p1 = shc[13] * x * (4.0 * zz - xx - yy);
    let l3p2 = shc[14] * z * (xx - yy);
    let l3p3 = shc[15] * x * (xx - 3.0 * yy);

    color += l3m3 * vec3<f32>(sh[27], sh[28], sh[29]);
    color += l3m2 * vec3<f32>(sh[30], sh[31], sh[32]);
    color += l3m1 * vec3<f32>(sh[33], sh[34], sh[35]);
    color += l3m0 * vec3<f32>(sh[36], sh[37], sh[38]);
    color += l3p1 * vec3<f32>(sh[39], sh[40], sh[41]);
    color += l3p2 * vec3<f32>(sh[42], sh[43], sh[44]);
    color += l3p3 * vec3<f32>(sh[45], sh[46], sh[47]);
#endif

#if SH_DEGREE_TIME > 0
    let duration = gaussian_uniforms.time_stop - gaussian_uniforms.time_start;
    let theta = dir_t / duration;

    let t1 = cos(2.0 * PI * theta);

    let l0m0 = shc[0];

    color += t1 * (
        l0m0 * vec3<f32>(sh[48], sh[49], sh[50]) +
        l1m1 * vec3<f32>(sh[51], sh[52], sh[53]) +
        l1m0 * vec3<f32>(sh[54], sh[55], sh[56]) +
        l1p1 * vec3<f32>(sh[57], sh[58], sh[59]) +
        l2m2 * vec3<f32>(sh[60], sh[61], sh[62]) +
        l2m1 * vec3<f32>(sh[63], sh[64], sh[65]) +
        l2m0 * vec3<f32>(sh[66], sh[67], sh[68]) +
        l2p1 * vec3<f32>(sh[69], sh[70], sh[71]) +
        l2p2 * vec3<f32>(sh[72], sh[73], sh[74]) +
        l3m3 * vec3<f32>(sh[75], sh[76], sh[77]) +
        l3m2 * vec3<f32>(sh[78], sh[79], sh[80]) +
        l3m1 * vec3<f32>(sh[81], sh[82], sh[83]) +
        l3m0 * vec3<f32>(sh[84], sh[85], sh[86]) +
        l3p1 * vec3<f32>(sh[87], sh[88], sh[89]) +
        l3p2 * vec3<f32>(sh[90], sh[91], sh[92]) +
        l3p3 * vec3<f32>(sh[93], sh[94], sh[95])
    );

    #if SH_DEGREE_TIME > 1
        let t2 = cos(4.0 * PI * theta);

        color += t2 * (
            l0m0 * vec3<f32>(sh[ 96], sh[ 97], sh[ 98]) +
            l1m1 * vec3<f32>(sh[ 99], sh[100], sh[101]) +
            l1m0 * vec3<f32>(sh[102], sh[103], sh[104]) +
            l1p1 * vec3<f32>(sh[105], sh[106], sh[107]) +
            l2m2 * vec3<f32>(sh[108], sh[109], sh[110]) +
            l2m1 * vec3<f32>(sh[111], sh[112], sh[113]) +
            l2m0 * vec3<f32>(sh[114], sh[115], sh[116]) +
            l2p1 * vec3<f32>(sh[117], sh[118], sh[119]) +
            l2p2 * vec3<f32>(sh[120], sh[121], sh[122]) +
            l3m3 * vec3<f32>(sh[123], sh[124], sh[125]) +
            l3m2 * vec3<f32>(sh[126], sh[127], sh[128]) +
            l3m1 * vec3<f32>(sh[129], sh[130], sh[131]) +
            l3m0 * vec3<f32>(sh[132], sh[133], sh[134]) +
            l3p1 * vec3<f32>(sh[135], sh[136], sh[137]) +
            l3p2 * vec3<f32>(sh[138], sh[139], sh[140]) +
            l3p3 * vec3<f32>(sh[141], sh[142], sh[143])
        );
    #endif
#endif

    return color;
}
