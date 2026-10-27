//! Write each virtual geometry pixel's material id as depth. The material
//! passes then draw one fullscreen triangle per material at that depth with an
//! `Equal` test, so the hardware's early depth test skips every pixel that
//! belongs to another material before the material's shader runs.

import super::types::VG_NOTHING;
import super::bindings::{vg_visibility, vg_visible, vg_instances};

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    let stored = textureLoad(vg_visibility, vec2<u32>(position.xy), 0).r;
    if stored == VG_NOTHING {
        discard;
    }
    let entry = vg_visible[(stored - 1u) >> 7u];
    let id = vg_instances[entry.x].material & 0xffffu;
    return f32(id) / 65535.0;
}
