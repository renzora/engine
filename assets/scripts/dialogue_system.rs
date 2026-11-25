//! A branching dialogue engine: fetch a JSON tree, drive a UI panel, and
//! advance on choice callbacks. Dialogue is a graph of nodes with choices, so
//! the tree is loaded once over HTTP and the current node id is the only state
//! that changes; a `choice` callback walks the graph and re-renders the panel.
//!
//! The tree is `{"start": "id", "nodes": {"id": {"text": "...", "choices":
//! [{"label": "...", "next": "id"}]}}}`; a bare node map works too, starting at
//! "start". A choice whose `next` is "end" closes the panel.
//!
//! Setup: a UI template "dialogue" with a `dlg_line` label and up to three
//! choice buttons labelled `dlg_choice1` .. `dlg_choice3`, each firing `choice`
//! with a `next` argument. Presentation is local; choices could be voted over
//! RPC as `net_emote_vote.rs` does.

use bevy::prelude::*;
use renzora::network::RpcArgs;
use renzora::script_bridge::ScriptAction;
use renzora::serde_json::Value;
use renzora::{ScriptCtx, ScriptHook};

const URL: &str = "https://example.com/dialogue/intro.json";
const CALLBACK: &str = "dialogue";
const TEMPLATE: &str = "dialogue";
const CHOICES: usize = 3;

#[derive(Component, Default)]
struct Dialogue {
    /// Node id to node.
    tree: Option<Value>,
    node: Option<String>,
}

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    let me = ctx.entity();
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Dialogue::default());
            ScriptAction::new("hui_spawn", me).arg("template", TEMPLATE).send(ctx.world());
            ScriptAction::new("http_get", me).arg("url", URL).arg("callback", CALLBACK).send(ctx.world());
        }
        ScriptHook::Http { callback: CALLBACK, status, body } => {
            if *status == 0 {
                set_text(ctx, "dlg_line", "(failed to load)".into());
                return;
            }
            let Ok(data) = renzora::serde_json::from_str::<Value>(body) else { return };
            let start = data.get("start").and_then(Value::as_str).unwrap_or("start").to_string();
            let tree = data.get("nodes").cloned().unwrap_or(data);
            if let Some(mut d) = ctx.get_mut::<Dialogue>() {
                d.tree = Some(tree);
                d.node = Some(start);
            }
            render(ctx);
        }
        ScriptHook::Ui { name: "choice", args, .. } => {
            let Some(next) = args.str("next").map(str::to_string) else { return };
            if next == "end" {
                ScriptAction::new("hui_despawn", me).arg("template", TEMPLATE).send(ctx.world());
                if let Some(mut d) = ctx.get_mut::<Dialogue>() {
                    d.node = None;
                }
            } else {
                if let Some(mut d) = ctx.get_mut::<Dialogue>() {
                    d.node = Some(next);
                }
                render(ctx);
            }
        }
        _ => {}
    }
}

fn render(ctx: &mut ScriptCtx) {
    let Some(d) = ctx.get::<Dialogue>() else { return };
    let (Some(tree), Some(node)) = (&d.tree, &d.node) else { return };
    let Some(n) = tree.get(node.as_str()) else { return };
    let line = n.get("text").and_then(Value::as_str).unwrap_or("...").to_string();
    let choices = n.get("choices").and_then(Value::as_array);
    // Unused buttons are blanked so a shorter node does not show stale labels.
    let labels: Vec<String> = (0..CHOICES)
        .map(|i| {
            choices
                .and_then(|c| c.get(i))
                .and_then(|c| c.get("label"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect();
    set_text(ctx, "dlg_line", line);
    for (i, label) in labels.into_iter().enumerate() {
        set_text(ctx, &format!("dlg_choice{}", i + 1), label);
    }
}

fn set_text(ctx: &mut ScriptCtx, name: &str, text: String) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", name).arg("text", text).send(ctx.world());
}

renzora::script!(update, hooks = hooks);
