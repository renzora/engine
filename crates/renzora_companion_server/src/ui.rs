//! The companion's window, in egui: the `--window` process.
//!
//! The companion starts this process when the editor's spanner asks for the
//! window and it ends when the window is closed, taking its GL context and
//! driver with it (see [`crate::window`] for why that has to be a process).
//! A reader thread takes in what the companion sends on stdin (see
//! [`crate::view`]): the [`View`] of its state, and the profile's packets,
//! which go into a [`Capture`] of the last few seconds that the Profiler,
//! Render and Debug screens draw from. A button writes a [`Command`] to
//! stdout. Log lines go to stderr, since stdout is the command pipe.
//!
//! The screens are one module each: [`profiler`], [`render_graph`],
//! [`debug`], [`jobs`], [`cache`] and [`games`].

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::capture::Capture;
use crate::games::ConnState;
use crate::log;
use crate::view::{self, Command, Received, ToWindow, View};
use crate::window::WINDOW_FRAMES;

mod cache;
mod debug;
mod games;
mod jobs;
mod profiler;
mod render_graph;

/// The screens on the rail.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Page {
    #[default]
    Profiler,
    Render,
    Debug,
    Jobs,
    Cache,
    Games,
}

impl Page {
    const ALL: [(Page, &'static str); 6] = [
        (Page::Profiler, "Profiler"),
        (Page::Render, "Render"),
        (Page::Debug, "Debug"),
        (Page::Jobs, "Jobs"),
        (Page::Cache, "Cache"),
        (Page::Games, "Games"),
    ];
}

pub(crate) const GOOD: Color32 = Color32::from_rgb(110, 190, 120);
pub(crate) const WARN: Color32 = Color32::from_rgb(230, 170, 70);
pub(crate) const BAD: Color32 = Color32::from_rgb(226, 92, 92);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(90, 140, 230);
pub(crate) const MUTED: Color32 = Color32::from_rgb(140, 145, 160);
pub(crate) const PANEL: Color32 = Color32::from_rgb(30, 31, 36);
pub(crate) const FAINT: Color32 = Color32::from_rgb(40, 42, 49);
pub(crate) const HOVER: Color32 = Color32::from_rgb(52, 55, 64);

/// Live views redraw at most this often while frames stream in.
const REDRAW: Duration = Duration::from_millis(33);

/// What the reader thread and the window share.
struct Latest {
    view: Mutex<View>,
    capture: Mutex<Capture>,
    /// Counts capture resets, so the screens drop a selection that belonged
    /// to the capture before.
    resets: AtomicU64,
    /// Whether arriving frames redraw the window ("Live view"). Off, data
    /// keeps arriving and is kept, but nothing redraws, so the window costs
    /// the machine almost nothing while you measure.
    live: AtomicBool,
    ctx: Mutex<Option<egui::Context>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run the window until it is closed, then exit.
pub(crate) fn main() -> ! {
    log::to_stderr();
    let latest = Arc::new(Latest {
        view: Mutex::new(View::default()),
        capture: Mutex::new(Capture::with_limit(WINDOW_FRAMES)),
        resets: AtomicU64::new(0),
        live: AtomicBool::new(true),
        ctx: Mutex::new(None),
    });
    {
        let latest = latest.clone();
        let spawned = std::thread::Builder::new().name("companion-window-in".into()).spawn(move || {
            let mut input = std::io::BufReader::new(std::io::stdin().lock());
            loop {
                match view::read(&mut input) {
                    Ok(Some(received)) => take(&latest, received),
                    Ok(None) => break,
                    Err(e) => {
                        log::warn(format!("the window lost its place in the companion's stream: {e}"));
                        break;
                    }
                }
            }
            // The companion went (the editor closed): a window for it would
            // show a state nothing is changing any more.
            std::process::exit(0);
        });
        if let Err(e) = spawned {
            log::error(format!("the window could not listen to the companion: {e}"));
            std::process::exit(1);
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Renzora Companion")
            .with_app_id("renzora-companion")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "renzora-companion",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            *lock(&latest.ctx) = Some(cc.egui_ctx.clone());
            Ok(Box::new(App::new(latest)))
        }),
    );
    if let Err(e) = result {
        log::warn(format!("could not open the window: {e}"));
        std::process::exit(1);
    }
    std::process::exit(0);
}

/// Fold one message from the companion into what the window draws.
fn take(latest: &Latest, received: Received) {
    let ctx = lock(&latest.ctx).clone();
    match received {
        Received::Line(ToWindow::View(view)) => {
            *lock(&latest.view) = *view;
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        }
        Received::Line(ToWindow::Focus) => {
            if let Some(ctx) = ctx {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        Received::Line(ToWindow::CaptureReset { source, from_file }) => {
            lock(&latest.capture).reset(&source, from_file.map(Into::into));
            latest.resets.fetch_add(1, Ordering::Relaxed);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        }
        Received::Packet(packet, overlapped) => {
            lock(&latest.capture).receive(packet, overlapped);
            if latest.live.load(Ordering::Relaxed) {
                if let Some(ctx) = ctx {
                    ctx.request_repaint_after(REDRAW);
                }
            }
        }
    }
}

/// Tell the companion something was pressed.
pub(crate) fn send(command: Command) {
    let Ok(line) = serde_json::to_string(&command) else { return };
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        // The companion is gone; the reader thread sees it too and exits.
        log::warn("the window could not reach the companion");
    }
}

struct App {
    latest: Arc<Latest>,
    page: Page,
    resets: u64,
    profiler: profiler::Profiler,
    render: render_graph::RenderGraph,
    debug: debug::Debug,
    games: games::Games,
}

impl App {
    fn new(latest: Arc<Latest>) -> Self {
        Self {
            latest,
            page: Page::default(),
            resets: 0,
            profiler: profiler::Profiler::default(),
            render: render_graph::RenderGraph::default(),
            debug: debug::Debug::default(),
            games: games::Games::default(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let view = lock(&self.latest.view).clone();
        let live = self.latest.live.load(Ordering::Relaxed);
        let resets = self.latest.resets.load(Ordering::Relaxed);
        if resets != self.resets {
            self.resets = resets;
            self.profiler.reset();
            self.render.reset();
        }

        egui::Panel::top("header").show(ui, |ui| header(ui, &view, live, &self.latest, &mut self.page));
        egui::Panel::bottom("footer").show(ui, |ui| footer(ui, &view, &self.latest));
        egui::Panel::left("rail").resizable(false).exact_size(96.0).show(ui, |ui| {
            ui.add_space(8.0);
            for (page, label) in Page::ALL {
                let selected = self.page == page;
                let button = egui::Button::selectable(selected, RichText::new(label).size(14.0))
                    .min_size(egui::vec2(ui.available_width(), 34.0));
                if ui.add(button).clicked() {
                    self.page = page;
                }
            }
        });

        // The Debug screen's probes are watched only while it shows; tell
        // the companion whenever that, or the connection, changes.
        let connected = view.profiling.state == ConnState::Connected;
        self.debug.sync_watch(self.page == Page::Debug, connected);

        egui::CentralPanel::default().show(ui, |ui| {
            // A copy of the capture is not taken: the screens read it under
            // the lock, which the reader thread only holds to add a packet.
            let capture = lock(&self.latest.capture);
            match self.page {
                Page::Profiler => self.profiler.ui(ui, &capture, live),
                Page::Render => self.render.ui(ui, &capture, &self.profiler, live),
                Page::Debug => self.debug.ui(ui, &capture),
                Page::Jobs => jobs::ui(ui, &view),
                Page::Cache => cache::ui(ui, &view.cache),
                Page::Games => {
                    if self.games.ui(ui, &view.profiling) {
                        self.page = Page::Profiler;
                    }
                }
            }
        });
        if !view.running.is_empty() {
            // A job's progress bar animates without any message to say so.
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn header(ui: &mut egui::Ui, view: &View, live: bool, latest: &Latest, page: &mut Page) {
    let profiling = &view.profiling;
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new("Renzora Companion").strong().size(15.0));
        ui.add_space(12.0);
        let from_file = lock(&latest.capture).from_file.clone();
        let (label, color) = match (&profiling.game, &from_file) {
            (Some(game), _) => (
                match &profiling.state {
                    ConnState::Connected => format!("{} · {}", game.name, game.place()),
                    ConnState::Connecting => format!("Connecting to {}...", game.name),
                    ConnState::Failed(why) => format!("{}: {why}", game.name),
                    ConnState::Idle => game.name.clone(),
                },
                Color32::WHITE,
            ),
            (None, Some(path)) => (
                format!("Capture: {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                Color32::WHITE,
            ),
            (None, None) => ("Pick a game to profile".to_string(), MUTED),
        };
        if ui.button(RichText::new(label).color(color)).clicked() {
            *page = Page::Games;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Save capture").clicked() {
                send(Command::SaveCapture);
            }
            let live_label = if live {
                RichText::new("Live view")
            } else {
                RichText::new("Record only (view paused)").color(WARN)
            };
            if ui.button(live_label).clicked() {
                latest.live.store(!live, Ordering::Relaxed);
            }
            let connected = profiling.state == ConnState::Connected;
            let record = if profiling.recording {
                RichText::new("Recording").color(BAD)
            } else {
                RichText::new("Record")
            };
            if ui.button(record).clicked() {
                if connected {
                    send(Command::Record { on: !profiling.recording });
                } else {
                    *page = Page::Games;
                }
            }
        });
    });
}

fn footer(ui: &mut egui::Ui, view: &View, latest: &Latest) {
    let mut parts = Vec::new();
    let frames = view.profiling.frames;
    if view.profiling.recording {
        parts.push(format!("Recording · {frames} frames kept"));
    } else if frames > 0 {
        parts.push(format!("{frames} frames"));
    }
    if frames > 0 {
        let gpu = lock(&latest.capture).frames.back().is_some_and(|f| !f.gpu.is_empty());
        parts.push(if gpu { "GPU timings on".into() } else { "No GPU timings from this game".into() });
    }
    let (running, waiting) = (view.running.len(), view.waiting_total);
    if running + waiting > 0 {
        parts.push(format!("Jobs: {running} running, {waiting} waiting"));
    }
    if let Some(notice) = &view.notice {
        parts.push(notice.clone());
    }
    ui.label(RichText::new(parts.join("   ·   ")).color(MUTED));
}

/// A titled box, the way each section of a page is drawn.
pub(crate) fn section(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).fill(PANEL).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong());
        ui.add_space(4.0);
        contents(ui);
    });
    ui.add_space(8.0);
}

/// A span or system name cut to its last `::` segment and without generics,
/// so `bevy_ui::layout::ui_layout_system` reads as `ui_layout_system`.
pub(crate) fn short(name: &str) -> &str {
    let trimmed = name.split('<').next().unwrap_or(name);
    trimmed.rsplit("::").next().unwrap_or(trimmed)
}

pub(crate) fn bytes(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.2} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{:.0} KB", n as f64 / 1024.0),
        n => format!("{n} B"),
    }
}

/// A duration in the unit that reads best: microseconds, milliseconds or
/// seconds.
pub(crate) fn fmt_ms(ms: f64) -> String {
    if ms.abs() < 1.0 {
        format!("{:.0} µs", ms * 1000.0)
    } else if ms.abs() < 100.0 {
        format!("{ms:.2} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_the_unit_that_fits() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(2048), "2 KB");
        assert_eq!(bytes(5 << 20), "5.0 MB");
        assert_eq!(bytes(3 << 30), "3.00 GB");
    }

    #[test]
    fn names_shorten_to_the_function() {
        assert_eq!(short("bevy_ui::layout::ui_layout_system"), "ui_layout_system");
        assert_eq!(short("my::thing<Foo>"), "thing");
        assert_eq!(short("main_opaque_pass_3d"), "main_opaque_pass_3d");
    }
}
