//! Rig a body standing in the template's T-pose with the humanoid rig, and
//! list the finger bones found on it. For checking finger detection against a
//! real model without the editor; the markers are the template's own joints,
//! so the model has to be posed and sized like a Mixamo character.
//!
//! ```text
//! cargo run -p renzora_rig --profile dist --example probe_body -- <model.glb> [--out <rigged.glb>]
//! ```

use bevy::math::Vec3;
use renzora_rig::humanoid::{self, BodyMarkers, TEMPLATE};
use renzora_rig::Glb;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: probe_body <model.glb> [--out <rigged.glb>]");
        return;
    };
    let t = |n: &str| TEMPLATE.iter().find(|b| b.0 == n).map(|b| Vec3::from_array(b.2)).expect("template bone");
    let markers = BodyMarkers {
        chin: Vec3::new(0.0, 1.55, 0.05),
        shoulder: t("LeftArm"),
        elbow: t("LeftForeArm"),
        wrist: t("LeftHand"),
        groin: t("Hips"),
        knee: t("LeftLeg"),
        ankle: t("LeftFoot"),
    };
    let mut glb = Glb::parse(&std::fs::read(path).expect("read model")).expect("parse");
    let clock = std::time::Instant::now();
    let tris = renzora_rig::fingers::triangles(&glb).expect("triangles");
    println!("triangles: {} in {:.2?}", tris.len(), clock.elapsed());
    let found = renzora_rig::fingers::detect(&tris, "Left", markers.elbow, markers.wrist, markers.groin.x);
    println!("finger detection, one hand: {} joints in {:.2?}", found.len(), clock.elapsed());
    let clock = std::time::Instant::now();
    let applied = humanoid::build(&mut glb, &markers).expect("rig");
    println!("{applied:?} in {:.2?}", clock.elapsed());
    for n in glb.nodes() {
        let name = n.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.contains("Hand") {
            println!("{name} {:?}", n.get("translation"));
        }
    }
    if let Some(out) = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)) {
        std::fs::write(out, glb.to_bytes()).expect("write");
        println!("wrote {out}");
    }
}
