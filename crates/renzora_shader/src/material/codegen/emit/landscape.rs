//! `landscape/*` node emitters.
//!
//! The painted weights arrive through `landscape_layer_weights` (see
//! `wgsl::landscape_helpers_wgsl`), read once per body and already in material
//! layer order, so a node only has to know a layer's index in
//! [`landscape_layer_infos`]. Off a terrain the helper returns one for the
//! first layer and zero for the rest.
//!
//! Every line here is flat: plain `let`s and arithmetic at function scope, no
//! branches and no loops. A blend over eight textured layers samples all of
//! them, and an earlier terrain shader that nested its samples inside branches
//! drew the whole terrain black on an RTX 3060 with no error anywhere.

use super::super::super::graph::{MaterialNode, NodeId};
use super::super::super::landscape::{
    blend_slot_active, blend_slot_name, landscape_layer_infos, sample_name, LandscapeBlend,
};
use super::super::super::nodes::LANDSCAPE_MAX_LAYERS;
use super::super::ctx::Ctx;

/// One active slot of a Layer Blend node, as the emitter needs it.
struct Slot {
    /// Slot number on the node (`layer_N`).
    slot: usize,
    /// Index into the material's layer list, which the weights are in.
    layer: usize,
    blend: LandscapeBlend,
    /// The slot's value, as a `vec4<f32>` expression.
    value: String,
    /// Variable holding the slot's weight as it is worked out.
    w: String,
}

impl Ctx<'_> {
    pub(crate) fn gen_landscape_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "landscape/layer_coords" => {
                // Terrain-local metres: the weight map's corner is the origin,
                // so the texture moves with the terrain rather than swimming
                // under it. Off a terrain the bounds are zero and this is
                // plain world X/Z.
                self.uses_landscape = true;
                let scale = self.input(node, "scale");
                let rotation = self.input(node, "rotation");
                let pan = self.input(node, "pan");
                let v = self.next_var("land_uv");
                self.emit(format!(
                    "    let {v}_p = (in.world_position.xz - landscape.bounds.xy + {pan}) / max({scale}, 0.0001);"
                ));
                self.emit(format!("    let {v}_r = radians({rotation});"));
                self.emit(format!(
                    "    let {v} = vec2<f32>(cos({v}_r) * {v}_p.x - sin({v}_r) * {v}_p.y, sin({v}_r) * {v}_p.x + cos({v}_r) * {v}_p.y);"
                ));
                self.set_out(id, "uv", v);
            }

            "landscape/layer_sample" => {
                let name = sample_name(node);
                let index = landscape_layer_infos(self.graph)
                    .iter()
                    .position(|l| l.name == name)
                    .unwrap_or(0);
                let weights = self.landscape_weights_var();
                let v = self.next_var("land_s");
                self.emit(format!("    let {v} = {weights}.w[{index}];"));
                self.set_out(id, "weight", v);
            }

            "landscape/slope" => {
                let v = self.next_var("land_slope");
                self.emit(format!("    let {v}_n = normalize(in.world_normal);"));
                self.emit(format!("    let {v} = degrees(acos(clamp({v}_n.y, -1.0, 1.0)));"));
                self.set_out(id, "degrees", v.clone());
                self.set_out(id, "height", "in.world_position.y".to_string());
                self.set_out(id, "normal_y", format!("{v}_n.y"));
            }

            "landscape/layer_blend" => self.gen_layer_blend(node, id),

            unknown => self.unknown_node(unknown),
        }
    }

    /// Mix the active slots by the terrain's painted weights.
    ///
    /// 1. Each slot starts from its layer's painted weight.
    /// 2. Each slot with an Auto Weight is painted on by it, in slot order:
    ///    everything is scaled by `1 - a` and the slot gains `a`. That is the
    ///    brush's own normalisation, so a rule behaves exactly as if the layer
    ///    had been painted there at that strength, and later slots win over
    ///    earlier ones (snow over rock).
    /// 3. Weight and Height slots are mixed together, normalised among
    ///    themselves; with nothing left the first of them takes over, so
    ///    unpainted ground is never black.
    /// 4. Height slots share their total by `height_mix`'s rule generalised to
    ///    any number of layers: each scores its weight plus its height, the
    ///    best score less a transition depth is the cut, and a layer shows by
    ///    how far it clears the cut. Two layers reduce to exactly the old
    ///    terrain shader's `height_mix`.
    /// 5. Alpha slots are laid over the result by their weights, in order.
    fn gen_layer_blend(&mut self, node: &MaterialNode, id: NodeId) {
        let infos = landscape_layer_infos(self.graph);
        let mut slots: Vec<Slot> = Vec::new();
        let v = self.next_var("land_b");
        for slot in 0..LANDSCAPE_MAX_LAYERS {
            if !blend_slot_active(self.graph, node, slot) {
                continue;
            }
            let name = blend_slot_name(node, slot);
            let layer = infos.iter().position(|l| l.name == name).unwrap_or(0);
            let blend = LandscapeBlend::from_pin(node.input_values.get(&format!("blend_{slot}")));
            let value = self.input(node, &format!("layer_{slot}"));
            slots.push(Slot { slot, layer, blend, value, w: format!("{v}_w{slot}") });
        }

        if slots.is_empty() {
            // Nothing wired: a neutral value, so a half-built graph still draws.
            self.set_blend_outs(id, "vec4<f32>(0.5, 0.5, 0.5, 1.0)".to_string());
            return;
        }
        // A blend with only Alpha layers has nothing to lay them over; the
        // first one is the ground instead.
        if slots.iter().all(|s| s.blend == LandscapeBlend::Alpha) {
            slots[0].blend = LandscapeBlend::Weight;
        }

        let weights = self.landscape_weights_var();
        for s in &slots {
            self.emit(format!("    var {} = {weights}.w[{}];", s.w, s.layer));
        }

        // 2. Auto weights. Only a wired pin or a non-zero constant costs code.
        for i in 0..slots.len() {
            let pin = format!("auto_{}", slots[i].slot);
            let wired = self.graph.connection_to(node.id, &pin).is_some();
            let constant = matches!(
                node.input_values.get(&pin),
                Some(super::super::super::graph::PinValue::Float(f)) if *f != 0.0
            );
            if !wired && !constant {
                continue;
            }
            let auto = self.input(node, &pin);
            let a = format!("{v}_a{}", slots[i].slot);
            self.emit(format!("    let {a} = saturate({auto});"));
            for s in &slots {
                self.emit(format!("    {} = {} * (1.0 - {a});", s.w, s.w));
            }
            let target = &slots[i].w;
            self.emit(format!("    {target} = {target} + {a};"));
        }

        // 3. The ground: Weight and Height slots.
        let ground: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].blend != LandscapeBlend::Alpha).collect();
        let sum = |idx: &[usize]| -> String {
            idx.iter().map(|&i| slots[i].w.clone()).collect::<Vec<_>>().join(" + ")
        };
        let first = &slots[ground[0]].w;
        self.emit(format!("    {first} = {first} + select(0.0, 1.0, ({}) < 0.0001);", sum(&ground)));
        let total = format!("{v}_t");
        self.emit(format!("    let {total} = max({}, 0.0001);", sum(&ground)));

        // 4. Height slots share their weight by height.
        let heights: Vec<usize> = ground.iter().copied().filter(|&i| slots[i].blend == LandscapeBlend::Height).collect();
        let mut share: Vec<(usize, String)> = ground.iter().map(|&i| (i, slots[i].w.clone())).collect();
        if heights.len() > 1 {
            let hb = self.input(node, "height_blending");
            self.emit(format!("    let {v}_hb = saturate({hb});"));
            self.emit(format!("    let {v}_c = 0.9 * {v}_hb;"));
            self.emit(format!("    let {v}_d = 0.08 + 0.4 * (1.0 - {v}_hb);"));
            let mut scores = Vec::new();
            for &i in &heights {
                let h = self.input(node, &format!("height_{}", slots[i].slot));
                let s = format!("{v}_s{}", slots[i].slot);
                let w = &slots[i].w;
                self.emit(format!("    let {s} = select(-10000.0, {w} + {v}_c * ({h}), {w} > 0.0001);"));
                scores.push(s);
            }
            let best = scores
                .iter()
                .skip(1)
                .fold(scores[0].clone(), |acc, s| format!("max({acc}, {s})"));
            self.emit(format!("    let {v}_m = {best} - {v}_d;"));
            let mut ks = Vec::new();
            for &i in &heights {
                let k = format!("{v}_k{}", slots[i].slot);
                self.emit(format!("    let {k} = max({v}_s{} - {v}_m, 0.0);", slots[i].slot));
                ks.push(k);
            }
            self.emit(format!("    let {v}_kt = max({}, 0.00001);", ks.join(" + ")));
            self.emit(format!("    let {v}_ht = {};", sum(&heights)));
            for (n, &i) in heights.iter().enumerate() {
                if let Some(entry) = share.iter_mut().find(|(j, _)| *j == i) {
                    entry.1 = format!("({v}_ht * {} / {v}_kt)", ks[n]);
                }
            }
        }

        let mix_terms: Vec<String> = share
            .iter()
            .map(|(i, w)| format!("({}) * {w}", slots[*i].value))
            .collect();
        self.emit(format!("    var {v} = ({}) / {total};", mix_terms.join(" + ")));

        // 5. Alpha slots over the ground.
        for s in slots.iter().filter(|s| s.blend == LandscapeBlend::Alpha) {
            self.emit(format!("    {v} = mix({v}, {}, saturate({}));", s.value, s.w));
        }
        self.set_blend_outs(id, v);
    }

    fn set_blend_outs(&mut self, id: NodeId, v: String) {
        self.set_out(id, "rgb", format!("{v}.rgb"));
        self.set_out(id, "normal", format!("normalize({v}.xyz)"));
        self.set_out(id, "value", format!("{v}.x"));
        self.set_out(id, "result", v);
    }
}
