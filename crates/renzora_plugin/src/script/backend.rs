//! The side of the boundary a language plugin writes.
//!
//! [`Backend`] is deliberately close to the engine's own `ScriptBackend` trait:
//! same hooks, same responsibilities, same names. Porting an interpreter that
//! already lived in the engine should be a change to the `use` line and the
//! context accessors, not a redesign — the same constraint that governs the
//! rest of this crate, where a plugin's Bevy code is meant to be Bevy code.
//!
//! Everything below the trait is plumbing an implementor never touches:
//! decoding the call, caching the frame context, catching panics, encoding the
//! reply.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::vec::Vec;

use super::context::{decode_bindings, AssetProgress, Binding, EntityContext, FrameContext, HookArgs};
use super::reply::ScriptReply;
use super::value::{ActionValue, PropValue, ScriptValue, VarDef};
use super::wire::Reader;
use super::{
    BlobRef, ByteSink, ScriptBackendDesc, ScriptCall, ScriptHostCalls, ScriptOp, ScriptStatus,
};
use crate::sys::{Str256, StrRef};
use core::ffi::c_void;

/// Which hook is running, and its arguments.
///
/// The op code and the argument blob are folded into one enum here so an
/// implementor writes a single `match` instead of correlating two values —
/// `ScriptOp::OnReady` and `ScriptOp::OnUpdate` both carry no arguments, which
/// is exactly the pair a two-value design gets wrong.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hook<'a> {
    Ready,
    Update,
    Rpc {
        name: &'a str,
        from: u64,
        args: &'a [(String, ActionValue)],
    },
    Ui {
        name: &'a str,
        entity_bits: u64,
        args: &'a [(String, ActionValue)],
    },
    /// Draw surface size in pixels.
    Draw {
        width: f32,
        height: f32,
    },
    AnimationEvent {
        name: &'a str,
        entity_bits: u64,
    },
    Http {
        callback: &'a str,
        status: u16,
        body: &'a str,
    },
    PlayerEvent {
        id: u64,
        joined: bool,
    },
}

impl Hook<'_> {
    /// The function name scripts are expected to define for this hook.
    ///
    /// Provided so every language plugin agrees on the names rather than each
    /// inventing its own — a script moved from Lua to Wren should not have to
    /// be renamed.
    pub fn fn_name(&self) -> &'static str {
        match self {
            Self::Ready => "on_ready",
            Self::Update => "on_update",
            Self::Rpc { .. } => "on_rpc",
            Self::Ui { .. } => "on_ui",
            Self::Draw { .. } => "on_draw",
            Self::AnimationEvent { .. } => "on_animation_event",
            Self::Http { .. } => "on_http",
            Self::PlayerEvent { joined: true, .. } => "on_player_joined",
            Self::PlayerEvent { joined: false, .. } => "on_player_left",
        }
    }
}

/// The script being run.
#[derive(Clone, Copy)]
pub struct ScriptRef<'a> {
    /// Resolved path. A cache key — **do not open it**; see the module docs on
    /// [`super`] for why file I/O stays with the host.
    pub path: &'a str,
    /// Source text, already read by the host.
    pub source: &'a str,
    /// Changes when `source` does. Compare against a cached VM's copy and
    /// rebuild on mismatch; that is the whole of hot-reload support.
    pub version: u64,
    /// `Entity::to_bits()` of the entity this script is attached to.
    pub entity: u64,
    /// Current prop values, as the inspector last set them.
    pub vars: &'a [(String, ScriptValue)],
}

/// Reads back into the engine, valid only for the current call.
///
/// The lifetime is doing real work: behind these function pointers is a
/// `&World` the host holds for the duration of the hook and drops afterwards.
/// A backend that stashed one of these and used it next frame would be reading
/// freed memory, and the borrow is what stops that at compile time.
#[derive(Clone, Copy)]
pub struct HostCalls<'a> {
    raw: &'a ScriptHostCalls,
}

/// Collects a host call's answer into a plugin-owned `Vec`.
///
/// # Safety
/// `ctx` must be a live `*mut Vec<u8>` and `bytes`/`len` a valid slice.
unsafe extern "C" fn collect(ctx: *mut c_void, bytes: *const u8, len: usize) {
    let buf = &mut *(ctx as *mut Vec<u8>);
    buf.extend_from_slice(core::slice::from_raw_parts(bytes, len));
}

fn str_ref(s: &str) -> StrRef {
    StrRef {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

/// `None` becomes a null pointer, which is how the boundary spells "the
/// script's own entity".
fn opt_str_ref(s: Option<&str>) -> StrRef {
    match s {
        Some(s) => str_ref(s),
        None => StrRef {
            ptr: core::ptr::null(),
            len: 0,
        },
    }
}

impl<'a> HostCalls<'a> {
    fn ask(&self, call: impl FnOnce(&ByteSink)) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        let sink = ByteSink {
            ctx: &mut buf as *mut Vec<u8> as *mut c_void,
            write: collect,
        };
        call(&sink);
        buf
    }

    /// One reflected field. `entity` of `None` means the script's own entity.
    pub fn get(&self, entity: Option<&str>, component: &str, field: &str) -> Option<PropValue> {
        let buf = self.ask(|sink| unsafe {
            (self.raw.get)(
                self.raw.ctx,
                opt_str_ref(entity),
                str_ref(component),
                str_ref(field),
                sink,
            )
        });
        let mut r = Reader::new(&buf);
        match r.bool() {
            Ok(true) => PropValue::decode(&mut r).ok(),
            _ => None,
        }
    }

    /// Every field of one component.
    pub fn get_component(
        &self,
        entity: Option<&str>,
        component: &str,
    ) -> Option<Vec<(String, PropValue)>> {
        let buf = self.ask(|sink| unsafe {
            (self.raw.get_component)(
                self.raw.ctx,
                opt_str_ref(entity),
                str_ref(component),
                sink,
            )
        });
        let mut r = Reader::new(&buf);
        match r.bool() {
            Ok(true) => r
                .list(|r| Ok((r.string()?, PropValue::decode(r)?)))
                .ok(),
            _ => None,
        }
    }

    /// Type names of every reflected component on an entity.
    pub fn get_components(&self, entity: Option<&str>) -> Vec<String> {
        let buf = self.ask(|sink| unsafe {
            (self.raw.get_components)(self.raw.ctx, opt_str_ref(entity), sink)
        });
        Reader::new(&buf).list(|r| r.string()).unwrap_or_default()
    }

    /// Asset-load progress, for a script driving a loading screen.
    pub fn asset_progress(&self) -> Option<AssetProgress> {
        let buf = self.ask(|sink| unsafe { (self.raw.asset_progress)(self.raw.ctx, sink) });
        let mut r = Reader::new(&buf);
        match r.bool() {
            Ok(true) => AssetProgress::decode(&mut r).ok(),
            _ => None,
        }
    }

    /// Localization lookup. Returns the key itself when there is no translation,
    /// matching the engine's own `t()`.
    pub fn translate(&self, key: &str) -> String {
        let buf = self.ask(|sink| unsafe {
            (self.raw.translate)(self.raw.ctx, str_ref(key), sink)
        });
        Reader::new(&buf)
            .string()
            .unwrap_or_else(|_| key.to_string())
    }
}

/// Everything a hook can see.
pub struct Ctx<'a> {
    /// Shared by every script this frame — decoded once, not per entity.
    pub frame: &'a FrameContext,
    pub entity: &'a EntityContext,
    pub host: HostCalls<'a>,
}

/// A scripting language.
///
/// The associated consts rather than methods are what let the registration
/// descriptor be built without an instance, so a plugin can register before
/// it has paid for an interpreter it might never use.
pub trait Backend: Default + 'static {
    /// Shown in logs and the editor's language picker.
    const NAME: &'static str;
    /// Extensions claimed, without the dot — `&["lua", "blueprint", "bp"]`.
    const EXTENSIONS: &'static [&'static str];

    /// The engine's declared bindings changed.
    ///
    /// These are the functions domain crates declare rather than write —
    /// `apply_force`, `nav_set_destination`, `tr`. Build them into every VM
    /// created from now on. Called at least once before the first hook, and
    /// again if a plugin adds more later.
    fn set_bindings(&mut self, _bindings: &[Binding]) {}

    /// Parse the props a script declares, for the inspector.
    fn props(&mut self, _script: &ScriptRef) -> Vec<VarDef> {
        Vec::new()
    }

    /// Run one hook.
    ///
    /// Returning `Ok` with an empty reply is correct for a script that does not
    /// define the hook — reporting that as an error would light up the console
    /// for every script that only implements `on_update`.
    fn hook(
        &mut self,
        script: &ScriptRef,
        hook: Hook,
        ctx: &Ctx,
        reply: &mut ScriptReply,
    ) -> Result<(), String>;

    /// Evaluate an expression for the console REPL.
    fn eval(&mut self, _expr: &str) -> Result<String, String> {
        Err(format!("{} has no expression evaluator", Self::NAME))
    }

    /// Drop cached state for a `(path, entity)` pair whose entity went away.
    fn evict(&mut self, _path: &str, _entity: u64) {}
}

/// A backend plus the per-frame decode cache.
///
/// The cache is the reason this type exists rather than the backend being
/// stored bare. Splitting the context into frame and entity halves only pays
/// off if the frame half is *decoded* once too; without this, every entity
/// would re-parse the same key and action tables and the split would have moved
/// the cost rather than removed it.
pub struct BackendState<B> {
    backend: B,
    frame: FrameContext,
    frame_seq: u64,
    /// `false` until the first frame is decoded — frame 0 is a real sequence
    /// number, so it cannot double as "nothing cached yet".
    has_frame: bool,
}

impl<B: Default> Default for BackendState<B> {
    fn default() -> Self {
        Self {
            backend: B::default(),
            frame: FrameContext::default(),
            frame_seq: 0,
            has_frame: false,
        }
    }
}

impl<B> BackendState<B> {
    pub fn backend(&mut self) -> &mut B {
        &mut self.backend
    }
}

/// Decode a call, run it, encode the reply.
///
/// # Safety
/// `call` must point at a live [`ScriptCall`] from the host, with every blob
/// valid for the duration of this function.
pub unsafe fn dispatch<B: Backend>(
    state: &mut BackendState<B>,
    call: *const ScriptCall,
) -> ScriptStatus {
    if call.is_null() {
        return ScriptStatus::Error;
    }
    let call = &*call;

    // A panic must not unwind out of the `extern "C"` frame the host called us
    // through — that is an abort, and taking the whole editor down because one
    // script indexed a nil is not a proportionate response. Catch here and let
    // the host disable the offending script.
    let result = catch_unwind(AssertUnwindSafe(|| run(state, call)));
    match result {
        Ok(status) => status,
        Err(_) => {
            let reply = ScriptReply {
                error: Some(format!("{} panicked", B::NAME)),
                ..Default::default()
            };
            if let Some(sink) = call.out.as_ref() {
                reply.write_to(sink);
            }
            ScriptStatus::Panicked
        }
    }
}

unsafe fn run<B: Backend>(state: &mut BackendState<B>, call: &ScriptCall) -> ScriptStatus {
    let mut reply = ScriptReply::default();

    let status = match call.op {
        ScriptOp::Bindings => {
            let mut r = Reader::new(call.args.as_slice());
            match decode_bindings(&mut r) {
                Ok(b) => {
                    state.backend.set_bindings(&b);
                    ScriptStatus::Ok
                }
                Err(e) => {
                    reply.error = Some(format!("binding list would not decode: {e}"));
                    ScriptStatus::Error
                }
            }
        }
        ScriptOp::Evict => {
            state
                .backend
                .evict(str_of(call.path), call.entity);
            ScriptStatus::Ok
        }
        ScriptOp::Eval => {
            let expr = match HookArgs::decode(&mut Reader::new(call.args.as_slice())) {
                Ok(HookArgs::Eval { expr }) => expr,
                _ => String::new(),
            };
            match state.backend.eval(&expr) {
                Ok(text) => {
                    reply.text = Some(text);
                    ScriptStatus::Ok
                }
                Err(e) => {
                    reply.error = Some(e);
                    ScriptStatus::Error
                }
            }
        }
        ScriptOp::Props => {
            let vars = decode_vars(call.vars);
            let script = script_ref(call, &vars);
            reply.props = state.backend.props(&script);
            ScriptStatus::Ok
        }
        op if hook_of(op).is_some() => {
            // Only refresh when the host says the frame moved on. Every entity
            // in a frame is handed the same bytes and the same sequence number.
            if !state.has_frame || state.frame_seq != call.frame_seq {
                match FrameContext::decode(&mut Reader::new(call.frame.as_slice())) {
                    Ok(f) => {
                        state.frame = f;
                        state.frame_seq = call.frame_seq;
                        state.has_frame = true;
                    }
                    Err(e) => {
                        reply.error = Some(format!("frame context would not decode: {e}"));
                        write_and_return(call, &reply);
                        return ScriptStatus::Error;
                    }
                }
            }

            let entity = match EntityContext::decode(&mut Reader::new(call.entity_ctx.as_slice()))
            {
                Ok(e) => e,
                Err(e) => {
                    reply.error = Some(format!("entity context would not decode: {e}"));
                    write_and_return(call, &reply);
                    return ScriptStatus::Error;
                }
            };

            let args = HookArgs::decode(&mut Reader::new(call.args.as_slice()))
                .unwrap_or(HookArgs::None);
            let Some(hook) = hook_with_args(call.op, &args) else {
                write_and_return(call, &reply);
                return ScriptStatus::UnknownOp;
            };

            let Some(raw_host) = call.host.as_ref() else {
                reply.error = Some("host call table was null".into());
                write_and_return(call, &reply);
                return ScriptStatus::Error;
            };

            let vars = decode_vars(call.vars);
            let script = script_ref(call, &vars);
            let ctx = Ctx {
                frame: &state.frame,
                entity: &entity,
                host: HostCalls { raw: raw_host },
            };

            match state.backend.hook(&script, hook, &ctx, &mut reply) {
                Ok(()) => ScriptStatus::Ok,
                Err(e) => {
                    reply.error = Some(e);
                    ScriptStatus::Error
                }
            }
        }
        _ => ScriptStatus::UnknownOp,
    };

    write_and_return(call, &reply);
    status
}

unsafe fn write_and_return(call: &ScriptCall, reply: &ScriptReply) {
    if let Some(sink) = call.out.as_ref() {
        reply.write_to(sink);
    }
}

unsafe fn str_of(s: StrRef) -> &'static str {
    if s.ptr.is_null() || s.len == 0 {
        ""
    } else {
        s.as_str()
    }
}

unsafe fn script_ref<'a>(call: &'a ScriptCall, vars: &'a [(String, ScriptValue)]) -> ScriptRef<'a> {
    ScriptRef {
        path: str_of(call.path),
        source: str_of(call.source),
        version: call.version,
        entity: call.entity,
        vars,
    }
}

unsafe fn decode_vars(blob: BlobRef) -> Vec<(String, ScriptValue)> {
    Reader::new(blob.as_slice())
        .list(|r| Ok((r.string()?, ScriptValue::decode(r)?)))
        .unwrap_or_default()
}

/// Whether an op is a script hook at all, without needing its arguments.
fn hook_of(op: ScriptOp) -> Option<()> {
    matches!(
        op,
        ScriptOp::OnReady
            | ScriptOp::OnUpdate
            | ScriptOp::OnRpc
            | ScriptOp::OnUi
            | ScriptOp::OnDraw
            | ScriptOp::OnAnimationEvent
            | ScriptOp::OnHttp
            | ScriptOp::OnPlayerEvent
    )
    .then_some(())
}

fn hook_with_args<'a>(op: ScriptOp, args: &'a HookArgs) -> Option<Hook<'a>> {
    Some(match (op, args) {
        (ScriptOp::OnReady, _) => Hook::Ready,
        (ScriptOp::OnUpdate, _) => Hook::Update,
        (ScriptOp::OnRpc, HookArgs::Rpc { name, from, args }) => Hook::Rpc {
            name,
            from: *from,
            args,
        },
        (ScriptOp::OnUi, HookArgs::Ui { name, entity_bits, args }) => Hook::Ui {
            name,
            entity_bits: *entity_bits,
            args,
        },
        (ScriptOp::OnDraw, HookArgs::Draw { width, height }) => Hook::Draw {
            width: *width,
            height: *height,
        },
        (ScriptOp::OnAnimationEvent, HookArgs::AnimationEvent { name, entity_bits }) => {
            Hook::AnimationEvent {
                name,
                entity_bits: *entity_bits,
            }
        }
        (ScriptOp::OnHttp, HookArgs::Http { callback, status, body }) => Hook::Http {
            callback,
            status: *status,
            body,
        },
        (ScriptOp::OnPlayerEvent, HookArgs::PlayerEvent { id, joined }) => Hook::PlayerEvent {
            id: *id,
            joined: *joined,
        },
        // An op that is a hook but whose arguments did not match it. The two
        // came from the same host in the same call, so this means the payload
        // was corrupt rather than that a version drifted.
        _ => return None,
    })
}

/// Build the registration descriptor for a backend.
///
/// `extensions` must outlive the call to `add_script_backend`, which the host
/// satisfies by copying immediately — it stores owned strings, not the pointer.
pub fn desc_for<B: Backend>(extensions: &[Str256], entry: super::ScriptEntry) -> ScriptBackendDesc {
    ScriptBackendDesc {
        name: Str256::new_truncating(B::NAME),
        extensions: extensions.as_ptr(),
        extension_count: extensions.len(),
        entry,
    }
}

/// Emit a backend's entry point and its registration helper.
///
/// A macro rather than a generic function because the entry point must be a
/// plain `extern "C" fn` with no captured state, so it needs a `static` — and a
/// `static` cannot be generic over the backend type.
///
/// ```ignore
/// struct LuaBackend { /* … */ }
/// impl renzora_plugin::script::Backend for LuaBackend { /* … */ }
/// renzora_plugin::script_backend!(LuaBackend);
///
/// impl Plugin for LuaPlugin {
///     fn build(&self, app: &mut App) {
///         app.add_script_backend(script_backend::desc());
///     }
/// }
/// ```
#[macro_export]
macro_rules! script_backend {
    ($ty:ty) => {
        /// Generated by `renzora_plugin::script_backend!`.
        pub mod script_backend {
            #[allow(unused_imports)]
            use super::*;

            type Backend = $ty;

            static STATE: ::std::sync::Mutex<
                ::std::option::Option<$crate::script::BackendState<Backend>>,
            > = ::std::sync::Mutex::new(::std::option::Option::None);

            /// # Safety
            /// Called only by the host, with a live `ScriptCall`.
            unsafe extern "C" fn entry(
                call: *const $crate::script::ScriptCall,
            ) -> $crate::script::ScriptStatus {
                // Recover from poisoning rather than refusing. The lock is
                // poisoned by a panic inside a hook, which the dispatcher
                // already caught and reported; treating that as fatal would
                // disable scripting for the rest of the session because one
                // script had a bad frame.
                let mut guard = match STATE.lock() {
                    ::std::result::Result::Ok(g) => g,
                    ::std::result::Result::Err(p) => p.into_inner(),
                };
                let state = guard.get_or_insert_with(::std::default::Default::default);
                $crate::script::dispatch(state, call)
            }

            fn extensions() -> ::std::vec::Vec<$crate::sys::Str256> {
                <Backend as $crate::script::Backend>::EXTENSIONS
                    .iter()
                    .map(|e| $crate::sys::Str256::new_truncating(e))
                    .collect()
            }

            /// The descriptor to hand to `App::add_script_backend`.
            pub fn desc() -> ($crate::script::ScriptBackendDesc, ::std::vec::Vec<$crate::sys::Str256>) {
                let exts = extensions();
                let d = $crate::script::desc_for::<Backend>(&exts, entry);
                (d, exts)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_names_match_the_engines_conventions() {
        assert_eq!(Hook::Ready.fn_name(), "on_ready");
        assert_eq!(Hook::Update.fn_name(), "on_update");
        assert_eq!(
            Hook::Draw {
                width: 0.0,
                height: 0.0
            }
            .fn_name(),
            "on_draw"
        );
        assert_eq!(
            Hook::PlayerEvent { id: 1, joined: true }.fn_name(),
            "on_player_joined"
        );
        assert_eq!(
            Hook::PlayerEvent {
                id: 1,
                joined: false
            }
            .fn_name(),
            "on_player_left"
        );
    }

    #[test]
    fn hook_ops_pair_with_their_arguments() {
        assert_eq!(
            hook_with_args(ScriptOp::OnUpdate, &HookArgs::None),
            Some(Hook::Update)
        );
        assert_eq!(
            hook_with_args(
                ScriptOp::OnDraw,
                &HookArgs::Draw {
                    width: 8.0,
                    height: 6.0
                }
            ),
            Some(Hook::Draw {
                width: 8.0,
                height: 6.0
            })
        );
    }

    #[test]
    fn a_hook_op_with_the_wrong_arguments_is_refused() {
        // Corrupt payload rather than a version drift — both came from the same
        // call — so there is nothing sensible to run.
        assert_eq!(hook_with_args(ScriptOp::OnHttp, &HookArgs::None), None);
    }

    #[test]
    fn non_hook_ops_are_not_treated_as_hooks() {
        assert!(hook_of(ScriptOp::Bindings).is_none());
        assert!(hook_of(ScriptOp::Props).is_none());
        assert!(hook_of(ScriptOp::Eval).is_none());
        assert!(hook_of(ScriptOp::Evict).is_none());
        assert!(hook_of(ScriptOp::OnUpdate).is_some());
        assert!(hook_of(ScriptOp(999)).is_none());
    }
}
