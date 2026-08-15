//! The wasm half: `showDirectoryPicker()` and what comes back from it.

use std::cell::RefCell;

use bevy::log::{error, info, warn};
use wasm_bindgen::prelude::*;

// The picker is declared as inline JS rather than reached through `web-sys`,
// for two reasons. `web-sys` puts the File System Access API behind
// `--cfg=web_sys_unstable_apis`, which would mean a RUSTFLAGS entry that every
// build of the workspace has to carry; and `handle.entries()` is an async
// iterator, which is considerably less painful to consume in three lines of JS
// than through `js_sys` by hand.
//
// Returning a plain object keeps the Rust side to field reads.
#[wasm_bindgen(inline_js = r#"
export async function __rz_pick_directory() {
  // `readwrite` up front: asking for read now and write at save time would
  // mean a second permission prompt, at the least convenient moment.
  const handle = await window.showDirectoryPicker({ mode: 'readwrite' });
  const files = [];
  const dirs = [];
  for await (const [name, entry] of handle.entries()) {
    (entry.kind === 'directory' ? dirs : files).push(name);
  }
  return { name: handle.name, handle, files, dirs };
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn __rz_pick_directory() -> Result<JsValue, JsValue>;
}

thread_local! {
    /// The directory the user last picked.
    ///
    /// A `thread_local` rather than a Bevy resource because a
    /// `FileSystemDirectoryHandle` is a `JsValue`, which is neither `Send` nor
    /// `Sync` and so cannot live in the `World`. wasm is single-threaded, so
    /// this is reachable from anywhere the engine actually runs.
    static HANDLE: RefCell<Option<JsValue>> = const { RefCell::new(None) };
}

/// Run `f` against the picked directory handle, if there is one.
pub fn with_handle<R>(f: impl FnOnce(&JsValue) -> R) -> Option<R> {
    HANDLE.with(|h| h.borrow().as_ref().map(f))
}

/// Open the browser's directory picker.
///
/// Returns immediately — the picker is asynchronous and the user may take as
/// long as they like over it, so this spawns and lets the result arrive later.
/// **Must be called from a click**; browsers refuse a picker that no gesture
/// asked for, and the refusal looks exactly like the user pressing cancel.
pub fn pick_directory() {
    wasm_bindgen_futures::spawn_local(async {
        let picked = match __rz_pick_directory().await {
            Ok(v) => v,
            Err(e) => {
                // Cancelling the dialog rejects with AbortError, which is a
                // perfectly normal thing for a user to do — say so quietly
                // rather than reporting it as a failure.
                if is_abort(&e) {
                    info!("[webfs] directory pick cancelled");
                } else {
                    error!("[webfs] directory pick failed: {}", describe(&e));
                }
                return;
            }
        };

        let name = string_field(&picked, "name").unwrap_or_else(|| "<unnamed>".into());
        let files = string_array(&picked, "files");
        let dirs = string_array(&picked, "dirs");

        if let Some(handle) = field(&picked, "handle") {
            HANDLE.with(|h| *h.borrow_mut() = Some(handle));
        } else {
            warn!("[webfs] picker returned no handle — reads will not work");
        }

        info!(
            "[webfs] picked '{name}': {} file(s), {} director(y/ies)",
            files.len(),
            dirs.len()
        );
        for d in &dirs {
            info!("[webfs]   dir  {d}/");
        }
        for f in &files {
            info!("[webfs]   file {f}");
        }
        // The tell that this is a Renzora project, and the next thing to read.
        if files.iter().any(|f| f == "project.toml") {
            info!("[webfs] project.toml present — this looks like a Renzora project");
        } else {
            warn!("[webfs] no project.toml here — not a Renzora project folder?");
        }
    });
}

fn field(obj: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

fn string_field(obj: &JsValue, key: &str) -> Option<String> {
    field(obj, key).and_then(|v| v.as_string())
}

fn string_array(obj: &JsValue, key: &str) -> Vec<String> {
    let Some(v) = field(obj, key) else {
        return Vec::new();
    };
    js_sys::Array::from(&v).iter().filter_map(|e| e.as_string()).collect()
}

/// Did the user simply cancel the dialog?
fn is_abort(err: &JsValue) -> bool {
    string_field(err, "name").as_deref() == Some("AbortError")
}

/// A readable message out of a `JsValue` error, which may be an `Error`, a
/// string, or something with no useful representation at all.
fn describe(err: &JsValue) -> String {
    string_field(err, "message")
        .or_else(|| err.as_string())
        .unwrap_or_else(|| format!("{err:?}"))
}
