//! Inventory and economy: a stackable item bag with a shop, a UI display, and
//! cloud persistence over HTTP. The capstone ties several systems together: the
//! bag is the model, UI labels are the view, buy and sell mutate gold with
//! validation, and the whole bag round-trips to a server as JSON so progress
//! survives restarts.
//!
//! The bag is per player, keyed on the server by this entity's name, and not
//! shared over the network.
//!
//! Setup: a UI template "inventory" with labels `inv_gold`, `inv_msg` and
//! `inv_slot1` .. `inv_slotN`, and buttons firing `buy` / `sell` (with an `item`
//! argument) and `save`. Autosaves every 30 seconds.

use std::collections::BTreeMap;

use bevy::prelude::*;
use renzora::network::RpcArgs;
use renzora::script_bridge::ScriptAction;
use renzora::serde_json::{json, Value};
use renzora::{ScriptCtx, ScriptHook};

const URL: &str = "https://example.com/inv/";
const START_GOLD: i64 = 100;
/// Seconds between autosaves.
const AUTOSAVE: f32 = 30.0;
/// Selling returns this fraction of the price.
const SELL_RATE: f64 = 0.5;

fn price(item: &str) -> Option<i64> {
    match item {
        "potion" => Some(25),
        "sword" => Some(80),
        "key" => Some(10),
        _ => None,
    }
}

#[derive(Component)]
struct Inventory {
    gold: i64,
    /// Item to count. Ordered, so each item keeps its slot between refreshes.
    bag: BTreeMap<String, i64>,
    autosave: f32,
    /// Slots filled by the last refresh, so emptied ones can be blanked.
    shown_slots: usize,
}

impl Inventory {
    fn add(&mut self, item: &str, n: i64) {
        let count = self.bag.entry(item.to_string()).or_default();
        *count += n;
        if *count <= 0 {
            self.bag.remove(item);
        }
    }
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut inv) = ctx.get_mut::<Inventory>() else { return };
    inv.autosave -= dt;
    if inv.autosave > 0.0 {
        return;
    }
    inv.autosave += AUTOSAVE;
    drop(inv);
    save(ctx);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    let me = ctx.entity();
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Inventory { gold: START_GOLD, bag: BTreeMap::new(), autosave: AUTOSAVE, shown_slots: 0 });
            ScriptAction::new("hui_spawn", me).arg("template", "inventory").send(ctx.world());
            let url = bag_url(ctx);
            ScriptAction::new("http_get", me).arg("url", url).arg("callback", "inv_load").send(ctx.world());
        }
        ScriptHook::Ui { name: "buy", args, .. } => {
            if let Some(item) = args.str("item") {
                buy(ctx, item);
            }
        }
        ScriptHook::Ui { name: "sell", args, .. } => {
            if let Some(item) = args.str("item") {
                sell(ctx, item);
            }
        }
        ScriptHook::Ui { name: "save", .. } => save(ctx),
        ScriptHook::Http { callback: "inv_load", status, body } => {
            if *status != 0 {
                if let Ok(d) = renzora::serde_json::from_str::<Value>(body) {
                    if let Some(mut inv) = ctx.get_mut::<Inventory>() {
                        if let Some(gold) = d.get("gold").and_then(Value::as_i64) {
                            inv.gold = gold;
                        }
                        inv.bag = d
                            .get("bag")
                            .and_then(Value::as_object)
                            .map(|bag| {
                                bag.iter()
                                    .filter_map(|(k, v)| Some((k.clone(), v.as_i64()?)))
                                    .filter(|(_, n)| *n > 0)
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                }
            }
            refresh_ui(ctx);
        }
        ScriptHook::Http { callback: "inv_save", status, .. } => {
            let text = if *status == 0 { "save failed" } else { "saved" };
            set_text(ctx, "inv_msg", text.to_string());
        }
        _ => {}
    }
}

fn buy(ctx: &mut ScriptCtx, item: &str) {
    let Some(cost) = price(item) else { return };
    let Some(mut inv) = ctx.get_mut::<Inventory>() else { return };
    if inv.gold < cost {
        drop(inv);
        set_text(ctx, "inv_msg", "Not enough gold!".to_string());
        return;
    }
    inv.gold -= cost;
    inv.add(item, 1);
    drop(inv);
    refresh_ui(ctx);
}

fn sell(ctx: &mut ScriptCtx, item: &str) {
    let Some(mut inv) = ctx.get_mut::<Inventory>() else { return };
    if !inv.bag.contains_key(item) {
        return;
    }
    inv.gold += (price(item).unwrap_or(0) as f64 * SELL_RATE).floor() as i64;
    inv.add(item, -1);
    drop(inv);
    refresh_ui(ctx);
}

fn refresh_ui(ctx: &mut ScriptCtx) {
    let Some(mut inv) = ctx.get_mut::<Inventory>() else { return };
    let gold = inv.gold;
    let mut lines: Vec<String> = inv.bag.iter().map(|(item, qty)| format!("{item} x{qty}")).collect();
    let filled = lines.len();
    // Lua left a sold-out item's label behind; blank the slots no longer used.
    lines.resize(filled.max(inv.shown_slots), String::new());
    inv.shown_slots = filled;
    drop(inv);
    set_text(ctx, "inv_gold", format!("Gold: {gold}"));
    for (i, line) in lines.into_iter().enumerate() {
        set_text(ctx, &format!("inv_slot{}", i + 1), line);
    }
}

fn save(ctx: &mut ScriptCtx) {
    let Some(inv) = ctx.get::<Inventory>() else { return };
    let body = json!({ "gold": inv.gold, "bag": inv.bag }).to_string();
    let me = ctx.entity();
    let url = bag_url(ctx);
    ScriptAction::new("http_post", me)
        .arg("url", url)
        .arg("body", body)
        .arg("callback", "inv_save")
        .send(ctx.world());
}

fn bag_url(ctx: &ScriptCtx) -> String {
    format!("{URL}{}", ctx.name().unwrap_or("unnamed"))
}

fn set_text(ctx: &mut ScriptCtx, name: &str, text: String) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", name).arg("text", text).send(ctx.world());
}

renzora::script!(update, hooks = hooks);
