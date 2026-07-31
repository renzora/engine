//! Animation for standalone plugins.
//!
//! ```ignore
//! use renzora_plugin::prelude::*;
//! use renzora_plugin::anim::{AnimCommands, AnimState};
//!
//! fn drive(q: Query<(Entity, &Locomotion, &AnimState)>, mut cmds: Commands) {
//!     for (entity, loco, anim) in &q {
//!         let want = if loco.speed > 4.0 { "run" } else { "idle" };
//!         if !anim.is_clip(want) {
//!             cmds.entity(entity).crossfade_animation(want, 0.2);
//!         }
//!     }
//! }
//! ```
//!
//! ## Why this is a module and not part of the ABI
//!
//! [`sys`](crate::sys) is the frozen mechanism — commands, queries, the
//! interface table — and it does not know that animation exists. It carries a
//! [`CommandKind::Service`](crate::sys::CommandKind::Service) payload as opaque
//! bytes. Everything below is an ordinary *user* of that mechanism, with no
//! privileged access, which is what keeps two things true:
//!
//! * **Adding a domain does not bump the ABI.** `sys::VERSION_MINOR` describes
//!   the boundary; this module changes the crate's own semver instead. A plugin
//!   that wants audio one day will not find itself declaring a minimum ABI that
//!   also encodes animation history.
//! * **A plugin that never animates anything pays nothing.** The module is
//!   behind the `anim` feature, and its types are plain data with no statics, so
//!   they emit no code even when compiled.
//!
//! Adding another domain means another module exactly like this one, plus a
//! `plugin_bridge` in whichever engine crate owns it. Neither touches [`sys`].
//!
//! ## How it reaches the engine
//!
//! Every method here encodes a plain-data payload and hands it to
//! [`EntityCommands::call_service`] under [`SERVICE`]. The host copies those
//! bytes into a queue without reading them; `renzora_animation::plugin_bridge`
//! takes the calls tagged with this service and turns them into real animation
//! commands. If that crate is absent — a dedicated server, a lean 2D export —
//! the calls are discarded at end of frame and nothing breaks.


use crate::ecs::EntityCommands;
use crate::sys::{self, Vec3};

/// Identifies this service in the host's queue.
///
/// Owner-qualified so two crates cannot collide by both picking `"animation"`.
pub const SERVICE: u64 = sys::service_id("renzora.animation");

// ── Operations ───────────────────────────────────────────────────────────────

/// Which operation an [`AnimCommand`] performs.
///
/// A newtype rather than an `enum`, for the same soundness reason the mechanism's
/// own enums are: the engine reads this value out of plugin memory, and
/// materialising an out-of-range discriminant into a Rust enum is undefined
/// behaviour — rustc attaches `!range` metadata to the load, so a `match` may
/// legally take an arbitrary arm. Any `u32` is a valid value here instead.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimOp(pub u32);

#[allow(non_upper_case_globals)]
impl AnimOp {
    /// Play `name`. `value` is speed, `flag` is looping.
    pub const Play: Self = Self(0);
    /// Stop playback.
    pub const Stop: Self = Self(1);
    /// Pause where it is.
    pub const Pause: Self = Self(2);
    /// Resume from where it was paused.
    pub const Resume: Self = Self(3);
    /// `value` is the new playback speed multiplier.
    pub const SetSpeed: Self = Self(4);
    /// `value` is a time in seconds.
    pub const Seek: Self = Self(5);
    /// Blend into `name` over `value` seconds. `flag` is looping.
    pub const Crossfade: Self = Self(6);
    /// Set state-machine float parameter `name` to `value`.
    pub const SetParam: Self = Self(7);
    /// Set state-machine bool parameter `name` to `flag`.
    pub const SetBool: Self = Self(8);
    /// Fire one-shot trigger `name`.
    pub const Trigger: Self = Self(9);
    /// Set layer `name`'s weight to `value`.
    pub const SetLayerWeight: Self = Self(10);
    /// Tween translation to `target` over `value` seconds, using `easing`.
    pub const TweenPosition: Self = Self(11);
    /// Tween rotation (Euler degrees in `target`) over `value` seconds.
    pub const TweenRotation: Self = Self(12);
    /// Tween scale to `target` over `value` seconds.
    pub const TweenScale: Self = Self(13);

    /// Whether this is a value this build knows.
    pub const fn is_known(self) -> bool {
        self.0 < 14
    }

    /// The operation name, or `"?"` for a value from a newer version.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Play",
            1 => "Stop",
            2 => "Pause",
            3 => "Resume",
            4 => "SetSpeed",
            5 => "Seek",
            6 => "Crossfade",
            7 => "SetParam",
            8 => "SetBool",
            9 => "Trigger",
            10 => "SetLayerWeight",
            11 => "TweenPosition",
            12 => "TweenRotation",
            13 => "TweenScale",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for AnimOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// Easing curve for the tween operations.
///
/// **These ordinals are frozen and must stay aligned with the engine's
/// `EasingFunction` declaration order.** The bridge maps them by name in a
/// `match`, precisely so that reordering that enum stops compiling instead of
/// silently remapping every plugin's easing from bounce to elastic.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Easing(pub u32);

#[allow(non_upper_case_globals)]
impl Easing {
    pub const Linear: Self = Self(0);
    pub const In: Self = Self(1);
    pub const Out: Self = Self(2);
    pub const InOut: Self = Self(3);
    pub const InQuad: Self = Self(4);
    pub const OutQuad: Self = Self(5);
    pub const InOutQuad: Self = Self(6);
    pub const InCubic: Self = Self(7);
    pub const OutCubic: Self = Self(8);
    pub const InOutCubic: Self = Self(9);
    pub const InBack: Self = Self(10);
    pub const OutBack: Self = Self(11);
    pub const InOutBack: Self = Self(12);
    pub const InElastic: Self = Self(13);
    pub const OutElastic: Self = Self(14);
    pub const InBounce: Self = Self(15);
    pub const OutBounce: Self = Self(16);

    /// Whether this is a value this build knows.
    pub const fn is_known(self) -> bool {
        self.0 < 17
    }
}

impl Default for Easing {
    fn default() -> Self {
        Self::InOut
    }
}

// ── The payload ──────────────────────────────────────────────────────────────

/// Longest animation name that crosses the boundary, in bytes.
///
/// Names are inline rather than a pointer so an [`AnimCommand`] is entirely
/// plain-old-data. The host copies a service payload as *bytes*, so a pointer
/// inside it would survive the copy as a pointer and be read after the calling
/// system returned — pointing at a stack frame that is gone.
pub const NAME_CAP: usize = 48;

/// A clip, state, parameter or layer name, inline.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AnimName {
    pub bytes: [u8; NAME_CAP],
    /// Bytes used. Always `<= NAME_CAP`; the engine clamps on read rather than
    /// trusting a plugin's length.
    pub len: u8,
}

impl AnimName {
    pub const EMPTY: Self = Self { bytes: [0; NAME_CAP], len: 0 };

    /// Copy a name in, or return `None` if it does not fit.
    ///
    /// `None` rather than truncating: a silently shortened name resolves to no
    /// clip at all, and "my animation doesn't play" is far harder to trace back
    /// than a message naming the limit.
    pub const fn new(s: &str) -> Option<Self> {
        let src = s.as_bytes();
        if src.len() > NAME_CAP {
            return None;
        }
        let mut out = Self::EMPTY;
        let mut i = 0;
        while i < src.len() {
            out.bytes[i] = src[i];
            i += 1;
        }
        out.len = src.len() as u8;
        Some(out)
    }

    /// The bytes actually in use, clamped to the capacity.
    pub fn as_bytes(&self) -> &[u8] {
        let n = (self.len as usize).min(NAME_CAP);
        &self.bytes[..n]
    }

    /// The name as UTF-8, or `""` if a plugin wrote bytes that are not.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or_default()
    }
}

impl core::fmt::Debug for AnimName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// One animation operation, as a service payload.
///
/// A single flat struct rather than a variant per op, because it crosses as
/// bytes: a Rust enum with payloads has no guaranteed layout. Fields are
/// op-specific — [`AnimOp`] documents which reads which — and unused ones are
/// zero.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AnimCommand {
    pub op: AnimOp,
    pub name: AnimName,
    /// Speed, seconds, weight or parameter value, depending on `op`.
    pub value: f32,
    /// Looping, or a bool parameter's value. 0 or 1.
    pub flag: u32,
    /// Destination for the tween ops; zero otherwise.
    pub target: Vec3,
    pub easing: Easing,
}

impl AnimCommand {
    /// An op that needs nothing but a name.
    pub const fn named(op: AnimOp, name: AnimName) -> Self {
        Self {
            op,
            name,
            value: 0.0,
            flag: 0,
            target: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
            easing: Easing::InOut,
        }
    }

    /// The payload bytes, for [`EntityCommands::call_service`].
    ///
    /// # Safety note
    /// Sound because `Self` is `#[repr(C)]` plain-old-data with no padding the
    /// caller could observe as uninitialised — every field is written by the
    /// constructors above.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: `#[repr(C)]`, no pointers, no `Drop`.
        unsafe {
            core::slice::from_raw_parts(
                (self as *const Self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

// ── Reading it back ──────────────────────────────────────────────────────────

/// The animator's state, mirrored for a plugin to read through a query.
///
/// A component rather than a call, so reads cost nothing: it arrives through the
/// same query cells everything else does, and a system that asks about animation
/// every frame makes no calls back into the engine at all.
///
/// ## Why names are hashes
///
/// A plugin has no `String` — component fields are a closed set of numeric
/// kinds — so `current_clip` cannot cross as text. It crosses as a 64-bit FNV-1a
/// hash, which answers the only question anyone asks of it: *is it this one?*
/// [`is_clip`](Self::is_clip) hashes the literal at the call site and compares
/// two integers.
///
/// The trade is that a plugin cannot **discover** a name it was not already
/// looking for, and a collision would report the wrong clip. 64-bit FNV-1a over
/// a handful of short names makes the latter not worth engineering around.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AnimState {
    /// FNV-1a of the playing clip's name; 0 when nothing is playing.
    pub clip: u64,
    /// FNV-1a of the current state-machine state; 0 when there is no machine.
    pub state: u64,
    /// Seconds spent in the current state.
    pub state_time: f32,
    /// Property-animation playback time, in seconds.
    pub time: f32,
    /// 1 while playing, 0 while paused or stopped.
    pub playing: u32,
    /// Keeps the struct 16-byte aligned in size, so appending a field later is a
    /// layout question answered once rather than every time.
    pub _reserved: u32,
}

/// Hash a clip or state name the way [`AnimState`] stores it.
pub const fn name_hash(name: &str) -> u64 {
    sys::fnv1a(name)
}

impl AnimState {
    /// Whether `name` is the clip currently playing.
    pub const fn is_clip(&self, name: &str) -> bool {
        self.clip == name_hash(name)
    }

    /// Whether `name` is the current state-machine state.
    pub const fn is_state(&self, name: &str) -> bool {
        self.state == name_hash(name)
    }

    /// Whether anything is playing.
    pub const fn is_playing(&self) -> bool {
        self.playing != 0
    }
}

// The engine side of this mirror is `renzora_animation::plugin_bridge`, which
// wraps this very struct so the two layouts cannot drift. The path below is the
// only thing tying them together — nothing links, and the host resolves it
// through its reflection registry at runtime. `plugin_bridge::install` asserts
// the two agree at startup, because a rename is otherwise silent on both sides.
crate::host_component!(AnimState, "renzora_animation::plugin_bridge::PluginAnimState");

// ── The plugin-facing API ────────────────────────────────────────────────────

/// Animation methods on [`EntityCommands`].
///
/// An extension trait because `EntityCommands` belongs to `renzora_plugin`, and
/// that crate does not know this one exists. Bring it into scope to use any of
/// the methods below.
pub trait AnimCommands {
    /// Queue one animation operation. The other methods are wrappers.
    fn anim(&mut self, cmd: AnimCommand) -> &mut Self;

    /// Play `name`, looping, at normal speed.
    fn play_animation(&mut self, name: &str) -> &mut Self;
    /// Play `name` at `speed`, looping or not.
    fn play_animation_with(&mut self, name: &str, speed: f32, looping: bool) -> &mut Self;
    /// Blend into `name` over `seconds`, looping.
    fn crossfade_animation(&mut self, name: &str, seconds: f32) -> &mut Self;
    fn stop_animation(&mut self) -> &mut Self;
    fn pause_animation(&mut self) -> &mut Self;
    fn resume_animation(&mut self) -> &mut Self;
    /// Set the playback speed multiplier.
    fn set_animation_speed(&mut self, speed: f32) -> &mut Self;
    /// Jump to `seconds` into the current clip.
    fn seek_animation(&mut self, seconds: f32) -> &mut Self;
    /// Set state-machine float parameter `name`.
    fn set_anim_param(&mut self, name: &str, value: f32) -> &mut Self;
    /// Set state-machine bool parameter `name`.
    fn set_anim_bool(&mut self, name: &str, value: bool) -> &mut Self;
    /// Fire one-shot trigger `name`.
    fn set_anim_trigger(&mut self, name: &str) -> &mut Self;
    /// Set animation layer `name`'s blend weight.
    fn set_layer_weight(&mut self, name: &str, weight: f32) -> &mut Self;
    /// Tween translation to `target` over `seconds`.
    fn tween_position(&mut self, target: Vec3, seconds: f32, easing: Easing) -> &mut Self;
    /// Tween rotation to `target`, as Euler degrees, over `seconds`.
    fn tween_rotation(&mut self, target: Vec3, seconds: f32, easing: Easing) -> &mut Self;
    /// Tween scale to `target` over `seconds`.
    fn tween_scale(&mut self, target: Vec3, seconds: f32, easing: Easing) -> &mut Self;
}

/// Resolve a name, logging and yielding `None` when it does not fit.
fn checked(name: &str) -> Option<AnimName> {
    match AnimName::new(name) {
        Some(n) => Some(n),
        None => {
            crate::ecs::error("animation name is longer than 48 bytes and was ignored");
            None
        }
    }
}

impl AnimCommands for EntityCommands<'_> {
    fn anim(&mut self, cmd: AnimCommand) -> &mut Self {
        self.call_service(SERVICE, cmd.op.0, cmd.as_bytes())
    }

    fn play_animation(&mut self, name: &str) -> &mut Self {
        self.play_animation_with(name, 1.0, true)
    }

    fn play_animation_with(&mut self, name: &str, speed: f32, looping: bool) -> &mut Self {
        let Some(name) = checked(name) else { return self };
        self.anim(AnimCommand {
            value: speed,
            flag: looping as u32,
            ..AnimCommand::named(AnimOp::Play, name)
        })
    }

    fn crossfade_animation(&mut self, name: &str, seconds: f32) -> &mut Self {
        let Some(name) = checked(name) else { return self };
        self.anim(AnimCommand {
            value: seconds,
            flag: 1,
            ..AnimCommand::named(AnimOp::Crossfade, name)
        })
    }

    fn stop_animation(&mut self) -> &mut Self {
        self.anim(AnimCommand::named(AnimOp::Stop, AnimName::EMPTY))
    }

    fn pause_animation(&mut self) -> &mut Self {
        self.anim(AnimCommand::named(AnimOp::Pause, AnimName::EMPTY))
    }

    fn resume_animation(&mut self) -> &mut Self {
        self.anim(AnimCommand::named(AnimOp::Resume, AnimName::EMPTY))
    }

    fn set_animation_speed(&mut self, speed: f32) -> &mut Self {
        self.anim(AnimCommand {
            value: speed,
            ..AnimCommand::named(AnimOp::SetSpeed, AnimName::EMPTY)
        })
    }

    fn seek_animation(&mut self, seconds: f32) -> &mut Self {
        self.anim(AnimCommand {
            value: seconds,
            ..AnimCommand::named(AnimOp::Seek, AnimName::EMPTY)
        })
    }

    fn set_anim_param(&mut self, name: &str, value: f32) -> &mut Self {
        let Some(name) = checked(name) else { return self };
        self.anim(AnimCommand { value, ..AnimCommand::named(AnimOp::SetParam, name) })
    }

    fn set_anim_bool(&mut self, name: &str, value: bool) -> &mut Self {
        let Some(name) = checked(name) else { return self };
        self.anim(AnimCommand {
            flag: value as u32,
            ..AnimCommand::named(AnimOp::SetBool, name)
        })
    }

    fn set_anim_trigger(&mut self, name: &str) -> &mut Self {
        let Some(name) = checked(name) else { return self };
        self.anim(AnimCommand::named(AnimOp::Trigger, name))
    }

    fn set_layer_weight(&mut self, name: &str, weight: f32) -> &mut Self {
        let Some(name) = checked(name) else { return self };
        self.anim(AnimCommand {
            value: weight,
            ..AnimCommand::named(AnimOp::SetLayerWeight, name)
        })
    }

    fn tween_position(&mut self, target: Vec3, seconds: f32, easing: Easing) -> &mut Self {
        self.anim(AnimCommand {
            value: seconds,
            target,
            easing,
            ..AnimCommand::named(AnimOp::TweenPosition, AnimName::EMPTY)
        })
    }

    fn tween_rotation(&mut self, target: Vec3, seconds: f32, easing: Easing) -> &mut Self {
        self.anim(AnimCommand {
            value: seconds,
            target,
            easing,
            ..AnimCommand::named(AnimOp::TweenRotation, AnimName::EMPTY)
        })
    }

    fn tween_scale(&mut self, target: Vec3, seconds: f32, easing: Easing) -> &mut Self {
        self.anim(AnimCommand {
            value: seconds,
            target,
            easing,
            ..AnimCommand::named(AnimOp::TweenScale, AnimName::EMPTY)
        })
    }
}
