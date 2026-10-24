//! Measure a head model's face and, with `--out`, rig it with markers guessed
//! from the measurement, writing the result to that path. For checking the
//! face rig against a real model without the editor.
//!
//! ```text
//! cargo run -p renzora_rig --profile dist --example probe_face -- <model.glb> [--out <rigged.glb>]
//! ```

use bevy::math::Vec3;
use renzora_rig::face::{self, FaceMarkers};
use renzora_rig::Glb;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: probe_face <model.glb> [--out <rigged.glb>]");
        return;
    };
    let bytes = std::fs::read(path).expect("read model");
    let mut glb = Glb::parse(&bytes).expect("parse");
    for (i, n) in glb.nodes().iter().enumerate() {
        if let Some(mesh) = n.get("mesh").and_then(|m| m.as_u64()) {
            let name = glb.mesh_name(mesh as usize);
            println!("mesh `{name}` (node {i}): {:?}", face::mesh_role(&name, &glb.node_name(i)));
        }
    }
    let f = face::measure(&glb).expect("measure");
    println!("{f:#?}");
    let Some(out) = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)) else { return };
    let s = f.scale;
    let mouth = (f.eye_l + f.eye_r) * 0.5 - f.up * s * 1.05 + f.forward * s * 0.25;
    let markers = FaceMarkers {
        mouth_corner: mouth + f.left * s * 0.4,
        upper_lip: mouth + f.up * s * 0.06,
        lower_lip: mouth - f.up * s * 0.06,
        chin: mouth - f.up * s * 0.65 - f.forward * s * 0.05 + Vec3::ZERO,
    };
    let (applied, _) = face::build(&mut glb, &markers).expect("rig");
    println!("{applied:?}");
    std::fs::write(out, glb.to_bytes()).expect("write");
    let check = Glb::parse(&std::fs::read(out).expect("reread")).expect("reparse");
    println!("wrote {out}: {} skins, {} nodes", check.skins().len(), check.nodes().len());
}
