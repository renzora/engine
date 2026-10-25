//! Live link, editor end. The game end, and the why of the whole thing, is in
//! `renzora_live_link`.
//!
//! While a game is connected, the editor's world is a mirror of it: what the
//! game changes is applied here, so the existing hierarchy, inspector and
//! viewport show the game as it runs, and what the user changes here is sent
//! there. That makes the editor's world hold a moment of gameplay, which is
//! why this end also keeps a ledger, [`Session`]:
//!
//! - the value of everything the game has touched, as it was when the game
//!   connected;
//! - the last value the user gave anything they touched;
//! - what the game spawned, and snapshots of what it despawned.
//!
//! When the game stops, the ledger is played back: the game's changes are
//! undone, the user's are kept (a user edit to something the game also moved
//! wins, at the value the user gave it), and the scene is exactly what the
//! user made of it. The Keep button ([`renzora::KeepLiveChangesRequested`])
//! instead adopts everything the game did so far.
//!
//! Saving is held off while mirroring ([`renzora::LivePlaySession`]), and runs
//! after the put-back if it was asked for.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use bevy::prelude::*;
use renzora::console_log::{console_info, console_success};
use renzora::{
    GameInput, GameLink, KeepLiveChangesRequested, LiveLinkEndpoint, LivePlaySession, SceneEntityId,
};
use renzora_engine::scene_io::{snapshot_entity_subtrees, spawn_entities_from_snapshot};
use renzora_live_link::apply::{apply, ApplySide, Quiet};
use renzora_live_link::tracker::Tracker;
use renzora_live_link::{decode_base64, LinkMessage};

/// How often the user's edits are gathered. Fast enough that a dragged slider
/// reads as live in the game, slow enough that a drag is not a value per frame.
const SCAN_INTERVAL: f32 = 1.0 / 20.0;

#[derive(Default)]
pub struct LiveLinkEditorPlugin;

impl Plugin for LiveLinkEditorPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some((endpoint, incoming)) = listen() else {
                return;
            };
            app.insert_resource(endpoint)
                .init_resource::<GameLink>()
                .insert_resource(LinkServer {
                    connections: Mutex::new(incoming),
                    client: None,
                })
                .add_systems(PreUpdate, receive)
                .add_systems(PostUpdate, send);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = app;
    }
}

renzora::add!(LiveLinkEditorPlugin, Editor);

enum Incoming {
    Message(LinkMessage),
    Closed,
}

#[derive(Resource)]
struct LinkServer {
    /// Connections that passed the token check, handed over by the listener
    /// thread. A `Mutex` only because a resource must be `Sync`.
    connections: Mutex<Receiver<TcpStream>>,
    client: Option<Client>,
}

struct Client {
    writer: TcpStream,
    incoming: Mutex<Receiver<Incoming>>,
    tracker: Tracker,
    session: Session,
    unknown: HashSet<String>,
    since_scan: f32,
    /// Releases to send later, for inputs given a hold time.
    releases: Vec<(f32, GameInput)>,
}

/// What it takes to put the scene back when the game stops. See the module
/// docs.
#[derive(Default)]
struct Session {
    /// Each scene entity's values when the game connected.
    baseline: HashMap<u64, HashMap<String, String>>,
    /// Components the game has set or removed.
    game_touched: HashSet<(u64, String)>,
    /// Components the user has set (`Some`) or removed (`None`), last value.
    user: HashMap<(u64, String), Option<String>>,
    /// Entities the game spawned.
    spawned: HashSet<u64>,
    /// Entities the game despawned, as snapshots to respawn them from.
    despawned: Vec<String>,
    /// Entities the user despawned during play, which must stay gone.
    user_despawned: HashSet<u64>,
}

/// Record, as the game's changes are applied, what it takes to undo them.
struct Mirror<'a>(&'a mut Session);

impl ApplySide for Mirror<'_> {
    fn spawned(&mut self, _world: &mut World, _entity: Entity, id: u64) {
        self.0.spawned.insert(id);
    }
    fn despawning(&mut self, world: &mut World, entity: Entity, id: u64) {
        // Something the game itself made needs no snapshot: putting things
        // back means it should not exist.
        if self.0.spawned.remove(&id) {
            return;
        }
        if let Some(snapshot) = snapshot_entity_subtrees(world, &[entity]) {
            self.0.despawned.push(snapshot);
        }
    }
    fn touching(&mut self, id: u64, ty: &str) {
        self.0.game_touched.insert((id, ty.to_string()));
    }
}

/// Bind a loopback port and accept the game on a thread of its own.
///
/// Port 0 lets the OS pick a free one, so two editors on one machine never
/// collide. Only loopback, and only a connection whose first line carries the
/// token, is ever handed to the main thread.
fn listen() -> Option<(LiveLinkEndpoint, Receiver<TcpStream>)> {
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(e) => {
            warn!("[live-link] could not listen: {e}; edits will not reach a running game");
            return None;
        }
    };
    let port = listener.local_addr().ok()?.port();
    let token = format!(
        "{:016x}{:016x}",
        SceneEntityId::new_random().0,
        SceneEntityId::new_random().0
    );
    let (tx, rx) = channel();
    let expected = token.clone();
    std::thread::Builder::new()
        .name("live-link-listen".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if accepts(&stream, &expected) && tx.send(stream).is_err() {
                    break;
                }
            }
        })
        .ok()?;
    Some((LiveLinkEndpoint { port, token }, rx))
}

/// Whether `stream`'s first line is a hello with the right token. Bounded by a
/// timeout so a connection that never speaks cannot hold the listener.
fn accepts(stream: &TcpStream, token: &str) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Ok(reader) = stream.try_clone() else { return false };
    let mut line = String::new();
    if BufReader::new(reader).read_line(&mut line).is_err() {
        return false;
    }
    let _ = stream.set_read_timeout(None);
    matches!(
        serde_json::from_str::<LinkMessage>(&line),
        Ok(LinkMessage::Hello { token: t }) if t == token
    )
}

/// Read the rest of a connection's lines on a thread, ending with `Closed`.
fn read_lines(stream: TcpStream, tx: Sender<Incoming>) {
    std::thread::Builder::new()
        .name("live-link-read".into())
        .spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                if let Ok(message) = serde_json::from_str::<LinkMessage>(&line) {
                    if tx.send(Incoming::Message(message)).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(Incoming::Closed);
        })
        .ok();
}

/// Take in a new game, and what the connected one sent.
fn receive(world: &mut World) {
    let connected = world
        .resource::<LinkServer>()
        .connections
        .lock()
        .ok()
        .and_then(|rx| rx.try_iter().last());
    if let Some(stream) = connected {
        // A newer game replaces the old one; put back what the old one did.
        end_session(world);
        start_session(world, stream);
        return;
    }

    let Some(mut client) = world.resource_mut::<LinkServer>().client.take() else {
        return;
    };
    let incoming: Vec<Incoming> = client
        .incoming
        .lock()
        .map(|rx| rx.try_iter().collect())
        .unwrap_or_default();
    let mut closed = false;
    let mut messages = Vec::new();
    for item in incoming {
        match item {
            Incoming::Message(m) => messages.push(m),
            Incoming::Closed => closed = true,
        }
    }

    // The user's edits go first. An edit made since the last scan has not been
    // sent yet, and the game's reports in `messages` were written before the
    // game knew of it; applied first, they would overwrite the edit with the
    // game's older value and it would never be sent at all. That lost every
    // edit to something the game keeps moving, which is the player, which is
    // the first thing anyone edits. So: gather and send the user's edits now,
    // and drop the game's reports about exactly those components.
    if !messages.is_empty() {
        let edits = client.tracker.scan(world);
        if !edits.is_empty() {
            let mut edited: HashSet<(u64, Option<String>)> = HashSet::new();
            for edit in &edits {
                record_user_edit(&mut client.session, edit);
                match edit {
                    LinkMessage::Set { id, ty, .. } | LinkMessage::Remove { id, ty } => {
                        edited.insert((*id, Some(ty.clone())));
                    }
                    LinkMessage::Despawn { id } => {
                        edited.insert((*id, None));
                    }
                    _ => {}
                }
            }
            messages.retain(|m| match m {
                LinkMessage::Set { id, ty, .. } | LinkMessage::Remove { id, ty } => {
                    !edited.contains(&(*id, Some(ty.clone()))) && !edited.contains(&(*id, None))
                }
                _ => true,
            });
            if !write_all(&mut client.writer, &edits) {
                closed = true;
            }
        }
    }

    let rest = apply(
        world,
        messages,
        &mut client.tracker,
        &mut Mirror(&mut client.session),
        &mut client.unknown,
    );
    for message in rest {
        if let LinkMessage::Screenshot { req, png } = message {
            let png = png.and_then(|b| decode_base64(&b).ok_or_else(|| "bad screenshot data".to_string()));
            world.resource_mut::<GameLink>().screenshots.insert(req, png);
        }
    }
    world.resource_mut::<LinkServer>().client = Some(client);
    if closed {
        end_session(world);
    }
}

/// Send the user's edits, and whatever tools queued for the game.
fn send(world: &mut World) {
    let Some(mut client) = world.resource_mut::<LinkServer>().client.take() else {
        // Keep the queue from growing while there is no game to send to.
        let mut link = world.resource_mut::<GameLink>();
        link.inputs.clear();
        for req in std::mem::take(&mut link.screenshot_requests) {
            link.screenshots.insert(req, Err("no game is running".into()));
        }
        return;
    };

    if world.remove_resource::<KeepLiveChangesRequested>().is_some() {
        keep(&mut client);
    }

    let dt = world.resource::<Time>().delta_secs();
    let mut messages = Vec::new();

    {
        let mut link = world.resource_mut::<GameLink>();
        for (input, hold) in std::mem::take(&mut link.inputs) {
            if let Some(hold) = hold {
                client.releases.push((hold, released(&input)));
            }
            messages.push(LinkMessage::Input(input));
        }
        for req in std::mem::take(&mut link.screenshot_requests) {
            messages.push(LinkMessage::ScreenshotRequest { req });
        }
    }
    client.releases.retain_mut(|(left, input)| {
        *left -= dt;
        if *left <= 0.0 {
            messages.push(LinkMessage::Input(input.clone()));
            false
        } else {
            true
        }
    });

    client.since_scan += dt;
    if client.since_scan >= SCAN_INTERVAL {
        client.since_scan = 0.0;
        for edit in client.tracker.scan(world) {
            record_user_edit(&mut client.session, &edit);
            messages.push(edit);
        }
    }

    let failed = !write_all(&mut client.writer, &messages);
    world.resource_mut::<LinkServer>().client = Some(client);
    if failed {
        end_session(world);
    }
}

/// Write `messages` as JSON lines. False when the game has gone.
fn write_all(writer: &mut TcpStream, messages: &[LinkMessage]) -> bool {
    for message in messages {
        let Ok(line) = serde_json::to_string(message) else { continue };
        if writeln!(writer, "{line}").is_err() {
            return false;
        }
    }
    true
}

/// The input that undoes `input` after its hold time.
fn released(input: &GameInput) -> GameInput {
    match input {
        GameInput::Key { code, .. } => GameInput::Key { code: code.clone(), pressed: false },
        GameInput::GamepadButton { button, .. } => {
            GameInput::GamepadButton { button: button.clone(), value: 0.0 }
        }
        GameInput::GamepadAxis { axis, .. } => GameInput::GamepadAxis { axis: axis.clone(), value: 0.0 },
    }
}

fn record_user_edit(session: &mut Session, edit: &LinkMessage) {
    match edit {
        LinkMessage::Set { id, ty, value } => {
            session.user.insert((*id, ty.clone()), Some(value.clone()));
        }
        LinkMessage::Remove { id, ty } => {
            session.user.insert((*id, ty.clone()), None);
        }
        LinkMessage::Despawn { id } => {
            session.user_despawned.insert(*id);
        }
        _ => {}
    }
}

fn start_session(world: &mut World, stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let Ok(reader) = stream.try_clone() else { return };
    let (tx, rx) = channel();
    read_lines(reader, tx);
    let tracker = Tracker::baseline(world);
    let session = Session { baseline: tracker.known().clone(), ..default() };
    world.resource_mut::<LinkServer>().client = Some(Client {
        writer: stream,
        incoming: Mutex::new(rx),
        tracker,
        session,
        unknown: HashSet::new(),
        since_scan: 0.0,
        releases: Vec::new(),
    });
    world.insert_resource(LivePlaySession::default());
    world.resource_mut::<GameLink>().connected = true;
    console_info(
        "LiveLink",
        "Game connected: edits here reach the game, and the game shows here while it runs",
    );
}

/// Adopt everything the game has done so far: from now on, Stop puts things
/// back only as far as this moment.
fn keep(client: &mut Client) {
    let session = &mut client.session;
    for (id, ty) in session.game_touched.drain() {
        match client.tracker.known().get(&id).and_then(|v| v.get(&ty)) {
            Some(value) => {
                session.baseline.entry(id).or_default().insert(ty, value.clone());
            }
            None => {
                if let Some(values) = session.baseline.get_mut(&id) {
                    values.remove(&ty);
                }
            }
        }
    }
    session.spawned.clear();
    session.despawned.clear();
    console_success("LiveLink", "Kept what the game changed; Stop will leave it in the scene");
}

/// The game is gone: put back what it changed, keep what the user changed.
fn end_session(world: &mut World) {
    let Some(client) = world.resource_mut::<LinkServer>().client.take() else {
        return;
    };
    let session = client.session;

    // Respawn what the game removed, then remove what it added.
    for snapshot in &session.despawned {
        spawn_entities_from_snapshot(world, snapshot);
    }
    let mut undo: Vec<LinkMessage> = session
        .spawned
        .iter()
        .filter(|id| !session.user_despawned.contains(id))
        .map(|&id| LinkMessage::Despawn { id })
        .collect();

    // Values: the user's last word where they had one, otherwise as it was.
    for (id, ty) in &session.game_touched {
        if session.spawned.contains(id) {
            continue;
        }
        let value = match session.user.get(&(*id, ty.clone())) {
            Some(user) => user.clone(),
            None => session.baseline.get(id).and_then(|v| v.get(ty)).cloned(),
        };
        undo.push(match value {
            Some(value) => LinkMessage::Set { id: *id, ty: ty.clone(), value },
            None => LinkMessage::Remove { id: *id, ty: ty.clone() },
        });
    }
    let restored = undo.len() + session.despawned.len();
    apply(world, undo, &mut Tracker::default(), &mut Quiet, &mut HashSet::new());

    world.resource_mut::<GameLink>().connected = false;
    let pending_save = world
        .remove_resource::<LivePlaySession>()
        .is_some_and(|s| s.pending_save);
    console_info(
        "LiveLink",
        if restored > 0 {
            "Game stopped: put back what the game changed, kept your edits"
        } else {
            "Game stopped"
        },
    );
    if pending_save {
        world.insert_resource(renzora::SaveSceneRequested);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// An editor world holding one scene entity, a crate with id 1 at the
    /// origin, with a game connected over a real loopback socket. Returns the
    /// game's end of that socket.
    fn editor_with_game() -> (World, Entity, TcpStream) {
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        {
            let mut r = registry.write();
            r.register::<Transform>();
            r.register::<GlobalTransform>();
            r.register::<Name>();
            r.register::<SceneEntityId>();
            r.register::<ChildOf>();
            r.register::<Visibility>();
        }
        world.insert_resource(registry);
        world.insert_resource(Time::<()>::default());
        world.init_resource::<GameLink>();
        world.insert_resource(LinkServer { connections: Mutex::new(channel().1), client: None });
        let crate_e = world
            .spawn((Name::new("crate"), SceneEntityId(1), Transform::default()))
            .id();

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let game = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (editor_end, _) = listener.accept().unwrap();
        start_session(&mut world, editor_end);
        (world, crate_e, game)
    }

    fn game_sends(game: &mut TcpStream, messages: &[LinkMessage]) {
        for m in messages {
            writeln!(game, "{}", serde_json::to_string(m).unwrap()).unwrap();
        }
    }

    /// Run `receive` until `done` holds, as frames would.
    fn frames_until(world: &mut World, done: impl Fn(&mut World) -> bool) {
        let start = Instant::now();
        while !done(world) {
            assert!(start.elapsed() < Duration::from_secs(5), "timed out waiting on the link");
            world.increment_change_tick();
            receive(world);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn named(world: &mut World, name: &str) -> Option<Entity> {
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
    }

    fn transform_value(t: Transform, world: &World) -> String {
        let registry = world.resource::<AppTypeRegistry>().read();
        ron::ser::to_string(&bevy::reflect::serde::TypedReflectSerializer::new(&t, &registry)).unwrap()
    }

    #[test]
    fn stop_puts_back_what_the_game_did_and_keeps_what_the_user_did() {
        let (mut world, crate_e, mut game) = editor_with_game();
        assert!(world.get_resource::<LivePlaySession>().is_some());

        // The game moves the crate and spawns a bullet.
        let moved = transform_value(Transform::from_xyz(5.0, 0.0, 0.0), &world);
        game_sends(
            &mut game,
            &[
                LinkMessage::Set { id: 1, ty: Transform::type_path().into(), value: moved },
                LinkMessage::Spawn { id: 99, parent: None, name: "bullet".into() },
            ],
        );
        frames_until(&mut world, |w| named(w, "bullet").is_some());
        assert_eq!(world.get::<Transform>(crate_e).unwrap().translation.x, 5.0);

        // The user hides the crate while the game runs.
        world.increment_change_tick();
        world.entity_mut(crate_e).insert(Visibility::Hidden);
        world.resource_mut::<LinkServer>().client.as_mut().unwrap().since_scan = 1.0;
        send(&mut world);

        // A save asked for now waits for Stop.
        world.resource_mut::<LivePlaySession>().pending_save = true;

        // The game stops.
        drop(game);
        frames_until(&mut world, |w| w.get_resource::<LivePlaySession>().is_none());

        assert_eq!(world.get::<Transform>(crate_e).unwrap().translation, Vec3::ZERO, "the game's move stayed");
        assert!(named(&mut world, "bullet").is_none(), "the game's bullet stayed");
        assert_eq!(world.get::<Visibility>(crate_e), Some(&Visibility::Hidden), "the user's edit was lost");
        assert!(world.get_resource::<renzora::SaveSceneRequested>().is_some(), "the held save never ran");
        assert!(!world.resource::<GameLink>().connected);
    }

    #[test]
    fn a_user_edit_beats_a_game_report_written_before_it() {
        let (mut world, crate_e, mut game) = editor_with_game();
        game.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

        // The user moves the crate; before the editor's next scan, a report
        // from the game arrives with the crate where the game last had it.
        world.increment_change_tick();
        world.get_mut::<Transform>(crate_e).unwrap().translation = Vec3::new(60.0, 3.0, 35.0);
        let stale = transform_value(Transform::from_xyz(0.0, -1.0, 6.0), &world);
        game_sends(&mut game, &[LinkMessage::Set { id: 1, ty: Transform::type_path().into(), value: stale }]);

        let start = Instant::now();
        let mut reader = BufReader::new(game.try_clone().unwrap());
        frames_until(&mut world, |w| {
            // Any frame's receive is enough once the report has arrived.
            start.elapsed() > Duration::from_millis(200) || w.get_resource::<LivePlaySession>().is_none()
        });

        assert_eq!(
            world.get::<Transform>(crate_e).unwrap().translation,
            Vec3::new(60.0, 3.0, 35.0),
            "the game's older report overwrote the user's edit"
        );
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let sent: LinkMessage = serde_json::from_str(&line).unwrap();
        let LinkMessage::Set { ty, value, .. } = sent else { panic!("sent {sent:?}") };
        assert_eq!(ty, Transform::type_path());
        assert!(value.contains("60.0"), "the game was not sent the user's edit: {value}");
    }

    #[test]
    fn keep_adopts_what_the_game_did() {
        let (mut world, crate_e, mut game) = editor_with_game();
        let moved = transform_value(Transform::from_xyz(0.0, 0.0, 3.0), &world);
        game_sends(
            &mut game,
            &[
                LinkMessage::Set { id: 1, ty: Transform::type_path().into(), value: moved },
                LinkMessage::Spawn { id: 7, parent: None, name: "rock".into() },
            ],
        );
        frames_until(&mut world, |w| named(w, "rock").is_some());

        world.insert_resource(KeepLiveChangesRequested);
        send(&mut world);
        drop(game);
        frames_until(&mut world, |w| w.get_resource::<LivePlaySession>().is_none());

        assert_eq!(world.get::<Transform>(crate_e).unwrap().translation.z, 3.0);
        assert!(named(&mut world, "rock").is_some());
    }
}
