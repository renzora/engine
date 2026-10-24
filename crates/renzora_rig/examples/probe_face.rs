//! Measure a head model's face and, with `--out`, rig it with markers guessed
//! from the measurement, writing the result to that path. For checking the
//! face rig against a real model without the editor.
//!
//! ```text
//! cargo run -p renzora_rig --profile dist --example probe_face -- <model.glb> [--out <rigged.glb>] [--from-rig <rigged.glb>]
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
    let guessed = FaceMarkers {
        mouth_corner: mouth + f.left * s * 0.4,
        upper_lip: mouth + f.up * s * 0.06,
        lower_lip: mouth - f.up * s * 0.06,
        chin: mouth - f.up * s * 0.65 - f.forward * s * 0.05 + Vec3::ZERO,
        eye: None,
    };
    // `--from-rig <rigged.glb>`: the points a face rig was built from, read
    // back from its bones, to rig the original again the same way. The lips
    // and corner are bones' places; the chin is the head pivot's height back.
    let markers = match args.iter().position(|a| a == "--from-rig").and_then(|i| args.get(i + 1)) {
        Some(rig) => {
            let rigged = Glb::parse(&std::fs::read(rig).expect("read rig")).expect("parse rig");
            let bones = renzora_rig::edit::read_bones(&rigged);
            let at = |n: &str| bones.iter().find(|b| b.name == n).map(|b| b.head).unwrap_or_else(|| panic!("{n} not in the rig"));
            let lower = at(face::names::LIP_LOWER);
            let mut chin = lower;
            chin.y = at(face::names::HEAD).y + s * 0.6;
            FaceMarkers { mouth_corner: at(face::names::CORNER_L), upper_lip: at(face::names::LIP_UPPER), lower_lip: lower, chin, eye: None }
        }
        None => guessed,
    };
    let (applied, _) = face::build(&mut glb, &markers).expect("rig");
    println!("{applied:?}");
    std::fs::write(out, glb.to_bytes()).expect("write");
    let check = Glb::parse(&std::fs::read(out).expect("reread")).expect("reparse");
    println!("wrote {out}: {} skins, {} nodes", check.skins().len(), check.nodes().len());
}
