//! A static file server for `cargo renzora wasm`, so the web build opens in a
//! browser with one command.
//!
//! A browser will not load a module script from `file://`, so the staged
//! bundles have to come over HTTP. Telling people to run
//! `python -m http.server` assumed a Python install and a step nobody would
//! guess, and xtask carries no dependencies (see its `Cargo.toml`), so this is
//! the smallest server that does the job on `std` alone: GET and HEAD of files
//! under one directory, on the loopback address only.
//!
//! What it must get right, and why:
//!
//! - **`application/wasm` for `.wasm`.** `WebAssembly.instantiateStreaming`
//!   refuses any other type, and the page's loader uses it.
//! - **`Cache-Control: no-store`.** The whole point is rebuild-and-reload; a
//!   browser that kept yesterday's 150 MB module would test the old build,
//!   which is exactly the trap renzora.com fell into with Cloudflare.
//! - **No path outside the directory.** Only plain path segments are accepted,
//!   so `..` and absolute paths are refused rather than resolved.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

/// The first port tried, then the next few if it is taken (another server, or a
/// second `cargo renzora wasm` still running).
const FIRST_PORT: u16 = 8000;
const PORTS_TRIED: u16 = 20;

/// Serve `root` on localhost and open `page` in the default browser. Runs until
/// the process is stopped with Ctrl+C.
pub fn serve(root: &Path, page: &str) -> ExitCode {
    let Some((listener, port)) = bind() else {
        eprintln!(
            "[xtask] could not listen on localhost ports {FIRST_PORT}-{}",
            FIRST_PORT + PORTS_TRIED - 1
        );
        return ExitCode::from(1);
    };
    let url = format!("http://localhost:{port}/{page}");
    println!("[xtask] serving {} at {url}", root.display());
    println!("[xtask] press Ctrl+C to stop");
    open_browser(&url);

    let root = root.to_path_buf();
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let root = root.clone();
        // One thread per connection: the browser opens several at once (the
        // page, its script, two large modules), and one slow 150 MB transfer
        // must not hold the rest of the page up.
        std::thread::spawn(move || {
            // A dropped connection (a reload mid-download) is not worth a
            // message: the browser simply asks again.
            let _ = handle(stream, &root);
        });
    }
    ExitCode::SUCCESS
}

fn bind() -> Option<(TcpListener, u16)> {
    (FIRST_PORT..FIRST_PORT + PORTS_TRIED).find_map(|port| {
        TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .ok()
            .map(|l| (l, port))
    })
}

fn handle(stream: TcpStream, root: &Path) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    // Drain the headers; nothing in them changes the answer.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }

    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let mut out = stream;

    if method != "GET" && method != "HEAD" {
        return respond(&mut out, "405 Method Not Allowed", "text/plain", b"method not allowed");
    }
    let Some(path) = resolve(root, target) else {
        return respond(&mut out, "404 Not Found", "text/plain", b"not found");
    };
    let Ok(mut file) = File::open(&path) else {
        return respond(&mut out, "404 Not Found", "text/plain", b"not found");
    };
    let len = file.metadata()?.len();
    write!(
        out,
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {len}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        content_type(&path)
    )?;
    if method == "GET" {
        io::copy(&mut file, &mut out)?;
    }
    out.flush()
}

fn respond(out: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )?;
    out.write_all(body)?;
    out.flush()
}

/// Map a request target to a file under `root`, or `None` for anything that
/// would leave it. The query string is dropped, `/` is the directory's
/// `index.html` if there is one, and percent-escapes are not decoded: every
/// file this serves has a plain ASCII name.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let relative = Path::new(path.trim_start_matches('/'));
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let full = root.join(relative);
    if full.is_dir() {
        Some(full.join("index.html"))
    } else {
        Some(full)
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        "ts" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Best effort: a machine with no default browser, or a headless one, still
/// has the URL printed above to open by hand.
fn open_browser(url: &str) {
    let opened = if cfg!(windows) {
        // `start`'s first quoted argument is the window title, hence the "".
        Command::new("cmd").args(["/C", "start", "", url]).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    };
    if !opened.is_ok_and(|s| s.success()) {
        println!("[xtask] open {url} in a browser");
    }
}
