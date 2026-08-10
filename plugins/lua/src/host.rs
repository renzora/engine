//! Reaching back into the engine from inside a Lua function.
//!
//! Most of the scripting API is one-way — a script asks for something and the
//! engine's command queue does it after the hook returns. Reads cannot work
//! that way: `get("Health.current")` has to answer before the next Lua
//! statement runs.
//!
//! The call table that answers those is valid only for the duration of one
//! hook. But the Lua functions that need it were registered when the VM was
//! *built*, possibly hundreds of frames earlier, and `mlua::create_function`
//! takes a `'static` callback with nowhere to thread a borrow through. So the
//! table goes in a thread-local for the length of the call and comes straight
//! back out.
//!
//! [`enter`] returns a guard that clears it on drop, including when a Lua error
//! unwinds through. Leaving a stale pointer behind would be the bad kind of
//! bug: the next frame's `get` would read a `&World` the engine had already
//! dropped, and it would usually appear to work.
//!
//! The function names below deliberately match the engine's old
//! `get_handler::call_*`, so the ~15 call sites in `interp.rs` needed a path
//! change and nothing else.

use std::cell::Cell;

use renzora_plugin::script::{AssetProgress, HostCalls, PropValue, SceneLoad, ScriptHostCalls};

thread_local! {
    static HOST: Cell<*const ScriptHostCalls> = const { Cell::new(std::ptr::null()) };
}

/// Clears the stashed table on drop.
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        HOST.with(|h| h.set(std::ptr::null()));
    }
}

/// Publish the call's host table for the duration of the returned guard.
pub fn enter(calls: HostCalls<'_>) -> Guard {
    HOST.with(|h| h.set(calls.raw() as *const ScriptHostCalls));
    Guard
}

/// Run `f` with the current call's host table, or return `None` outside a call.
///
/// Outside a call is reachable: a script can define a `props()` that calls
/// `get`, and props are parsed with no world to read. Returning `None` there
/// gives the script a `nil` rather than a crash.
fn with<R>(f: impl FnOnce(HostCalls) -> R) -> Option<R> {
    let ptr = HOST.with(|h| h.get());
    if ptr.is_null() {
        return None;
    }
    // SAFETY: non-null only between `enter` and its guard's drop, during which
    // the host holds the table alive for the call.
    let raw = unsafe { &*ptr };
    Some(f(HostCalls::new(raw)))
}

pub fn call_get(entity: Option<&str>, component: &str, field: &str) -> Option<PropValue> {
    with(|h| h.get(entity, component, field)).flatten()
}

pub fn call_get_component(
    entity: Option<&str>,
    component: &str,
) -> Option<Vec<(String, PropValue)>> {
    with(|h| h.get_component(entity, component)).flatten()
}

pub fn call_get_components(entity: Option<&str>) -> Vec<String> {
    with(|h| h.get_components(entity)).unwrap_or_default()
}

pub fn call_asset_progress() -> Option<AssetProgress> {
    with(|h| h.asset_progress()).flatten()
}

pub fn call_scene_load_state() -> Option<SceneLoad> {
    with(|h| h.scene_load_state()).flatten()
}

/// Localization lookup. Falls back to the key itself, matching the engine's
/// `t()`, so a `tr(...)` outside a call renders the key rather than an empty
/// string.
pub fn translate(key: &str) -> String {
    with(|h| h.translate(key)).unwrap_or_else(|| key.to_string())
}
