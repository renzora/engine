//! Networked emotes and a live voting tally over RPC. Emotes and votes are both
//! tiny broadcast messages, so they share one `net_msg` RPC with a `kind`
//! discriminator. Votes are kept per sender peer id (one vote each, the last
//! one wins) and every peer recounts the same way, so tallies agree; an emote
//! spawns a marker primitive above this pawn.
//!
//! Setup: attach to each player pawn and connect first. T: random emote;
//! 1 / 2: vote A / B. A UI text named `vote_leader` shows the leading option.

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::network::{self as net, Rpc, RpcArgs};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const EMOTES: [&str; 4] = ["wave", "laugh", "cheer", "cry"];

#[derive(Component, Default)]
struct Votes {
    /// Peer id to the option it voted for.
    voted: HashMap<u64, String>,
    emote_n: u32,
    seed: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let (emote, vote_a, vote_b) = match ctx.get_resource::<ScriptInput>() {
        Some(i) => (
            i.is_key_just_pressed(KeyCode::KeyT),
            i.is_key_just_pressed(KeyCode::Digit1),
            i.is_key_just_pressed(KeyCode::Digit2),
        ),
        None => return,
    };
    if !net::is_connected(ctx.world()) {
        return;
    }
    if emote {
        let Some(mut votes) = ctx.get_mut::<Votes>() else { return };
        let pick = EMOTES[(rand01(&mut votes.seed) * EMOTES.len() as f32) as usize % EMOTES.len()];
        drop(votes);
        Rpc::new("net_msg").arg("kind", "emote").arg("emote", pick).send(ctx.world());
    }
    if vote_a {
        Rpc::new("net_msg").arg("kind", "vote").arg("opt", "A").send(ctx.world());
    }
    if vote_b {
        Rpc::new("net_msg").arg("kind", "vote").arg("opt", "B").send(ctx.world());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            let seed = ctx.entity().to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
            ctx.insert(Votes { seed, ..default() });
            recount(ctx);
        }
        ScriptHook::Rpc { name: "net_msg", args, from } => match args.str("kind") {
            Some("emote") => {
                let Some(mut votes) = ctx.get_mut::<Votes>() else { return };
                votes.emote_n += 1;
                let name = format!("emote_{from}_{}", votes.emote_n);
                drop(votes);
                let Some(pos) = ctx.get::<Transform>().map(|t| t.translation) else { return };
                ctx.world().spawn((
                    Name::new(name),
                    Transform::from_translation(pos + Vec3::Y * 2.5),
                    Visibility::default(),
                    MeshPrimitive("sphere".into()),
                    MeshColor(Color::srgb(1.0, 1.0, 0.3)),
                ));
                renzora::clog_info!("Script", "peer#{} emotes {}", from, args.str("emote").unwrap_or("nil"));
            }
            Some("vote") => {
                let Some(opt) = args.str("opt") else { return };
                if let Some(mut votes) = ctx.get_mut::<Votes>() {
                    votes.voted.insert(*from, opt.to_string());
                }
                recount(ctx);
            }
            _ => {}
        },
        ScriptHook::PlayerLeft { id } => {
            let removed = ctx.get_mut::<Votes>().is_some_and(|mut v| v.voted.remove(id).is_some());
            if removed {
                recount(ctx);
            }
        }
        _ => {}
    }
}

/// Tally the votes and show the leader.
fn recount(ctx: &mut ScriptCtx) {
    let Some(votes) = ctx.get::<Votes>() else { return };
    // Ordered, so a tie resolves to the same option on every peer.
    let mut tally: BTreeMap<&str, i32> = BTreeMap::new();
    for opt in votes.voted.values() {
        *tally.entry(opt.as_str()).or_default() += 1;
    }
    let (who, best) = tally
        .iter()
        .fold(("-", -1), |acc, (opt, count)| if *count > acc.1 { (*opt, *count) } else { acc });
    let text = format!("Vote leader: {} ({})", who, best.max(0));
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", "vote_leader").arg("text", text).send(ctx.world());
}

/// A uniform value in 0..1 from a SplitMix64 step; scripts have no rand crate.
fn rand01(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update, hooks = hooks);
