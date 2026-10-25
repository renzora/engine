//! The game end: connect to the editor, apply what it sends, report back.
//!
//! Installed only when the process was launched with `--live-link <port>`,
//! which only Play does. A shipped game is never given the flag, so it opens
//! no connection and adds no systems.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::input::gamepad::{
    GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent,
    RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use crate::apply::{apply, Quiet};
use crate::tracker::Tracker;
use crate::{GameInput, LinkMessage};

/// How often the game reports what changed. Matches the editor's rate, so a
/// moving player reads as moving rather than teleporting.
const SCAN_INTERVAL: f32 = 1.0 / 20.0;

/// Screenshots are scaled down to fit this width before sending: enough to
/// read a HUD, small enough to send over a line of JSON in a few milliseconds.
const SCREENSHOT_WIDTH: u32 = 1280;

enum Incoming {
    Connected(TcpStream),
    Message(LinkMessage),
    Closed,
}

#[derive(Resource)]
struct Link {
    /// From the socket thread. A `Mutex` only because a resource must be
    /// `Sync` and a `Receiver` is not.
    rx: Mutex<Receiver<Incoming>>,
    writer: Option<TcpStream>,
    tracker: Option<Tracker>,
    unknown: HashSet<String>,
    since_scan: f32,
    /// Replies produced outside the systems below (a screenshot finishes in an
    /// observer, frames later), waiting to be written.
    outbox: Arc<Mutex<Vec<LinkMessage>>>,
    pad: VirtualPad,
}

/// A gamepad the editor plays through, for input sent over the link. It is a
/// real `Gamepad` entity as far as the game can tell, so a script that reads
/// gamepads needs nothing special to be played by an agent.
#[derive(Default)]
struct VirtualPad {
    entity: Option<Entity>,
    /// Inputs held until Bevy has finished setting the pad up, which takes a
    /// frame after it is announced.
    pending: Vec<GameInput>,
}

pub(crate) fn install(app: &mut App) {
    let Some((port, token)) = launch_args() else {
        return;
    };
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("live-link".into())
        .spawn(move || {
            // The editor was listening before it launched us, so the first try
            // normally succeeds. A few retries cover a slow socket stack, not a
            // missing editor: if it is gone there is nobody to talk to.
            let mut stream = None;
            for _ in 0..20 {
                match TcpStream::connect(("127.0.0.1", port)) {
                    Ok(s) => {
                        stream = Some(s);
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(250)),
                }
            }
            let Some(mut stream) = stream else {
                warn!("[live-link] could not reach the editor on port {port}");
                return;
            };
            let _ = stream.set_nodelay(true);
            let Ok(hello) = serde_json::to_string(&LinkMessage::Hello { token }) else { return };
            if writeln!(stream, "{hello}").is_err() {
                return;
            }
            let Ok(writer) = stream.try_clone() else { return };
            if tx.send(Incoming::Connected(writer)).is_err() {
                return;
            }
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<LinkMessage>(&line) {
                    Ok(message) => {
                        if tx.send(Incoming::Message(message)).is_err() {
                            return;
                        }
                    }
                    Err(e) => warn!("[live-link] unreadable message: {e}"),
                }
            }
            let _ = tx.send(Incoming::Closed);
        })
        .ok();
    app.insert_resource(Link {
        rx: Mutex::new(rx),
        writer: None,
        tracker: None,
        unknown: HashSet::new(),
        since_scan: 0.0,
        outbox: Arc::new(Mutex::new(Vec::new())),
        pad: VirtualPad::default(),
    })
    // After input processing, so an injected key press survives to `Update`
    // rather than being cleared as last frame's.
    .add_systems(PreUpdate, receive.after(InputSystems))
    // After the frame's gameplay, so what is reported is where things ended up.
    .add_systems(PostUpdate, report);
}

/// `--live-link <port> --live-link-token <token>`, both or neither.
fn launch_args() -> Option<(u16, String)> {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let port = value("--live-link")?.parse().ok()?;
    let token = value("--live-link-token")?;
    Some((port, token))
}

fn receive(world: &mut World) {
    let incoming: Vec<Incoming> = {
        let link = world.resource::<Link>();
        let Ok(rx) = link.rx.lock() else { return };
        rx.try_iter().collect()
    };
    let mut messages = Vec::new();
    for item in incoming {
        match item {
            Incoming::Connected(writer) => {
                let tracker = Tracker::baseline(world);
                let mut link = world.resource_mut::<Link>();
                link.writer = Some(writer);
                link.tracker = Some(tracker);
                info!("[live-link] connected to the editor; the two stay in step while the game runs");
            }
            Incoming::Message(message) => messages.push(message),
            Incoming::Closed => {
                let mut link = world.resource_mut::<Link>();
                link.writer = None;
                link.tracker = None;
                info!("[live-link] the editor closed the link");
            }
        }
    }

    let (mut tracker, mut unknown) = {
        let mut link = world.resource_mut::<Link>();
        (link.tracker.take(), std::mem::take(&mut link.unknown))
    };
    if let Some(tracker) = tracker.as_mut() {
        for message in apply(world, messages, tracker, &mut Quiet, &mut unknown) {
            match message {
                LinkMessage::Input(input) => handle_input(world, input),
                LinkMessage::ScreenshotRequest { req } => capture(world, req),
                _ => {}
            }
        }
    }
    {
        let mut link = world.resource_mut::<Link>();
        if link.writer.is_some() {
            link.tracker = tracker;
        }
        link.unknown = unknown;
    }
    flush_pad(world);
}

fn report(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let (mut tracker, due) = {
        let mut link = world.resource_mut::<Link>();
        if link.writer.is_none() {
            return;
        }
        link.since_scan += dt;
        let due = link.since_scan >= SCAN_INTERVAL;
        if due {
            link.since_scan = 0.0;
        }
        (link.tracker.take(), due)
    };
    let mut messages = match (&mut tracker, due) {
        (Some(t), true) => t.scan(world),
        _ => Vec::new(),
    };
    let mut link = world.resource_mut::<Link>();
    link.tracker = tracker;
    if let Ok(mut outbox) = link.outbox.lock() {
        messages.append(&mut outbox);
    }
    if messages.is_empty() {
        return;
    }
    let Some(writer) = link.writer.as_mut() else { return };
    for message in &messages {
        let Ok(line) = serde_json::to_string(message) else { continue };
        if writeln!(writer, "{line}").is_err() {
            link.writer = None;
            link.tracker = None;
            return;
        }
    }
}

fn handle_input(world: &mut World, input: GameInput) {
    match input {
        GameInput::Key { code, pressed } => {
            let Ok(key) = serde_json::from_value::<KeyCode>(serde_json::Value::String(code.clone())) else {
                warn!("[live-link] unknown key {code}");
                return;
            };
            if let Some(mut keys) = world.get_resource_mut::<ButtonInput<KeyCode>>() {
                if pressed {
                    keys.press(key);
                } else {
                    keys.release(key);
                }
            }
        }
        pad_input => world.resource_mut::<Link>().pad.pending.push(pad_input),
    }
}

/// Feed held gamepad input to the virtual pad, creating it on first use.
fn flush_pad(world: &mut World) {
    if world.resource::<Link>().pad.pending.is_empty() {
        return;
    }
    let Some(entity) = world.resource::<Link>().pad.entity else {
        let entity = world.spawn_empty().id();
        world.resource_mut::<Link>().pad.entity = Some(entity);
        world.write_message(GamepadConnectionEvent::new(
            entity,
            GamepadConnection::Connected {
                name: "Live link".into(),
                vendor_id: None,
                product_id: None,
            },
        ));
        return;
    };
    if world.get::<Gamepad>(entity).is_none() {
        return;
    }
    let pending = std::mem::take(&mut world.resource_mut::<Link>().pad.pending);
    for input in pending {
        match input {
            GameInput::GamepadButton { button, value } => {
                match serde_json::from_value::<GamepadButton>(serde_json::Value::String(button.clone())) {
                    Ok(b) => {
                        world.write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                            entity,
                            b,
                            value.clamp(0.0, 1.0),
                        )));
                    }
                    Err(_) => warn!("[live-link] unknown gamepad button {button}"),
                }
            }
            GameInput::GamepadAxis { axis, value } => {
                match serde_json::from_value::<GamepadAxis>(serde_json::Value::String(axis.clone())) {
                    Ok(a) => {
                        world.write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                            entity,
                            a,
                            value.clamp(-1.0, 1.0),
                        )));
                    }
                    Err(_) => warn!("[live-link] unknown gamepad axis {axis}"),
                }
            }
            GameInput::Key { .. } => {}
        }
    }
}

/// Capture the window; the reply goes out from the observer once the frame is
/// read back, which is a frame or two later.
fn capture(world: &mut World, req: u64) {
    let outbox = world.resource::<Link>().outbox.clone();
    world
        .spawn(Screenshot::primary_window())
        .observe(move |captured: On<ScreenshotCaptured>| {
            let png = encode_png(captured.image.clone());
            if let Ok(mut outbox) = outbox.lock() {
                outbox.push(LinkMessage::Screenshot { req, png });
            }
        });
}

fn encode_png(image: Image) -> Result<String, String> {
    let dynamic = image.try_into_dynamic().map_err(|e| e.to_string())?;
    let dynamic = if dynamic.width() > SCREENSHOT_WIDTH {
        let height = dynamic.height() * SCREENSHOT_WIDTH / dynamic.width();
        dynamic.resize(SCREENSHOT_WIDTH, height.max(1), image::imageops::FilterType::Triangle)
    } else {
        dynamic
    };
    let mut bytes = std::io::Cursor::new(Vec::new());
    dynamic
        .to_rgb8()
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(crate::b64::encode(bytes.get_ref()))
}
