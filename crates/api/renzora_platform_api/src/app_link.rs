//! `renzora://` links: how a web page opens something in the editor.
//!
//! The website's download buttons try `renzora://marketplace/<slug>` first, so
//! someone who has the engine lands on the item in the editor's own
//! Marketplace, where the install picks the right build for them, instead of
//! with a zip in their downloads folder.
//!
//! Three pieces, all here because `main` runs the first two before any `App`
//! exists and the editor crates read the third:
//!
//! * **Registration.** Every editor launch tells the OS that this executable
//!   opens `renzora://` (per user, so no administrator rights). The last
//!   editor started wins, which is the one a person would expect after
//!   switching between a nightly and a release.
//! * **Forwarding.** The OS answers a link by starting a *new* process. If an
//!   editor is already running, that process hands the link to it over a
//!   loopback socket and exits, so a click never opens a second editor. The
//!   running one advertises its port in a file in the user's local data
//!   directory; a stale file (the editor crashed) fails to connect and the new
//!   process simply becomes the editor itself.
//! * **The queue.** Links received either way wait in [`take`] until whoever
//!   handles that kind of link (the marketplace, for an item) picks them up.
//!
//! A link can only *show* something. Anything a local process or a web page
//! can make the editor do through here, the person still has to confirm by
//! pressing Install, so the parser accepts exactly the forms below and drops
//! the rest.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// The URL scheme, without the `://`.
pub const SCHEME: &str = "renzora";

/// A link the editor understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppLink {
    /// `renzora://marketplace/<slug>`: show this listing in the Marketplace.
    MarketplaceItem { slug: String },
}

impl AppLink {
    /// Read a `renzora://` URL. Anything but a known form is `None`.
    ///
    /// A slug is what the website puts in its own URLs: lowercase letters,
    /// digits, `-` and `_`. It is checked here because it goes straight into
    /// an API path.
    pub fn parse(url: &str) -> Option<Self> {
        let rest = url.trim().strip_prefix(SCHEME)?.strip_prefix(':')?;
        // Browsers and shells hand the URL over as `renzora://...`, but a few
        // launchers drop the slashes or add one at the end.
        let rest = rest.trim_start_matches('/').trim_end_matches('/');
        let rest = rest.split(['?', '#']).next().unwrap_or_default();
        let mut parts = rest.split('/');
        match (parts.next(), parts.next().map(percent_decode), parts.next()) {
            (Some("marketplace"), Some(Some(slug)), None) if valid_slug(&slug) => {
                Some(Self::MarketplaceItem { slug })
            }
            _ => None,
        }
    }

    /// The URL form, for logs and for forwarding.
    pub fn url(&self) -> String {
        match self {
            Self::MarketplaceItem { slug } => {
                format!("{SCHEME}://marketplace/{}", percent_encode(slug))
            }
        }
    }
}

/// What the website's slugifier produces: lowercased letters and digits from
/// any script, joined by `-`. Nothing that could leave its path segment.
fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 200
        && slug
            .chars()
            .all(|c| (c.is_alphanumeric() && !c.is_uppercase()) || c == '-' || c == '_')
}

/// `%XX` escapes to bytes, which must then be UTF-8. `None` for a malformed
/// escape.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Escape everything but unreserved ASCII, for putting a slug back in a URL
/// or an API path.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

static QUEUE: Mutex<Vec<AppLink>> = Mutex::new(Vec::new());
static PENDING: AtomicBool = AtomicBool::new(false);
static FOCUS: AtomicBool = AtomicBool::new(false);

/// Queue a link for the editor to act on. A link that does not parse is
/// dropped with a log line.
pub fn push(url: &str) {
    let Some(link) = AppLink::parse(url) else {
        bevy::log::warn!("[link] ignoring an unrecognised link: {url}");
        return;
    };
    bevy::log::info!("[link] received {}", link.url());
    if let Ok(mut queue) = QUEUE.lock() {
        queue.push(link);
        PENDING.store(true, Ordering::Release);
    }
}

/// Whether a link is waiting. Cheap enough for a run condition.
pub fn pending() -> bool {
    PENDING.load(Ordering::Acquire)
}

/// Every waiting link, oldest first.
pub fn take() -> Vec<AppLink> {
    let Ok(mut queue) = QUEUE.lock() else {
        return Vec::new();
    };
    PENDING.store(false, Ordering::Release);
    std::mem::take(&mut *queue)
}

/// Put links back that could not be handled yet (the marketplace waits for a
/// project to be open before it shows a non-plugin item).
pub fn requeue(links: Vec<AppLink>) {
    if links.is_empty() {
        return;
    }
    if let Ok(mut queue) = QUEUE.lock() {
        let newer = std::mem::take(&mut *queue);
        *queue = links;
        queue.extend(newer);
        PENDING.store(true, Ordering::Release);
    }
}

/// Whether a link arrived from another process since the last call: the
/// editor's window should come to the front. A link on the editor's own
/// command line does not ask, since that window is about to open anyway.
pub fn take_focus_request() -> bool {
    FOCUS.swap(false, Ordering::AcqRel)
}

/// The `renzora://` URL on this process's command line, if it was started to
/// open one.
pub fn from_args() -> Option<String> {
    std::env::args().skip(1).find(|a| is_link(a))
}

/// Whether a command-line argument is a link. A restart drops these, so that
/// relaunching the editor does not open the same item again.
pub fn is_link(arg: &str) -> bool {
    arg.get(..SCHEME.len()).is_some_and(|s| s.eq_ignore_ascii_case(SCHEME))
        && arg[SCHEME.len()..].starts_with(':')
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{forward, listen, register_scheme};

#[cfg(target_arch = "wasm32")]
pub fn forward(_url: &str) -> bool {
    false
}
#[cfg(target_arch = "wasm32")]
pub fn listen() {}
#[cfg(target_arch = "wasm32")]
pub fn register_scheme() {}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    /// The reply that says the other end is an editor and took the link. A
    /// port file can outlive its editor and the port be reused by something
    /// else entirely, so a connection alone proves nothing.
    const ACK: &str = "renzora-link-ok";

    /// Where the running editor writes its port. Per user, and shared by every
    /// install, so a link reaches whichever editor is open.
    fn port_file() -> Option<PathBuf> {
        #[cfg(windows)]
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(target_os = "macos")]
        let base = std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"));
        #[cfg(all(unix, not(target_os = "macos")))]
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")));
        Some(base?.join("Renzora").join("editor-link.port"))
    }

    /// Hand `url` to an editor that is already running. `true` means it took
    /// it and this process has nothing left to do.
    pub fn forward(url: &str) -> bool {
        let Some(port) = port_file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|s| s.trim().parse::<u16>().ok())
        else {
            return false;
        };
        // This process was started by the browser in answer to a click, so
        // Windows lets it bring a window to the front. The running editor was
        // not, and without this its request only flashes the taskbar button.
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(
                windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY,
            );
        }
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        if writeln!(stream, "{}", url.trim()).is_err() {
            return false;
        }
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply).is_ok() && reply.trim() == ACK
    }

    /// Accept links from later launches, for the rest of this process.
    pub fn listen() {
        let Some(file) = port_file() else {
            return;
        };
        let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
            Ok(l) => l,
            Err(e) => {
                bevy::log::warn!("[link] could not listen for renzora:// links: {e}");
                return;
            }
        };
        let Ok(port) = listener.local_addr().map(|a| a.port()) else {
            return;
        };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&file, port.to_string()) {
            bevy::log::warn!("[link] could not write {}: {e}", file.display());
            return;
        }
        let spawned = std::thread::Builder::new().name("renzora-link".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream);
            }
        });
        if let Err(e) = spawned {
            bevy::log::warn!("[link] could not start the link listener: {e}");
        }
    }

    fn serve(mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut line = String::new();
        // One short line. `take` bounds what a stray client can make us buffer.
        let Ok(reader) = stream.try_clone() else {
            return;
        };
        if BufReader::new(std::io::Read::take(reader, 1024)).read_line(&mut line).is_err() {
            return;
        }
        if super::AppLink::parse(&line).is_none() {
            return;
        }
        super::push(&line);
        super::FOCUS.store(true, Ordering::Release);
        let _ = writeln!(stream, "{ACK}");
    }

    /// Make this executable the one that opens `renzora://`, on a background
    /// thread: it touches the registry or the desktop database and nothing at
    /// startup should wait for that.
    pub fn register_scheme() {
        let _ = std::thread::Builder::new().name("renzora-link-register".into()).spawn(|| {
            let exe = std::env::var_os("APPIMAGE")
                .map(PathBuf::from)
                .or_else(|| std::env::current_exe().ok());
            let Some(exe) = exe else {
                return;
            };
            match platform::register(&exe) {
                Ok(true) => bevy::log::info!("[link] renzora:// now opens {}", exe.display()),
                Ok(false) => {}
                Err(e) => bevy::log::warn!("[link] could not register renzora:// links: {e}"),
            }
        });
    }

    /// Windows: `HKCU\Software\Classes\renzora`, the per-user half of
    /// `HKEY_CLASSES_ROOT`. Written only when the command differs, so an
    /// ordinary launch reads one value and writes nothing.
    #[cfg(windows)]
    mod platform {
        use std::path::Path;
        use windows_sys::Win32::System::Registry::{
            RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
        };

        const KEY: &str = r"Software\Classes\renzora";

        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        }

        fn read(sub: &str) -> Option<String> {
            let sub = wide(sub);
            let mut buf = vec![0u16; 2048];
            let mut size = (buf.len() * 2) as u32;
            let status = unsafe {
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    sub.as_ptr(),
                    std::ptr::null(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            if status != 0 {
                return None;
            }
            let len = (size as usize / 2).saturating_sub(1);
            Some(String::from_utf16_lossy(&buf[..len.min(buf.len())]))
        }

        fn write(sub: &str, name: Option<&str>, value: &str) -> Result<(), String> {
            let sub_w = wide(sub);
            let name_w = name.map(wide);
            let value_w = wide(value);
            let status = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    sub_w.as_ptr(),
                    name_w.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()),
                    REG_SZ,
                    value_w.as_ptr().cast(),
                    (value_w.len() * 2) as u32,
                )
            };
            if status == 0 {
                Ok(())
            } else {
                Err(format!("writing HKCU\\{sub} failed with error {status}"))
            }
        }

        pub(super) fn register(exe: &Path) -> Result<bool, String> {
            let exe = exe.display().to_string();
            let command = format!("\"{exe}\" \"%1\"");
            let command_key = format!(r"{KEY}\shell\open\command");
            if read(&command_key).as_deref() == Some(command.as_str()) {
                return Ok(false);
            }
            write(KEY, None, "URL:Renzora")?;
            write(KEY, Some("URL Protocol"), "")?;
            write(&format!(r"{KEY}\DefaultIcon"), None, &format!("\"{exe}\",0"))?;
            write(&command_key, None, &command)?;
            Ok(true)
        }
    }

    /// Linux: a hidden desktop entry claiming `x-scheme-handler/renzora`, made
    /// the default through `xdg-mime`. Rewritten only when it changes.
    #[cfg(all(unix, not(target_os = "macos")))]
    mod platform {
        use std::path::{Path, PathBuf};

        const ENTRY: &str = "renzora-link.desktop";

        pub(super) fn register(exe: &Path) -> Result<bool, String> {
            let data = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
                .ok_or("no home directory")?;
            let dir = data.join("applications");
            let file = dir.join(ENTRY);
            // The desktop entry spec's quoting: the path in double quotes, with
            // `"`, `` ` ``, `$` and `\` escaped inside them.
            let mut quoted = String::new();
            for c in exe.display().to_string().chars() {
                if matches!(c, '"' | '`' | '$' | '\\') {
                    quoted.push('\\');
                }
                quoted.push(c);
            }
            let contents = format!(
                "[Desktop Entry]\nType=Application\nName=Renzora\nExec=\"{quoted}\" %u\n\
                 NoDisplay=true\nTerminal=false\nMimeType=x-scheme-handler/renzora;\n"
            );
            if std::fs::read_to_string(&file).ok().as_deref() == Some(contents.as_str()) {
                return Ok(false);
            }
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(&file, contents).map_err(|e| e.to_string())?;
            let status = std::process::Command::new("xdg-mime")
                .args(["default", ENTRY, "x-scheme-handler/renzora"])
                .status()
                .map_err(|e| format!("xdg-mime: {e}"))?;
            if !status.success() {
                return Err(format!("xdg-mime exited with {status}"));
            }
            Ok(true)
        }
    }

    /// macOS hands a URL to an app as an Apple Event rather than on its command
    /// line, and only to a `.app` whose `Info.plist` declares the scheme. The
    /// event is not something winit passes on, so links do not reach the editor
    /// there yet and the website falls back to its download.
    #[cfg(target_os = "macos")]
    mod platform {
        pub(super) fn register(_exe: &std::path::Path) -> Result<bool, String> {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_marketplace_item() {
        let want = Some(AppLink::MarketplaceItem { slug: "lua-scripting".into() });
        assert_eq!(AppLink::parse("renzora://marketplace/lua-scripting"), want);
        assert_eq!(AppLink::parse("renzora://marketplace/lua-scripting/"), want);
        assert_eq!(AppLink::parse("renzora:marketplace/lua-scripting"), want);
        assert_eq!(AppLink::parse("renzora://marketplace/lua-scripting?from=web"), want);
        assert_eq!(AppLink::parse("  renzora://marketplace/lua-scripting\n"), want);
    }

    #[test]
    fn rejects_anything_else() {
        for url in [
            "renzora://",
            "renzora://marketplace",
            "renzora://marketplace/",
            "renzora://marketplace/a/b",
            "renzora://marketplace/../etc",
            "renzora://marketplace/Has%20Space",
            "renzora://marketplace/a%2Fb",
            "renzora://marketplace/bad%zz",
            "renzora://marketplace/UPPER",
            "renzora://settings/x",
            "https://renzora.com/marketplace/x",
            "renzoraa://marketplace/x",
        ] {
            assert_eq!(AppLink::parse(url), None, "{url}");
        }
    }

    #[test]
    fn spots_link_arguments() {
        assert!(is_link("renzora://marketplace/x"));
        assert!(is_link("RENZORA://marketplace/x"));
        assert!(!is_link("renzora"));
        assert!(!is_link("--project"));
        assert!(!is_link("renzora_editor.dll"));
    }

    #[test]
    fn decodes_non_ascii_slugs() {
        assert_eq!(
            AppLink::parse("renzora://marketplace/caf%C3%A9-pack-1a2b"),
            Some(AppLink::MarketplaceItem { slug: "café-pack-1a2b".into() })
        );
    }

    #[test]
    fn round_trips_through_url() {
        for slug in ["water", "café-pack-1a2b"] {
            let link = AppLink::MarketplaceItem { slug: slug.into() };
            assert_eq!(AppLink::parse(&link.url()), Some(link));
        }
    }
}
